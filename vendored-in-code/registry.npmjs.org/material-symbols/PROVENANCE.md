# material-symbols — vendored font files

**Upstream:** https://registry.npmjs.org/material-symbols
**Version pinned:** 0.45.9
**Tarball:** `https://registry.npmjs.org/material-symbols/-/material-symbols-0.45.9.tgz`
**Tarball sha256:** `9f03a3cd4de256e82bb0f11d9d3b5df4e305245b0f0352e4fd19b4cc728ba793`
**Licence:** Apache License 2.0 (`SPDX-License-Identifier: Apache-2.0`)
**Original project:** Material Symbols by Google — https://github.com/google/material-design-icons
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

- `material-symbols-outlined.woff2` — sha256 `a5b8dcb83da3c050fff603b7f72f071ebbcc0bf485546bc97cbddc9508c522e6`

Derived, subset copies used by the build live under `ui/src/assets/fonts/`.
Nothing in this directory is edited; it is the reference copy.
