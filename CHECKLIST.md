# CHECKLIST — OnDroid MediaForge

The coder contract. A coder reads **only its own task** — not the filesystem, not
neighbouring files, not "helpful" exploration. Every entity name below is taken
verbatim from `DOCS/ARCHITECTURE.md` §3; if a task needs a fact that is not in
the task, escalate to the architect rather than inventing it.

**Markers:** `[ ]` open → `[/]` assigned, coder acknowledges → `[X]` coder
completed → `✅` orchestrator semantically validated. Capital `X` is mandatory.
`✅` is flipped only by the orchestrator against the architecture entity row — a
passing command is not attestation. Commit before flipping a marker, one commit
per completed item.

**Accept clauses** are idempotent, side-effect-free, re-runnable end-state
assertions. Device observation and visual inspection are not Accept clauses;
those live in `DOCS/ARCHITECTURE.md` §6 Operator verification protocol.

**Per-task gate** is scoped to the task's own crate — never a transitive
workspace build.

---

## Stanza 1 — Workspace and pipeline model

### [X] T1 — Cargo workspace and crate skeletons

**Files:** create `Cargo.toml`, `crates/forge-core/Cargo.toml`,
`crates/forge-core/src/lib.rs`, `crates/forge-engines/Cargo.toml`,
`crates/forge-engines/src/lib.rs`, `crates/forge-ffi/Cargo.toml`,
`crates/forge-ffi/src/lib.rs`, `crates/forge-cli/Cargo.toml`,
`crates/forge-cli/src/main.rs`

**Do:** Root `Cargo.toml` declares `[workspace]` with members `crates/forge-core`,
`crates/forge-engines`, `crates/forge-ffi`, `crates/forge-cli` and
`resolver = "2"`. Each crate compiles empty. `forge-core` depends on `serde`
(derive), `serde_json`, `thiserror`. Define `CoreError` in `forge-core/src/lib.rs`
as a `thiserror` enum with variants `Io(std::io::Error)`, `Serde(serde_json::Error)`,
`Probe(String)`, `Engine(String)`.

**Accept:**
- `cargo check -p forge-core -p forge-engines -p forge-ffi -p forge-cli` exits 0
- `git ls-files Cargo.toml crates/forge-core/src/lib.rs | wc -l` returns 2

---

### [X] T2 — Graph types

**Files:** create `crates/forge-core/src/graph.rs`; modify
`crates/forge-core/src/lib.rs` to add `pub mod graph;`

**Entities (verbatim):** `PortType`, `NodeId`, `NodeKind`, `Port`, `NodeSpec`,
`Edge`, `Graph`, `Graph::topological_order`

**Do:** Implement each with the exact signature in `DOCS/ARCHITECTURE.md` §3.
All types derive `Debug, Clone, PartialEq, Serialize, Deserialize`. `NodeKind`
serialises in PascalCase. `Graph::topological_order` performs Kahn's algorithm and
returns `ValidationError::Cycle` carrying the node ids still in the cycle when one
is detected. Add a `ports_for(kind: NodeKind) -> (Vec<Port>, Vec<Port>)` free
function returning the declared input and output ports for each of the 22
`NodeKind` variants — this is the single source of port typing.

**Accept:**
- `cargo check -p forge-core` exits 0
- `grep -c 'enum PortType' crates/forge-core/src/graph.rs` returns 1
- `grep -c 'fn topological_order' crates/forge-core/src/graph.rs` returns 1

---

### [X] T3 — Capability model

> Order note: this task was previously numbered T4. Validation consumes
> `SocProfile`, so capability must land first or the dependent task cannot
> compile.

**Files:** create `crates/forge-core/src/capability.rs`; modify `lib.rs`

**Entities:** `DeviceTier`, `Backend`, `SocProfile`, `probe_device`,
`StageFamily`

**Do:** Implement per §3. `probe_device` reads the SoC identifier, attempts a
delegate load, and runs a timed micro-benchmark, returning a populated
`SocProfile`. Behind `#[cfg(not(target_os = "android"))]` it returns a desktop
profile with `tier: DeviceTier::T0`, `backends: vec![Backend::Cpu]`,
`npu_experimental: false`, so `forge-cli` runs without a device.

`SocProfile` additionally carries `probe_schema_version: u32` and
`model_budget_bytes: u64`. The schema version is compared against the running
binary's expected value on load; a mismatch forces a re-probe rather than
trusting a stale cache. The model budget is the memory available for resident
models after OS and app overhead, and is what stops mutually exclusive stage
families being co-resident.

