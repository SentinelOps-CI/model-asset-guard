#!/bin/bash

set -e

echo "Creating Model Asset Guard bundle..."

# Create bundle directory
BUNDLE_DIR="model-asset-guard-bundle"
rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR"

# Copy specification files
echo "Copying specification files..."
mkdir -p "$BUNDLE_DIR/Spec"
cp ModelAssetGuard/Weights.lean "$BUNDLE_DIR/Spec/"
cp ModelAssetGuard/Quant/Core.lean "$BUNDLE_DIR/Spec/"
cp ModelAssetGuard/Quant/LayerBound.lean "$BUNDLE_DIR/Spec/"
cp ModelAssetGuard/Token/Tokenizer.lean "$BUNDLE_DIR/Spec/"

# Copy HuggingFace integration files (G-3 requirement)
echo "Copying HuggingFace integration files..."
mkdir -p "$BUNDLE_DIR/bindings"
cp pytorch_guard.py "$BUNDLE_DIR/bindings/"
cp node_guard.js "$BUNDLE_DIR/bindings/"
cp test_huggingface_integration.py "$BUNDLE_DIR/bindings/"

# Copy perfect hash tokenizer files (T-3 requirement)
echo "Copying perfect hash tokenizer files..."
cp test_perfect_hash_integration.py "$BUNDLE_DIR/bindings/"
mkdir -p "$BUNDLE_DIR/docs"
cp docs/perfect-hash-tokenizer.md "$BUNDLE_DIR/docs/"

# Copy examples
echo "Copying examples..."
mkdir -p "$BUNDLE_DIR/examples"
cp examples/huggingface_example.py "$BUNDLE_DIR/examples/"

# Copy extracted Rust library
echo "Copying Rust library..."
mkdir -p "$BUNDLE_DIR/guardd"
if [ -f "guardd/target/release/libguardd.so" ]; then
    cp guardd/target/release/libguardd.so "$BUNDLE_DIR/guardd/"
else
    echo "Warning: libguardd.so not found, building..."
    cargo build --release --manifest-path guardd/Cargo.toml
    cp guardd/target/release/libguardd.so "$BUNDLE_DIR/guardd/"
fi

# Generate Lean kernel hash
echo "Generating Lean kernel hash..."
lake build
KERNEL_HASH=$(lake exe lean --version 2>/dev/null | head -1 | sha256sum | cut -d' ' -f1)
echo "$KERNEL_HASH" > "$BUNDLE_DIR/lean-hash.txt"

# Create README for bundle
cat > "$BUNDLE_DIR/README.md" << 'EOF'
# Model Asset Guard Bundle

This bundle contains the verified components of the Model Asset Guard system.

## Contents

- `Spec/` - Lean specification files with formal proofs
- `guardd/` - Rust sidecar library for high-performance validation
- `bindings/` - Python and Node.js bindings for HuggingFace integration (G-3)
- `docs/` - Documentation including perfect hash tokenizer (T-3)
- `lean-hash.txt` - Lean kernel hash for verification

## Verification

To verify this bundle:

1. Check the Lean kernel hash:
   ```bash
   lake exe lean --version | head -1 | sha256sum
   ```
   Compare with the contents of `lean-hash.txt`

2. Build and test the components:
   ```bash
   lake build
   lake test
   cargo test --manifest-path guardd/Cargo.toml
   ```

3. Run the benchmark suite:
   ```bash
   ./bench/run.sh
   ```

4. Test HuggingFace integration:
   ```bash
   pip install transformers torch numpy
   python3 bindings/test_huggingface_integration.py
   ```

5. Test perfect hash tokenizer:
   ```bash
   python3 bindings/test_perfect_hash_integration.py
   lake exe perfecthash test vocab.json
   ```

## Integration

### Python/HuggingFace Integration

The `pytorch_guard.py` module provides seamless integration with HuggingFace transformers:

```python
from bindings.pytorch_guard import checked_load_pretrained

# Load a model with full verification
model, tokenizer, verification_results = checked_load_pretrained(
    "gpt2",
    verify_quantization=True
)
```

### Node.js Integration

The `node_guard.js` module provides JavaScript/TypeScript bindings:

```javascript
const { createGuard } = require('./bindings/node_guard.js');

const guard = createGuard();
const verified = guard.verifyDigest('model.bin', expectedDigest);

// Perfect hash tokenizer
const tokens = guard.perfectHashEncode(vocabJson, "hello world");
const decoded = guard.perfectHashDecodeSequence(vocabJson, tokens);
```

### Direct FFI Integration

The `libguardd.so` library can be integrated directly:

```python
import ctypes
lib = ctypes.CDLL("./guardd/libguardd.so")
# Use the FFI functions for model validation
```

## License

MIT License - see the main repository for details.
EOF

# Create bundle archive
BUNDLE_NAME="model-asset-guard-$(date +%Y%m%d).zip"
echo "Creating bundle archive: $BUNDLE_NAME"
zip -r "$BUNDLE_NAME" "$BUNDLE_DIR"

# Generate SHA-256 of bundle
BUNDLE_SHA256=$(sha256sum "$BUNDLE_NAME" | cut -d' ' -f1)
echo "Bundle SHA-256: $BUNDLE_SHA256"

# Create release info
cat > "release-info.txt" << EOF
Model Asset Guard Bundle
Date: $(date)
Bundle: $BUNDLE_NAME
SHA-256: $BUNDLE_SHA256
Kernel Hash: $KERNEL_HASH
EOF

echo "Bundle created successfully!"
echo "Bundle: $BUNDLE_NAME"
echo "SHA-256: $BUNDLE_SHA256"
echo "Release info saved to: release-info.txt"

# Cleanup
rm -rf "$BUNDLE_DIR" 