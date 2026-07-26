//! `forge-cli` — the desktop harness (AD-10).
//!
//! The primary development loop: the graph model, validator, scheduler, tiler,
//! asset store and the ONNX Runtime CPU path all run here with no device
//! attached, and the same pipeline document runs here and on the phone. The
//! device is reserved for what only the device can prove.
//!
//! ```text
//! forge-cli run <pipeline.json> [--tier t0|t1|t2] [--work <dir>] [--units N]
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{anyhow, bail, Context};
use forge_core::assets::AssetStore;
use forge_core::capability::{probe_device, DeviceTier, SocProfile};
use forge_core::checkpoint::CheckpointStore;
use forge_core::entitlement::{gated_with_entitlement, NullEntitlementService};
use forge_core::graph::{NodeId, NodeKind};
use forge_core::pipeline::PipelineDoc;
use forge_core::scheduler::{
    CancelToken, JobPlan, ProgressSink, RunOutcome, Scheduler, Segment, SegmentId, SegmentRunner,
};
use forge_core::thermal::ThermalState;
use forge_core::validate::validate_graph;
use forge_core::CoreError;
use forge_engines::registry::{BackendChain, EngineRegistry};
use forge_engines::{DType, NullEngine, TensorRef};

/// Parsed command line.
struct Options {
    document: PathBuf,
    tier: Option<DeviceTier>,
    work: PathBuf,
    units_per_node: u64,
}

fn usage() -> &'static str {
    "usage: forge-cli run <pipeline.json> [--tier t0|t1|t2] [--work <dir>] [--units <n>]"
}

fn parse_tier(value: &str) -> anyhow::Result<DeviceTier> {
    match value.to_ascii_lowercase().as_str() {
        "t0" => Ok(DeviceTier::T0),
        "t1" => Ok(DeviceTier::T1),
        "t2" => Ok(DeviceTier::T2),
        other => bail!("unknown tier {other}; expected t0, t1 or t2"),
    }
}

fn parse_args(args: &[String]) -> anyhow::Result<Options> {
    match args.first().map(String::as_str) {
        Some("run") => {}
        Some(other) => bail!("unknown command {other}\n{}", usage()),
        None => bail!("{}", usage()),
    }

    let document = PathBuf::from(args.get(1).ok_or_else(|| anyhow!("{}", usage()))?);
    let mut options = Options {
        document,
        tier: None,
        // Working storage is project-relative, never /tmp (I-8).
        work: PathBuf::from(".tmp/forge-cli"),
        units_per_node: 1,
    };

    let mut index = 2;
    while index < args.len() {
        let flag = args[index].as_str();
        let value = || {
            args.get(index + 1)
                .cloned()
                .ok_or_else(|| anyhow!("{flag} needs a value"))
        };
        match flag {
            "--tier" => options.tier = Some(parse_tier(&value()?)?),
            "--work" => options.work = PathBuf::from(value()?),
            "--units" => {
                let raw = value()?;
                options.units_per_node = raw
                    .parse()
                    .with_context(|| format!("--units expects a number, got {raw}"))?;
            }
            other => bail!("unknown option {other}\n{}", usage()),
        }
        index += 2;
    }
    Ok(options)
}

/// Prints one line per completed segment, and each thermal transition.
struct ConsoleSink {
    total: usize,
    done: usize,
    last_state: Option<ThermalState>,
}

impl ProgressSink for ConsoleSink {
    fn segment_done(&mut self, id: SegmentId, elapsed_ms: u64) {
        self.done += 1;
        println!(
            "  [{:>3}/{:<3}] segment {id}  {elapsed_ms:>5} ms",
            self.done, self.total
        );
    }

    fn thermal(&mut self, state: ThermalState) {
        if self.last_state != Some(state) {
            println!("  thermal: {state:?}");
            self.last_state = Some(state);
        }
    }
}

/// Executes a segment through the engine registry, behind the entitlement gate.
///
/// Every segment passes through `gated_with_entitlement`, so the desktop run
/// exercises the same choke point the phone does — with `NullEntitlementService`
/// standing in for the billing bridge.
struct RegistryRunner {
    kinds: HashMap<NodeId, NodeKind>,
    registry: EngineRegistry,
    chain: BackendChain,
    caps: SocProfile,
    entitlement: NullEntitlementService,
}

