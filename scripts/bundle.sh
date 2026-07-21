#!/bin/bash
# Create a distributable bundle of specs, bindings, and a built guardd library.
# Does not claim formal completeness.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "Creating Model Asset Guard bundle..."

BUNDLE_DIR="model-asset-guard-bundle"
rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR"

echo "Copying specification files..."
mkdir -p "$BUNDLE_DIR/Spec/Quant" "$BUNDLE_DIR/Spec/Token" "$BUNDLE_DIR/docs"
cp src/lean/ModelAssetGuard.lean "$BUNDLE_DIR/Spec/" 2>/dev/null || true
cp src/lean/ModelAssetGuard/Weights.lean "$BUNDLE_DIR/Spec/"
cp src/lean/ModelAssetGuard/RustSidecar.lean "$BUNDLE_DIR/Spec/"
cp src/lean/ModelAssetGuard/Quant/Core.lean "$BUNDLE_DIR/Spec/Quant/"
cp src/lean/ModelAssetGuard/Quant/LayerBound.lean "$BUNDLE_DIR/Spec/Quant/"
cp src/lean/ModelAssetGuard/Quant/Fixed.lean "$BUNDLE_DIR/Spec/Quant/"
cp src/lean/ModelAssetGuard/Token/Tokenizer.lean "$BUNDLE_DIR/Spec/Token/"
cp docs/axioms.md "$BUNDLE_DIR/docs/"
cp docs/perfect-hash-tokenizer.md "$BUNDLE_DIR/docs/"
cp docs/float-obligation.md "$BUNDLE_DIR/docs/"
cp docs/sentencepiece-bpe.md "$BUNDLE_DIR/docs/"
cp docs/native-packaging.md "$BUNDLE_DIR/docs/"

echo "Copying bindings..."
mkdir -p "$BUNDLE_DIR/bindings/python" "$BUNDLE_DIR/bindings/nodejs"
cp bindings/python/pytorch_guard.py "$BUNDLE_DIR/bindings/python/"
cp bindings/nodejs/node_guard.js "$BUNDLE_DIR/bindings/nodejs/"
cp bindings/nodejs/package.json "$BUNDLE_DIR/bindings/nodejs/"
cp bindings/nodejs/README.md "$BUNDLE_DIR/bindings/nodejs/"
cp pyproject.toml "$BUNDLE_DIR/"

if [ -f examples/huggingface_example.py ]; then
  mkdir -p "$BUNDLE_DIR/examples"
  cp examples/huggingface_example.py "$BUNDLE_DIR/examples/"
fi

echo "Copying Rust library (platform-specific)..."
mkdir -p "$BUNDLE_DIR/guardd"
LIB_COPIED=0
for candidate in \
  src/rust/guardd/target/release/libguardd.so \
  src/rust/guardd/target/release/guardd.dll \
  src/rust/guardd/target/release/libguardd.dylib \
  src/rust/guardd/target/debug/libguardd.so \
  src/rust/guardd/target/debug/guardd.dll \
  src/rust/guardd/target/debug/libguardd.dylib
do
  if [ -f "$candidate" ]; then
    cp "$candidate" "$BUNDLE_DIR/guardd/"
    LIB_COPIED=1
    break
  fi
done

if [ "$LIB_COPIED" -eq 0 ]; then
  echo "guardd library not found; building release..."
  cargo build --release --manifest-path src/rust/guardd/Cargo.toml --locked
  for candidate in \
    src/rust/guardd/target/release/libguardd.so \
    src/rust/guardd/target/release/guardd.dll \
    src/rust/guardd/target/release/libguardd.dylib
  do
    if [ -f "$candidate" ]; then
      cp "$candidate" "$BUNDLE_DIR/guardd/"
      LIB_COPIED=1
      break
    fi
  done
fi

if [ "$LIB_COPIED" -eq 0 ]; then
  echo "Error: could not locate or build guardd shared library"
  exit 1
fi

# Copy CLI helpers when present (best-effort)
mkdir -p "$BUNDLE_DIR/guardd/bin"
for bin in guardd_sha256 guardd_perfect_hash guardd_bitflip guardd_quant128 gen_perfect_hash; do
  for ext in "" .exe; do
    for profile in release debug; do
      candidate="src/rust/guardd/target/${profile}/${bin}${ext}"
      if [ -f "$candidate" ]; then
        cp "$candidate" "$BUNDLE_DIR/guardd/bin/"
      fi
    done
  done
done

echo "Generating Lean toolchain stamp..."
lake build
KERNEL_HASH=$( (lake --version 2>/dev/null || true; cat lean-toolchain) | sha256sum | cut -d' ' -f1 )
echo "$KERNEL_HASH" > "$BUNDLE_DIR/lean-hash.txt"
cp lean-toolchain "$BUNDLE_DIR/"
cp lakefile.lean "$BUNDLE_DIR/"
cp LICENSE "$BUNDLE_DIR/" 2>/dev/null || true

cat > "$BUNDLE_DIR/README.md" << 'EOF'
# Model Asset Guard Bundle

Runtime-checked components plus Lean specifications. Formal completeness is
**not** claimed; see `docs/axioms.md`.

## Contents

- `Spec/` — Lean specifications (0 axioms; Float markers; Int/Fixed + BPE lemmas)
- `guardd/` — Rust sidecar library (+ `bin/` helpers when built)
- `bindings/` — Python and Node.js bindings
- `docs/axioms.md` — axiom inventory (must stay at 0)
- `docs/float-obligation.md` — Float bridge path (markers, not proofs)
- `docs/sentencepiece-bpe.md` — SP/BPE support matrix (honest gaps)
- `docs/perfect-hash-tokenizer.md` — open-addressed hash vocab (not MPH)
- `docs/native-packaging.md` — local stage + CI artifact path (no fake npm binaries)
- `lean-hash.txt` — toolchain/content stamp (not a proof certificate)

## Verification

```bash
lake build && lake exe tests
python scripts/check_axioms.py
cargo test --manifest-path src/rust/guardd/Cargo.toml --locked
pytest tests/e2e -m "not integration"
```

## Integration notes

- Python: `pip install .` from the bundle root (module `pytorch_guard`); set
  `GUARDD_LIB` or place the library under `bindings/python/native/`
- Node: `cd bindings/nodejs && npm install` — requires staged/built `guardd`
  (see `docs/native-packaging.md`); npm does not ship prebuilts
- Hash vocab is a deterministic open-addressed table, not a proven MPH
- SentencePiece: see `docs/sentencepiece-bpe.md` (not full SP parity)
EOF

BUNDLE_NAME="model-asset-guard-$(date +%Y%m%d).zip"
echo "Creating bundle archive: $BUNDLE_NAME"
zip -r "$BUNDLE_NAME" "$BUNDLE_DIR"

BUNDLE_SHA256=$(sha256sum "$BUNDLE_NAME" | cut -d' ' -f1)
echo "Bundle SHA-256: $BUNDLE_SHA256"

cat > "release-info.txt" << EOF
Model Asset Guard Bundle
Date: $(date)
Bundle: $BUNDLE_NAME
SHA-256: $BUNDLE_SHA256
Kernel Hash: $KERNEL_HASH
Formal completeness: NOT claimed (see docs/axioms.md)
EOF

echo "Bundle created successfully!"
echo "Bundle: $BUNDLE_NAME"
echo "SHA-256: $BUNDLE_SHA256"
echo "Release info saved to: release-info.txt"

rm -rf "$BUNDLE_DIR"
