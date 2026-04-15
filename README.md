# Model Asset Guard

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Lean 4](https://img.shields.io/badge/Lean-4%20stable-blue.svg)](https://leanprover.github.io/)
[![Rust](https://img.shields.io/badge/Rust-stable-orange.svg)](https://www.rust-lang.org/)
[![Python](https://img.shields.io/badge/Python-3.11+-green.svg)](https://www.python.org/)
[![CI/CD](https://github.com/fraware/model-asset-guard/workflows/Model%20Asset%20Guard%20CI/badge.svg)](https://github.com/fraware/model-asset-guard/actions)

> **Machine-verified integrity for fixed model artifacts**  
> Weights, quantization constraints, and tokenizer behavior validated from formal spec to runtime checks.

Model Asset Guard combines Lean 4 formal specifications with a Rust runtime sidecar and language bindings so teams can verify artifact integrity as part of normal model delivery.

## Why Model Asset Guard

ML artifact failures are often silent: corrupted checkpoints, drifted quantization assumptions, and tokenizer mismatches can pass standard unit tests and still break production behavior.  
This project closes that gap with a verification-first pipeline:

- **Formal guarantees** for key invariants in Lean 4
- **Runtime enforcement** in Rust for fast, practical checks
- **Workflow integration** through CI gates and Python/Node entry points

## What You Get

| Capability | What it verifies | Implementation |
| --- | --- | --- |
| Weight integrity | SHA-256 consistency and checkpoint validity | Lean + Rust |
| Quantization bounds | Layer error bounds for quantized weights | Lean |
| Tokenizer determinism | Stable encode/decode behavior | Lean + tests |
| Runtime guardrails | Fast validation in production paths | Rust FFI |
| Integration surface | Python and Node usage paths | Bindings |

## At a Glance

| Area | Current policy |
| --- | --- |
| Lean compiler warnings | **Fail build** via `warningAsError` in `lakefile.lean` |
| Formal completeness | CI fails on `sorry` in `src/lean` |
| Rust quality gate | `cargo clippy ... -D warnings` |
| Canonical local gate | `bash scripts/ci_preflight.sh` |

## Repository Map

```text
model-asset-guard/
├── src/
│   ├── lean/
│   │   ├── ModelAssetGuard/          # Core specs
│   │   └── cli/                      # Lean executables
│   └── rust/
│       └── guardd/                   # Runtime sidecar (FFI boundary)
├── bindings/
│   ├── python/
│   └── nodejs/
├── docs/
├── tests/
├── scripts/
├── lakefile.lean
└── lean-toolchain
```

## Quick Start

### 1) Prerequisites

- Lean 4 (`leanprover/lean4:stable`, managed with [`elan`](https://github.com/leanprover/elan))
- Rust stable + Cargo
- Python 3.11+
- Node.js 20+

### 2) Build and verify

```bash
git clone https://github.com/fraware/model-asset-guard.git
cd model-asset-guard

# Lean (warnings are errors)
lake build
lake exe tests

# Rust runtime
cargo build --release --manifest-path src/rust/guardd/Cargo.toml
cargo test --manifest-path src/rust/guardd/Cargo.toml --locked

# Optional: run full local CI gate
bash scripts/ci_preflight.sh
```

### 3) Python path setup (local development)

The Python bindings are source-based in this repo; add them to `PYTHONPATH`:

```bash
export PYTHONPATH="$(pwd)/bindings/python${PYTHONPATH:+:$PYTHONPATH}"
```

## CLI Usage

### Verify checkpoint integrity

```bash
lake exe verifyweights /path/to/checkpoint.bin
lake exe verifyweights /path/to/checkpoint.bin --digest <expected_digest>
```

### Check quantization bounds

```bash
lake exe quantbound /path/to/model.onnx --output bounds.json
lake exe quantbound /path/to/model.onnx --verify --tolerance 1e-6
```

### Test tokenizer behavior

```bash
lake exe tokenizertest /path/to/tokenizer.json --type bpe
lake exe tokenizertest /path/to/tokenizer.json --fuzz 1000000
lake exe perfecthash --help
```

## Language Integrations

### Python

```python
from bindings.python.pytorch_guard import ModelAssetGuard, HuggingFaceGuard

guard = ModelAssetGuard()
ok = guard.verify_digest("model.bin", expected_digest)
model_info = guard.checked_load("model.bin", expected_digest)

hf_guard = HuggingFaceGuard(guard)
model, tokenizer, info = hf_guard.checked_load_pretrained(
    "gpt2",
    verify_quantization=True,
)
```

### Node.js

```javascript
const { ModelAssetGuard } = require("./bindings/nodejs/node_guard.js");

const guard = new ModelAssetGuard();
const ok = guard.verifyDigest("model.bin", expectedDigest);
const modelInfo = guard.checkedLoad("model.bin", expectedDigest);
```

### Rust

```rust
use guardd::{checked_load, verify_digest};

let ok = verify_digest("model.bin", &expected_digest)?;
let info = checked_load("model.bin", Some(&expected_digest))?;
```

## API Surface (Core)

```rust
verify_digest(path: &str, expected_digest: &[u8]) -> Result<bool, GuarddError>
checked_load(path: &str, expected_digest: Option<&[u8]>) -> Result<ModelInfo, GuarddError>
verify_quantization_128_vectors(
  layer_name: &str,
  weights: &[f32],
  fan_in: u32,
  fan_out: u32,
  quant_type: &str
) -> Result<QuantizationResult, GuarddError>
```

`GuarddError` covers I/O and validation failures (`FileNotFound`, `InvalidDigest`, `QuantizationError`, `IoError`).

## Quality and CI

Main workflows in `.github/workflows/`:

- `ci.yml`: cross-platform quality checks, security checks, and performance bench step
- `formal-verify.yml`: Lean build + formal targets + `sorry` enforcement
- `bundle-push.yml`: release bundle and SBOM flow

Lean warnings are enforced as errors by project configuration in `lakefile.lean`, so formal and standard builds share the same warning-clean requirement.

## Documentation

- [Perfect hash tokenizer](docs/perfect-hash-tokenizer.md)

## Contributing

Until a dedicated contribution guide is added, use `scripts/ci_preflight.sh` and existing CI workflows as the acceptance baseline.

Typical flow:

1. Fork and branch from `main`
2. Implement changes with tests
3. Run local gates
4. Open a PR with a clear test plan

## License

MIT. See [LICENSE](LICENSE).

## Citation

```bibtex
@software{model_asset_guard,
  title={Model Asset Guard: Machine-verified integrity for ML artifacts},
  author={Model Asset Guard Contributors},
  year={2026},
  url={https://github.com/fraware/model-asset-guard}
}
```

## Support

- Issues: [GitHub Issues](https://github.com/fraware/model-asset-guard/issues)
- Discussions: [GitHub Discussions](https://github.com/fraware/model-asset-guard/discussions)
- Security reports: [GitHub Security Advisories](https://github.com/fraware/model-asset-guard/security/advisories)
