//! Probed device facts. Capability is physics; it is resolved once at first
//! launch, cached, and it outranks every commercial state (AD-9).
//!
//! The probe result is written to disk with a [`PROBE_SCHEMA_VERSION`] stamp.
//! A stale cache surviving a change to the probe logic would pin a device to
//! the wrong tier forever, so a version mismatch forces a re-probe rather than
//! being trusted.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::graph::NodeKind;
use crate::CoreError;

/// The probe logic's own version. Bump this whenever tier derivation, the
/// backend detection, or the micro-benchmark changes shape.
pub const PROBE_SCHEMA_VERSION: u32 = 1;

/// Hardware tier. Ordered: `T0 < T1 < T2`, so "required tier exceeds the
/// device tier" is a plain comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DeviceTier {
    T0,
    T1,
    T2,
}

/// Where a node executes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Backend {
    Npu,
    Gpu,
    Cpu,
}

/// Probed device facts, cached after first launch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SocProfile {
    pub soc_id: String,
    pub soc_name: String,
    pub tier: DeviceTier,
    pub ram_bytes: u64,
    /// Memory available for resident models once the OS and the app itself are
    /// accounted for. This is the budget that makes co-residency exclusion
    /// necessary — see [`exclusive_families`].
    pub model_budget_bytes: u64,
    pub backends: Vec<Backend>,
    pub npu_experimental: bool,
    pub probe_schema_version: u32,
}

impl SocProfile {
    /// Whether this profile was produced by the probe logic in the running
    /// binary. A cached profile that answers `false` must be discarded.
    pub fn is_current_schema(&self) -> bool {
        self.probe_schema_version == PROBE_SCHEMA_VERSION
    }

    /// Whether the device can execute on `backend`.
    pub fn has_backend(&self, backend: Backend) -> bool {
        self.backends.contains(&backend)
    }
}

/// Model class, for co-residency exclusion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StageFamily {
    Diffusion,
    LargeLanguage,
    Audio,
    Vision,
}

impl StageFamily {
    /// The family whose weights a node holds resident while it runs. Sources,
    /// sinks and the muxer hold no model and answer `None`.
    pub fn of(kind: NodeKind) -> Option<StageFamily> {
        use NodeKind::*;
        use StageFamily::*;
        match kind {
            SourceVideo | SourceImage | SourceAudio | AvMux | SinkGallery | SinkFiles => None,
            AudioSplit | AudioDenoise | AudioIsolateVoice | AudioStems | Transcribe | Diarize => {
                Some(Audio)
            }
            GenerativeFill => Some(Diffusion),
            MetadataGen | CaptionFrames => Some(LargeLanguage),
            ImageUpscale | ImageObjectRemove | ImageCutout | VideoUpscale | VideoRemoveBg
            | VideoInterpolate | MaskHelper => Some(Vision),
        }
    }
}

/// Stage-family pairs that must never be resident together at `tier`.
///
/// On the 12 GB floor the scheduler serialises stage families rather than
/// holding diffusion weights and an LLM at the same time, so `T0` and `T1`
/// exclude that pair. `T2` has the budget to hold both.
pub fn exclusive_families(tier: DeviceTier) -> Vec<(StageFamily, StageFamily)> {
    match tier {
        DeviceTier::T0 | DeviceTier::T1 => {
            vec![(StageFamily::Diffusion, StageFamily::LargeLanguage)]
        }
        DeviceTier::T2 => vec![],
    }
}

/// The lowest [`DeviceTier`] on which a node kind can execute at all.
///
/// This is the single source of the tier axis, read by `validate_graph` for
/// `TierUnavailable` and by `resolve_availability` for `TierLimited`; neither
/// re-derives it.
pub fn required_tier(kind: NodeKind) -> DeviceTier {
    use DeviceTier::*;
    use NodeKind::*;
    match kind {
        // Diffusion needs a real NPU budget.
        GenerativeFill => T2,
        // Real-time video stages need at least a usable GPU or NPU.
        VideoUpscale | VideoRemoveBg | VideoInterpolate | CaptionFrames => T1,
        // Everything else has a CPU path that is acceptable on any device.
        SourceVideo | SourceImage | SourceAudio | AudioSplit | AudioDenoise
        | AudioIsolateVoice | AudioStems | Transcribe | Diarize | ImageUpscale
        | ImageObjectRemove | ImageCutout | MetadataGen | MaskHelper | AvMux | SinkGallery
        | SinkFiles => T0,
    }
}

