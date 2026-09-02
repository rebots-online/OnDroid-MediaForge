#!/usr/bin/env bash
# Build the release artifacts for OnDroid MediaForge and stage them into dist/.
#
# Produces:
#   - Signed AAB (Android App Bundle) for Play Store upload
#   - Signed APK for direct installation
#   - Debug symbols (unstripped .so files) for crash symbolication
#
# All artifacts are staged into the tracked dist/ directory with slug-first
# names carrying the full stamped version:
#   dist/ondroid-mediaforge-<version>-build<build>-<type>.<ext>
#
# The script never deletes dist/. If a clean is needed it renames aside to
# dist.bak.<timestamp>/.
#
# Signing uses the organisation keystore at
# ~/Admin-Manual/CREDENTIALS/PlayStore/production.keystore
#
# Prerequisites:
#   - Rust toolchain with Android targets (aarch64-linux-android)
#   - Android SDK + NDK
#   - Node.js / pnpm for web asset build
#   - Tauri 2 CLI

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST="$ROOT/dist"
VERSION=$(cat "$ROOT/version.txt" | tr -d '[:space:]')
BUILD=$(git -C "$ROOT" rev-list --count HEAD 2>/dev/null || echo "0")
SLUG="ondroid-mediaforge"

# Keystore configuration.
KEYSTORE="${KEYSTORE:-$HOME/Admin-Manual/CREDENTIALS/PlayStore/production.keystore}"
KEY_ALIAS="${KEY_ALIAS:-production}"
KEYSTORE_PASS="${KEYSTORE_PASS:-changeit}"

echo "=== OnDroid MediaForge release build ==="
echo "version: $VERSION  build: $BUILD"
echo "dist:    $DIST"
echo ""

# --- 1. Ensure dist/ exists (never delete it) ---
if [ -d "$DIST" ]; then
    # If clean is requested via CLEAN=1, rename aside.
    if [ "${CLEAN:-0}" = "1" ]; then
        ts=$(date +%Y%m%d%H%M%S)
        bak="$ROOT/dist.bak.$ts"
        echo "renaming existing dist/ aside to $bak"
        mv "$DIST" "$bak"
    fi
fi
mkdir -p "$DIST"

# --- 2. Build web assets ---
echo "--- building web assets ---"
cd "$ROOT/ui"
if command -v pnpm &>/dev/null; then
    pnpm install --frozen-lockfile
    pnpm build
elif command -v npm &>/dev/null; then
    npm ci
    npm run build
else
    echo "ERROR: no pnpm or npm found" >&2
    exit 1
fi
echo "web assets built: $(ls ui/dist/ 2>/dev/null | wc -l) files"

# --- 3. Build Rust core for Android (arm64-v8a) ---
echo "--- building Rust core for arm64-v8a ---"
cd "$ROOT"
cargo build --release -p forge-ffi --target aarch64-linux-android 2>&1 || {
    echo "WARNING: Rust Android build failed — ensure aarch64-linux-android target is installed" >&2
    echo "  rustup target add aarch64-linux-android" >&2
    # Continue — the script stages what it can.
}

# --- 4. Build AAB via Tauri ---
echo "--- building AAB ---"
cd "$ROOT/crates/forge-ffi"
if command -v tauri &>/dev/null; then
    tauri android build --release --target aarch64 2>&1 || {
        echo "WARNING: tauri android build failed — ensure Android SDK + NDK are configured" >&2
    }
elif command -v npx &>/dev/null; then
    npx tauri android build --release --target aarch64 2>&1 || {
        echo "WARNING: tauri android build failed — ensure Android SDK + NDK are configured" >&2
    }
else
    echo "WARNING: tauri CLI not found — skipping AAB build" >&2
fi

# --- 5. Locate and sign artifacts ---
echo "--- staging artifacts ---"
AAB_SRC=""
APK_SRC=""

# Search for built AAB/APK in standard Tauri output locations.
for candidate in \
    "$ROOT/crates/forge-ffi/gen/android/app/build/outputs/bundle/release/app-release.aab" \
    "$ROOT/crates/forge-ffi/gen/android/app/build/outputs/apk/release/app-release.apk" \
    "$ROOT/crates/forge-ffi/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk"; do
    if [ -f "$candidate" ]; then
        case "$candidate" in
            *.aab) AAB_SRC="$candidate" ;;
            *.apk) APK_SRC="$candidate" ;;
        esac
    fi
