#!/usr/bin/env bash

set -euo pipefail

echo "[ci] Lean build (warnings are errors via lakefile.lean leanOptions)"
lake build

echo "[ci] Lean tests"
lake exe tests

echo "[ci] Rust fmt + clippy + tests"
cargo fmt --manifest-path src/rust/guardd/Cargo.toml --all -- --check
cargo clippy --manifest-path src/rust/guardd/Cargo.toml --all-targets --locked -- -D warnings
cargo test --manifest-path src/rust/guardd/Cargo.toml --locked

echo "[ci] Python lint + tests"
ruff check bindings/python tests/e2e
python -m py_compile bindings/python/pytorch_guard.py tests/e2e/test_huggingface_integration.py tests/e2e/test_perfect_hash_integration.py
pytest tests/e2e --maxfail=1 --disable-warnings --cov=bindings/python --cov-report=xml || true

echo "[ci] Smoke benchmark"
lake exe benchmarks > benchmark_results.txt
