# ARCHITECTURE — OnDroid MediaForge

End-state snapshot. The entity table is frozen vocabulary: `CHECKLIST.md` tasks
cite these names verbatim, and two coders reading only their own task must arrive
at the same paths and signatures.

Entity rows carry the module path rather than `file:line` because no source file
exists yet. The path and the signature are the contract; line numbers are
back-filled after first commit.

## 1. Shape

```
┌──────────────────────── Android app process ────────────────────────┐
│  ┌──────────── WebView (system) ────────────┐                       │
│  │ recipe view · node canvas · inspector    │                       │
│  │ job monitor · gallery · wallet           │                       │
│  └────────────────▲─────────────────────────┘                       │
│      Tauri 2 IPC  │ raw payloads for previews, convertFileSrc media │
│  ┌────────────────┴─────────────────┐   ┌────────────────────────┐  │
│  │      RUST CORE (cdylib)          │   │  Kotlin plugin layer   │  │
│  │  graph · validate · scheduler    │◄──┤  LiteRtBridge          │  │
│  │  assets · tiler · thermal        │JNI│  MediaCodecBridge      │  │
│  │  checkpoint · entitlement seam   │   │  BillingBridge         │  │
│  │  engines: ort · ggml · ncnn      │   │  JobForegroundService  │  │
│  └──────────────────────────────────┘   └────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
        Hexagon NPU · Adreno/Xclipse GPU · big-core CPU
```

The Rust core owns the pipeline. Kotlin owns Android. The boundary is narrow and
one-directional: Rust calls into Kotlin for LiteRT inference, hardware media
codecs, billing, and foreground-service lifecycle; Kotlin never reaches into
pipeline logic.

## 2. Architecture decisions

**AD-1 — Tauri 2 shell, with a documented fallback.** The shell is Tauri 2
targeting Android. It matches the organisation's default target architecture, its
Kotlin plugin system with JNI bridging is the exact shape needed for LiteRT,
MediaCodec and WorkManager, and raw IPC payloads avoid JSON serialisation for
media buffers. Two hazards are designed around rather than discovered: plugin
commands run on the main thread by default, so every command handler dispatches
to a worker and returns immediately (AD-4); and desktop node-graph libraries are
documented as unusable on touch, so the canvas ships a bespoke touch interaction
layer (AD-7). The fallback, if Tauri's mobile developer experience blocks UI work
through M1, is a Compose host calling the same Rust cdylib through UniFFI with
the same WebView canvas — the core and the UI both survive that swap; only the
shell changes. The decision point is the end of M1.

**AD-2 — Two runtimes in the core, two satellites.** ONNX Runtime with the QNN
execution provider is the workhorse for deterministic media operations: the audio
stack, LaMa, super-resolution and matting. It has the best Rust story and the
broadest model coverage. LiteRT with vendor NPU accelerators carries generative
and LLM stages, because that is where the platform optimisation effort is
concentrated. ggml covers batch speech-to-text everywhere and is the LLM
portability floor below T1. NCNN-Vulkan covers frame interpolation and the ncnn
super-resolution ports, and works on Exynos GPUs.

**AD-3 — Engines are acquired as published binaries, never built from source.**
ONNX Runtime, the Qualcomm QNN runtime, and the LiteRT delegate are all published
Maven artifacts. Building them from source for four ABIs is a multi-week trap
with a prebuilt path already available. Any large vendored source lands under
`vendored-in-code/<source-domain>/<component>/` with a provenance stub written
before the tree is assimilated.

**AD-4 — Nothing long-running executes on the main thread.** Tauri plugin
commands dispatch to a worker pool or a coroutine and return immediately. Jobs
run in a foreground service through WorkManager, never inside a command handler.
This is what keeps the app off the ANR path.

**AD-5 — Fixed-shape execution plus tiling.** The QNN execution provider requires
fixed input shapes. `Tiler` normalises arbitrary media into fixed tiles with
overlap blending — 512² for inpainting, 128² for super-resolution, matching
QuickSRNet's native design. This is also what bounds RAM and smooths thermal
load.

**AD-6 — Compilation caches are a first-class product feature.** ORT QNN context
binaries and LiteRT compilation caches are stored per device per model on first
run. The verified 7,465 ms cold to 198 ms warm initialisation delta is the
difference between a toy and a product, so the first-run compile is surfaced in
the UI as a one-time step rather than hidden.

