//! `forge-core` — the OnDroid MediaForge pipeline model and execution core.
//!
//! The core owns the pipeline: graph model, validation, capability probing,
//! availability resolution, the entitlement seam, tiling, thermal governance,
//! the content-addressed asset store, checkpointing and the scheduler.
//! Nothing here is Android-specific; `forge-cli` exercises the whole of it on
//! desktop Linux with no device attached (AD-10).

pub mod availability;
pub mod capability;
pub mod entitlement;
pub mod graph;
pub mod thermal;
pub mod tiler;
pub mod validate;

/// Every failure the core can surface to a host.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    /// A filesystem or stream operation failed.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// A pipeline document or cached profile could not be (de)serialised.
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
    /// The device capability probe could not complete.
    #[error("probe: {0}")]
    Probe(String),
    /// An inference engine refused to load or run.
    #[error("engine: {0}")]
    Engine(String),
}
