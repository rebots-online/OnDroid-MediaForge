# TEST RUBRIC — OnDroid MediaForge

The final gauntlet. Written before CODE begins; a project entering CODE without a
rubric has not exited PLAN.

## Rules of the gauntlet

**GR-1.** This document is a PLAN-phase artifact. It is authored before
implementation and is not edited to match what the implementation turned out to
do.

**GR-2.** The gauntlet runs only after `CHECKLIST.md` is fully attested — every
task at `✅`, not merely `[X]`.

**GR-3.** There are exactly two terminal verdicts: **SHIP-READY** and
**DEFECTIVE**. Any `❌` at any severity yields DEFECTIVE. There is no waived
state and no partial pass.

**GR-3a.** SHIP-READY requires every row — agent-executed and operator-executed
alike — at `✅`. If agent rows pass but operator rows are outstanding, the verdict
is `INCOMPLETE—pending-operator`, which is not a pass.

**GR-4.** Logs and greps are fail-triggers and supporting evidence only. The
absence of an error line is never, on its own, a PASS. Every row requires a
positive observation of the stated behaviour.

**Severity.** `S1` ships a broken or dishonest product. `S2` degrades a core
journey. `S3` is a defect users will hit but can work around. All three are `❌`
for verdict purposes; severity orders the fixing, not the passing.

---

## Section A — Pipeline core (agent-executed)

| # | Assertion | Severity | Verdict |
| --- | --- | --- | --- |
| A1 | The podcast-cleanup fixture round-trips through `PipelineDoc` to an identical `Graph` and validates clean on a desktop profile | S2 | ☐ |
| A2 | `validate_graph` reports **every** error in a graph containing a type mismatch, a cycle, and an unconnected required input — not just the first | S2 | ☐ |
| A3 | `Graph::topological_order` returns `ValidationError::Cycle` naming the participating nodes for a two-node cycle | S3 | ☐ |
| A4 | `Tiler` round-trips a synthetic gradient through tile-then-blend within one least-significant bit, with no visible seam at overlap boundaries | S2 | ☐ |
| A5 | `AssetStore::put` called twice with identical content yields one stored file and two equal `AssetKey`s | S3 | ☐ |
| A6 | `Scheduler::plan` emits the complete segment list before execution, so `JobPlan::total` is known up front | S2 | ☐ |
| A7 | A run interrupted after segment *n* and resumed executes every segment exactly once, with no segment repeated or skipped | S1 | ☐ |
| A8 | `CheckpointStore::resume` refuses a checkpoint whose `plan_hash` does not match the current pipeline, rather than producing mixed output | S1 | ☐ |
| A9 | `ThermalGovernor::step` returns at least three non-`Pause` actions before its first `Pause` under a monotonically rising headroom series | S1 | ☐ |
| A10 | `EngineRegistry::acquire` falls back to the next backend when the first in the chain fails to load, and errors only when the chain is exhausted | S1 | ☐ |
| A11 | `forge-cli run tests/fixtures/podcast-cleanup.json --tier t0` completes and prints one line per segment plus a summary, with no Android device attached | S2 | ☐ |

## Section B — Gating correctness (agent-executed)

This section exists because getting it wrong means charging people for things
their hardware cannot do. Every row is S1.

| # | Assertion | Severity | Verdict |
| --- | --- | --- | --- |
| B1 | `resolve_availability` returns `TierLimited` for FLUX generative fill on a T0 profile, and returns it for **both** `Entitlement::Free` and `Entitlement::Pro` — payment never changes the answer | S1 | ☐ |
| B2 | A `TierLimited` result always carries `Some(substitute)` naming a node that does run at that tier | S1 | ☐ |
| B3 | No code path other than `resolve_availability` derives gating precedence — the rule exists in exactly one place | S1 | ☐ |
| B4 | `gated_with_entitlement` refuses to execute for `TierLimited`, `ProLocked` and `Metered`-without-balance, and executes for `Ready`, `Accelerated` and `Experimental` | S1 | ☐ |
| B5 | Rendered `TierLimited` markup contains no lock glyph, no price, and no credit cost | S1 | ☐ |
| B6 | Running an imported preset succeeds under `Entitlement::Free` — sharing is never gated | S1 | ☐ |
| B7 | `CreditReserve::spend` issues no network call, and a reserve exhausted mid-job fails the node cleanly rather than blocking on connectivity | S1 | ☐ |

## Section C — Privacy claims (agent-executed)

Every row is S1: a false privacy claim is the one defect that cannot be patched
after shipping.

