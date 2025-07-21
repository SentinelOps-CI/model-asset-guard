# Model Asset Guard

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Lean 4](https://img.shields.io/badge/Lean-4.0.0--nightly--2024--01--15-blue.svg)](https://leanprover.github.io/)
[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![Python](https://img.shields.io/badge/Python-3.8+-green.svg)](https://www.python.org/)
[![CI/CD](https://github.com/fraware/model-asset-guard/workflows/Model%20Asset%20Guard%20CI/badge.svg)](https://github.com/fraware/model-asset-guard/actions)

> **Machine-verified integrity for every fixed model artifact** — weights, vocabularies, quantization tables, and tokenizers.

Model Asset Guard provides formal verification and runtime validation for machine learning model artifacts, ensuring their integrity, correctness, and reliability throughout the model lifecycle.

## Table of Contents

- [Overview](#overview)
- [Features](#features)
- [Architecture](#architecture)
- [Quick Start](#quick-start)
- [Installation](#installation)
- [Usage](#usage)
- [API Reference](#api-reference)
- [Development](#development)
- [Contributing](#contributing)
- [License](#license)

## Overview

Model Asset Guard addresses critical challenges in machine learning model deployment by providing:

- **Formal Verification**: Lean 4 specifications for mathematical correctness
- **Runtime Validation**: High-performance Rust sidecar for integrity checks
- **Cross-Platform Support**: Python and Node.js bindings for seamless integration
- **Production Ready**: Zero unsafe code, comprehensive testing, and CI/CD integration

### Problem Statement

Modern ML pipelines face significant challenges with model artifact integrity:

- **Bit Corruption**: Silent failures from storage or transmission errors
- **Quantization Errors**: Unbounded error accumulation in quantized models
- **Tokenizer Inconsistencies**: Non-deterministic encoding/decoding behavior
- **Version Drift**: Undetected changes in model artifacts

### Solution

Model Asset Guard provides a comprehensive solution through:

1. **SHA-256 Integrity Validation** with formal proofs
2. **Quantization Error Bounds** with mathematical guarantees
3. **Tokenizer Determinism Verification** with fuzz testing
4. **High-Performance Runtime Validation** via Rust sidecar
5. **Seamless Integration** with existing ML frameworks

## Features

### Core Capabilities

| Feature                    | Description                             | Technology     |
| -------------------------- | --------------------------------------- | -------------- |
| **Weight Integrity**       | SHA-256 validation with formal proofs   | Lean 4 + Rust  |
| **Quantization Bounds**    | Mathematical error bounds for int8/FP16 | Lean 4         |
| **Tokenizer Verification** | Determinism and surjectivity proofs     | Lean 4         |
| **Runtime Validation**     | High-performance integrity checks       | Rust           |
| **Framework Integration**  | HuggingFace, PyTorch, Node.js support   | Python/Node.js |

### Performance Characteristics

- **Startup Overhead**: < 10ms for integrity checks
- **Memory Footprint**: < 1MB binary size
- **Zero Heap Allocations**: Deterministic memory usage
- **100% Bit-Flip Detection**: Comprehensive corruption detection

## Architecture

### System Components

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Lean 4 Specs  │    │  Rust Sidecar   │    │ Language Bindings│
│                 │    │                 │    │                 │
│ • Formal Proofs │◄──►│ • High Perf     │◄──►│ • Python        │
│ • Math Bounds   │    │ • Zero Unsafe   │    │ • Node.js        │
│ • Verification  │    │ • FFI Interface │    │ • HuggingFace   │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

### Directory Structure

```
model-asset-guard/
├── src/                              # Source code
│   ├── lean/                         # Lean 4 specifications
│   │   ├── ModelAssetGuard/          # Core library
│   │   └── cli/                      # Command-line tools
│   ├── rust/                         # Rust sidecar
│   │   └── guardd/                   # High-performance library
│   └── python/                       # Python source code
├── bindings/                         # Language bindings
│   ├── python/                       # Python bindings
│   └── nodejs/                       # Node.js bindings
├── tests/                            # Test suite
│   ├── unit/                         # Unit tests
│   ├── integration/                  # Integration tests
│   ├── e2e/                          # End-to-end tests
│   └── performance/                  # Performance tests
├── scripts/                          # Build and utility scripts
```

## Quick Start

### Prerequisites

- **Lean 4**: `nightly-2024-01-15` or later
- **Rust**: `1.70+` with Cargo
- **Python**: `3.8+` (for bindings and testing)
- **Node.js**: `16.0+` (for Node.js bindings)

### Installation

```bash
# Clone the repository
git clone https://github.com/SentinelOps-Remote-CI/model-asset-guard.git
cd model-asset-guard

# Build Lean specifications
lake build

# Build Rust sidecar
cargo build --release --manifest-path src/rust/guardd/Cargo.toml

# Install Python bindings (optional)
pip install -e bindings/python/

# Run verification tests
lake test
cargo test --manifest-path src/rust/guardd/Cargo.toml
```

### Basic Usage

#### Verify Model Weights

```bash
# Verify a checkpoint file
lake exe verifyweights /path/to/checkpoint.bin

# Verify with expected digest
lake exe verifyweights /path/to/checkpoint.bin --digest abc123...
```

#### Generate Quantization Bounds

```bash
# Generate bounds for ONNX model
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

## Usage

### Python Integration

```python
from bindings.python.pytorch_guard import ModelAssetGuard, checked_load_pretrained

# Initialize guard
guard = ModelAssetGuard()

# Verify file integrity
verified = guard.verify_digest("model.bin", expected_digest)

# Load model with verification
model_info = guard.checked_load("model.bin", expected_digest)

# HuggingFace integration
from bindings.python.pytorch_guard import HuggingFaceGuard

hf_guard = HuggingFaceGuard(guard)
model, tokenizer, info = hf_guard.checked_load_pretrained(
    "gpt2",
    verify_quantization=True
)
```

### Node.js Integration

```javascript
const { ModelAssetGuard } = require("./bindings/nodejs/node_guard.js");

// Initialize guard
const guard = new ModelAssetGuard();

// Verify model integrity
const verified = guard.verifyDigest("model.bin", expectedDigest);

// Load model with verification
const modelInfo = guard.checkedLoad("model.bin", expectedDigest);
```

### Rust Integration

```rust
use guardd::{checked_load, verify_digest};

// Verify file integrity
let verified = verify_digest("model.bin", &expected_digest)?;

// Load model with verification
let model_info = checked_load("model.bin", Some(&expected_digest))?;
```

## API Reference

### Core Functions

#### `verify_digest(path: &str, expected_digest: &[u8]) -> Result<bool, GuarddError>`

Verifies SHA-256 digest of a file.

**Parameters:**

- `path`: Path to the file to verify
- `expected_digest`: Expected SHA-256 digest (32 bytes)

**Returns:** `true` if digest matches, `false` otherwise

#### `checked_load(path: &str, expected_digest: Option<&[u8]>) -> Result<ModelInfo, GuarddError>`

Loads and validates a model file.

**Parameters:**

- `path`: Path to the model file
- `expected_digest`: Optional expected digest for validation

**Returns:** Model information including path, size, and digest

#### `verify_quantization_128_vectors(layer_name: &str, weights: &[f32], fan_in: u32, fan_out: u32, quant_type: &str) -> Result<QuantizationResult, GuarddError>`

Verifies quantization bounds for a layer.

**Parameters:**

- `layer_name`: Name of the layer
- `weights`: Weight matrix as flat array
- `fan_in`: Input dimension
- `fan_out`: Output dimension
- `quant_type`: Quantization type ("int8", "fp16")

**Returns:** Quantization verification results

### Error Handling

```rust
#[derive(Debug)]
pub enum GuarddError {
    FileNotFound(String),
    InvalidDigest(String),
    QuantizationError(String),
    IoError(std::io::Error),
}
```

## Development

### Building from Source

```bash
# Clone with submodules
git clone --recursive https://github.com/fraware/model-asset-guard.git
cd model-asset-guard

# Build all components
make build

# Run all tests
make test

# Run benchmarks
make benchmark
```

### Development Environment

````bash
# Setup development environment
make setup-dev

# Run linting
make lint

# Run security audit
make security-audit

### Testing

```bash
# Run unit tests
lake test
cargo test --manifest-path src/rust/guardd/Cargo.toml

# Run integration tests
python tests/e2e/test_huggingface_integration.py

# Run performance tests
lake exe benchmarks
````

### Continuous Integration

The project includes comprehensive CI/CD pipelines:

- **Multi-platform Testing**: Ubuntu, macOS, Windows
- **Security Scanning**: Cargo audit, unsafe code detection
- **Performance Benchmarks**: Automated performance regression testing
- **Documentation Generation**: Automated API documentation

## Contributing

We welcome contributions from the community! Please see our [Contributing Guidelines](CONTRIBUTING.md) for details.

### Development Workflow

1. **Fork** the repository
2. **Create** a feature branch (`git checkout -b feature/amazing-feature`)
3. **Commit** your changes (`git commit -m 'Add amazing feature'`)
4. **Push** to the branch (`git push origin feature/amazing-feature`)
5. **Open** a Pull Request

### Code Standards

- **Lean 4**: Follow Lean 4 style guidelines
- **Rust**: Follow Rust coding standards, zero unsafe code
- **Python**: Follow PEP 8, type hints required
- **Tests**: Maintain >90% test coverage

### Testing Requirements

- All new features must include unit tests
- Integration tests for cross-language functionality
- Performance benchmarks for critical paths
- Security tests for all external interfaces

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- **Lean Community**: For the formal verification framework
- **Rust Community**: For the high-performance systems programming language
- **HuggingFace**: For the transformers library integration
- **Academic Contributors**: For mathematical foundations and proofs

## Citation

If you use Model Asset Guard in your research, please cite:

```bibtex
@software{model_asset_guard,
  title={Model Asset Guard: Machine-verified integrity for ML artifacts},
  author={Model Asset Guard Contributors},
  year={2025},
  url={https://github.com/fraware/model-asset-guard}
}
```

## Support

- **Issues**: [GitHub Issues](https://github.com/fraware/model-asset-guard/issues)
- **Discussions**: [GitHub Discussions](https://github.com/fraware/model-asset-guard/discussions)
- **Security**: [Security Policy](SECURITY.md)

---

**Model Asset Guard** - Ensuring the integrity of machine learning artifacts through formal verification and runtime validation.