`StageFamily` is `enum StageFamily { Diffusion, LargeLanguage, Audio, Vision }`.
Provide `fn exclusive_families(tier: DeviceTier) -> Vec<(StageFamily, StageFamily)>`
returning pairs that must never be resident together at that tier; at `T0` and
`T1` this includes `(Diffusion, LargeLanguage)`.

**Accept:**
- `cargo check -p forge-core` exits 0
- `grep -c 'npu_experimental' crates/forge-core/src/capability.rs` returns at least 1
- `grep -c 'probe_schema_version' crates/forge-core/src/capability.rs` returns at least 1
- `grep -c 'exclusive_families' crates/forge-core/src/capability.rs` returns at least 1

---

### [ ] T4 — Graph validation

> Order note: this task was previously numbered T3.

**Files:** create `crates/forge-core/src/validate.rs`; modify
`crates/forge-core/src/lib.rs` to add `pub mod validate;`

**Entities:** `ValidationError`, `validate_graph`

**Consumes:** `Graph`, `Edge`, `NodeId`, `PortType`, `ports_for` from `graph.rs`
(T2); `SocProfile`, `DeviceTier`, `StageFamily`, `exclusive_families` from
`capability.rs` (T3)

**Do:** `validate_graph` returns every error found rather than the first.
Detect: port type mismatch on an edge, cycles, a required input left unconnected,
a node whose required tier exceeds `caps.tier`, and a graph containing two nodes
from a pair returned by `exclusive_families` for this tier. Write unit tests
covering a valid chain, an audio-to-image mismatch, a two-node cycle, a T2-only
node on a T0 profile, and a graph pairing generative fill with LLM metadata on a
T0 profile.

**Accept:**
- `cargo test -p forge-core validate` exits 0
- `grep -c 'TypeMismatch' crates/forge-core/src/validate.rs` returns at least 1
- `grep -c 'ExclusiveFamilies' crates/forge-core/src/validate.rs` returns at least 1

---

### [ ] T5 — Availability resolution (the gating precedence rule)

**Files:** create `crates/forge-core/src/availability.rs`; modify `lib.rs`

**Entities:** `NodeAvailability`, `resolve_availability`

**Consumes:** `NodeKind`, `SocProfile`, `DeviceTier`, `Backend`, `Entitlement`

**Do:** Implement `resolve_availability` with the exact signature in §3. The
resolution order is fixed and must be implemented in this order:

1. If the node's required tier exceeds `caps.tier`, return
   `NodeAvailability::TierLimited { required, substitute }` and return
   **immediately**. Commercial state is never consulted. `substitute` names a
   node of the same media class that does run at `caps.tier` — `GenerativeFill`
   on T0 substitutes `ImageObjectRemove`.
2. If the model is absent, return `NeedsModel`.
3. If the node is experimental at this tier, return `Experimental`.
4. If the node requires Pro and `ent` is `Free`: return `Metered` when the node
   has a credit price, otherwise `ProLocked`.
5. Otherwise return `Accelerated` when `Backend::Npu` is available for the node,
   else `Ready(backend)`.

This function is the only place gating precedence is expressed. No caller may
re-derive it.

Write unit tests asserting: a T0 profile with `Entitlement::Free` asking for
`GenerativeFill` on FLUX returns `TierLimited` and **never** `ProLocked` or
`Metered`; a T2 profile with `Free` returns `Metered`; a T2 profile with `Pro`
returns `Accelerated`; and the `TierLimited` result always carries `Some(substitute)`.

**Accept:**
- `cargo test -p forge-core availability` exits 0
- `grep -c 'TierLimited' crates/forge-core/src/availability.rs` returns at least 4

---

### [ ] T6 — Entitlement seam

**Files:** create `crates/forge-core/src/entitlement.rs`; modify `lib.rs`

**Entities:** `Entitlement`, `EntitlementService`, `CreditReserve`,
`gated_with_entitlement`, `EntitlementError`, `GateError`

**Do:** Implement per §3. `gated_with_entitlement` calls `resolve_availability`
first and refuses to run the closure for any state other than `Ready`,
`Accelerated` or `Experimental`; for `Metered` it spends one unit from the
`CreditReserve` before running and returns `GateError::InsufficientCredits` when
the reserve cannot cover it. `CreditReserve::spend` never issues network calls.
Provide `struct NullEntitlementService` returning `Entitlement::Free` with zero
credits, used by `forge-cli`.

