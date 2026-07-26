# PRD — OnDroid MediaForge v1.0

Supersedes §6 of `ondevicemediapipelinereport.md` where the two differ. The
report remains authoritative for runtime, model, and measured-latency claims.

## 1. Inception

**Business case.** Creators pay recurring fees to cloud services for media work
their own phone hardware can already perform. Every high-end Android device since
roughly 2021 carries NPU silicon that sits idle. The intersection this product
occupies — simple node pipelines, fully local mobile inference, creator media
tasks — has no incumbent: every ComfyUI mobile client is a thin remote control
for a desktop server, Local Dream proves on-device diffusion but has no pipeline
concept, and Google's AI Edge Gallery is a single-model demo shelf.

**Alternative use of the time.** The same effort could extend an existing app in
the portfolio. It is spent here because the enabling platform shift is recent and
narrow: NNAPI was deprecated in Android 15 and its replacement, LiteRT with
vendor NPU accelerators, only reached general availability in 2025–26. For the
first time a solo developer can pull Qualcomm NPU acceleration from public Maven
Central with two Gradle lines. That window favours moving now.

**Deployment vector.** Product. Google Play, paid, with a free tier.

## 2. Problem & positioning

Creators do repetitive media cleanup — denoise, isolate voice, upscale, remove
objects and backgrounds, transcribe, caption, package metadata — through a
patchwork of cloud subscriptions, each one a privacy exposure, a render queue,
and a monthly fee.

**OnDroid MediaForge is a fully-local, pipeline-based media utility: chain simple
AI nodes like n8n, run them on your phone's silicon, own your footage end to
end.**

**Non-goals at v1:** timeline video editing, live-streaming effects, cloud
rendering of any kind, MediaTek and entry-level device support, and iOS.

## 3. Personas

1. **The Solo YouTuber** — batch-cleans talking-head footage, wants a podcast
   cleanup preset plus transcript, chapters and titles. Attended use; cares about
   time-to-done.
2. **The Archive Reviver** — upscales and interpolates old footage overnight.
   Unattended long jobs; cares about resumability and thermal safety.
3. **The Shorts Factory** — transcribes, finds hooks, captions, crops to 9:16
   with subject tracking. Runs pipelines many times a day and trades preset files
   with peers.

## 4. Hardware tiers

Minimum supported device is the Samsung Galaxy Z Fold3 5G: Snapdragon 888
(SM8350, Hexagon 780 = V68), 12 GB RAM, Android 12+, arm64-v8a. API 31+ is also
the floor LiteRT's own NPU path requires.

| Tier | Hardware | Guaranteed capability |
| --- | --- | --- |
| **T0 Baseline** | SD888 / Exynos 2100 class, 12 GB | Full audio stack; batch Whisper; LaMa object removal; QuickSRNet and Real-ESRGAN; RVM/MODNet matting; RIFE offline; MobileSAM; Gemma 4 E2B on CPU/GPU. SD1.5 generative fill flagged experimental via the raw-QNN V68 path |
| **T1 NPU standard** | 8 Gen 1/2; Exynos 2400 where LiteCore covers the model | All of the above NPU-accelerated; SD1.5 fill supported; multi-resolution img2img and inpaint |
| **T2 Flagship** | 8 Gen 3 / 8 Elite, 12–16 GB | SDXL-class and FLUX.2-klein-4B fill and outpaint; near-interactive SD1.5; 1080p video SR at speed; Gemma 4 E4B |

Capability is probed at first launch — SoC identification, delegate load test, and
a timed micro-benchmark — and cached. Foldables get the dual-view layout; the
large inner-screen layout is a P1 requirement because the canvas is the product
on that display.

## 5. Functional requirements

**FR1 Pipeline editor.** Create, edit and run DAGs of typed nodes. Linear recipe
view is the default; canvas view is the advanced toggle. Validation runs before
execution and catches type mismatches and tier-unavailable nodes.

**FR2 Node library.** The sixteen v1 nodes defined in the research report §3.2
are normative: sources, audio (denoise, isolate, stems, transcribe, diarize),
image (upscale, object-remove, generative-fill, cutout), video (upscale,
remove-bg, interpolate, object-remove-experimental), text (metadata, caption),
and sinks.

