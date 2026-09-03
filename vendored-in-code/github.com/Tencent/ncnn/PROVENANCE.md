# PROVENANCE — NCNN

## Component
NCNN — Vulkan-accelerated inference for RIFE frame interpolation and the
ncnn super-resolution ports. Works on Exynos GPUs (AD-2).

## Upstream
- **URL:** https://github.com/Tencent/ncnn
- **Pinned commit:** (to be filled when the tree is assimilated)
- **Release tag:** (to be filled when the tree is assimilated)

## Licence
- **SPDX-License-Identifier:** BSD-3-Clause
- **Full text:** https://github.com/Tencent/ncnn/blob/master/LICENSE.txt

## Retrieval
- **Date:** (to be filled when the tree is assimilated)
- **Method:** `git clone` straight into this directory (INC-15)

## Reason vendored
AD-3 requires every engine in the core to be vendored as source with a build
recipe we have run. NCNN provides RIFE frame interpolation and super-resolution
inference with Vulkan acceleration, which works across GPU vendors including
Exynos. A published binary without its source means we cannot patch it, cannot
reproduce it, and are at upstream's mercy for anything that changes underneath
us. Vendoring the source and proving the build discharges that obligation.

## Notes
- The `NcnnEngine` in `forge-engines` calls into this tree via its C API.
- Vulkan support is required for the GPU path; the NDK cross-compile must
  link against the Android Vulkan loader.
