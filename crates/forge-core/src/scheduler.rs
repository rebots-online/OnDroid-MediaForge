//! Segment planning and execution (AD-8).
//!
//! The whole segment list is emitted before execution starts, which is what
//! makes progress deterministic and non-resetting: `JobPlan::total` is known up
//! front, so a thermal pause, a cancellation or a process death never rewrites
//! the denominator. Each completed segment writes a checkpoint, and a resume
//! skips what is already recorded.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::assets::AssetStore;
use crate::capability::SocProfile;
use crate::checkpoint::{plan_hash, CheckpointStore, JobCheckpoint};
use crate::graph::{Graph, NodeId};
use crate::thermal::{ThermalAction, ThermalGovernor, ThermalState};
use crate::validate::{validate_graph, ValidationError};
use crate::CoreError;

/// Identity of one segment within a plan. Stable across a resume: it is the
/// segment's position in the plan, and the plan is emitted in full before
/// execution starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SegmentId(pub u64);

impl std::fmt::Display for SegmentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Where a segment stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SegmentState {
    Pending,
    Running,
    Complete,
    Failed,
}

/// One resumable unit of work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segment {
    pub id: SegmentId,
    pub node: NodeId,
    pub range: Range<u64>,
    pub state: SegmentState,
}

/// The full segment list produced before execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobPlan {
    pub job_id: String,
    pub segments: Vec<Segment>,
    pub total: usize,
}

impl JobPlan {
    /// Mark every segment up to and including `id` complete. The job monitor
    /// renders a plan, so a resumed job shows its already-finished segments
    /// without re-deriving them.
    pub fn mark_complete_through(&mut self, id: SegmentId) {
        for segment in &mut self.segments {
            if segment.id <= id {
                segment.state = SegmentState::Complete;
            }
        }
    }

    /// Segments not yet complete.
    pub fn remaining(&self) -> usize {
        self.segments
            .iter()
            .filter(|s| s.state != SegmentState::Complete)
            .count()
    }
}

/// Progress transport to UI.
pub trait ProgressSink {
    fn segment_done(&mut self, id: SegmentId, elapsed_ms: u64);
    fn thermal(&mut self, state: ThermalState);
}

/// User-initiated stop, checked at every segment boundary — which is already
/// the checkpoint boundary, so the cost is negligible.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(pub Arc<AtomicBool>);

impl CancelToken {
    /// A token that has not been cancelled.
    pub fn new() -> Self {
        CancelToken(Arc::new(AtomicBool::new(false)))
    }

    /// Ask the run to stop at the next segment boundary.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether a stop has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// The three ways a run ends. Governor pausing and user cancellation are
/// different events and the UI renders them differently.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunOutcome {
    Completed,
    Cancelled { at: SegmentId },
    PausedForHeat { at: SegmentId },
}

/// Executes one segment's work.
///
/// The core cannot depend on `forge-engines` — the dependency runs the other
/// way — and `Scheduler::run` takes no engine argument, so the work itself
/// arrives through this seam. `forge-cli` and the Android service supply an
/// implementation backed by the engine registry.
pub trait SegmentRunner {
    fn run_segment(&mut self, segment: &Segment) -> Result<Vec<u8>, CoreError>;
}

/// A runner that produces a deterministic marker for each segment. It performs
/// no inference, so it is what lets the plan, checkpoint and resume paths be
/// exercised end to end with no model and no accelerator present.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullSegmentRunner;

impl SegmentRunner for NullSegmentRunner {
    fn run_segment(&mut self, segment: &Segment) -> Result<Vec<u8>, CoreError> {
        Ok(format!("{}:{}:{}", segment.node, segment.range.start, segment.range.end).into_bytes())
    }
}

/// Plans a graph into segments and executes them.
pub struct Scheduler {
    pub assets: AssetStore,
    pub checkpoints: CheckpointStore,
    pub governor: ThermalGovernor,
    /// Segments each node is split into. Video jobs raise this from the media
    /// duration; a still-image or audio stage stays at one.
    pub units_per_node: u64,
    headroom: f32,
    runner: Box<dyn SegmentRunner>,
    /// Plan hash by job id, filled by `plan` and read by `run`. `run`'s
    /// signature carries only the plan, and a plan cannot regenerate the hash
    /// of the graph it came from.
    plan_hashes: Mutex<HashMap<String, String>>,
}

