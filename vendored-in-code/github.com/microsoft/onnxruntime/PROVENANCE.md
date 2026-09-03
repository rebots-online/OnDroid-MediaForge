# PROVENANCE — ONNX Runtime

## Component
ONNX Runtime (ORT) — the workhorse runtime for deterministic media operations:
the audio stack, LaMa, super-resolution and matting (AD-2).

## Upstream
- **URL:** https://github.com/microsoft/onnxruntime
- **Pinned commit:** (to be filled when the tree is assimilated)
- **Release tag:** (to be filled when the tree is assimilated)

## Licence
- **SPDX-License-Identifier:** MIT
- **Full text:** https://github.com/microsoft/onnxruntime/blob/main/LICENSE

## Retrieval
- **Date:** (to be filled when the tree is assimilated)
- **Method:** `git clone` straight into this directory (INC-15)

## Reason vendored
AD-3 requires every engine in the core to be vendored as source with a build
recipe we have run. ONNX Runtime is the primary inference runtime for
deterministic media operations. A published binary without its source means we
cannot patch it, cannot reproduce it, and are at upstream's mercy for anything
that changes underneath us. Vendoring the source and proving the build
discharges that obligation.

## Notes
- The QNN execution provider is a proprietary Qualcomm binary and is
  explicitly out of scope for from-source building (AD-2a). It stays a
  published binary confined to generative stages.
- The Rust bindings (`ort` crate) are consumed separately via crates.io and
  are not part of this vendored tree.