**AD-7 — Touch-first canvas over a conventional renderer.** Connection is
tap-port then tap-port, not drag. Long-press opens the palette. Ports snap
magnetically and every handle is at least 44 dp. The linear recipe view, not the
canvas, is the default presentation, because most creator pipelines are chains
rather than graphs.

**AD-8 — Segment-based processing with checkpointing.** Video processes in
N-second segments written to the asset store. A thermal pause or process death
resumes at the last completed segment. Progress is therefore always deterministic
and never resets.

**AD-9 — Entitlement is a swappable seam, and capability outranks it.**
`EntitlementService` is a trait; RevenueCat is the authoritative implementation
and no processor is load-bearing in the core. `resolve_availability` encodes the
precedence rule in one place: tier limitation beats every commercial state, so a
node the silicon cannot run can never render as a lock, a price, or a credit
cost. Credits spend from a locally-held signed reserve drawn in blocks while
online, so a generation never blocks on a network round-trip.

**AD-11 — Diagnosability is a product requirement, not a debug affordance.** The
product sells unattended overnight jobs on hardware that throttles. When one
fails at 3 a.m. the user has no console, no logcat, and no way to describe what
happened. Every job therefore writes a durable `JobRecord` with per-stage
timings, the backend actually used per stage, every thermal transition, every
backend fallback, and the terminating cause. `DiagnosticsSink` sits alongside
`ProgressSink` so the scheduler emits both from one pass. Records stay on the
device and leave only by explicit user action through the share sheet. The
payload is structurally incapable of carrying media: `DiagnosticEvent` has no
variant that holds a buffer, a user file path, or transcript text, which is
what makes the FR6 promise checkable rather than merely asserted.

**AD-10 — Desktop-first testability.** `forge-cli` runs the graph model,
validator, scheduler, tiler, asset store and the ONNX Runtime CPU path on desktop
Linux with no device attached. The same pipeline document runs there and on the
phone. This is the primary development loop; the device is reserved for what only
the device can prove.

## 3. Entity table

### `crates/forge-core` — pipeline model and execution

