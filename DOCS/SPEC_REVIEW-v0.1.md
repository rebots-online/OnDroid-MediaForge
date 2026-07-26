# Specification Review — OnDroid MediaForge v0.1

Expert panel critique of `DOCS/PRD.md`, `DOCS/ARCHITECTURE.md`, `CHECKLIST.md`,
`DOCS/TEST_RUBRIC.md` and `LIBS/UI/STITCH/DESIGN.md`, at commit `f70f633`.

Mode: critique. Panel: Wiegers (requirements), Fowler (interfaces), Nygard
(production failure modes), Newman (evolution and compatibility), Hohpe
(distributed state), Adzic (testability by example), Crispin (verification),
Cockburn (stakeholder goals).

Findings marked **[VERIFIED]** were confirmed by direct check rather than
inspection alone; the command and its output are given.

---

## Quality assessment

| Dimension | Score | Note |
| --- | --- | --- |
| Requirements clarity | 7.0 / 10 | Strong on gating and privacy; two unfalsifiable NFRs |
| Architecture coherence | 5.5 / 10 | One trait cannot express the models the product depends on |
| Testability | 6.0 / 10 | Two Accept clauses pass vacuously |
| Internal consistency | 5.0 / 10 | Node count contradicts between documents; task order is unbuildable |
| Operational readiness | 4.0 / 10 | No observability, no integrity checks, no cancellation |
| **Overall** | **5.5 / 10** | Sound intent, several defects that would stop implementation cold |

---

## Critical — would ship broken, or blocks the build

### C1. Node count contradicts between PRD and architecture **[VERIFIED]**
**Wiegers.** `DOCS/PRD.md:73` declares "The sixteen v1 nodes … are normative."
`DOCS/ARCHITECTURE.md` §3 defines `NodeKind` with 22 variants, and
`CHECKLIST.md` T2 instructs the coder to write `ports_for` covering "the 22
`NodeKind` variants."

```
$ grep -o 'enum NodeKind {[^}]*}' DOCS/ARCHITECTURE.md | tr ',' '\n' | grep -c '[A-Z]'
22
$ grep -n 'sixteen' DOCS/PRD.md
73:**FR2 Node library.** The sixteen v1 nodes defined in the research report §3.2
```

Two coders reading these documents will build different port tables. The entity
table is supposed to be frozen vocabulary; a normative count that disagrees with
it defeats the purpose.

**Recommendation.** Pick one. The 22-variant enum is correct — the report's
16-row table counts *user-facing* nodes and omits structural ones (`AudioSplit`,
`AvMux`, `MaskHelper`) and the source/sink family. Rewrite FR2 to enumerate the
22 `NodeKind` variants explicitly rather than delegating to a count in another
document.

### C2. Checklist task T3 depends on T4 but precedes it **[VERIFIED]**
**Wiegers, Crispin.** T3 (Graph validation, line 69) lists under **Consumes**:
"`SocProfile`, `DeviceTier` from `capability.rs` (T4)". T4 appears at line 91.

```
$ grep -n 'T3 — \|T4 — ' CHECKLIST.md
69:### [ ] T3 — Graph validation
91:### [ ] T4 — Capability model
```

T3's Accept clause is `cargo test -p forge-core validate`. That cannot pass,
because `validate_graph` takes a `&SocProfile` from a module that does not exist
yet. A coder assigned T3 in isolation hits a missing type, and the house rule
forbids it from self-rescuing by writing `capability.rs` itself. The task
deadlocks by construction.

**Recommendation.** Swap the two tasks so capability precedes validation, and
re-check every other **Consumes** line for the same inversion.

### C3. The `Engine` trait cannot express the models the product is built on
**Fowler.** `DOCS/ARCHITECTURE.md` §3 defines:

```rust
fn run(&mut self, input: TensorIo) -> Result<TensorIo, EngineError>
```

One tensor in, one tensor out. But a Stable Diffusion UNet step takes latents,
a timestep, and text-encoder hidden states, and Whisper takes mel features plus
decoder tokens and returns logits plus updated state. Neither fits. The
generative tier and the speech-to-text tier — two of the three things the
product sells — are unimplementable against this interface.

**Recommendation.** Make it plural and named:

```rust
fn run(&mut self, inputs: &[(&str, TensorRef<'_>)]) -> Result<Vec<(String, TensorIo)>, EngineError>
```

Named bindings also survive model swaps, which single positional tensors do not.

### C4. No cancellation path, though FR4 requires one
**Nygard.** `DOCS/PRD.md` FR4 requires jobs be "pausable". The UI specifies a
"Cancel" action on `e2-job-monitor`. `Scheduler::run(&mut self, plan, sink)` has
no cancellation token, no return path for "stopped early", and `ThermalAction`
covers only governor-initiated pausing — not user-initiated stopping.

A user tapping Cancel on a 148-segment job has no specified mechanism to stop it.

**Recommendation.** Add a `CancelToken` parameter checked at every segment
boundary, and extend the run result to distinguish completed, cancelled, and
paused-for-heat. The segment boundary is already the checkpoint boundary, so the
cost is near zero.