impl SegmentRunner for RegistryRunner {
    fn run_segment(&mut self, segment: &Segment) -> Result<Vec<u8>, CoreError> {
        let RegistryRunner {
            kinds,
            registry,
            chain,
            caps,
            entitlement,
        } = self;

        let kind = *kinds
            .get(&segment.node)
            .ok_or_else(|| CoreError::Engine(format!("no node {} in the graph", segment.node)))?;

        let payload = format!(
            "{}:{}..{}",
            segment.node, segment.range.start, segment.range.end
        )
        .into_bytes();

        let outcome = gated_with_entitlement(kind, entitlement, caps, || {
            let engine = registry.acquire(kind, chain)?;
            let shape = [payload.len()];
            let inputs = [("in", TensorRef::new(&shape, DType::U8, &payload))];
            engine.run(&inputs)
        })
        .map_err(|e| CoreError::Engine(e.to_string()))?;

        let outputs = outcome.map_err(|e| CoreError::Engine(e.to_string()))?;
        Ok(outputs
            .into_iter()
            .next()
            .map(|(_, tensor)| tensor.data)
            .unwrap_or_default())
    }
}

fn describe(caps: &SocProfile) {
    println!(
        "device: {} ({}) tier {:?}  ram {} MiB  model budget {} MiB  backends {:?}{}",
        caps.soc_name,
        caps.soc_id,
        caps.tier,
        caps.ram_bytes / (1024 * 1024),
        caps.model_budget_bytes / (1024 * 1024),
        caps.backends,
        if caps.npu_experimental {
            "  (npu experimental)"
        } else {
            ""
        }
    );
}

fn build_runner(doc: &PipelineDoc, caps: &SocProfile) -> RegistryRunner {
    let mut registry = EngineRegistry::new();
    // Desktop has no accelerator; the CPU engine is what every chain lands on.
    registry.register(
        forge_core::capability::Backend::Cpu,
        Box::new(|| Box::new(NullEngine::new())),
    );

    RegistryRunner {
        kinds: doc
            .graph
            .nodes
            .iter()
            .map(|n| (n.id.clone(), n.kind))
            .collect(),
        registry,
        chain: BackendChain::new(caps.backends.clone()),
        caps: caps.clone(),
        entitlement: NullEntitlementService,
    }
}

fn summarise(plan: &JobPlan, sink: &ConsoleSink, outcome: &RunOutcome, elapsed_ms: u128) {
    let verdict = match outcome {
        RunOutcome::Completed => "completed".to_string(),
        RunOutcome::Cancelled { at } => format!("cancelled at segment {at}"),
        RunOutcome::PausedForHeat { at } => format!("paused for heat at segment {at}"),
    };
    println!(
        "{verdict}: {}/{} segments in {elapsed_ms} ms",
        sink.done, plan.total
    );
}

/// Report validation errors one per line and fail the process.
///
/// The per-line output is the human-facing form; the returned error is what
/// makes the exit status non-zero.
fn reject(errors: &[forge_core::validate::ValidationError]) -> anyhow::Error {
    for error in errors {
        eprintln!("{error}");
    }
    anyhow!(
        "{} validation error{} — the pipeline cannot run",
        errors.len(),
        if errors.len() == 1 { "" } else { "s" }
    )
}

fn run(options: Options) -> anyhow::Result<()> {
    let doc = PipelineDoc::from_path(&options.document)
        .with_context(|| format!("reading {}", options.document.display()))?;

    let mut caps = probe_device().context("probing the desktop profile")?;
    if let Some(tier) = options.tier {
        caps.tier = tier;
    }

    println!("pipeline: {} (v{})", doc.name, doc.version);
    describe(&caps);

    if let Err(errors) = validate_graph(&doc.graph, &caps) {
        return Err(reject(&errors));
    }

    let work: &Path = &options.work;
    let mut scheduler = Scheduler::new(
        AssetStore::new(work.join("assets")),
        CheckpointStore::new(work.join("checkpoints")),
        Box::new(build_runner(&doc, &caps)),
    );
    scheduler.units_per_node = options.units_per_node;

    let plan = scheduler.plan(&doc.graph, &caps).map_err(|e| reject(&e))?;
    println!("plan {}: {} segments", plan.job_id, plan.total);

    let mut sink = ConsoleSink {
        total: plan.total,
        done: 0,
        last_state: None,
    };
    let started = Instant::now();
    let outcome = scheduler
        .run(&plan, &mut sink, &CancelToken::new())
        .context("running the plan")?;
    summarise(&plan, &sink, &outcome, started.elapsed().as_millis());

    match outcome {
        RunOutcome::Completed => Ok(()),
        RunOutcome::Cancelled { at } => Err(anyhow!("the run was cancelled at segment {at}")),
        RunOutcome::PausedForHeat { at } => {
            Err(anyhow!("the run paused for heat at segment {at}"))
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run(parse_args(&args)?)
}
