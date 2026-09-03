# PROVENANCE — LiteRT

## Component
LiteRT (formerly TensorFlow Lite) — carries generative and LLM stages,
because that is where the Android NPU acceleration story is strongest (AD-2).

## Upstream
- **URL:** https://github.com/google-ai-edge/LiteRT
- **Pinned commit:** (to be filled when the tree is assimilated)
- **Release tag:** (to be filled when the tree is assimilated)

## Licence
- **SPDX-License-Identifier:** Apache-2.0
- **Full text:** https://github.com/google-ai-edge/LiteRT/blob/main/LICENSE

## Retrieval
- **Date:** (to be filled when the tree is assimilated)
- **Method:** `git clone` straight into this directory (INC-15)

## Reason vendored
AD-3 requires every engine in the core to be vendored as source with a build
recipe we have run. LiteRT carries generative and LLM stages where the QNN
delegate provides decisive acceleration (AD-2a). The published Maven artifact
is consumed as a scaffold in T18; this vendored source is the from-source
substitution that T24 performs. A binary without its source means we cannot
patch it, cannot reproduce it, and are at upstream's mercy for anything that
changes underneath us.

## Notes
- The QNN delegate is a proprietary Qualcomm binary and is explicitly out of
  scope for from-source building (AD-2a). It stays a published binary confined
  to generative stages. The CPU/GPU path in LiteRT is fully open source and
  is what the from-source build produces.
- The `LiteRtBridge.kt` in the android module may consume the published Maven
  artifact as a scaffold until T24 substitutes this from-source build.
