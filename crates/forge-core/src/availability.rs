//! Node availability: the seven states the UI renders, and the one place the
//! gating precedence rule is expressed.
//!
//! **Capability outranks commerce (AD-9).** A node the silicon cannot run is
//! reported as [`NodeAvailability::TierLimited`] with a substitute, and the
//! entitlement, the balance and the price are never consulted for it. That is
//! why the tier test is first and returns immediately: no later branch can
//! turn a physically impossible node into a lock, a price, or a credit cost.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::capability::{required_tier, Backend, DeviceTier, SocProfile, StageFamily};
use crate::entitlement::Entitlement;
use crate::graph::NodeKind;

/// The seven UI states. `d1-node-state-legend` is the visual contract for this
/// enum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeAvailability {
    Ready(Backend),
    Accelerated,
    NeedsModel { bytes: u64, license: String },
    Experimental { estimate_ms: u64 },
    Metered { credits: u32 },
    ProLocked,
    TierLimited {
        required: DeviceTier,
        substitute: Option<NodeKind>,
    },
}

/// Per-node credit cost. Supplied by the entitlement layer, which is the only
/// component that knows current pricing; `resolve_availability` reads a price
/// here rather than inventing the number it reports.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NodePricing(pub HashMap<NodeKind, u32>);

impl NodePricing {
    /// A pricing table from `(kind, credits)` pairs.
    pub fn new(entries: impl IntoIterator<Item = (NodeKind, u32)>) -> Self {
        NodePricing(entries.into_iter().collect())
    }

    /// What one run of `kind` costs, or `None` when the node is not metered.
    pub fn cost(&self, kind: NodeKind) -> Option<u32> {
        self.0.get(&kind).copied()
    }
}

/// The model a node loads, when it loads one at all.
///
/// This is the single place the download size, licence identifier and nominal
/// per-unit estimate for a node's default model live. The figures are carried
/// here so `NeedsModel` and `Experimental` report a real number instead of a
/// placeholder; they must be reconciled against the published artefacts before
/// a release ships.
struct ModelReq {
    bytes: u64,
    license: &'static str,
    estimate_ms: u64,
}

const MB: u64 = 1024 * 1024;

fn model_requirement(kind: NodeKind) -> Option<ModelReq> {
    use NodeKind::*;
    let req = |bytes, license, estimate_ms| {
        Some(ModelReq {
            bytes,
            license,
            estimate_ms,
        })
    };
    match kind {
        // Sources, sinks and the muxer hold no weights.
        SourceVideo | SourceImage | SourceAudio | AvMux | SinkGallery | SinkFiles => None,
        // Track demux is a container operation, not inference.
        AudioSplit => None,

        AudioDenoise => req(2 * MB, "Apache-2.0", 40),
        AudioIsolateVoice => req(24 * MB, "MIT", 120),
        AudioStems => req(80 * MB, "MIT", 400),
        Transcribe => req(488 * MB, "MIT", 900),
        Diarize => req(28 * MB, "MIT", 300),

        ImageUpscale => req(6 * MB, "Apache-2.0", 60),
        ImageObjectRemove => req(200 * MB, "Apache-2.0", 700),
        GenerativeFill => req(6656 * MB, "FLUX-1-dev-non-commercial", 9000),
        ImageCutout => req(176 * MB, "MIT", 250),
        MaskHelper => req(38 * MB, "Apache-2.0", 90),

        VideoUpscale => req(6 * MB, "Apache-2.0", 3),
        VideoRemoveBg => req(176 * MB, "MIT", 30),
        VideoInterpolate => req(64 * MB, "MIT", 25),

        MetadataGen => req(1024 * MB, "Gemma-Terms-of-Use", 1800),
        CaptionFrames => req(1024 * MB, "Gemma-Terms-of-Use", 1200),
    }
}

/// A node of the same media class that does run at `tier`.
///
/// `TierLimited` is only useful if it offers a way forward, so every node that
/// can be tier-limited has an answer here. Generative fill on a device below
/// `T2` substitutes object removal: the same "make this go away" intent, on
/// silicon that exists.
fn substitute_for(kind: NodeKind, tier: DeviceTier) -> Option<NodeKind> {
    let candidate = match kind {
        NodeKind::GenerativeFill => Some(NodeKind::ImageObjectRemove),
        _ => None,
    };
    candidate.filter(|c| required_tier(*c) <= tier)
}

