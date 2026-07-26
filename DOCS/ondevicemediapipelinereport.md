# On-Droid MediaForge

# # A Local, On-Device AI Media Pipeline for Android — Research Report, Decision Matrices & PRD

**Prepared for:** Robin · **Date:** 19 July 2026
**Scope:** Fully local inference on Snapdragon/Exynos Android hardware, Galaxy Z Fold3 as the minimum supported device. Node-graph pipeline UI at Dagger/n8n complexity (not ComfyUI). Tauri 2 fit assessed but not mandated.
**Method:** Multi-agent web research across ~30 primary sources (Google AI Edge/LiteRT docs, Qualcomm AI Hub model cards, ExecuTorch/ONNX Runtime docs, GitHub repos), 65 extracted claims, each adversarially verified against live sources; 4 claims refuted and corrected below. Hugging Face Hub queried directly for current model availability, sizes, and licenses.

---

## 0. Executive Summary

1. **The moment is right.** NNAPI is dead (deprecated in Android 15); its replacement — LiteRT with vendor NPU accelerators — went GA in 2025-26 and, for the first time, a solo developer can pull Qualcomm NPU acceleration off **public Maven Central** with two Gradle lines. Samsung Exynos NPU access (Exynos AI LiteCore) now exists through the same Google API.
2. **Every pipeline stage you named has an on-device-feasible model** with a permissive or manageable license: speech separation/enhancement (GTCRN, UVR/Spleeter, DeepFilterNet3), STT/diarization (whisper.cpp, sherpa-onnx), upscaling (QuickSRNet at 2.2 ms/1080p-frame, Real-ESRGAN for stills), inpainting (LaMa-Dilated deterministic; SD1.5-inpaint generative at ~46 ms/UNet-step on current silicon), matting (RVM/MODNet), frame interpolation (RIFE), and small LLMs/VLMs for metadata (Gemma 4 E2B/E4B, FastVLM-0.5B via LiteRT-LM).
3. **Nobody has built your app.** Every ComfyUI mobile client is a thin remote control for a desktop server. Local Dream (2.6k★) proves NPU Stable Diffusion on phones but has no pipeline concept. Google AI Edge Gallery proves the LiteRT runtime path but is single-model demos. The "simple node pipeline × fully local mobile inference" quadrant is empty.
4. **The architecture that falls out:** a Rust pipeline core (DAG scheduler, media I/O, thermal governor) + four engine adapters (ONNX Runtime/QNN, LiteRT via Kotlin plugin, ggml family, NCNN-Vulkan) + a touch-first node canvas in a WebView. Tauri 2 works for this — with two hard caveats (main-thread plugin commands; desktop-grade node-graph libraries are demonstrably bad on touch and need a custom interaction layer).
5. **Z Fold3 (Snapdragon 888, 12 GB) is a workable floor** — not for everything, but for a credible product tier: the entire audio stack, batch Whisper, LaMa inpainting, QuickSRNet upscaling, matting, and RIFE all run on CPU/GPU at that class. Generative diffusion on the Fold3's Hexagon 780 (V68) is *provable* (Local Dream supports it) but ships as "experimental" because the Google-blessed LiteRT delegate officially starts at 8 Gen 1. The PRD in §8 encodes this as capability tiers rather than a single cutoff.

### The one-diagram version

```
                        ┌───────────────────────────────────────────┐
                        │   WebView UI (touch-first node canvas)    │
                        │   dag editor · presets · progress · gallery│
                        └────────────────┬──────────────────────────┘
                              Tauri 2 IPC (raw payloads, no JSON for media)
                        ┌────────────────▼──────────────────────────┐
                        │        RUST PIPELINE CORE                 │
                        │  DAG scheduler · asset store · tiler      │
                        │  thermal/battery governor · job resume    │
                        └──┬─────────┬──────────┬──────────┬────────┘
                           │         │          │          │
                  ┌────────▼──┐ ┌────▼─────┐ ┌──▼───────┐ ┌▼─────────────┐
                  │ ONNX RT   │ │ LiteRT   │ │ ggml     │ │ NCNN /       │
                  │ + QNN EP  │ │ (Kotlin  │ │ whisper/ │ │ Vulkan       │
                  │ (audio,   │ │ plugin): │ │ llama.cpp│ │ (RIFE,       │
                  │ LaMa, SR, │ │ NPU packs│ │ (STT,LLM │ │ RealESRGAN-  │
                  │ matting)  │ │ SD/Gemma │ │ fallback)│ │ ncnn)        │
                  └────┬──────┘ └────┬─────┘ └──┬───────┘ └┬─────────────┘
                       │            │           │          │
                  ─────▼────────────▼───────────▼──────────▼─────
                   Hexagon NPU (V68+) · Adreno/Xclipse GPU · big-core CPU
```

---

## 1. The Verified Landscape (what matters, with sources)

Facts below survived adversarial verification against live primary sources on 19 Jul 2026. Corrections to widely-circulated numbers are flagged **[CORRECTED]**.

### 1.1 Runtime & NPU access

