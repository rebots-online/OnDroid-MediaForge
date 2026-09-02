//! Behavioural tests for the scheduler: planning, resume, cancellation and
//! thermal pausing.

use super::*;
use crate::capability::{Backend, DeviceTier, PROBE_SCHEMA_VERSION};
use crate::diagnostics::VecSink;
use crate::graph::{Edge, NodeKind, NodeSpec};
use crate::testdir::TestDir;
use std::path::Path;

fn profile(tier: DeviceTier) -> SocProfile {
    SocProfile {
        soc_id: "test".to_string(),
        soc_name: "test".to_string(),
        tier,
        ram_bytes: 8 * 1024 * 1024 * 1024,
        model_budget_bytes: 1024 * 1024 * 1024,
        backends: vec![Backend::Cpu],
        npu_experimental: false,
        probe_schema_version: PROBE_SCHEMA_VERSION,
    }
}

fn node(id: &str, kind: NodeKind) -> NodeSpec {
    NodeSpec {
        id: NodeId::from(id),
        kind,
        model: None,
        params: serde_json::Map::new(),
    }
}

fn edge(from: (&str, &str), to: (&str, &str)) -> Edge {
    Edge {
        from: (NodeId::from(from.0), from.1.to_string()),
        to: (NodeId::from(to.0), to.1.to_string()),
    }
}

/// A three-node audio chain that validates on a T0 desktop profile.
fn chain() -> Graph {
    Graph {
        nodes: vec![
            node("src", NodeKind::SourceAudio),
            node("dnz", NodeKind::AudioDenoise),
            node("out", NodeKind::SinkFiles),
        ],
        edges: vec![
            edge(("src", "audio"), ("dnz", "in")),
            edge(("dnz", "out"), ("out", "in")),
        ],
    }
}

#[derive(Default)]
struct RecordingSink {
    done: Vec<SegmentId>,
    states: Vec<ThermalState>,
}

impl ProgressSink for RecordingSink {
    fn segment_done(&mut self, id: SegmentId, _elapsed_ms: u64) {
        self.done.push(id);
    }
    fn thermal(&mut self, state: ThermalState) {
        self.states.push(state);
    }
}

/// Logs every segment it executes, and can trip a cancel token part way
/// through so a kill can be simulated deterministically.
struct LoggingRunner {
    log: Arc<Mutex<Vec<SegmentId>>>,
    cancel_after: Option<(usize, CancelToken)>,
}

impl SegmentRunner for LoggingRunner {
    fn run_segment(&mut self, segment: &Segment) -> Result<Vec<u8>, CoreError> {
        let mut log = self.log.lock().expect("log");
        log.push(segment.id);
        if let Some((after, token)) = &self.cancel_after {
            if log.len() >= *after {
                token.cancel();
            }
        }
        Ok(format!("{}", segment.id).into_bytes())
    }
}

fn scheduler(root: &Path, runner: Box<dyn SegmentRunner>, units: u64) -> Scheduler {
    let mut s = Scheduler::new(
        AssetStore::new(root.join("assets")),
        CheckpointStore::new(root.join("checkpoints")),
        runner,
    );
    s.units_per_node = units;
    s
}

#[test]
fn the_whole_segment_list_is_emitted_before_execution() {
    let dir = TestDir::new("scheduler-plan");
    let s = scheduler(dir.path(), Box::new(NullSegmentRunner), 2);
    let plan = s.plan(&chain(), &profile(DeviceTier::T0)).expect("plan");

    assert_eq!(plan.total, 6, "three nodes at two units each");
    assert_eq!(plan.segments.len(), plan.total);
    assert_eq!(plan.remaining(), 6);
    let ids: Vec<u64> = plan.segments.iter().map(|s| s.id.0).collect();
    assert_eq!(ids, vec![0, 1, 2, 3, 4, 5]);
    // Topological order: source before denoise before sink.
    let nodes: Vec<&str> = plan.segments.iter().map(|s| s.node.as_str()).collect();
    assert_eq!(nodes, vec!["src", "src", "dnz", "dnz", "out", "out"]);
}

#[test]
fn an_invalid_graph_yields_errors_without_partial_work() {
    let dir = TestDir::new("scheduler-invalid");
    let s = scheduler(dir.path(), Box::new(NullSegmentRunner), 1);
    let bad = Graph {
        nodes: vec![
            node("src", NodeKind::SourceAudio),
            node("up", NodeKind::ImageUpscale),
        ],
        edges: vec![edge(("src", "audio"), ("up", "in"))],
    };
    let errors = s.plan(&bad, &profile(DeviceTier::T0)).unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, ValidationError::TypeMismatch { .. })));
    assert!(
        !dir.path().join("checkpoints").exists(),
        "a rejected plan must not touch the checkpoint store"
    );
}