| Entity | Module | Role | Signature |
| --- | --- | --- | --- |
| `PortType` | `graph.rs` | Typed port discriminant | `enum PortType { Audio, Video, Image, Mask, Text, Tensor }` |
| `NodeId` | `graph.rs` | Stable node identity within a graph | `struct NodeId(String)` |
| `NodeKind` | `graph.rs` | Which of the 16 v1 node types | `enum NodeKind { SourceVideo, SourceImage, SourceAudio, AudioSplit, AudioDenoise, AudioIsolateVoice, AudioStems, Transcribe, Diarize, ImageUpscale, ImageObjectRemove, GenerativeFill, ImageCutout, VideoUpscale, VideoRemoveBg, VideoInterpolate, MetadataGen, CaptionFrames, MaskHelper, AvMux, SinkGallery, SinkFiles }` |
| `Port` | `graph.rs` | One input or output socket | `struct Port { name: String, ty: PortType }` |
| `NodeSpec` | `graph.rs` | A node instance with its parameters | `struct NodeSpec { id: NodeId, kind: NodeKind, model: Option<String>, params: serde_json::Map<String, Value> }` |
| `Edge` | `graph.rs` | A typed connection | `struct Edge { from: (NodeId, String), to: (NodeId, String) }` |
| `Graph` | `graph.rs` | The pipeline itself | `struct Graph { nodes: Vec<NodeSpec>, edges: Vec<Edge> }` |
| `Graph::topological_order` | `graph.rs` | Execution order, cycle-detecting | `fn topological_order(&self) -> Result<Vec<NodeId>, ValidationError>` |
| `ValidationError` | `validate.rs` | Why a graph cannot run | `enum ValidationError { TypeMismatch { edge: Edge, expected: PortType, found: PortType }, Cycle(Vec<NodeId>), MissingInput { node: NodeId, port: String }, TierUnavailable { node: NodeId, required: DeviceTier }, ExclusiveFamilies { a: NodeId, b: NodeId, families: (StageFamily, StageFamily) } }` |
| `validate_graph` | `validate.rs` | Full pre-run validation | `fn validate_graph(g: &Graph, caps: &SocProfile) -> Result<(), Vec<ValidationError>>` |
| `PipelineDoc` | `pipeline.rs` | Serde form of a shareable pipeline file | `struct PipelineDoc { version: u32, name: String, graph: Graph }` |
| `DeviceTier` | `capability.rs` | Hardware tier | `enum DeviceTier { T0, T1, T2 }` |
| `Backend` | `capability.rs` | Where a node executes | `enum Backend { Npu, Gpu, Cpu }` |
| `SocProfile` | `capability.rs` | Probed device facts, cached after first launch. `probe_schema_version` guards against a stale cache surviving a probe-logic change and pinning a device to the wrong tier forever. `model_budget_bytes` is the memory available for resident models after OS and app overhead | `struct SocProfile { soc_id: String, soc_name: String, tier: DeviceTier, ram_bytes: u64, model_budget_bytes: u64, backends: Vec<Backend>, npu_experimental: bool, probe_schema_version: u32 }` |
| `StageFamily` | `capability.rs` | Model class for co-residency exclusion | `enum StageFamily { Diffusion, LargeLanguage, Audio, Vision }` |
| `exclusive_families` | `capability.rs` | Pairs that must never be co-resident at a tier. The source research is explicit that on the 12 GB floor the scheduler serialises stage families and never holds diffusion and an LLM together at T0/T1 | `fn exclusive_families(tier: DeviceTier) -> Vec<(StageFamily, StageFamily)>` |
| `probe_device` | `capability.rs` | First-launch probe: SoC id, delegate load test, timed micro-benchmark | `fn probe_device() -> Result<SocProfile, CoreError>` |
| `NodeAvailability` | `availability.rs` | The seven UI states | `enum NodeAvailability { Ready(Backend), Accelerated, NeedsModel { bytes: u64, license: String }, Experimental { estimate_ms: u64 }, Metered { credits: u32 }, ProLocked, TierLimited { required: DeviceTier, substitute: Option<NodeKind> } }` |
| `NodePricing` | `availability.rs` | Single source of per-node credit cost. Without it `resolve_availability` would have to invent the price it reports | `struct NodePricing(HashMap<NodeKind, u32>)` with `fn cost(&self, kind: NodeKind) -> Option<u32>` |
| `resolve_availability` | `availability.rs` | **The precedence rule lives here and nowhere else.** `TierLimited` is returned before any commercial state is considered. `balance` is what the user holds; `pricing` is what a node costs — the two were conflated in v0.1 and are now distinct | `fn resolve_availability(kind: NodeKind, caps: &SocProfile, ent: &Entitlement, balance: u32, pricing: &NodePricing, model_present: bool) -> NodeAvailability` |
| `Entitlement` | `entitlement.rs` | Commercial state | `enum Entitlement { Free, Pro { perpetual_version: Option<String> } }` |
| `EntitlementService` | `entitlement.rs` | The swappable seam | `trait EntitlementService { fn entitlement(&self) -> Entitlement; fn credit_balance(&self) -> u32; fn reserve_credits(&mut self, n: u32) -> Result<CreditReserve, EntitlementError>; fn reconcile(&mut self) -> Result<(), EntitlementError>; }` |
| `CreditReserve` | `entitlement.rs` | Locally-held signed block of credits spent offline, reconciled later | `struct CreditReserve { granted: u32, spent: u32, signature: Vec<u8> }` |
| `gated_with_entitlement` | `entitlement.rs` | Wrapper enforcing gating at the single choke point | `fn gated_with_entitlement<T>(kind: NodeKind, svc: &mut dyn EntitlementService, caps: &SocProfile, f: impl FnOnce() -> T) -> Result<T, GateError>` |
| `Segment` | `scheduler.rs` | One resumable unit of work | `struct Segment { id: SegmentId, node: NodeId, range: Range<u64>, state: SegmentState }` |
| `JobPlan` | `scheduler.rs` | The full segment list produced before execution, which is what makes progress deterministic | `struct JobPlan { job_id: String, segments: Vec<Segment>, total: usize }` |
| `Scheduler::plan` | `scheduler.rs` | Graph to segment plan | `fn plan(&self, g: &Graph, caps: &SocProfile) -> Result<JobPlan, Vec<ValidationError>>` |
| `CancelToken` | `scheduler.rs` | User-initiated stop, checked at every segment boundary — which is already the checkpoint boundary, so the cost is negligible | `struct CancelToken(Arc<AtomicBool>)` with `fn cancel(&self)` and `fn is_cancelled(&self) -> bool` |
| `RunOutcome` | `scheduler.rs` | Distinguishes the three ways a run ends. Governor pausing and user cancellation are different events and the UI renders them differently | `enum RunOutcome { Completed, Cancelled { at: SegmentId }, PausedForHeat { at: SegmentId } }` |
| `Scheduler::run` | `scheduler.rs` | Executes a plan, honouring the governor and the cancel token, emitting progress | `fn run(&mut self, plan: &JobPlan, sink: &mut dyn ProgressSink, cancel: &CancelToken) -> Result<RunOutcome, CoreError>` |
| `ProgressSink` | `scheduler.rs` | Progress transport to UI | `trait ProgressSink { fn segment_done(&mut self, id: SegmentId, elapsed_ms: u64); fn thermal(&mut self, state: ThermalState); }` |
| `AssetStore` | `assets.rs` | Content-addressed intermediate storage | `struct AssetStore { root: PathBuf }` with `fn put(&self, bytes: &[u8]) -> Result<AssetKey, CoreError>` and `fn get(&self, key: &AssetKey) -> Result<Vec<u8>, CoreError>` |
| `Tiler` | `tiler.rs` | Fixed-shape tiling with overlap blending | `struct Tiler { tile: u32, overlap: u32 }` with `fn tile(&self, w: u32, h: u32) -> Vec<TileSpec>` and `fn blend(&self, tiles: &[(TileSpec, Vec<u8>)], w: u32, h: u32) -> Vec<u8>` |
| `ThermalState` | `thermal.rs` | Five-state heat model driving the UI chip | `enum ThermalState { Idle, Running, Sustained, Throttling, Cooling }` |
| `ThermalGovernor::step` | `thermal.rs` | Degrades before pausing: NPU burst to GPU sustained, then widen stride, then pause | `fn step(&mut self, headroom: f32) -> ThermalAction` |
| `ThermalAction` | `thermal.rs` | Governor output | `enum ThermalAction { Continue, Derate(Backend), WidenStride(u32), Pause }` |
| `JobCheckpoint` | `checkpoint.rs` | Durable resume point | `struct JobCheckpoint { job_id: String, last_segment: SegmentId, plan_hash: String }` |
| `CheckpointStore::resume` | `checkpoint.rs` | Restores a killed job at its last completed segment | `fn resume(&self, job_id: &str) -> Result<Option<JobCheckpoint>, CoreError>` |
| `DiagnosticEvent` | `diagnostics.rs` | One recorded occurrence. **No variant carries a buffer, a user file path, or transcript text** — the type makes a media leak structurally impossible rather than merely forbidden | `enum DiagnosticEvent { StageStarted { node: NodeId, backend: Backend }, StageFinished { node: NodeId, elapsed_ms: u64 }, BackendFallback { node: NodeId, from: Backend, to: Backend, reason: String }, ThermalTransition { from: ThermalState, to: ThermalState, headroom: f32 }, Failed { node: NodeId, cause: String } }` |
| `JobRecord` | `diagnostics.rs` | Durable per-job history; last 20 retained and readable in-app | `struct JobRecord { job_id: String, pipeline_name: String, soc_id: String, started_unix: u64, outcome: RunOutcome, events: Vec<DiagnosticEvent> }` |
| `DiagnosticsSink` | `diagnostics.rs` | Emission point, alongside `ProgressSink` so the scheduler emits both in one pass | `trait DiagnosticsSink { fn record(&mut self, event: DiagnosticEvent); }` |
| `JobRecord::to_bundle` | `diagnostics.rs` | Serialises for the share sheet. Local by default; leaves the device only by explicit user action | `fn to_bundle(&self) -> Result<String, CoreError>` |

