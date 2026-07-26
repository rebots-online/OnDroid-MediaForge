# CLAUDE.md — OnDroid MediaForge

Project-level agent instructions. The global floor is `~/.claude/CLAUDE.md`;
cross-project doctrine is `~/Admin-Manual/DOCS/`. This file holds only what is
specific to this repository, and every command below has been run and observed in
this repository rather than recalled.

## What this is

A fully-local on-device AI media pipeline for Android: a Rust pipeline core plus
four inference-engine adapters, driven by a touch-first node-graph UI in a
WebView, shipped as a Tauri 2 Android app. Media never leaves the device;
inference is always local. The network is used only for model downloads,
entitlement sync, and opt-in telemetry — never for media.

Grounding research, including 65 adversarially-verified claims about runtimes,
models, and measured latencies, is `DOCS/ondevicemediapipelinereport.md`. Treat
its verified figures as authoritative and its "~" estimates as estimates.

## Identity

| Field | Value |
| --- | --- |
| Product name | OnDroid MediaForge |
| Package | `mba.robin.ondroidmediaforge` |
| Default branch | `master` |
| Visibility | PRIVATE |
| Distribution | Google Play only — sideload, F-Droid, and direct channels are out of scope |

## Verified toolchain

Observed on this workstation on 2026-07-26. Re-verify rather than trusting this
table if a build behaves unexpectedly.

| Tool | Version |
| --- | --- |
| rustc | 1.97.0 (2d8144b78 2026-07-07) |
| cargo | 1.97.0 (c980f4866 2026-06-30) |
| Rust Android targets | `aarch64-linux-android`, `armv7-linux-androideabi`, `i686-linux-android`, `x86_64-linux-android` — all installed |
| tauri-cli | 2.11.4 (`cargo tauri --version`) |
| node | v26.5.0 |
| pnpm | 10.33.2 |
| JDK | openjdk 21.0.11 (2026-04-21) |
| Android SDK | `/home/robin/Android/Sdk` (`ANDROID_HOME` and `ANDROID_SDK_ROOT` both set) |
| NDK | 27.0.12077973, 27.2.12479018 |
| cmake | 3.28.3 |
| ninja | 1.11.1 |
| adb | `/usr/bin/adb` |

## Layout

```
DOCS/            PRD.md · ARCHITECTURE.md · TEST_RUBRIC.md · the research report
                 sdk/<sdk>/ — local SDK doc snapshots, downloaded at ARCHITECT
LIBS/UI/STITCH/  frozen design system + screen exports (TC12)
crates/          forge-core · forge-engines · forge-ffi · forge-cli
android/         Kotlin plugin layer
ui/              WebView front end
scripts/         version stamping and build scripts
tests/           integration tests; hardware-dependent ones are #[ignore]-tagged
dist/            TRACKED release artifacts
CHECKLIST.md     the coder contract
```

Files stay under 500 lines. Nothing working lives at the repository root.

## Rules specific to this project

**The UI is not invented.** `LIBS/UI/STITCH/` is the frozen design complement and
it is the input to architecture, per TC12. Coders integrate those exports; they
never re-implement a screen from scratch and never improvise one that is not
there. If architecture or planning exposes a UI gap, go back and extend the
frozen complement rather than filling it in prose.

**Two gating axes, never conflated.** Hardware capability tier (T0/T1/T2) is
physics; entitlement is commerce. A node that the device cannot run renders as
tier-limited with a substitute offered — never as a lock, a price, or a credit
cost. This precedence is absolute and is specified in
`LIBS/UI/STITCH/DESIGN.md`.

**Privacy copy is load-bearing.** The app may state that media never leaves the
device. It may not state that the app is offline or uses no network, because it
downloads models and syncs entitlements. Getting this wrong ships a false privacy
claim.

**Prefer published binaries over building engines.** ONNX Runtime, the Qualcomm
QNN runtime, and the LiteRT delegate are all published Maven artifacts. INC-12
records an architect nearly commissioning weeks of NDK cross-compilation when a
prebuilt path already existed. Any large vendored source lands under
`vendored-in-code/<source-domain>/<component>/` with a provenance stub before
assimilation.

**`crates/forge-cli` is the fast path.** It runs the scheduler, tiler, asset
store, and the ONNX Runtime CPU path on desktop Linux with no device attached.
The same pipeline JSON runs there and on the phone. Test there first; the device
is for what only the device can prove.

**Ports** come from 30000–60000 and carry no visual pattern (I-16).

## Phase discipline

MAP, ARCHITECT and PLAN use CodeGraph only — `codegraph explore`,
`codegraph node`, or the `codegraph_*` MCP tools. No raw source reads and no
Explore/Plan subagents while a live index exists. The index is queried, never
rebuilt as a side effect.

`DOCS/ARCHITECTURE.md` must be complete and stub-free before CODE — no `TBD`, no
"future work", no "open issues". Every prose entity carries an entity-table row
with its exact name, path, role, and signature; checklist tasks cite those names
verbatim.

`CHECKLIST.md` is the coder contract and a coder reads only its own task — not
the filesystem, not neighbouring files. A stuck coder escalates rather than
inventing an entity or bridging architect debt.

Markers progress `[ ]` → `[/]` → `[X]` → `✅`, capital X mandatory. `✅` is
flipped only by the orchestrator on independent semantic evaluation against the
architecture entity row. A passing grep, exit code, or smoke test is not
attestation. Commit before flipping a marker, one commit per completed item.

Accept clauses are idempotent, side-effect-free, re-runnable end-state
assertions. On-device observation, `adb logcat` tailing, and visual inspection
are forbidden as Accept clauses — those live in an **Operator verification
protocol** subsection of `DOCS/ARCHITECTURE.md` instead. The allowed palette is a
compile gate ending `BUILD SUCCESSFUL`, `sha256sum`, `grep -c '<exact pattern>'`
against a committed file, `git rev-parse --verify <sha>^{commit}`,
`git ls-files <path> | wc -l`, and `git check-attr`.

Do not create smoke tests. Specify the real behaviour and let the next real run
exercise it.

## Build and release

Manual builds until a public release succeeds; no CI runner is stood up or
invoked before then. CI encodes a proven recipe rather than discovering one.

`version.txt` at the repository root is the single source of truth every other
stamped location derives from. Tags are `v<MAJOR>.<MINOR>` with no patch segment;
MINOR increments on every build invocation regardless of outcome. Commit messages
carry a `v<MAJOR.MINOR.BUILD>:` prefix once a version is established.

Release artifacts are staged in the **tracked** root `dist/`. Never `rm -rf
dist/` and never use a destructive clean; rename aside to
`dist.bak.<timestamp>/`. Every persisted artifact carries the product slug and
full stamped version in its filename. Large binaries stage through git-LFS on
`forgejo.robin.mba` — never GitHub LFS.

Web bundlers empty their output directory by default. Build web first, then stage
into `dist/`; never stage an artifact into a directory a later build step will
empty.

Commit and push at every task boundary and at hand-back.