**Accept:**
- `cargo test -p forge-core entitlement` exits 0
- `grep -c 'trait EntitlementService' crates/forge-core/src/entitlement.rs` returns 1

---

## Stanza 2 — Execution

### [ ] T7 — Tiler

**Files:** create `crates/forge-core/src/tiler.rs`; modify `lib.rs`

**Entities:** `Tiler`, `TileSpec`

**Do:** `Tiler::tile` partitions a width and height into fixed tiles of
`self.tile` with `self.overlap` pixels of margin, covering edges with partial
tiles. `Tiler::blend` reassembles tiles using a linear ramp across the overlap so
seams are not visible. Test that tiling then blending a synthetic gradient
reproduces the input within one least-significant bit.

**Accept:**
- `cargo test -p forge-core tiler` exits 0

---

### [ ] T8 — Thermal governor

**Files:** create `crates/forge-core/src/thermal.rs`; modify `lib.rs`

**Entities:** `ThermalState`, `ThermalGovernor::step`, `ThermalAction`,
`ThermalPolicy`

**Do:** `step` takes a thermal headroom float where values approaching 1.0 mean
approaching the limit. It must derate at least three times before ever returning
`Pause`: first `Derate(Backend::Gpu)`, then `WidenStride`, then `Pause`. Sustained
throughput planning uses 0.7× of burst. Test that a monotonically rising headroom
series produces at least three non-`Pause` actions before the first `Pause`.

**Accept:**
- `cargo test -p forge-core thermal` exits 0
- `grep -c 'ThermalAction::Pause' crates/forge-core/src/thermal.rs` returns at least 1

---

### [ ] T9 — Asset store and checkpoints

**Files:** create `crates/forge-core/src/assets.rs`,
`crates/forge-core/src/checkpoint.rs`; modify `lib.rs`

**Entities:** `AssetStore`, `AssetKey`, `JobCheckpoint`, `CheckpointStore::resume`

**Do:** `AssetStore` is content-addressed by SHA-256 of the payload; `put` is
idempotent and returns the existing key when the content already exists.
`JobCheckpoint` records `plan_hash` so a resume against a modified pipeline is
rejected rather than silently producing mixed output. Test that `put` twice
yields one file and equal keys, and that `resume` returns `None` for an unknown
job.

**Accept:**
- `cargo test -p forge-core assets checkpoint` exits 0

---

### [ ] T10 — Scheduler

**Files:** create `crates/forge-core/src/scheduler.rs`; modify `lib.rs`

**Entities:** `Segment`, `SegmentId`, `SegmentState`, `JobPlan`, `Scheduler::plan`,
`Scheduler::run`, `ProgressSink`

**Consumes:** `Graph::topological_order`, `validate_graph`, `ThermalGovernor::step`,
`CheckpointStore::resume`, `AssetStore`

**Do:** `plan` validates first and returns errors without partial work. It emits
the complete segment list up front — `JobPlan::total` is known before execution
starts, which is what makes UI progress deterministic and non-resetting. `run`
processes segments in order, consults the governor between segments, writes a
checkpoint after each, and reports through `ProgressSink`. On resume it skips
segments already recorded complete. Test that a killed-and-resumed run executes
each segment exactly once.

**Accept:**
- `cargo test -p forge-core scheduler` exits 0
- `grep -c 'fn plan' crates/forge-core/src/scheduler.rs` returns 1

---

### [ ] T11 — Pipeline document serde

**Files:** create `crates/forge-core/src/pipeline.rs`; modify `lib.rs`; create
`tests/fixtures/podcast-cleanup.json`

**Entities:** `PipelineDoc`

**Do:** Implement `PipelineDoc` per §3 and the JSON shape in §4. The fixture is
the podcast cleanup pipeline exactly as printed in §4. Test that the fixture
round-trips to an identical `Graph` and passes `validate_graph` on a desktop
`SocProfile`.

**Accept:**
- `cargo test -p forge-core pipeline` exits 0
- `git ls-files tests/fixtures/podcast-cleanup.json | wc -l` returns 1

---

## Stanza 3 — Engines

### [ ] T12 — Engine trait and registry

