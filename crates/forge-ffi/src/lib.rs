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

use tauri::Manager;

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
#[allow(dead_code)]
pub(crate) fn data_root() -> Result<&'static Path, String> {
    DATA_ROOT
        .get()
        .map(PathBuf::as_path)
        .ok_or_else(|| "the app data directory is not resolved yet".to_string())
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
        .run(tauri::generate_context!())
        .expect("the OnDroid MediaForge shell failed to start");
}