### C5. Credits can be double-spent across devices and sibling apps
**Hohpe, Newman.** Three decisions combine badly. Credits live in a virtual
currency pool **shared across the publisher's app group** (`PRD.md` §7). Credits
are drawn into a **local signed reserve** and spent offline (`AD-9`). Nothing
specifies a reservation authority.

Install MediaForge on two devices, or run MediaForge alongside a sibling app.
Each draws a reserve of 20 from a balance of 24. Both spend. The pool goes
negative, or one reconciliation silently loses the other's spend. This is not a
rare race; it is the ordinary case for a user with a phone and a tablet.

**Recommendation.** Reserves must be server-authoritative and exclusive: a draw
debits the shared balance *at reservation time*, not at reconciliation. An
unreconciled reserve expires and returns to the pool after a defined TTL. Specify
that TTL, and specify the idempotency key that makes reconciliation exactly-once
— rubric row F8 already asserts "exactly once" but nothing in the architecture
delivers it.

### C6. `resolve_availability` returns a credit cost it has no way to know
**Fowler.** The signature is:

```rust
fn resolve_availability(kind, caps, ent, credits: u32, model_present: bool) -> NodeAvailability
```

`credits` is the user's *balance*. But the function returns
`Metered { credits: u32 }`, which is a *price*. No parameter supplies per-node
pricing, and no entity in the table owns a price table.

**Recommendation.** Add a `NodePricing` entity as the single source of per-node
credit cost and pass it in. Without it the coder will invent a pricing table
inline, which is exactly what the frozen-vocabulary rule exists to prevent.

---

## Major — degrades a core journey

### M1. `TensorIo` forces a copy of every tensor
**Fowler, Nygard.** `TensorIo { shape, dtype, data: Vec<u8> }` owns its buffer.
The performance envelope promises QuickSRNet at 2.2 ms per 1080p frame. A 1080p
RGB frame is about 6 MB; an owned `Vec<u8>` per frame per stage means allocation
and memcpy dominating a 2.2 ms budget, and heap churn under NFR3's 100 ms UI
constraint.

**Recommendation.** Introduce a borrowed `TensorRef<'_>` for inputs and an
arena or pool for outputs. Specify it now — retrofitting zero-copy after the
scheduler exists is a rewrite.

### M2. No model integrity verification
**Nygard.** FR5 covers download, size, license, delete and re-download. It says
nothing about verifying what arrived. A truncated or corrupted 466 MB Whisper
download will fail deep inside an inference call with an opaque runtime error.

**Recommendation.** Every model manifest entry carries an expected SHA-256;
verification happens before the model is marked installed. `AssetStore` already
hashes content, so the primitive exists.

### M3. The RAM co-residency constraint was dropped in translation
**Nygard.** The research report §5 states plainly: on the 12 GB floor, "the
scheduler serializes stage families accordingly (never co-resident SD + Gemma on
Tier 0/1)." That constraint appears nowhere in the architecture. `Scheduler::plan`
has no concept of model memory budget or mutually-exclusive stage families.

A "Shorts factory" pipeline chaining generative fill and Gemma metadata will
attempt both on a Fold3 and be killed by the OS.

**Recommendation.** Add a memory budget to `SocProfile` and a stage-family
exclusion set consulted by `plan`. This is a verified hardware constraint from the
source research and losing it is the most consequential omission in the document
set.

### M4. Pipeline documents have a version field and no migration policy
**Newman.** `PipelineDoc { version: u32, … }` and FR3 makes presets the sharing
mechanic. Nothing specifies what a v2 app does with a v1 document, or what a v1
app does with a v2 one. Shared files outlive app versions by definition.

**Recommendation.** State the compatibility contract: forward-compatible reads
with unknown-node rejection naming the missing node, and an explicit
`min_app_version` field so a newer preset fails with a useful message instead of
a validation error.

### M5. Cached probe results have no schema version
**Nygard.** `SocProfile` is "cached after first launch" so the probe is not
re-run. When probe logic changes in an update, every existing install keeps a
stale tier — potentially a wrong one — permanently.

**Recommendation.** Add `probe_schema_version` to `SocProfile` and re-probe when
it does not match the running binary's expectation.

### M6. Reserve signing key management is unspecified
**Nygard.** `CreditReserve` carries a `signature: Vec<u8>`. Nothing says who
signs, where the key lives, or what the signature defends against. If the client
signs, a rooted device forges credits. If the server signs, the client needs the
public key and a verification step — neither is specified.

**Recommendation.** Server-signed, client-verified, with the public key pinned in
the binary. State it explicitly; a signature field with undefined provenance
reads as security theatre.

### M7. Two Accept clauses pass vacuously **[VERIFIED]**
**Crispin.** T17's Accept is
`grep -rc 'TierLimited' ui/src/screens | wc -l` returns at least 1.
`grep -rc` prints one line per file *including files with zero matches*, so
`wc -l` counts files, not matches.

```
$ printf 'TierLimited\nTierLimited\n' > a.txt; printf 'nothing\n' > b.txt
$ grep -rc 'TierLimited' . | wc -l
2          # two files — but only one contains the string
```

