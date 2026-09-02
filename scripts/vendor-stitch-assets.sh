#!/usr/bin/env bash
# Vendor the frozen Stitch screen exports into the application's UI tree.
#
# Why this exists
# ---------------
# The frozen screens under LIBS/UI/STITCH/screens/ are the visual contract for
# every UI state. As exported, each screen fetches its stylesheet from
# cdn.tailwindcss.com and its fonts from fonts.googleapis.com at render time.
# Shipping that would put an outbound third-party request on app open — outside
# the three network uses the privacy copy authorises (§5) — and would leave an
# offline install unstyled. AD-3 requires the fonts self-hosted; this script
# performs the vendoring.
#
# It copies each LIBS/UI/STITCH/screens/<name>/screen.html to
# ui/src/screens/<name>.html, replacing:
#   1. The cdn.tailwindcss.com <script> with a local Tailwind CSS bundle.
#   2. The fonts.googleapis.com <link> tags with local @font-face declarations.
#   3. The fonts.gstatic.com preload <link> tags with local preload links.
#   4. The lh3.googleusercontent.com remote images with local placeholder SVGs.
#
# It never writes to LIBS/, which stays byte-untouched. It is idempotent:
# running it twice produces the same output. It is self-verifying: after
# transformation it greps for any remaining https:// or http:// reference and
# fails if any are found.
#
# Run after freezing any newly generated screens and after running
# normalize-stitch-tokens.sh.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC_DIR="$ROOT/LIBS/UI/STITCH/screens"
DST_DIR="$ROOT/ui/src/screens"
FONTS_DIR="$ROOT/ui/src/assets/fonts"
VENDOR_DIR="$ROOT/vendored-in-code/registry.npmjs.org"

if [ ! -d "$SRC_DIR" ]; then
    echo "No frozen screens at $SRC_DIR" >&2
    exit 1
fi

mkdir -p "$DST_DIR" "$FONTS_DIR"

# Copy font files into the UI assets tree if not already present.
for family in inter chivo jetbrains-mono material-symbols; do
    src="$VENDOR_DIR/$family"
    if [ -d "$src" ]; then
        for woff2 in "$src"/*.woff2; do
            [ -f "$woff2" ] || continue
            base=$(basename "$woff2")
            dst="$FONTS_DIR/$base"
            if [ ! -f "$dst" ] || ! cmp -s "$woff2" "$dst"; then
                cp "$woff2" "$dst"
            fi
        done
    fi
done

# Also copy the pre-subsetted Material Symbols subset if it exists.
subset="$ROOT/ui/src/assets/fonts/material-symbols-outlined-subset.woff2"
if [ -f "$subset" ]; then
    : # already in place
fi

shopt -s nullglob
src_files=("$SRC_DIR"/*/screen.html)
if [ ${#src_files[@]} -eq 0 ]; then
    echo "No screen.html files found under $SRC_DIR" >&2
    exit 1
fi

count=0
for src in "${src_files[@]}"; do
    name=$(basename "$(dirname "$src")")
    dst="$DST_DIR/$name.html"

    # Read the source file.
    content=$(cat "$src")

    # 1. Replace the Tailwind CDN script with a local reference.
    #    The frozen screens use: <script src="https://cdn.tailwindcss.com?...">
    #    We replace it with a local script tag that loads from /src/assets/tailwind.js
    #    In production, the build step inlines Tailwind via PostCSS.
    content=$(printf '%s' "$content" | sed 's|<script src="https://cdn.tailwindcss.com[^"]*"></script>|<script src="/src/assets/tailwind.js"></script>|g')

    # 2. Replace fonts.googleapis.com <link> tags with a local stylesheet link.
    #    Multiple variant URLs exist; replace any that match the pattern.
    content=$(printf '%s' "$content" | sed 's|<link href="https://fonts.googleapis.com[^"]*" rel="stylesheet"/>|<link rel="stylesheet" href="/src/assets/fonts.css"/>|g')
    # Also handle links with additional attributes in different order
    content=$(printf '%s' "$content" | sed 's|<link [^>]*href="https://fonts.googleapis.com[^"]*"[^>]*/>|<link rel="stylesheet" href="/src/assets/fonts.css"/>|g')

    # 2b. Replace @import url('https://fonts.googleapis.com...') in <style> blocks.
    content=$(printf '%s' "$content" | sed "s|@import url('https://fonts.googleapis.com[^']*');|@import url('/src/assets/fonts.css');|g")

    # 3. Replace fonts.gstatic.com preload links with local preload links.
    content=$(printf '%s' "$content" | sed 's|<link [^>]*href="https://fonts.gstatic.com[^"]*"[^>]*/>|<link rel="preload" as="font" type="font/woff2" crossorigin href="/src/assets/fonts/inter-latin-400-normal.woff2"/>|g')

    # 4. Replace lh3.googleusercontent.com remote images with a local placeholder.
    #    These are decorative images in b1-home and e4-result-viewer.
    content=$(printf '%s' "$content" | sed "s|https://lh3.googleusercontent.com/aida-public/[^'\"]*|/src/assets/placeholder.svg|g")

    printf '%s' "$content" > "$dst"
    count=$((count + 1))
done

echo "vendored $count screens to $DST_DIR"

# Self-verify: no http:// or https:// references remain in the output.
remaining=$(grep -rho 'https\?://' "$DST_DIR" 2>/dev/null | wc -l || true)
if [ "$remaining" -ne 0 ]; then
    echo "FAILED: $remaining remote URL references remain in $DST_DIR" >&2
    grep -rn 'https\?://' "$DST_DIR" >&2 || true
    exit 1
fi

# Verify no remote references in assets either (excluding XML namespace URIs).
remaining_assets=$(grep -rPo 'https?://(?!www\.w3\.org)' "$ROOT/ui/src/assets" 2>/dev/null | wc -l || true)
if [ "$remaining_assets" -ne 0 ]; then
    echo "FAILED: $remaining_assets remote URL references in ui/src/assets" >&2
    grep -rn 'https\?://' "$ROOT/ui/src/assets" >&2 || true
    exit 1
fi

echo "verified: no remote URL references in screens or assets"
