# Model Asset Guard

Machine-checks every fixed model artefact—weights, vocab, quant tables, tokenizers.

## North-Star Outcomes

| Tag   | Outcome                                                                                       | Success Metric                                                                          |
| ----- | --------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| MAG-1 | Weight-Integrity Guard — Lean proof + Rust sidecar validating SHA-256 & file size before load | 100% of checkpoints rejected on bit-flip corpus; pass-through adds < 10ms to startup    |
| MAG-2 | Quantisation-Error Proof Kit for per-layer int8 / FP16 static quantisers                      | Symbolic ε bound auto-generated; verified error ≤ spec across 99% of layers in LLama-7B |
| MAG-3 | Tokenizer Determinism Proof — encode ∘ decode = id ∧ surjective_utf8                          | Proof compiles in < 2s; fuzz-tester reaches 1M random strings with zero failures        |
| MAG-4 | Guardd Sidecar — libguardd.so (Rust, no unsafe) exposing checked_load() & verify_quant()      | Consumed by HuggingFace transformers via pre_load_hook; < 1MB binary, zero heap allocs  |
| MAG-5 | Artefact Bundle Generator — bundle.sh emits spec, extracted Rust/C, lean-hash.txt             | Bundle accepted by SentinelOps CI with zero schema warnings                             |

## Quick Start

### Prerequisites

- Lean 4 (nightly-2024-01-15)
- Rust 1.70+
- Python 3.8+ (for test harness)
- Cargo

### Installation

```bash
# Clone the repository
git clone https://github.com/fraware/model-asset-guard.git

# Build the project
lake build

# Build Rust sidecar
cargo build --release --manifest-path guardd/Cargo.toml

# Run tests
lake test
```

### Usage

#### Verify Model Weights

```bash
# Verify a checkpoint
lake exe verifyweights /path/to/checkpoint.bin

# Verify with custom digest
lake exe verifyweights /path/to/checkpoint.bin --digest abc123...
```

#### Generate Quantization Bounds

```bash
# Generate bounds for a model
lake exe quantbound /path/to/model.onnx --output bounds.json

# Verify quantization error
lake exe quantbound /path/to/model.onnx --verify --tolerance 1e-6
```

#### Test Tokenizer Determinism

```bash
# Test BPE tokenizer
lake exe tokenizertest /path/to/tokenizer.json --type bpe

# Fuzz test with 1M random strings
lake exe tokenizertest /path/to/tokenizer.json --fuzz 1000000
```

#### Use Guardd Sidecar

```python
import torch
from guardd import checked_load

# Load model with integrity checks
model = checked_load("/path/to/checkpoint.bin")
```

## Architecture

### Core Components

1. **Weight Integrity Module** (`Weights.lean`) - SHA-256 validation with formal proofs
2. **Quantization Error Bounds** (`Quant.Core.lean`) - Mathematical bounds for quantization errors
3. **Tokenizer Determinism** (`Token/`) - Formal verification of tokenizer properties
4. **Guardd Sidecar** (`guardd/`) - Rust library for high-performance validation
5. **Benchmarks** (`bench/`) - Performance testing harness

### File Structure

```
model-asset-guard/
├── lakefile.lean          # Build configuration
├── ModelAssetGuard/       # Core Lean library
│   ├── Weights.lean       # Weight integrity proofs
│   ├── Quant/             # Quantization error bounds
│   └── Token/             # Tokenizer determinism
├── guardd/                # Rust sidecar library
│   ├── src/lib.rs         # FFI interface
│   └── Cargo.toml         # Rust dependencies
├── Tests/                 # Test suite
├── Benchmarks/            # Performance benchmarks
├── bench/                 # Python test harness
└── docs/                  # Documentation
```

## Development

### Running Tests

```bash
# Run all tests
lake test

# Run specific test suite
lake exe tests

# Run benchmarks
lake exe benchmarks
```

### Building Documentation

```bash
# Build docs
lake build ModelAssetGuard:docs

# Serve docs locally
python -m http.server 8000 --directory build/doc
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests for new functionality
5. Ensure all tests pass
6. Submit a pull request

## License

MIT License - see LICENSE file for details.