/// Memory left for resident model weights: a third of physical RAM, less a
/// 1.5 GB allowance for the OS and the app's own working set, floored so the
/// budget is never zero on a small device.
fn model_budget(ram_bytes: u64) -> u64 {
    const OS_AND_APP_OVERHEAD: u64 = 1_536 * 1024 * 1024;
    const FLOOR: u64 = 512 * 1024 * 1024;
    (ram_bytes / 3).saturating_sub(OS_AND_APP_OVERHEAD).max(FLOOR)
}

/// Physical RAM from `/proc/meminfo`, which Android and desktop Linux both
/// expose. Falls back to 4 GiB when the field cannot be read.
fn physical_ram_bytes() -> u64 {
    const FALLBACK: u64 = 4 * 1024 * 1024 * 1024;
    let Ok(text) = std::fs::read_to_string("/proc/meminfo") else {
        return FALLBACK;
    };
    text.lines()
        .find_map(|l| l.strip_prefix("MemTotal:"))
        .and_then(|v| v.split_whitespace().next())
        .and_then(|kb| kb.parse::<u64>().ok())
        .map(|kb| kb * 1024)
        .unwrap_or(FALLBACK)
}

/// First-launch probe: SoC id, delegate load test, timed micro-benchmark.
///
/// On desktop there is no NPU and no vendor delegate, so the profile is fixed:
/// tier `T0`, CPU only, nothing experimental. That is what lets `forge-cli`
/// exercise the whole core with no device attached (AD-10).
#[cfg(not(target_os = "android"))]
pub fn probe_device() -> Result<SocProfile, CoreError> {
    let ram_bytes = physical_ram_bytes();
    Ok(SocProfile {
        soc_id: "desktop".to_string(),
        soc_name: "Desktop host".to_string(),
        tier: DeviceTier::T0,
        ram_bytes,
        model_budget_bytes: model_budget(ram_bytes),
        backends: vec![Backend::Cpu],
        npu_experimental: false,
        probe_schema_version: PROBE_SCHEMA_VERSION,
    })
}

/// First-launch probe: SoC id, delegate load test, timed micro-benchmark.
#[cfg(target_os = "android")]
pub fn probe_device() -> Result<SocProfile, CoreError> {
    let (soc_id, soc_name) = android::read_soc_identity()?;
    let ram_bytes = physical_ram_bytes();
    let (backends, npu_experimental) = android::probe_backends();
    let bench_ms = micro_benchmark_ms();
    let tier = classify_tier(ram_bytes, &backends, npu_experimental, bench_ms);
    Ok(SocProfile {
        soc_id,
        soc_name,
        tier,
        ram_bytes,
        model_budget_bytes: model_budget(ram_bytes),
        backends,
        npu_experimental,
        probe_schema_version: PROBE_SCHEMA_VERSION,
    })
}

/// Probe, using `cache` when it holds a profile this binary's probe logic
/// produced. A schema mismatch, a missing file or unreadable JSON all force a
/// fresh probe, and the fresh result is written back.
pub fn probe_device_cached(cache: &Path) -> Result<SocProfile, CoreError> {
    if let Ok(text) = std::fs::read_to_string(cache) {
        if let Ok(profile) = serde_json::from_str::<SocProfile>(&text) {
            if profile.is_current_schema() {
                return Ok(profile);
            }
        }
    }
    let fresh = probe_device()?;
    if let Some(parent) = cache.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(cache, serde_json::to_vec_pretty(&fresh)?)?;
    Ok(fresh)
}