- **NNAPI is deprecated as of Android 15.** Google's official migration path for custom models is LiteRT (TFLite in Play services); the migration page itself only mentions the GPU delegate — the NPU story lives in LiteRT Next docs. Do not build anything on NNAPI. *(developer.android.com/ndk/guides/neuralnetworks/migration-guide, updated 2026-03-06)*
- **LiteRT Next exposes five vendor NPU stacks** through one CompiledModel API: Google Tensor (AOT only, beta), **Qualcomm AI Engine Direct**, MediaTek NeuroPilot, Intel OpenVINO, and **Samsung Exynos AI LiteCore** — i.e., both your target SoC families have a Google-supported NPU path in one runtime. Deployment on Android uses Play for On-device AI (AI Packs + Feature Delivery), **requires API 31+ and arm64-v8a only**, with automatic CPU/GPU fallback and compilation caching (ResNet152 init: 7,465 ms → 198 ms warm). *(developers.google.com/edge/litert/next/npu, updated 2026-06-16)*
- **The Qualcomm LiteRT delegate is publicly on Maven Central** (`com.qualcomm.qti:qnn-runtime`, `qnn-litert-delegate`; v2.34.0 cited in docs, versions through 2.48.0 published as of 2026-07-02). Officially supported SoCs: **Snapdragon 8 Gen 1 (SM8450), 8 Gen 2, 8 Gen 3, 8 Elite** "and more" — note the floor is 8 Gen 1, one generation above the Fold3's SD888. Measured MobileNetV2 on Galaxy S25: **NPU 0.3 ms vs GPU 1.8 ms vs CPU 2.8 ms** (~6× over GPU). *(developers.google.com/edge/litert/android/npu/qualcomm, updated 2026-05-28)*
- **Google × Qualcomm claim, on a 72-model suite:** up to 100× over CPU / 10× over GPU; 64/72 models fully delegate to NPU; on 8 Elite Gen 5, 56+ models run under 5 ms. FastVLM-0.5B (int8/int16): 0.12 s TTFT on 1024² images, >100 tok/s decode. *(developers.googleblog.com, 2025-11-24)*
- **ExecuTorch Qualcomm backend** delegates to Hexagon NPU *and* Adreno GPU via QNN; quantization schemes down to blockwise 4-bit (16a4w_block); verified on SM8450/SM8550, tutorials target SM8650; requires the proprietary QNN SDK (2.37.0 recommended) and a Linux build host. *(docs.pytorch.org/executorch/stable/backends-qualcomm.html)*
- **ONNX Runtime QNN Execution Provider** works on Android (prebuilt `onnxruntime-android-qnn` on Maven, 1.21.1–1.23.2); QNN libs are bundled since 1.18 (no separate Qualcomm SDK download); HTP backend requires **fixed input shapes** and supports an op subset (no Loop/If); compiled-context binary caching (EPContext) makes multi-model session loading fast. **[CORRECTED]** The claim "HTP only runs quantized models" is an overreach: `enable_htp_fp16_precision` (default **on**) runs fp32 models on the NPU as fp16. Integer quantization is for peak throughput, not admission. *(onnxruntime.ai QNN EP docs; Qualcomm QAIRT docs)*
- **Qualcomm AI Hub** is the pre-optimized model zoo (BSD-3 tooling; per-model licenses vary) covering nearly every stage you need — Real-ESRGAN-x4plus, QuickSRNet S/M/L, XLSR, SESR-M5, **LaMa-Dilated, AOT-GAN**, Whisper variants, **Stable-Diffusion-v1.5, ControlNet** — each benchmarked on real cloud-hosted devices (S21→S25 class) and exportable to QNN, LiteRT, or ONNX. *(github.com/qualcomm/ai-hub-models)*
- **Exynos reality check:** Exynos AI LiteCore gives 2400/2500-class NPUs a LiteRT AOT path — new and thinner than Qualcomm's. The dependable cross-vendor floor on Exynos remains **GPU via Vulkan/OpenCL (Xclipse)** and CPU. Plan Exynos as "GPU-first, NPU where LiteCore AOT covers the model."

### 1.2 Model-side headline numbers

- **Super-resolution is effectively free on NPU:** QuickSRNet-Small = 33.3K params, **41.7 KB** quantized (w8a8); 2× upscale to 1080p in **2.2 ms/frame on Snapdragon 8 Gen 1** (0.69 ms @540p, 4.25 ms @1440p). INT8-robust by design; beats XLSR/SESR/ESPCN on PSNR after quantization. BSD-3. *(CVPRW 2023 paper + AI Hub card)*
- **Generative fill is now interactive-speed on flagships. [CORRECTED]** The current qualcomm/Stable-Diffusion-v1.5 card (updated 2026-07-15, w8a16, QAIRT 2.42/2.45): text encoder **1.49 ms**, **UNet 46.2 ms/step**, VAE 121.4 ms on 8 Elite Gen 5 → a 20-step 512² generation ≈ **1.1 s** of compute. The oft-quoted 255 ms/step figures are the 2024 Galaxy S23 Ultra (8 Gen 2) numbers — still the right mental model for *older* tiers (20 steps ≈ 5–6 s on 8 Gen 2).
- **Local Dream (v2.6.1, Jun 2026, 2.6k★)** proves the practical envelope: SD1.5 on NPU for **any Hexagon V68+ SoC — which includes the Fold3's SD888**; SDXL only on 8 Gen 3+ (experimental Apr 2026, fixed 1024²); MNN CPU/GPU fallback for everything else; low-RAM mode default for SDXL; img2img + inpainting at all supported resolutions. Kotlin + C++.
- **Audio is the easy win.** sherpa-onnx (Apache-2.0, ~11.9k★, v1.13.0 Apr 2026) is one framework covering offline STT (Whisper, Moonshine, Zipformer, SenseVoice, Parakeet), **speaker diarization**, **speech enhancement (GTCRN)**, **source separation (Spleeter/UVR)**, VAD, TTS — with prebuilt Android APKs and first-class **Rust and Kotlin APIs**. GTCRN itself: **~48K params, ~33 MMACs/s**, PESQ 2.87 on VCTK-DEMAND (beats RNNoise 2.29, edges DeepFilterNet 2.81), streaming ONNX, MIT.
- **whisper.cpp:** tiny = 75 MiB disk/~273 MB RAM → small = 466 MiB/~852 MB RAM; batch transcription on Android CPU runs faster than real time (RTF ~0.2–0.4, tiny-q8); **streaming mode is ~5× slower than real time with unbounded latency growth** (open issue) → design around batch/segmented transcription, not live streaming. MIT, Vulkan GPU path is cross-vendor (works on Exynos Xclipse too).
- **HTDemucs is exportable to ONNX** (Mixxx GSoC 2025 documented the path and its pain) → full 4-stem separation is feasible as an *offline job* on-device; UVR/MDX-class vocal isolation via sherpa-onnx is the interactive-speed option.
- **LiteRT-LM model zoo is rich as of mid-2026** (HF `litert-community`): Gemma 4 E2B (3.2M downloads) / E4B / 12B, Qwen3 0.6B–14B, Phi-4-mini, SmolLM3, FastVLM-0.5B (VLM), Qwen3-TTS, Parakeet ASR — plus **FLUX.2-klein-4B and Z-Image-Turbo as int8 LiteRT diffusion-transformer packages** (Jul 2026), which is the first credible non-SD on-device generative image path.
- **Thermals are the real budget:** Snapdragon 8 Elite sustains only **74–77% of peak CPU** over 15–60 min loads; GPU stress stability 83%; at launch its NPU drivers weren't even ready for Geekbench AI. Headline TOPS are burst numbers — a video pipeline must be engineered against the *sustained* envelope (§7).

### 1.3 Prior art / competitive gap