### `crates/forge-engines` — inference adapters

| Entity | Module | Role | Signature |
| --- | --- | --- | --- |
| `Engine` | `lib.rs` | Uniform inference interface. Inputs and outputs are **named and plural** — a diffusion UNet step takes latents, timestep and encoder hidden states, and Whisper takes mel features plus decoder state, so a single-tensor signature cannot express the models this product depends on | `trait Engine { fn load(&mut self, model: &ModelRef) -> Result<(), EngineError>; fn run(&mut self, inputs: &[(&str, TensorRef<'_>)]) -> Result<Vec<(String, TensorIo)>, EngineError>; fn backend(&self) -> Backend; }` |
| `TensorRef` | `lib.rs` | Borrowed input tensor. Inputs are borrowed rather than owned because a 1080p RGB frame is roughly 6 MB and the super-resolution budget is 2.2 ms per frame — an owned copy per stage per frame would dominate that budget | `struct TensorRef<'a> { shape: &'a [usize], dtype: DType, data: &'a [u8] }` |
| `TensorIo` | `lib.rs` | Owned output tensor, allocated from `TensorPool` | `struct TensorIo { shape: Vec<usize>, dtype: DType, data: Vec<u8> }` |
| `TensorPool` | `lib.rs` | Reuses output buffers across segments so steady-state inference does not churn the heap | `struct TensorPool { buffers: Vec<Vec<u8>> }` with `fn take(&mut self, bytes: usize) -> Vec<u8>` and `fn give(&mut self, buf: Vec<u8>)` |
| `BackendChain` | `registry.rs` | Ordered fallback list per node, e.g. `[Npu, Gpu, Cpu]` | `struct BackendChain(Vec<Backend>)` |
| `EngineRegistry::acquire` | `registry.rs` | Walks the chain and returns the first engine that loads; a load failure falls back rather than failing the pipeline | `fn acquire(&mut self, kind: NodeKind, chain: &BackendChain) -> Result<&mut dyn Engine, EngineError>` |
| `OrtEngine` | `ort.rs` | ONNX Runtime with QNN EP; owns EPContext cache paths | `struct OrtEngine { session: ort::Session, ctx_cache: PathBuf }` |
| `GgmlEngine` | `ggml.rs` | whisper.cpp and llama.cpp over FFI | `struct GgmlEngine { ctx: *mut c_void }` |
| `NcnnEngine` | `ncnn.rs` | NCNN-Vulkan for RIFE and ncnn SR ports | `struct NcnnEngine { net: ncnn::Net }` |
| `LiteRtEngine` | `litert.rs` | Proxies to Kotlin `LiteRtBridge` across JNI; used for generative and LLM stages | `struct LiteRtEngine { bridge: JniBridgeHandle }` |