/// Whether a node's stage family is behind Pro. The generative and language
/// families are the paid stages; audio and vision stages are included in the
/// free product, and sources, sinks and the muxer hold no model at all.
fn requires_pro(kind: NodeKind) -> bool {
    matches!(
        StageFamily::of(kind),
        Some(StageFamily::Diffusion) | Some(StageFamily::LargeLanguage)
    )
}

/// Whether the node would run on an NPU whose delegate the probe flagged as
/// experimental. Nodes that hold no model never touch the delegate.
fn is_experimental(kind: NodeKind, caps: &SocProfile) -> bool {
    caps.npu_experimental
        && caps.has_backend(Backend::Npu)
        && StageFamily::of(kind).is_some()
}

/// The best backend the device can offer for a node that is not accelerated.
fn best_backend(caps: &SocProfile) -> Backend {
    if caps.has_backend(Backend::Gpu) {
        Backend::Gpu
    } else {
        Backend::Cpu
    }
}

/// Resolve the UI state of a node. **The precedence rule lives here and
/// nowhere else** — no caller may re-derive it.
///
/// The order is fixed:
/// 1. tier limitation, returned immediately and without consulting commerce;
/// 2. missing model;
/// 3. experimental delegate;
/// 4. Pro gating, metered when the node carries a price;
/// 5. otherwise accelerated, or ready on the best available backend.
pub fn resolve_availability(
    kind: NodeKind,
    caps: &SocProfile,
    ent: &Entitlement,
    balance: u32,
    pricing: &NodePricing,
    model_present: bool,
) -> NodeAvailability {
    // 1. Physics. This branch returns before any commercial state is read, so
    //    a node the device cannot run can never render as a lock, a price or a
    //    credit cost — however much the user has paid or holds.
    let required = required_tier(kind);
    if required > caps.tier {
        return NodeAvailability::TierLimited {
            required,
            substitute: substitute_for(kind, caps.tier),
        };
    }

    // The balance is deliberately not part of the precedence rule. Being
    // offered a node and being able to afford it are different questions: the
    // reserve is spent in `gated_with_entitlement`, and this function only
    // reports what a run would cost.
    let _ = balance;

    // 2. Model.
    let model = model_requirement(kind);
    if let Some(req) = &model {
        if !model_present {
            return NodeAvailability::NeedsModel {
                bytes: req.bytes,
                license: req.license.to_string(),
            };
        }
    }

    // 3. Experimental delegate at this tier.
    if is_experimental(kind, caps) {
        let estimate_ms = model.as_ref().map(|m| m.estimate_ms).unwrap_or(0);
        return NodeAvailability::Experimental { estimate_ms };
    }

    // 4. Commerce.
    if requires_pro(kind) && matches!(ent, Entitlement::Free) {
        return match pricing.cost(kind) {
            Some(credits) => NodeAvailability::Metered { credits },
            None => NodeAvailability::ProLocked,
        };
    }

    // 5. Where it runs.
    if caps.has_backend(Backend::Npu) {
        NodeAvailability::Accelerated
    } else {
        NodeAvailability::Ready(best_backend(caps))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::PROBE_SCHEMA_VERSION;

    fn profile(tier: DeviceTier, backends: Vec<Backend>) -> SocProfile {
        SocProfile {
            soc_id: "test".to_string(),
            soc_name: "test".to_string(),
            tier,
            ram_bytes: 12 * 1024 * 1024 * 1024,
            model_budget_bytes: 2 * 1024 * 1024 * 1024,
            backends,
            npu_experimental: false,
            probe_schema_version: PROBE_SCHEMA_VERSION,
        }
    }

    fn t0() -> SocProfile {
        profile(DeviceTier::T0, vec![Backend::Cpu])
    }

    fn t2() -> SocProfile {
        profile(
            DeviceTier::T2,
            vec![Backend::Npu, Backend::Gpu, Backend::Cpu],
        )
    }

    fn pro() -> Entitlement {
        Entitlement::Pro {
            perpetual_version: None,
        }
    }

    fn fill_pricing() -> NodePricing {
        NodePricing::new([(NodeKind::GenerativeFill, 4)])
    }

    /// The rule the whole product depends on: on a device that cannot run the
    /// node, commercial state is never reached. Free and Pro must give the
    /// same answer, and it must not be a lock or a price.
    #[test]
    fn tier_limitation_beats_every_commercial_state() {
        for ent in [Entitlement::Free, pro()] {
            let got = resolve_availability(
                NodeKind::GenerativeFill,
                &t0(),
                &ent,
                10_000,
                &fill_pricing(),
                true,
            );
            assert_eq!(
                got,
                NodeAvailability::TierLimited {
                    required: DeviceTier::T2,
                    substitute: Some(NodeKind::ImageObjectRemove),
                },
                "GenerativeFill on a T0 profile with {ent:?}"
            );
            assert_ne!(got, NodeAvailability::ProLocked);
            assert!(!matches!(got, NodeAvailability::Metered { .. }));
        }
    }

    /// Precedence holds regardless of the other inputs: no model, no credits
    /// and no price still resolve to the same tier limitation.
    #[test]
    fn tier_limitation_holds_with_no_model_and_no_credits() {
        let got = resolve_availability(
            NodeKind::GenerativeFill,
            &t0(),
            &Entitlement::Free,
            0,
            &NodePricing::default(),
            false,
        );
        assert_eq!(
            got,
            NodeAvailability::TierLimited {
                required: DeviceTier::T2,
                substitute: Some(NodeKind::ImageObjectRemove),
            }
        );
    }

    #[test]
    fn t2_free_is_metered_at_the_node_price() {
        let got = resolve_availability(
            NodeKind::GenerativeFill,
            &t2(),
            &Entitlement::Free,
            0,
            &fill_pricing(),
            true,
        );
        assert_eq!(got, NodeAvailability::Metered { credits: 4 });
    }

    #[test]
    fn t2_free_without_a_price_is_pro_locked() {
        let got = resolve_availability(
            NodeKind::GenerativeFill,
            &t2(),
            &Entitlement::Free,
            0,
            &NodePricing::default(),
            true,
        );
        assert_eq!(got, NodeAvailability::ProLocked);
    }

    #[test]
    fn t2_pro_is_accelerated() {
        let got = resolve_availability(
            NodeKind::GenerativeFill,
            &t2(),
            &pro(),
            0,
            &fill_pricing(),
            true,
        );
        assert_eq!(got, NodeAvailability::Accelerated);
    }

    /// A tier limitation without a way forward is a dead end in the UI, so the
    /// substitute is never `None` for any kind at any tier.
    #[test]
    fn tier_limited_always_carries_a_substitute() {
        let profiles = [
            t0(),
            profile(DeviceTier::T1, vec![Backend::Gpu, Backend::Cpu]),
            t2(),
        ];
        let mut seen = 0usize;
        for caps in &profiles {
            for kind in NodeKind::ALL {
                let got = resolve_availability(
                    kind,
                    caps,
                    &Entitlement::Free,
                    0,
                    &NodePricing::default(),
                    true,
                );
                if let NodeAvailability::TierLimited { substitute, .. } = got {
                    seen += 1;
                    assert!(
                        substitute.is_some(),
                        "{kind:?} is tier-limited at {:?} with no substitute",
                        caps.tier
                    );
                }
            }
        }
        assert!(seen > 0, "the sweep never produced a TierLimited result");
    }

    #[test]
    fn a_missing_model_is_reported_before_commerce_but_after_tier() {
        let got = resolve_availability(
            NodeKind::Transcribe,
            &t0(),
            &Entitlement::Free,
            0,
            &NodePricing::default(),
            false,
        );
        assert!(matches!(got, NodeAvailability::NeedsModel { .. }));
    }

    #[test]
    fn an_experimental_delegate_outranks_pro_gating() {
        let mut caps = t2();
        caps.npu_experimental = true;
        let got = resolve_availability(
            NodeKind::GenerativeFill,
            &caps,
            &Entitlement::Free,
            0,
            &fill_pricing(),
            true,
        );
        assert!(matches!(got, NodeAvailability::Experimental { .. }));
    }

    #[test]
    fn a_free_stage_on_a_cpu_device_is_ready_on_cpu() {
        let got = resolve_availability(
            NodeKind::AudioDenoise,
            &t0(),
            &Entitlement::Free,
            0,
            &NodePricing::default(),
            true,
        );
        assert_eq!(got, NodeAvailability::Ready(Backend::Cpu));
    }
}
