//! `forge-ffi` — the Tauri 2 shell boundary (AD-1). Command handlers are
//! dispatch-only per AD-4; nothing long-running executes on the calling thread.
//!
//! The shell owns three things and no more: where the app's data lives on the
//! device, the builder that starts the WebView, and the command surface the
//! front end calls. The pipeline itself is `forge-core`'s, the engines are
//! `forge-engines`', and the platform services are the Kotlin plugin layer's.
//!
//! `run` is the single entry point. On Android the `mobile_entry_point`
//! attribute exports it for the generated `MainActivity` to call; on desktop it
//! is an ordinary function, which is what lets the same shell start from a host
//! binary with no device attached.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use forge_core::assets::AssetStore;
use forge_core::availability::{resolve_availability, NodeAvailability};
use forge_core::capability::{probe_device_cached, SocProfile};
use forge_core::checkpoint::CheckpointStore;
use forge_core::entitlement::{EntitlementService, NullEntitlementService};
use forge_core::graph::NodeKind;
use forge_core::pipeline::PipelineDoc;
use forge_core::scheduler::{NullSegmentRunner, Scheduler};
use forge_core::validate::{validate_graph, ValidationError};
use tauri::{async_runtime, Manager};

/// Everything the shell writes on the device lives under one root: the cached
/// `SocProfile`, the content-addressed asset store and the job checkpoints.
///
/// It is resolved once, in `setup`, from the platform's app-data directory. The
/// command handlers take no `AppHandle` — their signatures are frozen in
/// `DOCS/ARCHITECTURE.md` §3 — so the path reaches them through here rather
/// than through an argument.
static DATA_ROOT: OnceLock<PathBuf> = OnceLock::new();

/// The app-data root, or an error naming the reason it is missing.
///
/// A handler that runs before `setup` finished has nowhere to write, and saying
/// so is better than silently choosing a directory the platform will wipe.
pub(crate) fn data_root() -> Result<&'static Path, String> {
    DATA_ROOT
        .get()
        .map(PathBuf::as_path)
        .ok_or_else(|| "the app data directory is not resolved yet".to_string())
}

/// Runs `work` on the async runtime's blocking pool and awaits its answer.
///
/// This is the AD-4 dispatch and every command goes through it. The handler
/// itself only sends work away and yields: the probe reads sysfs, the plan
/// hashes the graph and the store touches the filesystem, and none of that may
/// happen on the thread the WebView called from. A worker that panics is
/// reported as a failed command rather than taking the app down with it.
async fn dispatch<T, F>(work: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| format!("the worker did not finish: {e}"))?
}

/// The device profile, from the cache when this binary's probe logic wrote it.
///
/// Always called from a worker: `probe_device_cached` may fall through to a
/// full probe, which loads a vendor delegate and runs a timed benchmark.
fn profile() -> Result<SocProfile, String> {
    probe_device_cached(&data_root()?.join("soc-profile.json")).map_err(|e| e.to_string())
}

/// Whether the weights a node needs are already on the device.
///
/// There is no model acquisition path in v0.1 — nothing downloads a model and
/// no entity in `DOCS/ARCHITECTURE.md` §3 owns a model store — so no node's
/// weights are present, and `NeedsModel` is the truthful state for every node
/// that holds any. This becomes a real query against the model store on the day
/// one exists; it is a fact about the build, not a placeholder standing in for
/// one.
const MODEL_PRESENT: bool = false;

/// Full pre-run validation of a pipeline document.
///
/// Returns every error the graph carries rather than the first, so the editor
/// annotates the whole pipeline in one pass. An empty vector means the graph
/// runs on this device.
#[tauri::command]
async fn cmd_validate(doc: PipelineDoc) -> Result<Vec<ValidationError>, String> {
    dispatch(move || {
        let caps = profile()?;
        Ok(validate_graph(&doc.graph, &caps).err().unwrap_or_default())
    })
    .await
}

/// Plans a pipeline and hands the plan over for execution, returning the job id.
///
/// The plan is written to `jobs/<job_id>.json` under the app data root and the
/// handler returns. It deliberately does **not** execute: AD-4 puts jobs in a
/// foreground service through WorkManager and nowhere else, so running one here
/// — even on a worker — would be the exact hazard the decision exists to avoid.
/// `JobForegroundService` (T18) is what picks the plan up.
///
/// Planning validates first, so an invalid graph is rejected with every reason
/// at once and leaves no plan file behind.
#[tauri::command]
async fn cmd_start_job(doc: PipelineDoc) -> Result<String, String> {
    dispatch(move || {
        let root = data_root()?;
        let caps = profile()?;

        // `plan` consults no runner — it validates, orders and segments the
        // graph. The executor supplies the real `SegmentRunner`.
        let scheduler = Scheduler::new(
            AssetStore::new(root.join("assets")),
            CheckpointStore::new(root.join("checkpoints")),
            Box::new(NullSegmentRunner),
        );

        let plan = scheduler.plan(&doc.graph, &caps).map_err(|errors| {
            errors
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        })?;

        let jobs = root.join("jobs");
        std::fs::create_dir_all(&jobs).map_err(|e| e.to_string())?;
        let handover = jobs.join(format!("{}.json", plan.job_id));
        let bytes = serde_json::to_vec_pretty(&plan).map_err(|e| e.to_string())?;
        std::fs::write(&handover, bytes).map_err(|e| e.to_string())?;

        Ok(plan.job_id)
    })
    .await
}

/// The resolved availability of every node kind, for the palette and the editor.
///
/// One entry per `NodeKind`, resolved through `resolve_availability` — the
/// single place the gating precedence rule is expressed. This command reads that
/// answer and re-derives none of it, which is what keeps a tier-limited node
/// from ever reaching the UI as a lock, a price or a credit cost.
///
/// The entitlement seam is `NullEntitlementService` — free, no credits, no
/// prices — until `BillingBridge` (T18) puts RevenueCat behind it. That is the
/// seam working as designed (AD-9), not a stand-in: nothing in the core is bound
/// to a processor.
#[tauri::command]
async fn cmd_availability() -> Result<Vec<(NodeKind, NodeAvailability)>, String> {
    dispatch(|| {
        let caps = profile()?;
        let service = NullEntitlementService;
        let entitlement = service.entitlement();
        let balance = service.credit_balance();
        let pricing = service.pricing();

        Ok(NodeKind::ALL
            .iter()
            .map(|&kind| {
                let state = resolve_availability(
                    kind,
                    &caps,
                    &entitlement,
                    balance,
                    &pricing,
                    MODEL_PRESENT,
                );
                (kind, state)
            })
            .collect())
    })
    .await
}

/// Starts the shell.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let root = app.path().app_data_dir()?;
            std::fs::create_dir_all(&root)?;
            // `set` can only fail if `setup` somehow ran twice. The first root
            // wins and the second is discarded rather than panicking an app
            // that has already launched.
            let _ = DATA_ROOT.set(root);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            cmd_validate,
            cmd_start_job,
            cmd_availability
        ])
        .run(tauri::generate_context!())
        .expect("the OnDroid MediaForge shell failed to start");
}
