# Perfect hash tokenizer

This document describes the hash-vocab tokenizer track (T-3) in Model Asset Guard.

## What works today

| Layer | Status |
| --- | --- |
| Rust `PerfectHashVocab` | **CHD minimal perfect hash** by default (`kind: "chd_mph"`); encode/decode round-trip with string verify |
| Legacy open-addressing | Still **loaded** when JSON has `hash_table` / `table_size` (or `kind: "open_address"`) |
| `gen_perfect_hash` CLI | Emits CHD MPH JSON schema |
| C ABI / Python FFI | `guardd_perfect_hash_encode` / `guardd_perfect_hash_decode` |
| Safe Rust API | `perfect_hash_encode` / `perfect_hash_decode` |
| Lean `lake exe perfecthash` | Delegates to Rust `guardd_perfect_hash` |
| Lean specs | `perfect_hash_injective_of_unique` + key round-trip; Waves 18–24 `Token/CHD.lean` / `CHDBuild.lean` placement injectivity, bucket merge, compositional displace gates — **not** full hash-family ∃d |
| SentencePiece `model_proto` | Pieces + CHAR/BPE/UNIGRAM/WORD encode when support gate passes; internal piece table also uses `PerfectHashVocab::build` (CHD) |
| C ABI SP encode/decode | `guardd_sp_encode` / `guardd_sp_decode` (+ nbest/sample); safe Rust: `sp_encode` / `sp_decode` / … |

Wave 17 cut over the **runtime default** from open-addressing to a real CHD MPH.
Wave 18–19: Lean placement / bucket-place merge; fetch scripts; release native zips.
Wave 20: Lean `HashSeeded` mirrors Rust `hash_seeded` (ASCII goldens); displace
candidates theorem once a displace is found; Float half-unit aliases (no Mathlib).
Wave 21: `CHDBuild.findDisplace` / `buildAsciiMph` executable search + singleton
`wellSized` / `candidateSlots_*` theorems; completeness of ∃d still an obligation.
Wave 22: `findDisplace_eq_some_imp` (partial correctness); `n=1` slot lemmas + smoke;
`buildAsciiMphBucketed` (seed0 groups, largest-first) closer to Rust `try_chd`.
Wave 23: Bool↔Prop gates (`wellFormed_of_wellFormedBool`, `displaceCandidatesOk_of_candidatesOkBool`);
`findDisplace_singleton_freeOccupied` / `findDisplace_nil_keys`; Rust-aligned bucket plans +
pilots (`tryAsciiMphBucketed` / `buildAsciiMphBucketedN`); Lean↔Rust slot/pilot goldens.
Wave 24: stable bucket order (largest length, then ascending bucket id) shared by Rust
`bucket_process_order` and Lean `bucketProcessOrder`; first-successful λ-plan goldens
(`buildAsciiMphBucketedFull` / `test_chd_lean_golden_lambda_plans`); compositional
`wellSized_of_findDisplace_success` + `wellSized_merge_bucket_places`; restricted
`findDisplace_returns_zero_of_candidatesOk_at_zero` (not a full hash-family theorem).
Multi-key ∃d / general hash-family completeness remains `chd_displace_search_obligation`.
Search budgets: Rust `MAX_DISPLACE = 4e6`; Lean smoke/golden default `leanSmokeMaxD = 1e4`.
Legacy open-address JSON remains readable. Formal completeness is still **not** claimed.

## Schema (CHD MPH — default)

```json
{
  "vocab": [["hello", 1], ["world", 2]],
  "kind": "chd_mph",
  "pilots": [0, 1],
  "n_buckets": 1,
  "indices": [0, 1],
  "seed0": 11400714819323198485,
  "seed1": 13787848793156543929
}
```

- `vocab`: `(word, token_id)` pairs (tokens `1..N` when generated from a word list; `0` is unknown)
- `pilots`: per-bucket displace values (CHD)
- `indices`: `indices[slot] → vocab index` for the unique slot in `0..n-1`
- Lookup verifies the stored string (unknown keys → token `0`)

## Schema (legacy open-addressing)

```json
{
  "vocab": [["hello", 1], ["world", 2]],
  "hash_table": [0, 18446744073709551615, 1],
  "table_size": 3
}
```

- `hash_table`: slot → vocab index; empty slots are `usize::MAX`
- Generator historically emitted this; new builds emit CHD instead

CLI:

```bash
cargo run --manifest-path src/rust/guardd/Cargo.toml --bin gen_perfect_hash -- vocab.txt out.json
cargo run --manifest-path src/rust/guardd/Cargo.toml --bin gen_perfect_hash -- vocab.json --output out.json
```

## Lean entry points

- `lake exe perfecthash -- encode|decode|test|generate ...` — shells to `guardd_perfect_hash`
- `lake exe tokenizertest -- <vocab.json>` — runs Rust round-trip validation
- `lake exe tokenizertest` — separate tokenizer harness (see that CLI for its own honesty status).

Lean builds treat warnings as errors (`lakefile.lean`, `warningAsError`).

## Tests

Offline (default preflight):

```bash
pytest tests/e2e/test_offline_guard.py tests/e2e/test_perfect_hash_integration.py --maxfail=1
```

Optional integration (Node FFI / Lean stub honesty):

```bash
pytest tests/e2e -m integration --maxfail=1
```

## Related files

| Area | Path |
| ---- | ---- |
| Runtime vocab | `src/rust/guardd/src/perfect_hash.rs` |
| SP encode | `src/rust/guardd/src/sentencepiece.rs` + [`sentencepiece-bpe.md`](sentencepiece-bpe.md) |
| Structural BPE | `src/rust/guardd/src/bpe.rs` (Lean-aligned merges) |
| Generator | `src/rust/guardd/src/bin/gen_perfect_hash.rs` |
| FFI | `src/rust/guardd/src/lib.rs` |
| Lean tokenizer spec | `src/lean/ModelAssetGuard/Token/Tokenizer.lean` |
| Lean CHD placement | `src/lean/ModelAssetGuard/Token/CHD.lean` |
| Lean CHD builder | `src/lean/ModelAssetGuard/Token/CHDBuild.lean` |
| Lean CLI stub | `src/lean/cli/PerfectHash/Main.lean` |
| Python bindings | `bindings/python/pytorch_guard.py` |
| Native packaging | [`native-packaging.md`](native-packaging.md) |
