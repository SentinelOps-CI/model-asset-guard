<div align="center">
    
<pre>
####################################################################################################
#                                                                                                  #
#                                                                                                  #
#         __  __           _      _      _                 _      ____                     _       #
#        |  \/  | ___   __| | ___| |    / \   ___ ___  ___| |_   / ___|_   _  __ _ _ __ __| |      #
#        | |\/| |/ _ \ / _` |/ _ \ |   / _ \ / __/ __|/ _ \ __| | |  _| | | |/ _` | '__/ _` |      #
#        | |  | | (_) | (_| |  __/ |  / ___ \\__ \__ \  __/ |_  | |_| | |_| | (_| | | | (_| |      #
#        |_|  |_|\___/ \__,_|\___|_| /_/   \_\___/___/\___|\__|  \____|\__,_|\__,_|_|  \__,_|      #
#                                                                                                  #
#                                                                                                  #
#                                                                                                  #
####################################################################################################
</pre>

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Lean 4](https://img.shields.io/badge/Lean-4%20stable-blue.svg)](https://leanprover.github.io/)
[![Rust](https://img.shields.io/badge/Rust-stable-orange.svg)](https://www.rust-lang.org/)
[![Python](https://img.shields.io/badge/Python-3.10+-green.svg)](https://www.python.org/)

> Integrity checks for fixed model artifacts — Lean specs where they exist, Rust runtime where it matters.  
> This README describes **what works today**, not aspirational completeness.

Model Asset Guard combines Lean 4 specifications (partial; Float claims are obligation markers, not axioms) with a Rust runtime sidecar (`guardd`) and language bindings for digest, quantization sampling, and deterministic hash-vocab tokenization.

</div>

---

## Honesty status (read this first)

| Claim area | Reality today |
| --- | --- |
| Weight SHA-256 | **Rust** computes real SHA-256 (`guardd_verify_digest`, `guardd_sha256` CLI). Lean `simpleDigest` is a **placeholder** and is not used for CLI verify. |
| `lake exe verifyweights --digest` | Calls Rust `guardd_sha256`. Exits non-zero if the binary is missing — does **not** fake success. |
| Quantization bounds | **Rust** runs 128-vector sampling checks (`guardd_verify_quant_128_vectors`). Legacy `guardd_verify_quant` is **unimplemented** (always null; fail-closed). Lean has Int half-ULP / L1 theorems; Float claims are obligation markers. |
| Tokenizer / hash vocab | **Rust** CHD MPH (default) + legacy open-address load + FFI. Lean `perfecthash` / `tokenizertest` shell to Rust. SP `.model`: CHAR/BPE/UNIGRAM/WORD + charsmap/`byte_fallback`/denormalizer/PrefixMatcher; ExtraOptions `bos`/`eos`/`reverse`; UNIGRAM n-best/sample/WOR SampleEncodeAndScore; BPE-dropout; silent-`<unk>` opt-in ([`docs/sentencepiece-bpe.md`](docs/sentencepiece-bpe.md)). Lean `bpe_encode` / `sp_encode` in `Tokenizer.lean` remain **char-level stubs** — not the Rust ModelProto path. |
| Formal completeness | **Not** claimed. **0** Lean `axiom`s; Float bridges are obligation markers — see below. |
| Safe Rust API | Thin safe wrappers exist (`verify_digest`, `checked_load`, …) in addition to the C ABI. |
| Node `checkedLoad` | Reports `valid` from digest comparison; does not hardcode `true`. |
| Native packaging | Stage locally via `scripts/stage_guardd_native.sh`, or `fetch_guardd_native` (**release zip first**, CI fallback). **No** fake npm/PyPI prebuilt publish — see [`docs/native-packaging.md`](docs/native-packaging.md). |
| CI pytest | Offline e2e tests **must pass**. `@pytest.mark.integration` (HF/network/Node/Lean) is excluded from default preflight. |

### Lean axioms vs runtime

Canonical inventory: [`docs/axioms.md`](docs/axioms.md) (CI-gated by `scripts/check_axioms.py`).

Current axiom count: **0**. Former Float half-ULP axiom is now
`round_int8_error_bound_float_obligation` (marker, not a proof).

Proven (not axioms): fixed-point half-ULP (`Fixed.roundDivInt_error`),
entrywise / row-wise Int L1 bounds (`layer_row_l1_bound`, `layer_rows_l1_bound`),
faithful-vocab / single-char nonempty tokenizer lemmas, BPE merge length /
identity / composition lemmas, hash-vocab injectivity under unique keys.

Honest non-axioms: `per_channel_error_bound` (trivial theorem),
`verify_128_vectors_sound` / `layer_verification_sound` /
`layer_error_bound_float_l2_obligation` /
`round_int8_error_bound_float_obligation` (`Prop := True` markers; L2 runtime is Rust).
Float bridge (Wave 28 aborted): [`docs/float-obligation.md`](docs/float-obligation.md).
SP/BPE support matrix: [`docs/sentencepiece-bpe.md`](docs/sentencepiece-bpe.md).

Runtime CLIs shell out to Rust `guardd_*` helpers; missing binaries fail closed.
Formal completeness is **not** claimed.

## Why Model Asset Guard

ML artifact failures are often silent: corrupted checkpoints, drifted quantization assumptions, and tokenizer mismatches can pass standard unit tests and still break production behavior. This repo provides:

- Lean 4 specifications and proof obligations (no `axiom`s; Float markers remain unproven)
- Rust runtime enforcement for fast SHA-256 and quantization sampling
- Python / Node bindings over the C ABI
- CI gates that fail on real test failures (no `|| true` on pytest)

## What You Get

| Capability | What it verifies | Implementation status |
| --- | --- | --- |
| Weight integrity | SHA-256 of file bytes | Rust (real). Lean CLI delegates to Rust. |
| Quantization bounds | Layer error stats over 128 random activations | Rust runtime. Lean Int theorems + Float markers. |
| Perfect-hash vocab | CHD minimal perfect hash (default); legacy open-address still loads | Rust `guardd_perfect_hash` / FFI. Lean CLI delegates. |
| Bit-flip corpus | Corruption detection (default ≤64 MiB) | Rust `guardd_bitflip` (large sizes need opt-in) |
| HuggingFace helpers | Local verify-before-load; remote fail-closed | Python (network tests marked `integration`) |

## At a Glance

| Area | Current policy |
| --- | --- |
| Lean compiler warnings | Fail build via `warningAsError` in `lakefile.lean` |
| `sorry` in `src/lean` | Enforced separately where CI checks for it |
| Rust quality gate | `cargo clippy ... -D warnings` |
| Canonical local gate | `bash scripts/ci_preflight.sh` |
| Default pytest | Offline only (`-m 'not integration'`) |

## Repository Map

```text
model-asset-guard/
├── src/
│   ├── lean/
│   │   ├── ModelAssetGuard/          # Specs (0 axioms; Float markers)
│   │   └── cli/                      # Lean executables (some stubs)
│   └── rust/
│       └── guardd/                   # Runtime sidecar (C ABI + safe Rust API)
├── bindings/
│   ├── python/
│   └── nodejs/
├── docs/
├── tests/e2e/                        # Offline + integration pytest
├── scripts/
├── lakefile.lean
└── lean-toolchain
```

## Quick Start

### 1) Prerequisites

- Lean 4 (`leanprover/lean4:stable`, managed with [`elan`](https://github.com/leanprover/elan))
- Rust stable + Cargo
- Python 3.10+ (`requires-python` in `pyproject.toml`)
- Node.js 20+ (optional; Node tests are `integration`)

### 2) Build and verify

```bash
git clone https://github.com/SentinelOps-CI/model-asset-guard.git
cd model-asset-guard

# Lean (warnings are errors)
lake build
lake exe tests

# Rust runtime (library + tools)
cargo build --release --manifest-path src/rust/guardd/Cargo.toml
cargo test --manifest-path src/rust/guardd/Cargo.toml --locked

# Offline Python e2e (requires built libguardd)
export PYTHONPATH="$(pwd)/bindings/python${PYTHONPATH:+:$PYTHONPATH}"
pytest tests/e2e --maxfail=1

# Optional: full local CI gate (Lean + Rust + offline pytest)
bash scripts/ci_preflight.sh
```

### 3) Python path setup (local development)

```bash
export PYTHONPATH="$(pwd)/bindings/python${PYTHONPATH:+:$PYTHONPATH}"
```

## CLI Usage

### Verify checkpoint integrity (SHA-256 via Rust)

```bash
# Build the helper used by the Lean CLI
cargo build --release --manifest-path src/rust/guardd/Cargo.toml --bin guardd_sha256

# Print digest
lake exe verifyweights /path/to/checkpoint.bin

# Compare digest (hex)
lake exe verifyweights /path/to/checkpoint.bin --digest <expected_sha256_hex>

# Or call Rust directly
cargo run --manifest-path src/rust/guardd/Cargo.toml --bin guardd_sha256 -- /path/to/checkpoint.bin
```

### Quantization / tokenizer Lean CLIs

`quantbound`, `quantverify128`, `bitflipcorpus`, `tokenizertest`, and `perfecthash`
delegate to Rust `guardd_*` helpers (fail closed if binaries are missing).
`lake exe benchmarks` is an honest smoke stub (no fake MB/s).

Working perfect-hash path:

```bash
cargo run --manifest-path src/rust/guardd/Cargo.toml --bin gen_perfect_hash -- vocab.txt out.json
# then encode/decode via Python ModelAssetGuard.perfect_hash_* or C ABI
```

`lake exe perfecthash ...` shells to Rust `guardd_perfect_hash` (fail closed if missing).

## Language Integrations

### Python

```python
from pytorch_guard import ModelAssetGuard

guard = ModelAssetGuard()  # loads libguardd / guardd.dll
ok = guard.verify_digest("model.bin", expected_digest)  # False on mismatch (no raise)
model_info = guard.checked_load("model.bin", expected_digest)
```

HuggingFace helpers require `transformers` / `torch` and are covered by `@pytest.mark.integration` tests.

### Node.js

```javascript
const { ModelAssetGuard } = require("./bindings/nodejs/node_guard.js");

const guard = new ModelAssetGuard();
const ok = guard.verifyDigest("model.bin", expectedDigest);
const modelInfo = guard.checkedLoad("model.bin", expectedDigest);
// modelInfo.valid is true only when expectedDigest matches
```

Requires `koffi` (`cd bindings/nodejs && npm install`) and a built `libguardd`
(or `GUARDD_LIB` pointing at it). Preferred local path:

```bash
cargo build --release --manifest-path src/rust/guardd/Cargo.toml
bash scripts/stage_guardd_native.sh
cd bindings/nodejs && npm install && npm test
```

See `bindings/nodejs/README.md` and [`docs/native-packaging.md`](docs/native-packaging.md)
(CI uploads per-OS artifacts; npm does **not** ship prebuilts).

### Rust

Public surface is primarily a **C ABI** (`extern "C" fn guardd_*`). Thin safe wrappers are also available when linking the `rlib`:

```rust
use guardd::{checked_load, verify_digest};

let ok = verify_digest("model.bin", &expected_digest)?;
let info = checked_load("model.bin", Some(&expected_digest))?;
```

C ABI equivalents: `guardd_verify_digest`, `guardd_checked_load`, `guardd_verify_quant_128_vectors`, `guardd_verify_model_128_vectors`, `guardd_perfect_hash_encode`, `guardd_perfect_hash_decode`, `guardd_sp_*`, bitflip helpers.

Safe Rust also exposes `perfect_hash_encode` / `sp_encode` / `run_bitflip_corpus_test` (platform temp dir by default — not a hardcoded `/tmp`).

`guardd_verify_quant` is **deprecated and unimplemented**: it always returns null
(fail-closed; null means unavailable, not success). Use
`guardd_verify_quant_128_vectors` / `guardd_verify_model_128_vectors`.

### Model quantization JSON contract

Python and Rust agree on object-form layers:

```json
[{
  "name": "linear.weight",
  "weights": [0.1, 0.2, 0.3, 0.4, 0.5, 0.6],
  "fan_in": 3,
  "fan_out": 2,
  "quant_type": "int8"
}]
```

### Perfect-hash vocab JSON contract

Default builds emit a CHD minimal perfect hash:

```json
{
  "vocab": [["hello", 1], ["world", 2]],
  "kind": "chd_mph",
  "pilots": [0],
  "n_buckets": 1,
  "indices": [0, 1],
  "seed0": 11400714819323198485,
  "seed1": 13787848793156543929
}
```

Produced by `gen_perfect_hash`; consumed by FFI encode/decode. Legacy
`hash_table` / `table_size` open-address JSON still loads. See
[`docs/perfect-hash-tokenizer.md`](docs/perfect-hash-tokenizer.md).

## Quality and CI

Main workflows in `.github/workflows/`:

- `ci.yml`: runs `scripts/ci_preflight.sh` (Lean + Rust + offline pytest) and
  uploads `guardd-native-<os>` release library artifacts
- `formal-verify.yml`: Lean build + formal targets + `sorry` enforcement
- `bundle-push.yml`: release bundle and SBOM flow

Pytest failures fail the gate. Integration tests (HF downloads, Node FFI, Lean stub checks) are opt-in:

```bash
pytest tests/e2e -m integration
```

## Documentation

- [Perfect hash tokenizer](docs/perfect-hash-tokenizer.md)
- [SentencePiece / BPE support matrix](docs/sentencepiece-bpe.md)
- [Native packaging](docs/native-packaging.md)
- [Lean axiom inventory](docs/axioms.md)
- [Float obligations](docs/float-obligation.md)

## Contributing

Use `scripts/ci_preflight.sh` and existing CI workflows as the acceptance baseline.

1. Fork and branch from `main`
2. Implement changes with tests that assert real behavior
3. Run local gates
4. Open a PR with a clear test plan

## License

MIT. See [LICENSE](LICENSE).

## Citation

```bibtex
@software{model_asset_guard,
  title={Model Asset Guard: integrity checks for ML artifacts},
  author={Model Asset Guard Contributors},
  year={2026},
  url={https://github.com/SentinelOps-CI/model-asset-guard}
}
```

## Support

- Issues: [GitHub Issues](https://github.com/SentinelOps-CI/model-asset-guard/issues)
- Discussions: [GitHub Discussions](https://github.com/SentinelOps-CI/model-asset-guard/discussions)
- Security reports: [GitHub Security Advisories](https://github.com/SentinelOps-CI/model-asset-guard/security/advisories)