**Files:** create `crates/forge-engines/src/lib.rs`,
`crates/forge-engines/src/registry.rs`

**Entities:** `Engine`, `TensorIo`, `DType`, `EngineError`, `BackendChain`,
`EngineRegistry::acquire`

**Do:** Implement per §3. `acquire` walks the `BackendChain` in order and returns
the first engine that loads successfully; a load failure falls back to the next
backend and only an exhausted chain is an error. Provide
`struct NullEngine` reporting `Backend::Cpu` and echoing its input, so the
registry and scheduler are testable without a real runtime. Test that a chain
whose first entry fails to load yields the second entry's engine.

**Accept:**
- `cargo test -p forge-engines registry` exits 0
- `grep -c 'trait Engine' crates/forge-engines/src/lib.rs` returns 1

---

### [ ] T13 — ONNX Runtime engine, CPU path

**Files:** create `crates/forge-engines/src/ort.rs`; modify
`crates/forge-engines/src/lib.rs` and `crates/forge-engines/Cargo.toml`

**Entities:** `OrtEngine`

**Do:** Add the `ort` crate. Implement `Engine` for `OrtEngine` against the CPU
execution provider only — the QNN execution provider is a later task. `load`
stores the session and the EPContext cache directory in `ctx_cache`; when a cached
context binary exists for the model it is used rather than recompiling. Reports
`Backend::Cpu`. This crate must build on desktop Linux so `forge-cli` can run it.

**Accept:**
- `cargo check -p forge-engines` exits 0
- `grep -o 'ctx_cache' crates/forge-engines/src/ort.rs | wc -l` returns at least 2

> Clause note: `grep -c` counts matching *lines*, not occurrences, so two uses on
> one line would report 1 and fail spuriously.

---

### [ ] T14 — Desktop harness

**Files:** modify `crates/forge-cli/src/main.rs`, `crates/forge-cli/Cargo.toml`

**Entities:** `main`

**Consumes:** `PipelineDoc`, `Scheduler::plan`, `Scheduler::run`, `probe_device`,
`NullEntitlementService`, `EngineRegistry::acquire`

**Do:** Implement `forge-cli run <pipeline.json> [--tier t0|t1|t2]`. It loads the
document, probes the desktop profile, overrides the tier when `--tier` is given,
validates, plans, runs, and prints one line per completed segment with elapsed
milliseconds plus a final summary. Validation errors print one per line and exit
non-zero. This binary is the primary development loop; it must not require an
Android device or an NPU.

**Accept:**
- `cargo run -p forge-cli -- run tests/fixtures/podcast-cleanup.json --tier t0` exits 0
- `grep -c 'fn main' crates/forge-cli/src/main.rs` returns 1

---

## Stanza 4 — Shell and Android layer

### [ ] T15 — Tauri 2 Android shell

**Files:** create `ui/package.json`, `ui/index.html`, `ui/src/main.ts`,
`crates/forge-ffi/tauri.conf.json`; modify `crates/forge-ffi/src/lib.rs`

**Do:** Scaffold the Tauri 2 app with `mba.robin.ondroidmediaforge` as the
identifier, targeting Android, arm64-v8a, minSdk 31. The web front end is a
placeholder that renders the frozen design system's colour tokens — the real
screens are wired in T17. Any dev server port is chosen from 30000–60000 with no
visual pattern.

**Accept:**
- `grep -c 'mba.robin.ondroidmediaforge' crates/forge-ffi/tauri.conf.json` returns at least 1
- `git ls-files crates/forge-ffi/tauri.conf.json ui/package.json | wc -l` returns 2

---

### [ ] T16 — Tauri commands, dispatch-only

**Files:** modify `crates/forge-ffi/src/lib.rs`

**Entities:** `cmd_validate`, `cmd_start_job`, `cmd_availability`

**Do:** Implement the three commands with the exact signatures in §3. Per AD-4
each handler dispatches to a worker and returns immediately; no handler performs
inference, file walking, or blocking I/O on the calling thread. `cmd_availability`
returns the resolved `NodeAvailability` for every `NodeKind`, which is what the
palette and editor render.

**Accept:**
- `cargo check -p forge-ffi` exits 0
- `grep -c '#\[tauri::command\]' crates/forge-ffi/src/lib.rs` returns 3

---

### [ ] T17 — Wire the frozen screens

**Files:** create `ui/src/screens/` from `LIBS/UI/STITCH/screens/`

