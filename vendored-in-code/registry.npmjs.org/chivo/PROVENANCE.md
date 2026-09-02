# @fontsource/chivo — vendored font files

**Upstream:** https://registry.npmjs.org/@fontsource/chivo
**Version pinned:** 5.2.8
**Tarball:** `https://registry.npmjs.org/@fontsource/chivo/-/chivo-5.2.8.tgz`
**Tarball sha256:** `065ae1b985815623a20262a6660dffb18c2852ae68a45b4433e8b965f8f125cf`
**Licence:** SIL Open Font License 1.1 (`SPDX-License-Identifier: OFL-1.1`)
**Original project:** Chivo by Omnibus-Type — https://github.com/Omnibus-Type/Chivo
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

- `chivo-latin-400-normal.woff2` — sha256 `e0d2fe7ce671a90326e0bc1a17c9fd8450cd9c7a56baaccf693abe3fb1e7cbae`
- `chivo-latin-600-normal.woff2` — sha256 `94ad3cc906c2c49d3bfc778195a9b65118b904d56ef3bd660f903f409fd2a937`
- `chivo-latin-700-normal.woff2` — sha256 `f02d0229588e8d5fe6670bb34215b75b19e2cd6ff1779fa2dabe7970fe3292a1`

Derived, subset copies used by the build live under `ui/src/assets/fonts/`.
Nothing in this directory is edited; it is the reference copy.
