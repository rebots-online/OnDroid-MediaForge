#!/usr/bin/env bash
# Normalise frozen Stitch screen markup to the canonical design tokens.
#
# Why this exists
# ---------------
# LIBS/UI/STITCH/DESIGN.md is the single source of truth for this product's
# design tokens. The copy of that system stored inside Stitch drifted away from
# it: given a copper accent and no explicit neutral, the tool re-derived the
# whole neutral ramp as a Material tonal palette seeded from that accent and
# produced a warm-brown ground in place of the intended graphite. Generated
# screens therefore arrive carrying brown surface tokens.
#
# Attempting to correct the stored system through update_design_system was
# rejected by the API ("Request contains an invalid argument"), and the two
# override fields that would pin the neutral appear not to be accepted by this
# deployment. So the correction is applied to the artifact instead, which is
# the thing we actually own and freeze.
#
# This is a pure hex-for-hex substitution with a fixed table. It is idempotent:
# running it twice changes nothing the second time, because the canonical values
# are not themselves keys in the table.
#
# Run after freezing any newly generated screens.
#
# Known limitation: Stitch collapsed `surface` and `surface-dim` onto the same
# stored value, so the dim step cannot be recovered by substitution. Both map to
# `surface`. Any screen needing the dim step must set it explicitly.

set -euo pipefail

SCREENS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/LIBS/UI/STITCH/screens"

if [ ! -d "$SCREENS_DIR" ]; then
    echo "No frozen screens at $SCREENS_DIR" >&2
    exit 1
fi

# drifted-stored -> canonical, per DESIGN.md frontmatter.
read -r -d '' TOKEN_MAP <<'EOF' || true
1c110b 121316
160c07 08090a
251913 181a1e
291d17 1d1f23
342720 24262b
40322b 2c2e34
f5ded3 e8e6e3
dfc0b1 b3afa9
a78b7d 8a857e
584237 3f4045
ffb68e ffb077
99cbff a8c7e8
EOF

shopt -s nullglob
files=("$SCREENS_DIR"/*/screen.html)
if [ ${#files[@]} -eq 0 ]; then
    echo "No screen.html files found under $SCREENS_DIR" >&2
    exit 1
fi

total=0
while read -r from to; do
    [ -z "${from:-}" ] && continue
    n=$(grep -oiE "#$from" "${files[@]}" 2>/dev/null | wc -l || true)
    if [ "$n" -gt 0 ]; then
        printf '  #%s -> #%s  (%d)\n' "$from" "$to" "$n"
        sed -i "s/#$from/#$to/gI" "${files[@]}"
        total=$((total + n))
    fi
done <<< "$TOKEN_MAP"

echo "normalised $total token occurrences across ${#files[@]} screens"

remaining=$(grep -oiE '#1c110b|#160c07|#251913|#291d17|#342720|#40322b|#f5ded3|#dfc0b1|#a78b7d|#584237|#ffb68e|#99cbff' "${files[@]}" 2>/dev/null | wc -l || true)
if [ "$remaining" -ne 0 ]; then
    echo "FAILED: $remaining drifted tokens still present" >&2
    exit 1
fi
echo "verified: no drifted tokens remain"
