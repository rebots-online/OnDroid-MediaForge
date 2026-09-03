# PROVENANCE — GGML

## Component
GGML (whisper.cpp and llama.cpp) — carries Whisper transcription and LLM
diarisation/metadata stages via FFI (AD-2).

## Upstream
- **URL:** https://github.com/ggml-org/ggml
- **Pinned commit:** (to be filled when the tree is assimilated)
- **Release tag:** (to be filled when the tree is assimilated)

## Licence
- **SPDX-License-Identifier:** MIT
- **Full text:** https://github.com/ggml-org/ggml/blob/main/LICENSE

## Retrieval
- **Date:** (to be filled when the tree is assimilated)
- **Method:** `git clone` straight into this directory (INC-15)

## Reason vendored
AD-3 requires every engine in the core to be vendored as source with a build
recipe we have run. GGML provides the Whisper and LLM inference paths that
the audio stack and metadata generation depend on. A published binary without
its source means we cannot patch it, cannot reproduce it, and are at
upstream's mercy for anything that changes underneath us. Vendoring the
source and proving the build discharges that obligation.

## Notes
- The `GgmlEngine` in `forge-engines` calls into this tree via FFI.
- whisper.cpp and llama.cpp are downstream of the ggml library; both are
  needed for the full audio + LLM pipeline.