**Do:** Integrate the frozen Stitch exports as the application's screens. Coders
**do not re-implement or redesign** any screen and do not invent one that is not
in the frozen complement. The seven `NodeAvailability` variants render exactly as
`d1-node-state-legend` specifies, and `TierLimited` must render with no lock, no
price and no credit cost anywhere in its markup.

**Accept:**
- `git ls-files ui/src/screens | wc -l` returns at least 24
- `grep -rho 'TierLimited' ui/src/screens | wc -l` returns at least 1

> Clause note: this previously used `grep -rc … | wc -l`, which counts *files
> listed* — including files with zero matches — and so passed as long as any file
> existed. `grep -rho` emits one line per actual occurrence.

---

### [ ] T18 — Kotlin plugin layer

**Files:** create `android/MediaForgePlugin.kt`, `android/LiteRtBridge.kt`,
`android/MediaCodecBridge.kt`, `android/BillingBridge.kt`,
`android/JobForegroundService.kt`, `android/ThermalReader.kt`

**Do:** Implement the roles in §3. Every `@Command` dispatches to a coroutine.
`JobForegroundService` hosts long jobs via WorkManager so they survive
backgrounding. `ThermalReader` subscribes to `PowerManager.getThermalHeadroom()`
and feeds `ThermalGovernor`. `BillingBridge` implements the concrete
`EntitlementService` over Play Billing with RevenueCat as the authoritative store.
Engine binaries are consumed as published Maven artifacts per AD-3 — nothing is
built from source.

**Accept:**
- `./gradlew :android:compileReleaseKotlin` ends `BUILD SUCCESSFUL`
- `git ls-files android/*.kt | wc -l` returns 6

> Clause note: this previously gated on a full `assemble`, which compiles the
> Rust core, the FFI layer and the web bundle. Under hermetic per-task verify a
> task's gate checks only its own output — a transitive gate turns one upstream
> failure into a cascade of downstream escalations. The full assemble belongs to
> T19.

---

### [ ] T19 — Release build to `dist/`

**Files:** create `scripts/update-version.sh`, `scripts/build-release.sh`

**Do:** `update-version.sh` increments MINOR in `version.txt` unconditionally and
stamps every derived location. `build-release.sh` builds the web assets first,
then the AAB, APK and debug symbols, then stages them into the tracked root
`dist/` with slug-first names carrying the full stamped version. The script must
never delete `dist/`; if a clean is needed it renames aside to
`dist.bak.<timestamp>/`. Signing uses the organisation keystore.

**Accept:**
- `git ls-files scripts/update-version.sh scripts/build-release.sh | wc -l` returns 2
- `grep -c 'dist.bak' scripts/build-release.sh` returns at least 1
- `grep -c 'rm -rf' scripts/build-release.sh` returns 0

---

## Stanza 5 — Diagnosability

### [ ] T20 — Job diagnostics

**Files:** create `crates/forge-core/src/diagnostics.rs`; modify
`crates/forge-core/src/lib.rs` and `crates/forge-core/src/scheduler.rs`

**Entities:** `DiagnosticEvent`, `JobRecord`, `DiagnosticsSink`,
`JobRecord::to_bundle`

**Consumes:** `NodeId` (T2), `Backend`, `ThermalState`, `RunOutcome`,
`SegmentId` (T3, T8, T10)

**Do:** Implement per §3 and AD-11. Extend `Scheduler::run` to take a
`&mut dyn DiagnosticsSink` alongside its existing `ProgressSink`, and emit a
`StageStarted` and `StageFinished` per segment, a `BackendFallback` whenever
`EngineRegistry::acquire` falls down the chain, a `ThermalTransition` on every
governor state change, and a `Failed` on termination by error. Retain the last
20 `JobRecord`s, evicting oldest first.

`DiagnosticEvent` must have no variant capable of holding a media buffer, a
user-chosen file path, or transcript text. Write a test asserting that a
serialised bundle from a job whose pipeline references a user file at a known
path does not contain that path anywhere in its output — this is the mechanical
check behind the privacy claim, not a review comment.

**Accept:**
- `cargo test -p forge-core diagnostics` exits 0
- `grep -o 'DiagnosticEvent' crates/forge-core/src/diagnostics.rs | wc -l` returns at least 3
- `grep -c 'fn to_bundle' crates/forge-core/src/diagnostics.rs` returns 1
