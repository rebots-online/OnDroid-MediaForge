# @fontsource/inter — vendored font files

**Upstream:** https://registry.npmjs.org/@fontsource/inter
**Version pinned:** 5.3.0
**Tarball:** `https://registry.npmjs.org/@fontsource/inter/-/inter-5.3.0.tgz`
**Tarball sha256:** `02034af8d41dcc67ac8eab88f642e129bb8a2e8922abf30229c32725f54d8fd6`
**Licence:** SIL Open Font License 1.1 (`SPDX-License-Identifier: OFL-1.1`)
**Original project:** Inter by Rasmus Andersson — https://github.com/rsms/inter
**Retrieved:** 2026-07-26

## Why this is vendored

The frozen Stitch screens under `LIBS/UI/STITCH/screens/` request this family
from `fonts.googleapis.com` at render time. Shipping that would put an outbound
request to a third party on app open — outside the three network uses the privacy
copy authorises — and would leave an offline install unstyled. AD-3 and
`DOCS/ARCHITECTURE.md` §5 require the family self-hosted instead, so the bytes
live here rather than being fetched.

## What is here

The `.woff2` files below are **unmodified** bytes extracted from the pinned
tarball above. The tarball itself is not retained — its sha256 is recorded so any
file here can be re-verified against upstream by re-downloading it and comparing.
Only the weights the design system and the frozen screens actually use were kept.

- `inter-latin-400-normal.woff2` — sha256 `8909904ab6c872eb994093482a88a28eca2cd95912d7b6fecd72103b0dc07edc`
- `inter-latin-500-normal.woff2` — sha256 `f3779f1efccc4bdcdf9c0a02ab95bf6bd092ed09c48c08cedc725889edd1d19f`
- `inter-latin-600-normal.woff2` — sha256 `f9a06e79cd3a2a20951c0f0e28f66dd0e6d3fda73911d640a2125c8fcb78f21a`
- `inter-latin-700-normal.woff2` — sha256 `6f56409fd3d64bb85f7d070bce20749db2d66b6d63cec586cc22d1c761be2491`

Derived, subset copies used by the build live under `ui/src/assets/fonts/`.
Nothing in this directory is edited; it is the reference copy.