**FR3 Presets.** At least six shipped preset pipelines. Import and export as JSON.
Presets open as editable graphs. **Running an imported preset never requires a
paid entitlement** — presets are the sharing mechanic and gating them would break
it.

**FR4 Jobs.** Queued, backgrounded in a foreground service, pausable, resumable
across app restarts. Per-segment progress. Battery and thermal policy visible in
plain language.

**FR5 Model manager.** On-demand downloads with per-model size and license shown
before download. Delete and re-download. Nothing bundled in the base APK beyond
tiny models. Base APK ≤ 300 MB; typical installed footprint with the starter set
≤ 2 GB.

**FR6 Privacy — zero media egress.** Media never leaves the device, and no
pipeline stage transmits media or derived media anywhere. The network is used for
exactly three purposes: model downloads, entitlement synchronisation, and opt-in
anonymous performance telemetry. In-app copy states that media stays on the
device; it must never claim the app is offline or uses no network, because both
are false. A visible media-local indicator is persistent.

> This amends the original report's FR6, which said no network permission was
> needed after model download. That is incompatible with online entitlement
> sync and does not hold.

**FR7 Storage.** SAF and MediaStore compliant. Output to a user-chosen tree.
Content-addressed intermediate cache with one-tap clear.

**FR8 Capability probe and graceful tiering** per §4, with nodes rendering as
available, accelerated, experimental or unavailable accordingly.

**FR9 Entitlement and gating** per §7.

## 6. Non-functional requirements

**NFR1 Thermal.** No job may drive the device to thermal shutdown. The governor
derates at least three steps before pausing. Sustained jobs are sized at 0.7× of
burst throughput, matching the verified 74–77% sustained-CPU reality.

**NFR2 Reliability.** A job interrupted at segment *n* resumes at segment *n*.
Model-load failures fall back down the backend list and never crash the pipeline.

**NFR3 Responsiveness.** The UI thread is never blocked more than 100 ms by
engine work. All inference runs off the main thread; plugin command handlers
dispatch only.

**NFR4 Licensing hygiene.** Every shipped model's license is surfaced in-app. No
GPL code is linked into the app binary — RVM's licensing is verified or MODNet is
substituted. Qualcomm AI Hub compiled-asset terms and Stable Diffusion
OpenRAIL-M terms are legal-reviewed before store release. FLUX.2-klein
(Apache-2.0) is preferred for any generative marketing claim.

**NFR5 Size and startup.** Cold start ≤ 2.5 s on the Fold3. First-run model
compilation is communicated as a one-time cached step — the verified 7.5 s → 0.2 s
initialisation delta makes caching mandatory, not optional.

## 7. Entitlement and payment architecture

**Gating: yes.**

**Two axes, never conflated.** Capability tier is physics; entitlement is
commerce. Their interaction resolves through a single precedence rule: a node the
device cannot run renders as tier-limited, greyed and factual, naming the
required hardware and offering a substitute that does run. It never carries a
lock, a price, or a credit cost. If a node is both unaffordable and unrunnable it
renders only as tier-limited. We do not take money for capability the silicon
cannot deliver.

**Three doors above a free floor.** Occasional and heavy users get shapes that
fit them rather than one interchangeable tier.

- **Free** — all source and sink nodes; the full audio stack; photo object
  removal; photo and video upscaling; matting; the mask helper; running any
  preset; linear recipe view; three saved custom pipelines; one job at a time.
- **Credits** — per-generation micropayments for the expensive classes:
  generative fill and outpaint, 4-stem separation, frame interpolation, and the
  LLM/VLM metadata nodes.
- **Pro** — unlimited use of the above plus authoring depth: the node canvas and
  branching graphs, unlimited saved pipelines, job queue and batch, and overnight
  plugged-in mode.

**SKUs.** Monthly and annual subscriptions, plus a lifetime SKU which is the
promoted hero. Lifetime means a perpetual license to the major version purchased
— pay once, keep that version forever — not a claim on all future majors.
Consumable credit packs sit alongside.