impl Scheduler {
    /// A scheduler writing to `assets` and `checkpoints`, executing through
    /// `runner`.
    pub fn new(
        assets: AssetStore,
        checkpoints: CheckpointStore,
        runner: Box<dyn SegmentRunner>,
    ) -> Self {
        Scheduler {
            assets,
            checkpoints,
            governor: ThermalGovernor::default(),
            units_per_node: 1,
            headroom: 0.0,
            runner,
            plan_hashes: Mutex::new(HashMap::new()),
        }
    }

    /// Latest thermal headroom, pushed by the platform's thermal reader.
    /// Consulted between segments.
    pub fn set_headroom(&mut self, headroom: f32) {
        self.headroom = headroom;
    }

    /// Graph to segment plan.
    ///
    /// Validation runs first and its errors are returned whole, so a rejected
    /// graph leaves no partial plan and no files behind.
    pub fn plan(&self, g: &Graph, caps: &SocProfile) -> Result<JobPlan, Vec<ValidationError>> {
        validate_graph(g, caps)?;
        let order = g.topological_order().map_err(|e| vec![e])?;

        // The job id is the pipeline's shape, so editing a parameter resumes
        // the same job; the plan hash covers the parameters too, so that resume
        // is then refused rather than splicing two graphs' output together.
        let shape: String = order
            .iter()
            .filter_map(|id| g.node(id).map(|n| format!("{}:{:?};", n.id, n.kind)))
            .collect();
        let job_id = format!("job-{}", &plan_hash(&shape)[..16]);
        let canonical = serde_json::to_string(g).unwrap_or_else(|_| shape.clone());
        let hash = plan_hash(&canonical);

        let mut segments = Vec::new();
        for node in order {
            for unit in 0..self.units_per_node.max(1) {
                segments.push(Segment {
                    id: SegmentId(segments.len() as u64),
                    node: node.clone(),
                    range: unit..unit + 1,
                    state: SegmentState::Pending,
                });
            }
        }

        if let Ok(mut cache) = self.plan_hashes.lock() {
            cache.insert(job_id.clone(), hash);
        }

        let total = segments.len();
        Ok(JobPlan {
            job_id,
            segments,
            total,
        })
    }

    /// Executes a plan, honouring the governor and the cancel token, emitting
    /// progress.
    ///
    /// Segments run in order. Before each one the cancel token and the governor
    /// are consulted; after each one the output is stored and a checkpoint
    /// written. A resume skips every segment the checkpoint already records as
    /// complete, so each segment executes exactly once across a kill.
    pub fn run(
        &mut self,
        plan: &JobPlan,
        sink: &mut dyn ProgressSink,
        cancel: &CancelToken,
    ) -> Result<RunOutcome, CoreError> {
        let hash = self
            .plan_hashes
            .lock()
            .ok()
            .and_then(|c| c.get(&plan.job_id).cloned())
            .unwrap_or_else(|| plan_hash(&plan.job_id));

        // A checkpoint written under a different plan is not a resume point.
        // Discarding it restarts the job whole rather than mixing output from
        // two graphs.
        let resume_after = match self.checkpoints.resume(&plan.job_id)? {
            Some(checkpoint) if checkpoint.matches_plan(&hash) => Some(checkpoint.last_segment),
            Some(_) => {
                self.checkpoints.clear(&plan.job_id)?;
                None
            }
            None => None,
        };

        for segment in &plan.segments {
            if resume_after.is_some_and(|last| segment.id <= last) {
                continue;
            }
            if cancel.is_cancelled() {
                return Ok(RunOutcome::Cancelled { at: segment.id });
            }
            if self.governor.step(self.headroom) == ThermalAction::Pause {
                sink.thermal(self.governor.state());
                return Ok(RunOutcome::PausedForHeat { at: segment.id });
            }
            sink.thermal(self.governor.state());

            let started = Instant::now();
            let output = self.runner.run_segment(segment)?;
            self.assets.put(&output)?;
            self.checkpoints
                .record(&JobCheckpoint::new(&plan.job_id, segment.id, &hash))?;
            sink.segment_done(segment.id, started.elapsed().as_millis() as u64);
        }

        self.checkpoints.clear(&plan.job_id)?;
        Ok(RunOutcome::Completed)
    }
}

#[cfg(test)]
mod tests;