done

# Sign AAB if found and keystore exists.
if [ -n "$AAB_SRC" ] && [ -f "$KEYSTORE" ]; then
    AAB_DST="$DIST/${SLUG}-${VERSION}-build${BUILD}-release.aab"
    cp "$AAB_SRC" "$AAB_DST"
    # jarsigner for v1 signing; Play Store uses v2/v3 via bundletool.
    if command -v jarsigner &>/dev/null; then
        jarsigner -keystore "$KEYSTORE" -storepass "$KEYSTORE_PASS" \
            "$AAB_DST" "$KEY_ALIAS"
        echo "signed: $AAB_DST"
    else
        echo "WARNING: jarsigner not found — $AAB_DST is unsigned" >&2
    fi
elif [ -n "$AAB_SRC" ]; then
    AAB_DST="$DIST/${SLUG}-${VERSION}-build${BUILD}-release-unsigned.aab"
    cp "$AAB_SRC" "$AAB_DST"
    echo "staged (unsigned, no keystore): $AAB_DST"
else
    echo "WARNING: no AAB found — skipping"
fi

# Sign APK if found and keystore exists.
if [ -n "$APK_SRC" ] && [ -f "$KEYSTORE" ]; then
    APK_DST="$DIST/${SLUG}-${VERSION}-build${BUILD}-release.apk"
    cp "$APK_SRC" "$APK_DST"
    if command -v jarsigner &>/dev/null; then
        jarsigner -keystore "$KEYSTORE" -storepass "$KEYSTORE_PASS" \
            "$APK_DST" "$KEY_ALIAS"
        # Zip-align after signing.
        if command -v zipalign &>/dev/null; then
            zipalign -f 4 "$APK_DST" "${APK_DST}.aligned"
            mv "${APK_DST}.aligned" "$APK_DST"
        fi
        echo "signed: $APK_DST"
    else
        echo "WARNING: jarsigner not found — $APK_DST is unsigned" >&2
    fi
elif [ -n "$APK_SRC" ]; then
    APK_DST="$DIST/${SLUG}-${VERSION}-build${BUILD}-release-unsigned.apk"
    cp "$APK_SRC" "$APK_DST"
    echo "staged (unsigned, no keystore): $APK_DST"
else
    echo "WARNING: no APK found — skipping"
fi

# --- 6. Stage debug symbols ---
echo "--- staging debug symbols ---"
SYMBOLS_DIR="$DIST/${SLUG}-${VERSION}-build${BUILD}-symbols"
mkdir -p "$SYMBOLS_DIR"
so_found=0
for so in \
    "$ROOT/target/aarch64-linux-android/release/libforge_ffi.so" \
    "$ROOT/target/aarch64-linux-android/release/libforge_core.so"; do
    if [ -f "$so" ]; then
        cp "$so" "$SYMBOLS_DIR/"
        so_found=1
    fi
done
if [ "$so_found" = "1" ]; then
    echo "debug symbols staged: $SYMBOLS_DIR"
else
    echo "WARNING: no .so files found — debug symbols not staged"
    rmdir "$SYMBOLS_DIR" 2>/dev/null || true
fi

# --- 7. Write build manifest ---
MANIFEST="$DIST/${SLUG}-${VERSION}-build${BUILD}-manifest.txt"
cat > "$MANIFEST" <<EOF
OnDroid MediaForge Release Build Manifest
=========================================
Version: $VERSION
Build:   $BUILD
Date:    $(date -u +%Y-%m-%dT%H:%M:%SZ)
Commit:  $(git -C "$ROOT" rev-parse HEAD 2>/dev/null || echo "unknown")
Branch:  $(git -C "$ROOT" rev-parse --abbrev-ref HEAD 2>/dev/null || echo "unknown")

Artifacts:
$(ls -1 "$DIST" | grep "^${SLUG}-${VERSION}-build${BUILD}" | sed 's/^/  /')
EOF
echo "manifest: $MANIFEST"

echo ""
echo "=== release build complete ==="
echo "artifacts in: $DIST"
ls -la "$DIST" | grep "^-" | awk '{print "  " $NF}'
