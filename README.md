# OnDroid MediaForge

Chain simple AI nodes like n8n, run them on your phone's own silicon, and keep
your footage on the device.

Creators do the same media cleanup over and over — denoise a voice track, isolate
it from music, upscale old footage, remove an object or a background, transcribe,
caption, write the metadata. Today that means a patchwork of cloud subscriptions,
each one a privacy question, a render queue, and a monthly fee. Meanwhile the
phone in your pocket has been shipping with idle NPU silicon since about 2021.

MediaForge runs that work where the footage already is.

## What it does

Build a pipeline by chaining nodes — a linear recipe by default, a full node
canvas when you want branching. Sixteen nodes at v1 cover the common jobs:

- **Audio** — denoise, isolate voice from music, 4-stem studio separation,
  transcribe, tag speakers
- **Image** — upscale to 4×, remove objects, generative fill and outpaint, cut
  out a subject
- **Video** — upscale, remove background, smooth slow-motion, remove objects
- **Text** — generate titles, descriptions and chapters; caption frames

Pipelines are plain JSON files. Share one and it runs on someone else's phone
exactly as it ran on yours.

## Your footage stays yours

Media never leaves the device. Inference is always local — there is no render
queue and no upload step. The app does use the network for three things, and
says so plainly rather than claiming to be offline: downloading models, syncing
your entitlements, and optional anonymous performance telemetry you opt into.

## Will it run on my phone?

MediaForge checks your actual silicon at first launch and tells you what it can
do, in seconds and megabytes. Nodes your device cannot run are shown as such,
with a substitute offered — never dressed up as something to buy.

- **Baseline** (Snapdragon 888 / Exynos 2100 class, 12 GB, Android 12+) runs the
  whole audio stack, transcription, object removal, upscaling, background
  removal, slow-motion, and on-device metadata generation. Generative fill is
  available but marked experimental and slow.
- **NPU standard** (8 Gen 1/2, Exynos 2400) accelerates all of that on the neural
  engine and supports generative fill properly.
- **Flagship** (8 Gen 3 / 8 Elite) adds SDXL-class and FLUX generative work and
  near-interactive fill.

The minimum supported device is the Galaxy Z Fold3. Unfolded, you get a two-pane
canvas — on that display the node canvas is the point, not a stretched phone
screen.

## Pricing

Free covers every deterministic operation and running any pipeline someone shares
with you. Beyond that you choose what fits: buy credits and pay per generation,
or unlock everything. The headline option is paying once and keeping that version
for good.

## Status

Pre-implementation. This repository currently holds the research that grounds the
design, the product and architecture specifications, and the frozen UI design
system. Application code has not been written yet.

The design is backed by a research report covering roughly 30 primary sources
with 65 claims individually verified against live documentation — see
`DOCS/ondevicemediapipelinereport.md`.

## Licensing

Every model shipped with the app has its license shown in-app before you download
it. Model weights are downloadable content rather than application code, and no
GPL-licensed code is linked into the app binary.