### `crates/forge-ffi` and `crates/forge-cli`

| Entity | Module | Role | Signature |
| --- | --- | --- | --- |
| `cmd_validate` | `forge-ffi/src/lib.rs` | Tauri command; dispatch-only per AD-4 | `#[tauri::command] async fn cmd_validate(doc: PipelineDoc) -> Result<Vec<ValidationError>, String>` |
| `cmd_start_job` | `forge-ffi/src/lib.rs` | Hands a plan to the foreground service and returns immediately | `#[tauri::command] async fn cmd_start_job(doc: PipelineDoc) -> Result<String, String>` |
| `cmd_availability` | `forge-ffi/src/lib.rs` | Per-node UI state for the palette and editor | `#[tauri::command] async fn cmd_availability() -> Result<Vec<(NodeKind, NodeAvailability)>, String>` |
| `main` | `forge-cli/src/main.rs` | Desktop harness: `forge-cli run <pipeline.json> --tier t0` | `fn main() -> anyhow::Result<()>` |

### `android/` — Kotlin plugin layer

| Entity | File | Role |
| --- | --- | --- |
| `MediaForgePlugin` | `MediaForgePlugin.kt` | Tauri mobile plugin entry; every `@Command` dispatches to a coroutine |
| `LiteRtBridge` | `LiteRtBridge.kt` | LiteRT and LiteRT-LM inference, NPU accelerator selection, compilation-cache management |
| `MediaCodecBridge` | `MediaCodecBridge.kt` | Hardware decode and encode of HEVC/AV1 via MediaCodec |
| `BillingBridge` | `BillingBridge.kt` | Play Billing plus RevenueCat sync; the concrete `EntitlementService` behind the Rust seam |
| `JobForegroundService` | `JobForegroundService.kt` | WorkManager-driven foreground service hosting long jobs |
| `ThermalReader` | `ThermalReader.kt` | Subscribes to `PowerManager.getThermalHeadroom()` and feeds `ThermalGovernor` |

## 4. Pipeline document format

Deterministic, JSON-serialisable, and shareable — the unit of virality.