/// Timed micro-benchmark: a fixed 128³ f32 matrix multiply, which is small
/// enough to run inside first-launch latency and large enough to separate a
/// big core from a little one. Returns wall-clock milliseconds.
#[cfg(target_os = "android")]
fn micro_benchmark_ms() -> u64 {
    const N: usize = 128;
    let a: Vec<f32> = (0..N * N).map(|i| (i % 97) as f32).collect();
    let b: Vec<f32> = (0..N * N).map(|i| (i % 89) as f32).collect();
    let mut c = vec![0f32; N * N];
    let start = std::time::Instant::now();
    for i in 0..N {
        for k in 0..N {
            let aik = a[i * N + k];
            for j in 0..N {
                c[i * N + j] += aik * b[k * N + j];
            }
        }
    }
    // Keep the result observable so the loop is not optimised away.
    std::hint::black_box(&c);
    start.elapsed().as_millis() as u64
}

/// Tier from the probed facts. A device only reaches `T2` with a non-
/// experimental NPU, a 12 GB-class memory budget and a big core fast enough to
/// keep the fallback paths usable; `T1` needs GPU-or-NPU acceleration and an
/// 8 GB-class budget; everything else is `T0`.
#[cfg(target_os = "android")]
fn classify_tier(
    ram_bytes: u64,
    backends: &[Backend],
    npu_experimental: bool,
    bench_ms: u64,
) -> DeviceTier {
    const GIB: u64 = 1024 * 1024 * 1024;
    let npu = backends.contains(&Backend::Npu);
    let gpu = backends.contains(&Backend::Gpu);
    if npu && !npu_experimental && ram_bytes >= 12 * GIB && bench_ms <= 40 {
        DeviceTier::T2
    } else if (npu || gpu) && ram_bytes >= 8 * GIB {
        DeviceTier::T1
    } else {
        DeviceTier::T0
    }
}

#[cfg(target_os = "android")]
mod android {
    use super::{Backend, CoreError};

    /// Vendor delegate libraries, most capable first. Presence alone is not
    /// enough — the library is opened so a stub that cannot resolve its own
    /// dependencies is rejected here rather than at first inference.
    const DELEGATES: &[(&str, Backend, bool)] = &[
        ("libQnnHtp.so", Backend::Npu, false),
        ("libQnnHtpV73Stub.so", Backend::Npu, true),
        ("libneuron_adapter.so", Backend::Npu, true),
        ("libtensorflowlite_gpu_delegate.so", Backend::Gpu, false),
        ("libGLES_mali.so", Backend::Gpu, false),
    ];

    /// SoC identity from sysfs, which both Qualcomm and Exynos populate.
    pub fn read_soc_identity() -> Result<(String, String), CoreError> {
        let read = |p: &str| {
            std::fs::read_to_string(p)
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        };
        let id = read("/sys/devices/soc0/soc_id")
            .or_else(|| read("/sys/devices/system/soc/soc0/id"))
            .ok_or_else(|| CoreError::Probe("no soc id in sysfs".to_string()))?;
        let name = read("/sys/devices/soc0/machine")
            .or_else(|| read("/sys/devices/soc0/family"))
            .unwrap_or_else(|| format!("soc{id}"));
        Ok((id, name))
    }

    /// Attempt to load each vendor delegate. CPU is always present.
    pub fn probe_backends() -> (Vec<Backend>, bool) {
        let mut backends = Vec::new();
        let mut experimental = false;
        for (lib, backend, is_experimental) in DELEGATES {
            if backends.contains(backend) {
                continue;
            }
            // Safety: opening a vendor delegate runs its initialisers. These
            // are the platform's own accelerator libraries; a failure to load
            // is the answer we are probing for, not an error.
            let loaded = unsafe { libloading::Library::new(lib) }.is_ok();
            if loaded {
                backends.push(*backend);
                if *backend == Backend::Npu && *is_experimental {
                    experimental = true;
                }
            }
        }
        backends.push(Backend::Cpu);
        (backends, experimental)
    }
}
