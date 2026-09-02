# Self-hosted fonts — provenance

The `.woff2` files in this directory are **unmodified** copies of the pinned
tarballs vendored under `vendored-in-code/registry.npmjs.org/`. They are copied
here by `scripts/vendor-stitch-assets.sh` so the WebView can load them from the
app bundle without an outbound request.

## Families

| Family | Upstream | Version | SPDX Licence | Weights |
| --- | --- | --- | --- | --- |
| Inter | `registry.npmjs.org/@fontsource/inter` | 5.3.0 | OFL-1.1 | 400, 500, 600, 700 |
| Chivo | `registry.npmjs.org/@fontsource/chivo` | 5.3.0 | OFL-1.1 | 400, 600, 700 |
| JetBrains Mono | `registry.npmjs.org/@fontsource/jetbrains-mono` | 5.3.0 | OFL-1.1 | 400, 500, 700 |
| Material Symbols Outlined | `registry.npmjs.org/@fontsource/material-symbols` | 5.3.0 | Apache-2.0 | 100–700 (subset) |

Each family's `PROVENANCE.md` under `vendored-in-code/registry.npmjs.org/`
records the tarball sha256, retrieval date, and per-file sha256 for integrity
verification. The copies here are byte-identical to those verified files.

## Why self-hosted

The frozen Stitch screens under `LIBS/UI/STITCH/screens/` request these
families from `fonts.googleapis.com` at render time. Shipping that would put
an outbound request to a third party on app open — outside the three network
uses the privacy copy authorises (model downloads, entitlement sync, opt-in
telemetry) — and would leave an offline install unstyled. AD-3 and
`DOCS/ARCHITECTURE.md` §5 require the families self-hosted instead.

## Material Symbols subset

`material-symbols-outlined-subset.woff2` is a subset of the full
`material-symbols-outlined.woff2`, produced by
`ui/scripts/subset-icon-font.mjs` to include only the ligatures the frozen
screen complement uses. The full font is retained under
`vendored-in-code/registry.npmjs.org/material-symbols/` for re-subsetting.