#[test]
fn a_killed_and_resumed_run_executes_each_segment_exactly_once() {
    let dir = TestDir::new("scheduler-resume");
    let log = Arc::new(Mutex::new(Vec::new()));
    let token = CancelToken::new();

    // First run: the runner trips the token after three segments.
    let mut first = scheduler(
        dir.path(),
        Box::new(LoggingRunner {
            log: Arc::clone(&log),
            cancel_after: Some((3, token.clone())),
        }),
        2,
    );
    let plan = first.plan(&chain(), &profile(DeviceTier::T0)).expect("plan");
    let mut sink = RecordingSink::default();
    let mut diag = VecSink::default();
    let outcome = first.run(&plan, &mut sink, &mut diag, &token).expect("first run");
    assert_eq!(outcome, RunOutcome::Cancelled { at: SegmentId(3) });
    assert_eq!(sink.done, vec![SegmentId(0), SegmentId(1), SegmentId(2)]);

    // Second run: a fresh scheduler over the same stores, as after a
    // process death. plan() precedes run(), as it always does.
    let mut second = scheduler(
        dir.path(),
        Box::new(LoggingRunner {
            log: Arc::clone(&log),
            cancel_after: None,
        }),
        2,
    );
    let replan = second.plan(&chain(), &profile(DeviceTier::T0)).expect("plan");
    assert_eq!(replan.job_id, plan.job_id, "the same graph is the same job");
    let mut sink = RecordingSink::default();
    let mut diag = VecSink::default();
    let fresh = CancelToken::new();
    let outcome = second.run(&replan, &mut sink, &mut diag, &fresh).expect("second run");
    assert_eq!(outcome, RunOutcome::Completed);
    assert_eq!(
        sink.done,
        vec![SegmentId(3), SegmentId(4), SegmentId(5)],
        "a resume must not re-report finished segments"
    );

    let executed = log.lock().expect("log").clone();
    let mut sorted = executed.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        executed.len(),
        sorted.len(),
        "a segment executed twice: {executed:?}"
    );
    assert_eq!(sorted.len(), 6, "every segment must execute: {executed:?}");
}

#[test]
fn an_edited_pipeline_does_not_resume_onto_the_old_checkpoint() {
    let dir = TestDir::new("scheduler-edited");
    let token = CancelToken::new();
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut first = scheduler(
        dir.path(),
        Box::new(LoggingRunner {
            log: Arc::clone(&log),
            cancel_after: Some((3, token.clone())),
        }),
        2,
    );
    let plan = first.plan(&chain(), &profile(DeviceTier::T0)).expect("plan");
    let mut sink = RecordingSink::default();
    let mut diag = VecSink::default();
    first.run(&plan, &mut sink, &mut diag, &token).expect("first run");

    // Same shape, different parameters: the job id is unchanged but the
    // plan hash is not, so the checkpoint is discarded.
    let mut edited = chain();
    edited.nodes[1]
        .params
        .insert("strength".to_string(), serde_json::json!(0.9));

    let edited_log = Arc::new(Mutex::new(Vec::new()));
    let mut second = scheduler(
        dir.path(),
        Box::new(LoggingRunner {
            log: Arc::clone(&edited_log),
            cancel_after: None,
        }),
        2,
    );
    let replan = second.plan(&edited, &profile(DeviceTier::T0)).expect("plan");
    assert_eq!(replan.job_id, plan.job_id);
    let mut sink = RecordingSink::default();
    let mut diag = VecSink::default();
    let outcome = second
        .run(&replan, &mut sink, &mut diag, &CancelToken::new())
        .expect("second run");
    assert_eq!(outcome, RunOutcome::Completed);
    assert_eq!(
        sink.done.len(),
        6,
        "an edited pipeline restarts whole rather than splicing output"
    );
}

#[test]
fn a_hot_device_pauses_instead_of_running_the_next_segment() {
    let dir = TestDir::new("scheduler-heat");
    let mut s = scheduler(dir.path(), Box::new(NullSegmentRunner), 2);
    let plan = s.plan(&chain(), &profile(DeviceTier::T0)).expect("plan");
    s.set_headroom(1.0);
    let mut sink = RecordingSink::default();
    let mut diag = VecSink::default();
    let outcome = s
        .run(&plan, &mut sink, &mut diag, &CancelToken::new())
        .expect("run");

    // Three derations run their segments; the fourth boundary pauses.
    assert_eq!(outcome, RunOutcome::PausedForHeat { at: SegmentId(3) });
    assert_eq!(sink.done, vec![SegmentId(0), SegmentId(1), SegmentId(2)]);
    assert_eq!(sink.states.last(), Some(&ThermalState::Throttling));
}

#[test]
fn marking_complete_through_updates_the_plan_for_the_monitor() {
    let dir = TestDir::new("scheduler-mark");
    let s = scheduler(dir.path(), Box::new(NullSegmentRunner), 2);
    let mut plan = s.plan(&chain(), &profile(DeviceTier::T0)).expect("plan");
    plan.mark_complete_through(SegmentId(2));
    assert_eq!(plan.remaining(), 3);
    assert_eq!(plan.total, 6, "the denominator never moves");
}
