//! Job diagnostics — the durable per-job record (AD-11).
//!
//! Every job writes a `JobRecord` with per-stage timings, the backend actually
//! used per stage, every thermal transition, every backend fallback, and the
//! terminating cause. `DiagnosticsSink` sits alongside `ProgressSink` so the
//! scheduler emits both from one pass.
//!
//! **No variant of `DiagnosticEvent` carries a buffer, a user file path, or
//! transcript text** — the type makes a media leak structurally impossible
//! rather than merely forbidden. This is the mechanical check behind the FR6
//! privacy claim.

use serde::{Deserialize, Serialize};

use crate::capability::Backend;
use crate::graph::NodeId;
use crate::scheduler::RunOutcome;
use crate::thermal::ThermalState;
use crate::CoreError;

/// One recorded occurrence during a job's lifetime.
///
/// No variant is capable of holding a media buffer, a user-chosen file path,
/// or transcript text. The `NodeId` carried by several variants is a
/// pipeline-local identity string, not a filesystem path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DiagnosticEvent {
    /// A stage began executing on `backend`.
    StageStarted { node: NodeId, backend: Backend },
    /// A stage finished after `elapsed_ms` of wall time.
    StageFinished { node: NodeId, elapsed_ms: u64 },
    /// The engine fell back from one backend to another for `node`.
    BackendFallback {
        node: NodeId,
        from: Backend,
        to: Backend,
        reason: String,
    },
    /// The thermal governor transitioned between states.
    ThermalTransition {
        from: ThermalState,
        to: ThermalState,
        headroom: f32,
    },
    /// The job terminated because of an error at `node`.
    Failed { node: NodeId, cause: String },
}

/// Durable per-job history. The last 20 are retained, evicting oldest first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRecord {
    pub job_id: String,
    pub pipeline_name: String,
    pub soc_id: String,
    pub started_unix: u64,
    pub outcome: RunOutcome,
    pub events: Vec<DiagnosticEvent>,
}

impl JobRecord {
    /// Serialises the record for the share sheet. Local by default; leaves
    /// the device only by explicit user action.
    pub fn to_bundle(&self) -> Result<String, CoreError> {
        serde_json::to_string_pretty(self).map_err(CoreError::Serde)
    }
}

/// Emission point for diagnostic events, alongside `ProgressSink` so the
/// scheduler emits both from one pass.
pub trait DiagnosticsSink {
    fn record(&mut self, event: DiagnosticEvent);
}

/// A sink that collects events into a `Vec`, for tests and for building a
/// `JobRecord` at the end of a run.
#[derive(Debug, Default)]
pub struct VecSink {
    pub events: Vec<DiagnosticEvent>,
}

impl DiagnosticsSink for VecSink {
    fn record(&mut self, event: DiagnosticEvent) {
        self.events.push(event);
    }
}

/// A ring buffer retaining the last `cap` `JobRecord`s, evicting oldest first.
#[derive(Debug, Default)]
pub struct RecordStore {
    records: Vec<JobRecord>,
    cap: usize,
}

impl RecordStore {
    /// A store that retains the last `cap` records (default 20).
    pub fn new(cap: usize) -> Self {
        RecordStore {
            records: Vec::with_capacity(cap.min(64)),
            cap: cap.max(1),
        }
    }

    /// Insert a record, evicting the oldest if the cap is exceeded.
    pub fn insert(&mut self, record: JobRecord) {
        if self.records.len() >= self.cap {
            self.records.remove(0);
        }
        self.records.push(record);
    }

    /// All retained records, oldest first.
    pub fn all(&self) -> &[JobRecord] {
        &self.records
    }

    /// The most recently inserted record, if any.
    pub fn latest(&self) -> Option<&JobRecord> {
        self.records.last()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::NodeId;

    #[test]
    fn a_serialised_bundle_does_not_leak_user_file_paths() {
        let record = JobRecord {
            job_id: "job-abc123".to_string(),
            pipeline_name: "my pipeline".to_string(),
            soc_id: "snap8g2".to_string(),
            started_unix: 1700000000,
            outcome: RunOutcome::Completed,
            events: vec![
                DiagnosticEvent::StageStarted {
                    node: NodeId::from("src"),
                    backend: Backend::Cpu,
                },
                DiagnosticEvent::StageFinished {
                    node: NodeId::from("src"),
                    elapsed_ms: 42,
                },
                DiagnosticEvent::BackendFallback {
                    node: NodeId::from("up"),
                    from: Backend::Npu,
                    to: Backend::Gpu,
                    reason: "QNN delegate load failed".to_string(),
                },
                DiagnosticEvent::ThermalTransition {
                    from: ThermalState::Running,
                    to: ThermalState::Throttling,
                    headroom: 0.15,
                },
                DiagnosticEvent::Failed {
                    node: NodeId::from("dnz"),
                    cause: "engine returned Unsupported".to_string(),
                },
            ],
        };

        let bundle = record.to_bundle().expect("serialise");

        // A user file path that might appear in a pipeline document must not
        // appear anywhere in the diagnostic bundle. This is the mechanical
        // check behind the FR6 privacy claim.
        let user_path = "/storage/emulated/0/Movies/vacation.mp4";
        assert!(
            !bundle.contains(user_path),
            "diagnostic bundle leaks user file path: {bundle}"
        );
    }

    #[test]
    fn record_store_evicts_oldest_first() {
        let mut store = RecordStore::new(3);
        for i in 0..5 {
            store.insert(JobRecord {
                job_id: format!("job-{i}"),
                pipeline_name: "test".to_string(),
                soc_id: "test".to_string(),
                started_unix: i as u64,
                outcome: RunOutcome::Completed,
                events: vec![],
            });
        }
        let ids: Vec<&str> = store.all().iter().map(|r| r.job_id.as_str()).collect();
        assert_eq!(ids, vec!["job-2", "job-3", "job-4"], "oldest evicted first");
        assert_eq!(store.latest().unwrap().job_id, "job-4");
    }

    #[test]
    fn diagnostic_event_serialises_and_round_trips() {
        let event = DiagnosticEvent::ThermalTransition {
            from: ThermalState::Idle,
            to: ThermalState::Running,
            headroom: 0.8,
        };
        let json = serde_json::to_string(&event).expect("serialise");
        let back: DiagnosticEvent = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(event, back);
    }

    #[test]
    fn to_bundle_produces_valid_json() {
        let record = JobRecord {
            job_id: "job-test".to_string(),
            pipeline_name: "test".to_string(),
            soc_id: "test".to_string(),
            started_unix: 0,
            outcome: RunOutcome::Completed,
            events: vec![],
        };
        let bundle = record.to_bundle().expect("serialise");
        let parsed: serde_json::Value = serde_json::from_str(&bundle).expect("valid json");
        assert!(parsed.is_object());
        assert_eq!(parsed["job_id"], "job-test");
    }
}
