#!/usr/bin/env bash
# Stage built guardd shared libraries into binding-friendly native/ dirs.
# Does not download or publish binaries — copies from a local cargo build only.
#
# Usage:
#   cargo build --release --manifest-path src/rust/guardd/Cargo.toml
#   bash scripts/stage_guardd_native.sh
#   # optional: PROFILE=debug bash scripts/stage_guardd_native.sh

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

PROFILE="${PROFILE:-release}"
TARGET_DIR="src/rust/guardd/target/${PROFILE}"

CANDIDATES=(
  "${TARGET_DIR}/libguardd.so"
  "${TARGET_DIR}/libguardd.dylib"
  "${TARGET_DIR}/guardd.dll"
)

FOUND=()
for c in "${CANDIDATES[@]}"; do
  if [[ -f "$c" ]]; then
    FOUND+=("$c")
  fi
done

if [[ ${#FOUND[@]} -eq 0 ]]; then
  echo "Error: no guardd shared library under ${TARGET_DIR}"
  echo "Build first:"
  echo "  cargo build --${PROFILE} --manifest-path src/rust/guardd/Cargo.toml"
  exit 1
fi

DESTS=(
  "dist/native"
  "bindings/nodejs/native"
  "bindings/python/native"
)

for dest in "${DESTS[@]}"; do
  mkdir -p "$dest"
  for lib in "${FOUND[@]}"; do
    cp -f "$lib" "$dest/"
    echo "Staged $(basename "$lib") → ${dest}/"
  done
  # Sidecar metadata (honest: local build, not a published npm binary).
  {
    echo "source_profile=${PROFILE}"
    echo "staged_at=$(date -u +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date)"
    echo "note=Local cargo build copy. Not an npm/PyPI prebuilt publish."
  } > "${dest}/STAGED.txt"
done

echo "Done. Point GUARDD_LIB at dist/native/* or rely on binding search order."
echo "See docs/native-packaging.md"