**Authority.** RevenueCat is the authoritative entitlement store. Play Billing
handles store purchases. BTCPay and Blink Lightning PoS report changes into
RevenueCat. Clients never grant entitlements directly.

**Virtual currency.** Credits are denominated in the shared RevenueCat
virtual-currency pool used across the publisher's app group. Every app in the
group integrates at the same level — accessing the shared pool is what sets that
bar, and this app meets it rather than defining its own. Balance therefore
carries between apps.

**Backends in scope:** RevenueCat (authoritative), Play Billing, BTCPay, Blink
Lightning.

**Trait seam:** `EntitlementService` in `crates/forge-core/src/entitlement.rs`,
with a `gated_with_entitlement` wrapper. The implementation is swappable; no
processor is load-bearing in the core.

**Separation of powers.** No single party holds more than one of distribution,
payment, identity, and entitlement.

**Credit spending does not block on the network.** Credits are drawn from
RevenueCat into a locally-held signed reserve in blocks while online, spent
against that reserve during jobs, and reconciled on next connect. A generation
never waits on a round-trip, and RevenueCat stays authoritative.

## 8. Distribution

**Google Play only at v1.** Sideload, F-Droid, and direct desktop channels are
out of scope — they are not aligned to the target segment. There are no
per-channel build variants. Release artifacts are AAB, APK, and debug symbols.

## 9. Build and release

Manual builds until a public release succeeds; no CI runner is stood up before
then. `version.txt` at the repository root is the single source of truth. Tags
are `v<MAJOR>.<MINOR>`; MINOR increments on every build invocation regardless of
outcome. Artifacts stage in the tracked root `dist/` with product slug and full
stamped version in every filename, and large binaries go through Forgejo LFS.

## 10. Milestones

| Milestone | Contents | Exit criterion |
| --- | --- | --- |
| **M0 Spike** | Rust core skeleton; ORT-QNN on one device; GTCRN, LaMa and QuickSRNet nodes driven from `forge-cli`; Tauri 2 shell boots on Fold3 and an 8 Gen 3 device | LaMa tile inpaint under 3 s on 8 Gen 3 NPU; the same pipeline runs, slower, on Fold3 GPU |
| **M1 Audio vertical** | Recipe UI; full audio stack; batch Whisper; podcast cleanup and transcribe-and-chapter presets | A 30-minute recording cleaned, transcribed and chaptered entirely on-device on a Fold3 without a thermal pause |
| **M2 Visual vertical** | Upscale, matting, RIFE, cutout nodes; job system with resume; model manager; canvas view | The old-footage-revival preset survives a forced app kill and resumes at its last segment |
| **M3 Generative and entitlement tier** | SD1.5 fill via LiteRT-QNN; raw-QNN experimental toggle for T0; FLUX-klein on T2; capability probe UI; entitlement seam, Play Billing, credits | Fill is interactive on 8 Gen 3 at ≤ 5 s and correctly gated and labelled on Fold3; a credit spend reconciles against RevenueCat after a period offline |
| **M4 Beta** | LLM metadata nodes; preset sharing; polish; closed beta with 20–50 creators | At least one pipeline run per week per active beta user |

## 11. Success metrics

North star: **pipelines completed per weekly-active device.**

Guardrails: job completion rate ≥ 97% excluding user cancellation; thermal-pause
rate under 10% of jobs; median podcast-cleanup wall time ≤ 1.5× audio duration on
T1; store rating ≥ 4.3.

## 12. Open questions

Carried from the research report §7 and still open. None blocks design or M0.

1. Exynos LiteCore performance in practice — no independent benchmarks exist;
   needs a device-lab test on an Exynos 2400 before any Exynos marketing claim.
2. RVM licensing — GPL-3 repository versus MIT weights; substitute MODNet if
   counsel is unhappy.
3. Video object-removal quality bar — the mask, LaMa and temporal-blend approach
   needs a golden-clip evaluation set before leaving experimental status.
4. Distribution of raw-QNN HTP binaries for T0 diffusion — per-SoC binaries
   versus dropping T0 generative entirely; decided on real Fold3 measurements at
   M3.
5. Trademark clearance on the product name before beta.
