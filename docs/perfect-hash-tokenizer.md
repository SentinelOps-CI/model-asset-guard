# Perfect hash tokenizer

This document describes the perfect-hash tokenizer track (T-3) in Model Asset Guard: deterministic encoding and decoding aligned with the Lean specifications in `src/lean/ModelAssetGuard/Token/Tokenizer.lean`, exercised by CLI tools and integration tests.

## Lean entry points

- `lake exe perfecthash` — perfect-hash tokenizer CLI (vocabulary-driven encode/decode).
- `lake exe tokenizertest` — tokenizer tests and fuzzing against a tokenizer artifact.

Build the project first:

```bash
lake build
```

Lean builds treat warnings as errors (see `lakefile.lean`, package option `warningAsError`).

## Runtime validation

The Rust sidecar (`src/rust/guardd`) exposes C ABI helpers for perfect-hash encode/decode used by bindings. See `guardd_perfect_hash_encode` and `guardd_perfect_hash_decode` in `src/rust/guardd/src/lib.rs`.

## Integration tests

Python coverage lives in `tests/e2e/test_perfect_hash_integration.py`. Run after installing test dependencies:

```bash
pytest tests/e2e/test_perfect_hash_integration.py --maxfail=1
```

## Related files

| Area | Path |
| ---- | ---- |
| Lean tokenizer spec | `src/lean/ModelAssetGuard/Token/Tokenizer.lean` |
| Perfect-hash CLI | `src/lean/cli/PerfectHash/Main.lean` |
| Python bindings | `bindings/python/pytorch_guard.py` |