| App / project                                                     | Inference location                     | Pipeline/node concept   | Relevance                                                                                                          |
| ----------------------------------------------------------------- | -------------------------------------- | ----------------------- | ------------------------------------------------------------------------------------------------------------------ |
| ComfyUI mobile clients (ComfyChair, Comfy Portal, ComfyMobileUI…) | Remote server                          | Yes (remote)            | UI prior art only; none run locally                                                                                |
| Local Dream                                                       | **On-device (QNN NPU + MNN fallback)** | No — single-task SD app | Proves NPU diffusion incl. SD888; no pipelines                                                                     |
| SDAI FOSS                                                         | On-device ONNX mode (+ remote modes)   | No                      | Solo-dev precedent, ONNX path                                                                                      |
| Google AI Edge Gallery                                            | On-device (LiteRT/LiteRT-LM)           | No — demo gallery       | Reference implementation for LiteRT + model mgmt UX                                                                |
| MNN-LLM app (Alibaba)                                             | On-device (CPU/GPU; QNN partial)       | No                      | Multimodal single-runtime proof                                                                                    |
| Galaxy AI (Generative Edit, Audio Eraser)                         | Mixed on-device/cloud                  | No                      | The incumbent baseline on your exact target devices                                                                |
| n8n / Dagger / Litegraph / React Flow / Drawflow / Rete           | n/a (UI libraries/products)            | Yes                     | Interaction models to borrow; **React Flow officially awkward on touch** (issue #1323, docs recommend workarounds) |

**Conclusion:** the intersection you're targeting — *simple node pipeline × fully local mobile inference × creator media tasks* — has no incumbent.

---

## 2. Runtime Layer — Alternatives Matrix

All runtimes examined, whether or not they made the cut:

| #   | Runtime                                   | NPU path (Snapdragon)                                              | Exynos path                             | Model classes it serves best                                                           | Rust/Tauri integration                          | License                                    | Maturity for this app                               | Verdict                                            |
| --- | ----------------------------------------- | ------------------------------------------------------------------ | --------------------------------------- | -------------------------------------------------------------------------------------- | ----------------------------------------------- | ------------------------------------------ | --------------------------------------------------- | -------------------------------------------------- |
| R1  | **LiteRT + QNN delegate / LiteRT Next**   | ✅ Official, Maven Central, SM8450+                                 | ✅ Exynos AI LiteCore (AOT, new)         | Diffusion (FLUX.2-klein, Z-Image, SD), Gemma/LLM via LiteRT-LM, MediaPipe-style vision | Via Kotlin plugin (Java API); no clean Rust API | Apache-2.0 (delegate binaries proprietary) | High; Google's anointed path                        | **Adopt** — for NPU generative + LLM stages        |
| R2  | **ONNX Runtime Mobile + QNN EP**          | ✅ Prebuilt `onnxruntime-android-qnn`; HTP default; fp32-as-fp16 OK | ⚠️ XNNPACK CPU (+ NNAPI EP dying)       | Audio stack (sherpa-onnx), LaMa, SR, matting — the long tail of ONNX exports           | ✅ Excellent — C API, `ort` Rust crate           | MIT                                        | High                                                | **Adopt** — workhorse for deterministic media ops  |
| R3  | **ExecuTorch (QNN backend)**              | ✅ Hexagon + Adreno; 4-bit schemes                                  | ❌ (Vulkan backend generic)              | PyTorch-native custom exports                                                          | ✅ C++ runtime, Rust bindings exist              | BSD-3                                      | Medium; Linux-only toolchain, fewer prebuilt models | Hold — revisit if you train custom models          |
| R4  | **NCNN (Vulkan)**                         | GPU only (excellent Vulkan)                                        | ✅ GPU (Xclipse Vulkan)                  | RIFE, Real-ESRGAN-ncnn, waifu2x — the ncnn model ecosystem                             | ✅ C API, Rust bindings                          | BSD-3                                      | High, battle-tested on Android                      | **Adopt (narrow)** — RIFE + ncnn SR ports          |
| R5  | **MNN**                                   | ⚠️ QNN "not fully supported" per own docs                          | GPU via Vulkan/OpenCL                   | LLM + diffusion in one runtime (MNN-Diffusion)                                         | C++; thinner Rust story                         | Apache-2.0                                 | High in Alibaba ecosystem                           | Fallback only (it's Local Dream's CPU path)        |
| R6  | **ggml family (whisper.cpp / llama.cpp)** | ❌ NPU; CPU NEON + Vulkan/OpenCL                                    | ✅ Same (vendor-neutral)                 | STT (whisper), LLM fallback for non-NPU devices                                        | ✅ Native C, trivial FFI                         | MIT                                        | Very high                                           | **Adopt** — STT + LLM portability floor            |
| R7  | **Raw QNN / AI Engine Direct SDK**        | ✅ Deepest access, incl. **SD888 (V68)**                            | ❌                                       | Squeezing NPU on pre-8Gen1 devices (Local Dream's approach)                            | C API, usable from Rust                         | Proprietary SDK                            | Medium; per-SoC HTP binaries                        | Optional "experimental tier" enabler for Fold3 NPU |
| R8  | **NNAPI**                                 | Deprecated (Android 15)                                            | Deprecated                              | —                                                                                      | —                                               | —                                          | —                                                   | **Reject**                                         |
| R9  | **MLC-LLM / wllama (WASM)**               | GPU (Vulkan/WebGPU)                                                | Same                                    | LLM only                                                                               | WebView-side or Rust                            | Apache-2.0                                 | Medium                                              | Reject for core (LiteRT-LM + llama.cpp cover it)   |
| R10 | **Samsung ENN / Exynos SDK direct**       | n/a                                                                | ⚠️ Restricted access, thin public story | Exynos NPU                                                                             | Poor                                            | Proprietary                                | Low public maturity                                 | Reject; use LiteCore-via-LiteRT instead            |

### 2.1 Multicriteria decision matrix — runtimes

Weights reflect your stated priorities (flagship-only, solo dev, commercial-friendly, Tauri-curious). Scores 1–5.

| Criterion (weight)                       | R1 LiteRT         | R2 ORT+QNN | R3 ExecuTorch | R4 NCNN  | R5 MNN   | R6 ggml  | R7 raw QNN |
| ---------------------------------------- | ----------------- | ---------- | ------------- | -------- | -------- | -------- | ---------- |
| Snapdragon NPU perf & access (0.20)      | 5                 | 4          | 4             | 2        | 2        | 1        | 5          |
| Cross-SoC coverage incl. Exynos (0.10)   | 4                 | 3          | 2             | 4        | 3        | 5        | 1          |
| Model-class coverage for this app (0.15) | 4                 | 5          | 3             | 3        | 3        | 2        | 2          |
| Rust/Tauri integration ease (0.15)       | 2                 | 5          | 3             | 4        | 3        | 5        | 3          |
| Commercial licensing clarity (0.10)      | 4                 | 5          | 5             | 5        | 5        | 5        | 2          |
| Maturity / maintenance / docs (0.15)     | 4                 | 5          | 3             | 4        | 4        | 5        | 3          |
| Packaging & app-size economics (0.15)    | 5 (Play AI Packs) | 4          | 3             | 5        | 4        | 5        | 2          |
| **Weighted total**                       | **4.00**          | **4.50**   | **3.30**      | **3.75** | **3.35** | **3.85** | **2.80**   |

**Decision:** a deliberate **two-runtime core + two satellites**:

- **ONNX Runtime + QNN EP (4.50)** — primary engine for all deterministic media ops (audio stack via sherpa-onnx, LaMa, SR, matting). Best Rust story, best model coverage.
- **LiteRT + NPU accelerators (4.00)** — generative/LLM stages (SD-class, FLUX.2-klein, Gemma 4, FastVLM), because that's where Google + Qualcomm + Samsung are pouring optimization work, and Play AI Packs solve the "gigabytes in the APK" problem.
- Satellites: **ggml** (whisper.cpp batch STT everywhere; llama.cpp LLM fallback below Tier 1) and **NCNN-Vulkan** (RIFE interpolation; ncnn SR ports; the whole ncnn zoo runs on Exynos GPUs too).
- **Raw QNN** is not in the core, but is the documented unlock for the *experimental* Fold3-NPU diffusion tier (§8), exactly as Local Dream ships it.

---

## 3. Model Layer — Alternatives per Stage, and the Final Stack

### 3.1 Alternatives examined (full matrix)

Per stage: candidates, size class, feasibility on (A) Fold3-class SD888/12GB and (B) 8 Gen 3+ flagships, license, and status.

**Audio — enhancement & separation**

| Model                                     | Params / size            | A: Fold3                | B: 8G3+       | License                | Verdict                                        |
| ----------------------------------------- | ------------------------ | ----------------------- | ------------- | ---------------------- | ---------------------------------------------- |
| **GTCRN** (sherpa-onnx)                   | ~48K / <1 MB             | ✅ realtime CPU          | ✅             | MIT                    | **Pick** — denoise node                        |
| DeepFilterNet3                            | ~2M / ~10 MB, Rust libDF | ✅ realtime CPU          | ✅             | MIT/Apache-2           | **Pick (alt)** — 48 kHz full-band; native Rust |
| RNNoise                                   | tiny                     | ✅                       | ✅             | BSD                    | Reject — PESQ 2.29 vs GTCRN 2.87               |
| **UVR/MDX vocal isolation** (sherpa-onnx) | ~20–60 MB                | ✅ (seconds/track)       | ✅             | Apache-2 (models vary) | **Pick** — voice-isolation node                |
| Spleeter (sherpa-onnx)                    | ~35 MB                   | ✅                       | ✅             | MIT                    | Alt stems option                               |
| HTDemucs v4 (ONNX export)                 | ~80–160 MB               | ⚠️ offline job, minutes | ✅ offline job | MIT                    | **Pick (offline)** — 4-stem "studio" node      |
| MossFormer2 / TF-GridNet                  | 10–60 M                  | ❌ no mobile port        | ⚠️ unproven   | varies                 | Reject for v1 — no mobile deployment path      |
| pyannote speech-separation-ami            | pipeline                 | ⚠️                      | ✅             | MIT                    | Hold — overlapping-speaker edge cases          |

**Speech-to-text & diarization**

| Model                                                | Size / RAM            | A                   | B   | License   | Verdict                                 |
| ---------------------------------------------------- | --------------------- | ------------------- | --- | --------- | --------------------------------------- |
| **whisper.cpp small / distil**                       | 466 MiB / ~852 MB RAM | ✅ batch RTF<1       | ✅   | MIT       | **Pick** — batch transcription node     |
| whisper tiny/base                                    | 75–142 MiB            | ✅                   | ✅   | MIT       | Quality floor / preview mode            |
| Moonshine tiny (sherpa-onnx)                         | ~60 MB                | ✅                   | ✅   | MIT       | **Pick** — short-clip fast path (EN)    |
| Parakeet 0.6B (LiteRT tflite)                        | ~600 MB               | ⚠️                  | ✅   | CC-BY-4.0 | Alt; NPU path via LiteRT                |
| **sherpa-onnx diarization** (segmentation+embedding) | ~50 MB                | ✅                   | ✅   | Apache-2  | **Pick** — speaker-tagging node         |
| Live streaming whisper                               | —                     | ❌ 5× slower than RT | ❌   | —         | Reject — verified pathology; batch only |

**Image/video upscaling & restoration**

| Model                                 | Size       | A                                  | B                            | License        | Verdict                                            |
| ------------------------------------- | ---------- | ---------------------------------- | ---------------------------- | -------------- | -------------------------------------------------- |
| **QuickSRNet S/M/L**                  | 42 KB–1 MB | ✅ GPU/CPU; 2.2 ms/frame on NPU@8G1 | ✅ NPU                        | BSD-3          | **Pick** — video upscale node                      |
| **Real-ESRGAN-x4plus / general-x4v3** | ~17–64 MB  | ⚠️ stills only (sec/img)           | ✅ (also LiteRT build exists) | BSD-3          | **Pick** — still/photo upscale node                |
| XLSR / SESR-M5                        | <100 KB    | ✅                                  | ✅                            | BSD-3 (AI Hub) | Alt to QuickSRNet (it wins PSNR)                   |
| SwinIR-light                          | ~1 M       | ⚠️ slow                            | ⚠️                           | Apache-2       | Reject v1 — transformer cost, thin mobile ports    |
| RealBasicVSR / video SR (temporal)    | ~6 M+      | ❌                                  | ❌                            | Apache-2       | Reject — not mobile-feasible; per-frame SR instead |
| waifu2x / RealSR ncnn                 | ~5–20 MB   | ✅ GPU                              | ✅                            | MIT            | Optional style-specific nodes                      |

**Inpainting / outpainting (stills)**

| Model                                 | Size          | A                                     | B                  | License                                                                 | Verdict                                                    |
| ------------------------------------- | ------------- | ------------------------------------- | ------------------ | ----------------------------------------------------------------------- | ---------------------------------------------------------- |
| **LaMa-Dilated** (AI Hub)             | ~200 MB class | ✅ GPU/CPU, ~sec                       | ✅ NPU fast         | AI Hub per-model                                                        | **Pick** — deterministic object removal                    |
| MI-GAN                                | ~6 M          | ✅                                     | ✅                  | MIT                                                                     | Alt — lighter, mobile-designed                             |
| AOT-GAN (AI Hub)                      | ~15 M         | ✅                                     | ✅                  | AI Hub per-model                                                        | Alt                                                        |
| **SD1.5-inpaint (QNN/LiteRT, w8a16)** | ~1–1.5 GB     | ⚠️ experimental (V68 NPU via raw QNN) | ✅ ~1–3 s/img       | CreativeML OpenRAIL-M + Qualcomm asset license — **review before ship** | **Pick** — generative fill/outpaint                        |
| SDXL-class inpaint                    | ~4 GB class   | ❌                                     | ✅ 8G3+ only, 1024² | OpenRAIL++                                                              | Tier-2 node                                                |
| **FLUX.2-klein-4B LiteRT (int8)**     | ~4 GB class   | ❌                                     | ✅ (new, Jul 2026)  | **Apache-2.0** ✅                                                        | **Pick (Tier 2)** — cleanest license of the generative set |
| Z-Image-Turbo LiteRT                  | int8 DiT      | ❌                                     | ✅                  | Apache-2.0                                                              | Alt turbo t2i                                              |
| MediaPipe image generator             | SD-based      | ⚠️                                    | ✅                  | Apache-2 wrapper                                                        | Superseded by LiteRT diffusion path                        |

**Video object removal / temporal edits**

| Approach                                                               | A              | B                                               | Verdict                                                                          |
| ---------------------------------------------------------------------- | -------------- | ----------------------------------------------- | -------------------------------------------------------------------------------- |
| ProPainter / E2FGVI                                                    | ❌              | ❌ (GB-scale VRAM, minutes/clip on desktop GPUs) | Reject — not on-device-feasible in 2026                                          |
| **MobileSAM/FastSAM mask + tracker + per-frame LaMa + temporal blend** | ⚠️ short clips | ✅ short clips                                   | **Pick (experimental node)** — honest scope: static-ish backgrounds, ≤10 s clips |

**Matting / background removal**

| Model                        | Size                 | A                  | B   | License                                      | Verdict                                |
| ---------------------------- | -------------------- | ------------------ | --- | -------------------------------------------- | -------------------------------------- |
| **RVM (RobustVideoMatting)** | ~15 MB (mobilenetv3) | ✅ realtime-ish GPU | ✅   | GPL-3 ⚠️ (weights MIT per repo) — **verify** | **Pick** — video BG node               |
| MODNet                       | ~25 MB               | ✅                  | ✅   | Apache-2                                     | **Pick (alt)** — safe-license fallback |
| MediaPipe Selfie Seg         | ~1 MB                | ✅ fast, coarse     | ✅   | Apache-2                                     | Preview/low-power mode                 |
| BiRefNet-lite                | ~170 MB              | ⚠️ stills          | ✅   | MIT                                          | High-quality still cutout node         |

**Frame interpolation & misc creator nodes**

| Model                      | A                          | B   | License  | Verdict                                   |
| -------------------------- | -------------------------- | --- | -------- | ----------------------------------------- |
| **RIFE (ncnn-Vulkan)**     | ✅ (slow-mo as offline job) | ✅   | MIT      | **Pick** — 2×/4× interpolation node       |
| Depth-Anything-V2 (AI Hub) | ✅                          | ✅   | AI Hub   | Optional — depth for parallax/bokeh nodes |
| MobileSAM / FastSAM        | ✅                          | ✅   | Apache-2 | **Pick** — mask-authoring utility node    |

**LLM / VLM for metadata, captions, orchestration**

| Model                       | RAM class | A                        | B             | License                   | Verdict                                 |
| --------------------------- | --------- | ------------------------ | ------------- | ------------------------- | --------------------------------------- |
| **Gemma 4 E2B (LiteRT-LM)** | ~2–3 GB   | ✅ (CPU/GPU; NPU on 8G1+) | ✅             | Apache-2.0 (litert build) | **Pick** — titles/descriptions/chapters |
| Gemma 4 E4B                 | ~4–5 GB   | ⚠️                       | ✅             | Apache-2.0                | Tier-2 quality bump                     |
| **FastVLM-0.5B (LiteRT)**   | <1 GB     | ✅                        | ✅ 0.12 s TTFT | apple-amlr ⚠️ review      | **Pick** — thumbnail/frame captioning   |
| Qwen3 0.6B–4B (LiteRT-LM)   | 0.6–4 GB  | ✅ small                  | ✅             | Apache-2.0                | Alt family                              |
| Phi-4-mini                  | ~4 GB     | ⚠️                       | ✅             | MIT                       | Alt                                     |
| llama.cpp (any GGUF)        | varies    | ✅                        | ✅             | MIT engine                | Fallback engine below Tier 1            |

### 3.2 The final workable stack (functions → models → engine)

| Pipeline node (user-facing)  | Model(s)                                | Engine                                      | Runs on Fold3?   |
| ---------------------------- | --------------------------------------- | ------------------------------------------- | ---------------- |
| Denoise voice                | GTCRN (DeepFilterNet3 "strong" option)  | ORT / libDF-Rust                            | ✅                |
| Isolate voice / remove music | UVR-MDX via sherpa-onnx                 | ORT                                         | ✅                |
| Studio stems (4-track)       | HTDemucs-ONNX                           | ORT (offline job)                           | ✅ (slow job)     |
| Transcribe                   | whisper.cpp small (Moonshine fast path) | ggml                                        | ✅                |
| Speaker tags                 | sherpa-onnx diarization                 | ORT                                         | ✅                |
| Upscale video 2×/3×          | QuickSRNet M                            | ORT-QNN / NCNN                              | ✅ GPU, NPU 8G1+  |
| Upscale photo 4×             | Real-ESRGAN-x4plus                      | ORT-QNN / NCNN                              | ✅ (seconds)      |
| Remove object (photo)        | LaMa-Dilated (MI-GAN light)             | ORT-QNN                                     | ✅                |
| Generative fill / outpaint   | SD1.5-inpaint w8a16                     | LiteRT-QNN (raw QNN on Fold3, experimental) | ⚠️               |
| Generative fill HQ           | FLUX.2-klein-4B int8 / SDXL-inpaint     | LiteRT NPU                                  | ❌ (Tier 2: 8G3+) |
| Remove background (video)    | RVM (MODNet fallback)                   | ORT / NCNN                                  | ✅                |
| Cut out subject (photo)      | BiRefNet-lite                           | ORT                                         | ⚠️ slow / ✅      |
| Smooth slow-mo               | RIFE                                    | NCNN-Vulkan                                 | ✅ (offline job)  |
| Auto title/desc/chapters     | Gemma 4 E2B                             | LiteRT-LM (llama.cpp fallback)              | ✅                |
| Caption frames / thumbnails  | FastVLM-0.5B                            | LiteRT                                      | ✅                |
| Mask helper                  | MobileSAM/FastSAM                       | ORT                                         | ✅                |

---

## 4. Preliminary Architecture

### 4.1 Process & module view

```
┌─────────────────────────── Android app process ───────────────────────────┐
│                                                                           │
│  ┌─────────────── WebView (system) ───────────────┐                       │
│  │  Node canvas (custom touch layer over Svelte    │                       │
│  │  Flow/Litegraph-class renderer) · inspector ·   │                       │
│  │  gallery · job monitor                          │                       │
│  └───────────────▲────────────────────────────────┘                       │
│         Tauri IPC │ raw payloads for previews; convertFileSrc for media    │
│  ┌───────────────┴────────────────────────────────┐   ┌────────────────┐  │
│  │           RUST CORE (cdylib)                    │   │ Kotlin plugin  │  │
│  │  • Graph model + validation (typed ports:      │◄──┤ layer (Tauri   │  │
│  │    Audio, Video, Image, Mask, Text, Tensor)    │JNI│ mobile plugin) │  │
│  │  • DAG scheduler: topological, chunk-streaming │   │ • LiteRT/      │  │
│  │  • Asset store (content-addressed, resumable)  │   │   LiteRT-LM    │  │
│  │  • Tiler (SR/inpaint tiles w/ overlap blend)   │   │ • Play AI Packs│  │
│  │  • Thermal/battery governor                    │   │ • MediaCodec   │  │
│  │  • Engines: ort(QNN) · ggml · ncnn · libDF     │   │ • WorkManager/ │  │
│  └────────────────────────────────────────────────┘   │   FG service   │  │
│                                                       └────────────────┘  │
└───────────────────────────────────────────────────────────────────────────┘
```

Key architectural decisions, each traceable to a verified finding:

1. **Rust core owns the pipeline; Kotlin owns Android.** Tauri 2's mobile plugin system bridges Rust→Kotlin over JNI (`run_mobile_plugin`), and plugin commands run **on the main thread by default** — so every engine call is dispatched to worker threads/coroutines, and long jobs live in a **foreground service via WorkManager**, not in Tauri command handlers (ANR avoidance).
2. **Two engine tiers per node.** Every node declares an ordered backend list, e.g. Upscale: `[QNN-NPU, GPU, CPU-XNNPACK]`. LiteRT's built-in fallback covers its models; the Rust core implements the same policy for ORT/NCNN. One pipeline file runs on every device; only speed differs.
3. **Fixed-shape execution + tiling.** QNN EP requires fixed input shapes → the tiler normalizes arbitrary media into fixed tile sizes (e.g., 512² inpaint tiles, 128² SR tiles — QuickSRNet's native design) with overlap blending. This is also what makes RAM bounded and thermal load smooth.
4. **EPContext/compilation caches are first-class.** ORT QNN context binaries and LiteRT compilation caches are stored per-device-per-model at first run ("Optimizing for your device… once") — the verified 7.5 s → 0.2 s init delta is the difference between a toy and a product.
5. **Media I/O via MediaCodec (Kotlin side)**, not ffmpeg-in-Rust, for hardware decode/encode of the user's actual camera footage (HEVC/AV1), with an LGPL ffmpeg build only if container coverage demands it.
6. **Segment-based video processing with checkpointing.** Videos process in N-second segments written to the asset store; a thermal pause or app death resumes at the last segment.
7. **Thermal governor:** subscribes to `PowerManager.getThermalHeadroom()`; policy = *degrade before pausing* (drop from NPU burst to GPU sustained, widen frame stride, then pause). Sized against the verified 74–77% sustained-CPU reality, treating **~70% of burst throughput as the planning number** for any job >10 min.

### 4.2 Pipeline definition (what a "workflow" is)

```json
{
  "nodes": [
    {"id": "in",   "type": "source.video", "uri": "content://..."},
    {"id": "aud",  "type": "audio.split"},
    {"id": "dnz",  "type": "audio.denoise",    "model": "gtcrn", "strength": 0.8},
    {"id": "voc",  "type": "audio.isolate_voice", "model": "uvr-mdx"},
    {"id": "sr",   "type": "video.upscale",    "model": "quicksrnet-m", "scale": 2},
    {"id": "bg",   "type": "video.remove_bg",  "model": "rvm"},
    {"id": "mux",  "type": "av.mux"},
    {"id": "out",  "type": "sink.gallery"}
  ],
  "edges": [["in","aud"],["aud.voice","dnz"],["dnz","voc"],
            ["in.frames","sr"],["sr","bg"],["bg","mux"],["voc","mux"],["mux","out"]]
}
```

Deterministic, JSON-serializable, shareable — the unit of virality (creators trade preset pipelines the way they trade LUTs).

### 4.3 UI: node canvas at n8n complexity, not ComfyUI

- **Verified warning:** React Flow-class libraries are *officially* awkward on touch (connect-by-drag fails; maintainers document workarounds; issue #1323 "unusable on touch devices"). So the canvas ships a **touch-first interaction layer**: tap-port → tap-port to connect, long-press for node palette, magnetic snapping, 44 dp+ hit targets; the rendering layer underneath can still be Svelte Flow/Litegraph-class.
- **Linear-first presentation.** Most creator pipelines are chains, not graphs. Default view = a vertical "recipe" list (n8n-style) that *is* the DAG; the 2-D canvas is an "advanced view" toggle. Fold3 bonus: cover screen = recipe view; unfolded = canvas view.
- **Preset packs over blank canvas:** "Podcast cleanup," "Old footage revival" (SR + RIFE), "Remove background," "Shorts factory" (transcribe → chapter → caption). Each opens as an editable pipeline — the on-ramp from consumer to power user.
- Node inspector = bottom sheet; previews render tile-progressively; every generative node shows seed + "re-roll."

### 4.4 Tauri 2 assessment (your bonus question)

Verdict: **viable, with eyes open.** For: stable Android target since Oct 2024 (system WebView, ~small APK), Kotlin plugin system with `@Command`/JNI bridge is exactly the shape needed for LiteRT + MediaCodec + WorkManager, raw IPC payloads avoid JSON serialization for media buffers, Rust core is where your pipeline logic wants to live anyway, and the same core/UI reuses on desktop later (a real distribution hedge). Against: mobile DX admitted below desktop parity at 2.0; main-thread command default (design around it — §4.1.1); official plugins incomplete on mobile; WebView variance across OEMs; 16 KB page-alignment requirement for new Play submissions (NDK note). **Plan B (de-risked):** identical architecture minus Tauri — Kotlin + Compose host with the same Rust cdylib via UniFFI, and the same WebView canvas in a `WebView` composable. The Rust core and the UI survive a framework swap; only the shell changes.

---

## 5. Performance Envelope (what to promise users)

| Operation                                | Fold3-class (SD888, 12 GB)                                                | 8 Gen 2 class | 8 Gen 3 / 8 Elite class          |
| ---------------------------------------- | ------------------------------------------------------------------------- | ------------- | -------------------------------- |
| Voice denoise (GTCRN)                    | ≫ real time                                                               | ≫ real time   | ≫ real time                      |
| Vocal isolation (UVR)                    | ~fraction-of-RT, seconds/track                                            | faster        | faster                           |
| 4-stem HTDemucs                          | offline job, ~minutes/track                                               | minutes       | ~1–2 min/track                   |
| Transcribe 10 min (whisper small, batch) | ~2–4 min CPU                                                              | ~1.5–3 min    | ≤1–2 min (NPU/GPU assist)        |
| Photo 4× upscale (Real-ESRGAN)           | ~2–10 s GPU                                                               | ~1–3 s        | <1 s NPU                         |
| Video 2× upscale (QuickSRNet)            | ~few-ms/frame GPU → ~faster-than-RT 1080p                                 | RT+           | 2.2 ms/frame class on NPU        |
| Object removal photo (LaMa)              | ~1–3 s                                                                    | ~0.5–1 s      | ~0.2–0.5 s NPU                   |
| Generative fill 512² (SD1.5, 20 steps)   | ~30–90 s experimental NPU / slower CPU                                    | ~5–6 s        | **~1–3 s** (46 ms/step verified) |
| SDXL/FLUX-klein 1024²                    | —                                                                         | —             | ~5–15 s                          |
| RIFE 2× interpolation, 1080p             | offline job                                                               | offline       | near-RT short clips              |
| Gemma 4 E2B metadata gen                 | ~5–15 tok/s CPU                                                           | ~15–20 tok/s  | 20+ tok/s (NPU assist)           |
| **Sustained-load derating**              | plan ×0.7 of the above for >10 min jobs; governor degrades before pausing | ×0.75         | ×0.75                            |

RAM budget on the 12 GB floor: OS + app ≈ 4–5 GB leaves ~6 GB workable → one generative model *or* (audio stack + SR + matting) resident; the scheduler serializes stage families accordingly (never co-resident SD + Gemma on Tier 0/1).

---

## 6. PRD — NodeForge v1.0

### 6.1 Problem & positioning

YouTubers and content creators do repetitive media cleanup (denoise, isolate voice, upscale, remove objects/backgrounds, transcribe, caption, package metadata) through a patchwork of cloud microsaas subscriptions — each a privacy leak, a rendering queue, and a monthly fee. High-end Android phones since ~2021 carry idle NPU/GPU silicon capable of all of it. **NodeForge is a fully-local, pipeline-based media utility: chain simple AI nodes like n8n, run them on your phone's silicon, own your footage end-to-end.**

**Non-goals (v1):** timeline video editing (CapCut exists), live-streaming effects, cloud rendering of any kind, MediaTek/entry-level support, iOS (v2 candidate via the same Rust core).

### 6.2 Personas

1. **The Solo YouTuber** — batch-cleans talking-head footage; wants "Podcast cleanup" preset, transcript, chapters, titles. Attended use, cares about time-to-done.
2. **The Archive Reviver** — upscales/interpolates old footage overnight; unattended long jobs; cares about resumability and thermal safety.
3. **The Shorts Factory** — transcribe → find hooks → caption → 9:16 crop with subject tracking; runs pipelines many times a day; trades preset files with peers.

### 6.3 Minimum & recommended hardware (the Fold3 floor)

**Minimum supported device: Samsung Galaxy Z Fold3 5G** — Snapdragon 888 (SM8350, Hexagon 780 = **V68**), 12 GB RAM, Android 12+ (device shipped w/ 11, upgradable to 14; **API 31+ is also LiteRT-NPU's own floor**), arm64-v8a. Rationale: 12 GB RAM comfortably clears every Tier-0 model set; SD888's GPU (Adreno 660) runs the ORT/NCNN Vulkan-class workloads; its V68 NPU is *provably* SD1.5-capable (Local Dream) even though the official LiteRT-QNN delegate floor is 8 Gen 1 — hence Tier 0 treats NPU as experimental, not assumed.

| Tier                                                                      | Hardware class                       | Guaranteed capability                                                                                                                                                                                                     |
| ------------------------------------------------------------------------- | ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **T0 — Baseline** (min: Z Fold3 / SD888 / Exynos 2100, 12 GB)             | CPU + GPU (Vulkan/OpenCL)            | Full audio stack; batch Whisper; LaMa object removal; QuickSRNet & Real-ESRGAN; RVM/MODNet matting; RIFE (offline); MobileSAM; Gemma 4 E2B (CPU/GPU); **SD1.5 generative fill flagged Experimental** via raw-QNN V68 path |
| **T1 — NPU standard** (8 Gen 1/2; Exynos 2400 w/ LiteCore-covered models) | + official LiteRT-QNN / LiteCore NPU | Everything above NPU-accelerated; SD1.5 fill supported (~5–6 s); multi-resolution img2img/inpaint                                                                                                                         |
| **T2 — Flagship** (8 Gen 3 / 8 Elite+; 12–16 GB)                          | + big-model NPU                      | SDXL-class & FLUX.2-klein-4B fill/outpaint; near-interactive SD1.5 (~1–3 s); 1080p video SR at speed; Gemma 4 E4B                                                                                                         |

Device capability is **probed at first launch** (SoC ID + delegate load test + timed micro-bench) and stored; nodes render as available/accelerated/experimental/unavailable accordingly. Foldables get the dual-view UI (§4.3); large-inner-screen layout is a P1 requirement, not a nicety — the canvas is the product on that display.

### 6.4 Functional requirements

- **FR1 Pipeline editor:** create/edit/run DAGs of typed nodes; linear "recipe" view default, canvas view advanced; validation before run (type mismatches, tier-unavailable nodes).
- **FR2 Node library v1.0 (16 nodes):** the §3.2 table is normative — sources (video/image/audio/text), audio (denoise, isolate, stems, transcribe, diarize), image (upscale, object-remove, generative-fill, cutout), video (upscale, remove-bg, interpolate, object-remove-experimental), text/LLM (metadata-gen, caption), sinks (gallery, files, share-sheet).
- **FR3 Presets:** ≥6 shipped preset pipelines; import/export as JSON files; presets open as editable graphs.
- **FR4 Jobs:** queued, background (foreground-service), pausable, resumable across app restarts; per-segment progress; battery/thermal policy visible ("paused: cooling down, resumes automatically").
- **FR5 Model manager:** on-demand model downloads (Play AI Packs for LiteRT models; CDN for ORT/ggml assets) with per-model size/license display; delete/re-download; nothing bundled in the base APK beyond GTCRN-class tiny models. Base APK ≤ 300 MB; typical installed footprint with starter set ≤ 2 GB.
- **FR6 Privacy: zero media egress.** No network permission needed after model download; a visible "local-only" indicator; opt-in anonymous perf telemetry only.
- **FR7 Storage:** SAF/MediaStore compliant; outputs to user-chosen tree; content-addressed intermediate cache with a one-tap "clear cache."
- **FR8 Capability probe & graceful tiering** per §6.3.

### 6.5 Non-functional requirements

- **NFR1 Thermal:** no job may drive the device to thermal shutdown; governor derates ≥3 steps before pausing; sustained jobs sized at 0.7× burst throughput.
- **NFR2 Reliability:** any job interrupted at segment *n* resumes at *n*; model-load failures fall back down the backend list, never crash the pipeline.
- **NFR3 Responsiveness:** UI thread never blocked >100 ms by engine work (all inference off main thread; Tauri command handlers dispatch-only).
- **NFR4 Licensing hygiene:** every shipped model's license surfaced in-app; no GPL code linked into the app binary (RVM license verified or MODNet substituted); Qualcomm AI Hub *compiled-asset* license and Stable Diffusion OpenRAIL-M terms legal-reviewed before store release; FLUX.2-klein (Apache-2.0) preferred for generative marketing claims.
- **NFR5 Size/perf budget:** cold start ≤ 2.5 s on Fold3; first-run model compile clearly communicated (one-time, cached).

### 6.6 Milestones

| Milestone                     | Contents                                                                                                                                 | Exit criterion                                                                                       |
| ----------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| **M0 Spike (4–6 wk)**         | Rust core skeleton + ORT-QNN on one device; GTCRN + LaMa + QuickSRNet nodes CLI-driven; Tauri 2 shell boots on Fold3 & an 8 Gen 3 device | LaMa tile inpaint < 3 s on 8G3 NPU; same pipeline runs (slower) on Fold3 GPU                         |
| **M1 Audio vertical (8 wk)**  | Recipe UI (linear only) + full audio stack + whisper batch + presets "Podcast cleanup," "Transcribe & chapter"                           | A 30-min recording cleaned + transcribed + chaptered, fully offline, on Fold3, without thermal pause |
| **M2 Visual vertical (8 wk)** | Upscale/matting/RIFE/cutout nodes; job system w/ resume; model manager; canvas view                                                      | "Old footage revival" preset survives a forced app-kill and resumes                                  |
| **M3 Generative tier (6 wk)** | SD1.5 fill via LiteRT-QNN (T1/T2), raw-QNN experimental toggle for T0; FLUX-klein on T2; capability probe UI                             | Fill node interactive on 8G3 (≤5 s); correctly gated & labeled on Fold3                              |
| **M4 Beta**                   | LLM metadata nodes, preset sharing, polish, closed beta with 20–50 creators                                                              | Retention of ≥1 pipeline-run/week per active beta user                                               |

### 6.7 Risks & mitigations

| Risk                                                | Likelihood | Impact                   | Mitigation                                                                                                            |
| --------------------------------------------------- | ---------- | ------------------------ | --------------------------------------------------------------------------------------------------------------------- |
| Tauri mobile rough edges stall UI work              | Med        | Med                      | Plan-B shell (Compose + UniFFI) shares 90% of code; decide by end of M1                                               |
| Touch node-canvas UX fails users                    | Med        | High                     | Linear recipe view is the primary UX; canvas optional; usability-test at M1                                           |
| Qualcomm compiled-asset / OpenRAIL license friction | Med        | High for generative tier | Deterministic nodes (LaMa etc.) carry the product; FLUX-klein (Apache-2.0) as flagship generative; legal review in M3 |
| Exynos NPU (LiteCore) coverage gaps                 | High       | Med                      | Exynos = GPU-first by design; NPU is upside, never assumed                                                            |
| Thermal/battery complaints on long jobs             | High       | Med                      | Governor + explicit overnight "plugged-in mode"; set expectations in UI                                               |
| Model zoo churn (mid-2026 pace)                     | High       | Low                      | Engine-adapter abstraction; models are downloadable content, not app code                                             |
| whisper streaming expectations                      | Low        | Low                      | Market batch transcription only; never promise live captions in v1                                                    |

### 6.8 Success metrics (v1)

North star: **pipelines completed per weekly-active device**. Guardrails: job completion rate ≥ 97% (excl. user-cancel), thermal-pause rate < 10% of jobs, median "Podcast cleanup" wall-time ≤ 1.5× audio duration on T1, store rating ≥ 4.3.

---

## 7. Open Questions (deliberately unresolved)

1. **Exynos LiteCore in practice** — no independent benchmarks surfaced; needs a device-lab test on an Exynos 2400 S24 before committing Exynos marketing claims.
2. **RVM licensing** (GPL-3 repo vs weights) — substitute MODNet if counsel is unhappy.
3. **Video object-removal quality bar** — the mask+LaMa+blend approach needs a golden-clip eval set before it's allowed out of "experimental."
4. **Distribution of raw-QNN HTP binaries for T0 diffusion** — per-SoC binaries (Local Dream's approach) vs dropping T0 generative entirely; decide on real Fold3 measurements at M3.
5. **Name.** "NodeForge" is a placeholder; check trademark space before beta.

---

## 8. Source Index (primary unless noted)

**Runtimes/NPU:** developer.android.com NNAPI migration guide (2026-03-06) · developers.google.com/edge/litert/next/npu (2026-06-16) · developers.google.com/edge/litert/android/npu/qualcomm (2026-05-28) · developers.googleblog.com "Unlocking Peak Performance on Qualcomm NPU with LiteRT" (2025-11-24) · docs.pytorch.org/executorch/stable/backends-qualcomm.html · onnxruntime.ai QNN EP docs · Maven Central (com.qualcomm.qti artifacts; com.microsoft.onnxruntime:onnxruntime-android-qnn).
**Models:** github.com/qualcomm/ai-hub-models · huggingface.co/qualcomm/{Stable-Diffusion-v1.5, Real-ESRGAN-x4plus, LaMa-Dilated, AOT-GAN, SESR-M5, QuickSRNet*} · QuickSRNet paper (CVPRW 2023) · github.com/k2-fsa/sherpa-onnx (v1.13.0) · github.com/Xiaobin-Rong/gtcrn · github.com/Rikorose/DeepFilterNet · github.com/ggml-org/whisper.cpp (+ discussion #3567) · mixxx.org GSoC 2025 Demucs-to-ONNX · huggingface.co/litert-community/{gemma-4-E2B-it-litert-lm, FLUX.2-klein-4B-LiteRT, Z-Image-Turbo-LiteRT, FastVLM-0.5B, parakeet-*, Qwen3-*, real-esrgan-x4v3-litert}.
**Prior art:** github.com/xororz/local-dream (v2.6.1) · github.com/google-ai-edge/gallery · github.com/alibaba/MNN (3.5.0) · awesome-alternative-uis-for-comfyui · F-Droid SDAI FOSS · Samsung Galaxy AI support docs.
**Framework/UI:** v2.tauri.app (2.0 announcement; mobile plugin docs, 2026-05-14) · reactflow.dev touch-device example · xyflow issue #1323.
**Performance:** Beebom Snapdragon 8 Elite sustained-load testing (2024-11) *(secondary)*.

*Verification note: 65 claims triple-checked against live sources 2026-07-19; four refuted and corrected in-text (ONNX-QNN fp16 admission; SD1.5 current-card latencies/quantization; two scope overreads). Figures marked "~" are engineering estimates interpolated from verified neighbors, not measurements.*
