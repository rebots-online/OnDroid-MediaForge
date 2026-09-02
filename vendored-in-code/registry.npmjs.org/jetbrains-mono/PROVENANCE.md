# @fontsource/jetbrains-mono — vendored font files

**Upstream:** https://registry.npmjs.org/@fontsource/jetbrains-mono
**Version pinned:** 5.2.8
**Tarball:** `https://registry.npmjs.org/@fontsource/jetbrains-mono/-/jetbrains-mono-5.2.8.tgz`
**Tarball sha256:** `6626da48530eb76b183d06acef52b4609d8ea3a35a7ed5d4fd8ef656eaca0a3c`
**Licence:** SIL Open Font License 1.1 (`SPDX-License-Identifier: OFL-1.1`)
**Original project:** JetBrains Mono — https://github.com/JetBrains/JetBrainsMono
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

- `jetbrains-mono-latin-400-normal.woff2` — sha256 `14425ba9c695763c1547f48a206b7aa60350a33ae23de09f0407877f3fcd89eb`
- `jetbrains-mono-latin-500-normal.woff2` — sha256 `cb182feeed4d798ff6961d3c79f7026279448fca0676438aaecb21f3fc39553a`
- `jetbrains-mono-latin-700-normal.woff2` — sha256 `d0d4e818808f2a0ba39b2b09d1989366f63494e295f003c7ef436697378507e8`

Derived, subset copies used by the build live under `ui/src/assets/fonts/`.
Nothing in this directory is edited; it is the reference copy.