| # | Assertion | Severity | Verdict |
| --- | --- | --- | --- |
| C1 | No pipeline stage transmits media, frames, audio, or derived media to any network destination | S1 | ☐ |
| C2 | Every network call in the codebase serves exactly one of: model download, entitlement sync, or opt-in telemetry | S1 | ☐ |
| C3 | Telemetry is off by default and its payload contains timings and chip identification only — never media, filenames, or transcript content | S1 | ☐ |
| C4 | No shipped user-facing string claims the app is offline, needs no network, or uses no internet permission | S1 | ☐ |
| C5 | The persistent media-local indicator is present on every non-modal screen | S2 | ☐ |

## Section D — Licensing (agent-executed)

| # | Assertion | Severity | Verdict |
| --- | --- | --- | --- |
| D1 | Every downloadable model has its license shown in-app before the download begins | S1 | ☐ |
| D2 | No GPL-licensed code is linked into the app binary; RVM is either license-cleared or MODNet is substituted | S1 | ☐ |
| D3 | Base APK is ≤ 300 MB and no model weights beyond the tiny class are bundled | S2 | ☐ |

## Section E — Build and release (agent-executed)

| # | Assertion | Severity | Verdict |
| --- | --- | --- | --- |
| E1 | `dist/` is tracked in git — `git check-ignore dist/` exits non-zero | S1 | ☐ |
| E2 | AAB, APK and debug symbols are all present in `dist/`, each carrying the product slug and full stamped version in its filename | S1 | ☐ |
| E3 | No build script contains `rm -rf` against `dist/`; a clean renames aside to `dist.bak.<timestamp>/` | S1 | ☐ |
| E4 | Web assets build before staging, so no bundler empties a directory containing staged artifacts | S1 | ☐ |
| E5 | `version.txt` is tracked, and every derived stamped location matches it | S2 | ☐ |
| E6 | The release commit message carries the `v<MAJOR.MINOR.BUILD>:` prefix | S3 | ☐ |

## Section F — On-device (operator-executed)

These require real silicon and cannot be agent-attested. They correspond to
`DOCS/ARCHITECTURE.md` §6. Until every row here is `✅`, the verdict is
`INCOMPLETE—pending-operator`.

| # | Assertion | Device | Severity | Verdict |
| --- | --- | --- | --- | --- |
| F1 | The capability probe reports Tier 0 on the Galaxy Z Fold3 and Tier 2 on an 8 Gen 3 device, and marks generative fill experimental on the Fold3 | Both | S1 | ☐ |
| F2 | Forcing a QNN delegate load failure results in GPU then CPU execution, with the pipeline completing rather than failing | 8 Gen 3 | S1 | ☐ |
| F3 | A video upscale exceeding ten minutes shows the governor derating at least three steps before pausing, with the thermal chip tracking each transition | Fold3 | S1 | ☐ |
| F4 | Progress never resets or moves backwards across a thermal pause and resume | Fold3 | S2 | ☐ |
| F5 | A force-killed job resumes at its last completed segment after relaunch | Fold3 | S1 | ☐ |
| F6 | FLUX.2-klein renders as tier-limited on the Fold3 with a substitute offered and no lock, price, or credit cost visible | Fold3 | S1 | ☐ |
| F7 | The Pro paywall on the Fold3 explicitly states that Pro will not add 8 Gen 3-only features to that device | Fold3 | S1 | ☐ |
| F8 | Credits spent in airplane mode reconcile against RevenueCat exactly once on reconnect — not zero times, not twice | Fold3 | S1 | ☐ |
| F9 | First model load shows a one-time compile step, and the second launch of the same model is materially faster | Both | S2 | ☐ |
| F10 | A 30-minute recording is cleaned, transcribed and chaptered entirely on-device on the Fold3 without a thermal pause | Fold3 | S2 | ☐ |
| F11 | The unfolded inner display presents the two-pane canvas layout, not a stretched phone view, and port handles are reliably tappable | Fold3 | S2 | ☐ |
| F12 | Cold start is ≤ 2.5 s on the Fold3 | Fold3 | S3 | ☐ |
| F13 | The app is installed from a Play internal-testing track and the purchase flow completes end to end | Fold3 | S1 | ☐ |

---

## Verdict

**Agent sections (A–E):** ☐ all `✅`
**Operator section (F):** ☐ all `✅`

**Terminal verdict:** ☐ SHIP-READY ☐ DEFECTIVE ☐ INCOMPLETE—pending-operator

Recorded by: ______________  Version: ______________  Date: ______________
