#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[ci] Lean build (warnings are errors via lakefile.lean leanOptions)"
lake build
# CLI tools used by docs and smoke steps (not all are default_target)
lake build verifyweights perfecthash benchmarks quantbound quantverify128 bitflipcorpus tokenizertest

echo "[ci] Lean tests"
lake exe tests

echo "[ci] Axiom inventory gate (no formal-completeness claim)"
python scripts/check_axioms.py

echo "[ci] Rust fmt + clippy + tests"
cargo fmt --manifest-path src/rust/guardd/Cargo.toml --all -- --check
cargo clippy --manifest-path src/rust/guardd/Cargo.toml --all-targets --locked -- -D warnings
cargo test --manifest-path src/rust/guardd/Cargo.toml --locked

echo "[ci] Build libguardd + Lean/CLI helper bins"
cargo build --manifest-path src/rust/guardd/Cargo.toml --locked
cargo build --manifest-path src/rust/guardd/Cargo.toml --locked \
  --bin guardd_sha256 --bin guardd_perfect_hash --bin guardd_bitflip --bin guardd_quant128

# Optional local convenience: stage debug lib into binding native/ dirs when present.
if [[ "${STAGE_GUARDD_NATIVE:-0}" == "1" ]]; then
  echo "[ci] Staging guardd native (STAGE_GUARDD_NATIVE=1)"
  PROFILE=debug bash scripts/stage_guardd_native.sh || true
fi

echo "[ci] Python lint + offline e2e tests (integration markers excluded)"
ruff check bindings/python tests/e2e scripts/check_axioms.py
python -m py_compile bindings/python/pytorch_guard.py tests/e2e/test_offline_guard.py \
  tests/e2e/test_huggingface_integration.py tests/e2e/test_perfect_hash_integration.py \
  scripts/check_axioms.py
# Default addopts in pyproject.toml exclude @pytest.mark.integration
pytest tests/e2e --maxfail=1 --disable-warnings \
  --cov=bindings/python --cov-report=xml

echo "[ci] Smoke benchmark (measured via guardd_sha256)"
lake exe benchmarks > benchmark_results.txt