```json
{
  "version": 1,
  "name": "Podcast cleanup",
  "graph": {
    "nodes": [
      {"id": "in",  "kind": "SourceVideo", "params": {"uri": "content://..."}},
      {"id": "aud", "kind": "AudioSplit",  "params": {}},
      {"id": "dnz", "kind": "AudioDenoise","model": "gtcrn", "params": {"strength": 0.8}},
      {"id": "txt", "kind": "Transcribe",  "model": "whisper-small", "params": {}},
      {"id": "out", "kind": "SinkFiles",   "params": {}}
    ],
    "edges": [
      {"from": ["in","audio"],  "to": ["aud","in"]},
      {"from": ["aud","voice"], "to": ["dnz","in"]},
      {"from": ["dnz","out"],   "to": ["txt","in"]},
      {"from": ["txt","out"],   "to": ["out","in"]}
    ]
  }
}
```

## 5. UI Design Reference

The UI is not invented here. It is frozen under `LIBS/UI/STITCH/` per TC12 and
coders integrate those exports rather than re-implementing screens.

`LIBS/UI/STITCH/DESIGN.md` is the design system: token set, the six typed-port
hue-and-geometry pairs, the five-state heat colour language, the seven node
states with their absolute precedence rule, and the anti-pattern list.

Screen exports live in `LIBS/UI/STITCH/screens/<screen-id>/`, one folder per
screen, each carrying its name, one-sentence intent, and linked CHECKLIST task
IDs. The complement is 24 screens:

- **First run and capability** — `a1-welcome`, `a2-capability-result`,
  `a3-model-packs`, `a4-storage-grant`
- **Home and presets** — `b1-home`, `b2-preset-gallery`, `b3-preset-detail`
- **Editor** — `c1-recipe-view`, `c2-node-palette`, `c3-node-inspector`,
  `c4-validation-error`, `c5-canvas-tablet` (tablet, two-pane)
- **Gating and entitlements** — `d1-node-state-legend`,
  `d2-paywall-device-aware`, `d3-tier-limited-sheet`, `d4-experimental-consent`,
  `d5-wallet-entitlement`
- **Jobs** — `e1-run-preflight`, `e2-job-monitor`, `e3-thermal-pause`,
  `e4-result-viewer`
- **Assets and settings** — `f1-model-manager`, `f2-model-license`,
  `f3-settings`

`NodeAvailability` maps one-to-one onto the seven states rendered in
`d1-node-state-legend`; that screen is the visual contract for the enum. If
implementation exposes a UI gap, the frozen complement is extended before coding
continues — UI is never improvised in prose.

## 6. Operator verification protocol

These are verified by the operator on real hardware. They are **not** CHECKLIST
tasks, because on-device observation and visual inspection cannot serve as
idempotent Accept clauses.

**OV-1 Capability probe correctness.** Install on a Galaxy Z Fold3 and on an
8 Gen 3 device. Confirm each reports the expected tier and that the T0 device
marks generative fill experimental rather than available.

**OV-2 Backend fallback.** Force a QNN delegate load failure and confirm the node
executes on GPU, then CPU, without the pipeline failing.

**OV-3 Thermal governor.** Run a video upscale exceeding ten minutes and observe
the governor derating at least three steps before pausing, with the UI chip
tracking each transition and progress never resetting.

**OV-4 Job resume.** Force-kill the app mid-job and confirm it resumes at the
last completed segment with the same plan hash.

**OV-5 Gating precedence.** On the Fold3, confirm FLUX.2-klein renders as
tier-limited with a substitute offered and carries no lock, price, or credit
cost, and that the Pro paywall explicitly states Pro will not add it to that
device.

**OV-6 Credit reconciliation.** Spend from a credit reserve while in airplane
mode, reconnect, and confirm the balance reconciles against RevenueCat exactly
once.

**OV-7 First-run compile.** Confirm the cold-to-warm initialisation delta on
first model load and that the UI presents it as a one-time step.

## 7. Build and release

Manual builds until a public release succeeds. `version.txt` is the single source
of truth; MINOR increments per build invocation regardless of outcome. Release
artifacts — AAB, APK, and debug symbols — stage in the tracked root `dist/`,
slug-first, carrying the full stamped version. Never delete `dist/`; rename aside
to `dist.bak.<timestamp>/`. Web assets build before staging, because bundlers
empty their output directory by default. Large binaries go through Forgejo LFS,
never GitHub LFS.