The clause passes as long as any file exists. It asserts nothing. T13's
`grep -c 'ctx_cache' … at least 2` has a milder version of the same flaw: `grep -c`
counts matching *lines*, so two occurrences on one line yield 1 and fail.

**Recommendation.** Use `grep -rho 'TierLimited' ui/src/screens | wc -l` for
occurrence counts. Audit every `grep -c` clause in the checklist for the
line-versus-occurrence distinction.

### M8. T18's gate is a transitive build
**Crispin.** T18's Accept is "Gradle assemble for the android module ends
`BUILD SUCCESSFUL`". That compiles the Rust core, the FFI layer and the web
bundle. Under the hermetic-verify rule a task's gate checks only that task's own
output; here a failure anywhere upstream fails T18, and the incident record shows
exactly this pattern cascading 22 downstream escalations from one task.

**Recommendation.** Gate T18 on compiling the Kotlin sources alone, and move the
full assemble to the release task where it belongs.

### M9. Two NFRs are unfalsifiable as written
**Wiegers.** NFR1: "No job may drive the device to thermal shutdown." You cannot
demonstrate the absence of a condition across all inputs; you can only fail to
observe it. NFR3: "The UI thread is never blocked more than 100 ms" specifies no
measurement method or instrumentation.

**Recommendation.** Restate as observable properties: NFR1 becomes "the governor
issues at least three derating actions before any pause, and thermal headroom
never falls below X during a Y-minute job" — which rubric row A9 already checks.
NFR3 becomes "frame-time traces captured over a 10-minute job show no main-thread
block exceeding 100 ms", naming the capture tool.

---

## Minor

### m1. Three node kinds exist in code but in no requirement or screen
**Cockburn.** `AvMux`, `MaskHelper` and `SinkGallery` appear in `NodeKind` but
in neither FR2's list nor any of the 24 frozen screens. Either they are real and
users need a way to reach them, or they are internal and should not be user-
selectable node kinds.

### m2. Lifetime entitlement has no specified user experience at the next major
**Cockburn.** "Perpetual license to the major version purchased" is precise
legally and silent experientially. What does a lifetime holder see when v2 ships
— a prompt, a locked feature, nothing? The wallet screen `d5-wallet-entitlement`
shows "v1.x, yours permanently" but no journey covers the transition.

### m3. The north-star metric has no target
**Wiegers.** "Pipelines completed per weekly-active device" is a good metric with
no number attached. The guardrails all have thresholds; the headline does not.

### m4. Rubric row B3 is unfalsifiable
**Crispin.** "No code path other than `resolve_availability` derives gating
precedence" cannot be established by observation. Restate as a mechanical check:
`NodeAvailability::TierLimited` is constructed in exactly one file.

### m5. Rubric section F defines no evidence artifact
**Crispin, Gregory.** Thirteen operator rows with a checkbox each and no
specified evidence. The house convention already archives rubric runs as
screencasts under `dist/rubric-runs/v<VERSION>-<device>-<stamp>/`; section F
should name that path and the per-row filename pattern.

### m6. Entity rows carry no `file:line`
**Wiegers.** The convention requires exact name, `file:line`, role and signature.
The document justifies the omission — no source exists yet — which is honest, but
the back-fill after first commit is stated as intent with no task owning it. No
checklist task updates the entity table.

---

## Panel consensus

Three points drew agreement across the panel:

1. **The gating design is the strongest part of the specification set.** Encoding
   precedence in a single function, forbidding a hardware limit from rendering as
   a paywall, and making both directions S1 rubric rows is disciplined work that
   most specifications never reach. It should be preserved exactly as written.

2. **The engine interface is the weakest.** C3 is not a refinement; the trait
   cannot express the models the product is sold on, and it is referenced by four
   checklist tasks. Fix it before any coder touches T12.

3. **Operational concerns are systematically absent.** There is no logging
   specification, no crash reporting, no field-diagnosis story for a failed job,
   and no observability requirement anywhere in the PRD's NFR set. For a product
   whose core promise is long unattended jobs on thermally constrained hardware,
   this is the largest category-level gap.

### Disagreement

Fowler and Nygard split on M1. Fowler holds that zero-copy tensors are a
premature optimisation that will complicate the trait before any profile exists.
Nygard holds that the 2.2 ms per-frame figure is already in the specification as
a promise, so the copy cost is a known defect rather than a speculative one. The
panel did not resolve this; it is recorded as a decision the architect owes.

---

## Improvement roadmap

**Immediate — before any coder is dispatched**
- C1 node count, C2 task ordering, C3 engine trait, C6 pricing entity
- M7 vacuous Accept clauses, M8 transitive gate

**Short term — before M1 exit**
- C4 cancellation, C5 reserve authority and TTL, M3 RAM co-residency
- M2 model integrity, M5 probe schema version, M9 falsifiable NFRs
- Add an observability section to the PRD's NFRs

**Longer term — before beta**
- M1 tensor ownership (after the architect resolves the recorded disagreement)
- M4 pipeline document compatibility contract, M6 signing key custody
- m1 through m6
