#!/usr/bin/env bash
# Increment the MINOR version in version.txt and stamp every derived location.
#
# version.txt holds a single line: MAJOR.MINOR (e.g. "0.1").
# This script increments MINOR unconditionally and updates:
#   - version.txt
#   - crates/forge-ffi/tauri.conf.json (version field)
#   - crates/forge-ffi/Cargo.toml (version field)
#   - crates/forge-core/Cargo.toml (version field)
#   - crates/forge-engines/Cargo.toml (version field)
#   - crates/forge-cli/Cargo.toml (version field)
#   - android/build.gradle.kts (versionName)
#
# The build number is derived from the git commit count, so it is monotonic
# within a branch and does not require manual maintenance.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION_FILE="$ROOT/version.txt"

if [ ! -f "$VERSION_FILE" ]; then
    echo "No version.txt at $VERSION_FILE" >&2
    exit 1
fi

# Read and parse the current version.
current=$(cat "$VERSION_FILE" | tr -d '[:space:]')
if ! [[ "$current" =~ ^([0-9]+)\.([0-9]+)$ ]]; then
    echo "Invalid version format: '$current' (expected MAJOR.MINOR)" >&2
    exit 1
fi

major="${BASH_REMATCH[1]}"
minor="${BASH_REMATCH[2]}"

# Increment MINOR unconditionally.
new_minor=$((minor + 1))
new_version="${major}.${new_minor}"

echo "version: ${current} -> ${new_version}"

# Stamp version.txt.
echo "$new_version" > "$VERSION_FILE"

# Stamp tauri.conf.json if it exists.
tauri_conf="$ROOT/crates/forge-ffi/tauri.conf.json"
if [ -f "$tauri_conf" ]; then
    sed -i "s/\"version\"[[:space:]]*:[[:space:]]*\"[^\"]*\"/\"version\": \"$new_version\"/" "$tauri_conf"
    echo "  stamped: $tauri_conf"
fi

# Stamp Cargo.toml files.
for cargo in \
    "$ROOT/crates/forge-core/Cargo.toml" \
    "$ROOT/crates/forge-engines/Cargo.toml" \
    "$ROOT/crates/forge-ffi/Cargo.toml" \
    "$ROOT/crates/forge-cli/Cargo.toml"; do
    if [ -f "$cargo" ]; then
        sed -i "s/^version[[:space:]]*=[[:space:]]*\"[^\"]*\"/version = \"$new_version\"/" "$cargo"
        echo "  stamped: $cargo"
    fi
done

# Stamp android/build.gradle.kts versionName.
gradle="$ROOT/android/build.gradle.kts"
if [ -f "$gradle" ]; then
    sed -i "s/versionName[[:space:]]*=[[:space:]]*\"[^\"]*\"/versionName = \"$new_version\"/" "$gradle"
    echo "  stamped: $gradle"
fi

# Derive build number from git commit count (monotonic within a branch).
build_number=$(git -C "$ROOT" rev-list --count HEAD 2>/dev/null || echo "0")
echo "build number: $build_number (from git commit count)"

echo "done: version stamped to $new_version (build $build_number)"
