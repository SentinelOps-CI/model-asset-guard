# Lean axiom inventory

Model Asset Guard does **not** claim formal completeness. The Lean
sources type-check and build under `warningAsError`. There are currently
**no** `axiom` declarations under `src/lean/`; remaining Float-facing
properties are honest obligation markers (`Prop := True`), not axioms.

## Allowed axioms (inventory)

No axioms. Wave 6 reformulated the last Float bridge
(`round_int8_error_bound`) into `round_int8_error_bound_float_obligation`
(obligation marker). Wave 7 kept count at 0 and documented the Float
refinement path in `float-obligation.md`. Proven Int analogue remains
`Fixed.roundDivInt_error` / `fixed_round_half_ulp`. Wave 28 evaluated a
Mathlib / IEEE refinement gate and **aborted** (opaque `Float`, CI cost,
no non-vacuous 0-axiom morphism); markers unchanged — see
`float-obligation.md`.

**Count:** 0 axioms.

## Proven (Wave 4–18) — not axioms

| Module | Theorem | Statement (informal) |
| --- | --- | --- |
| `ModelAssetGuard.Quant.Fixed` | `roundDivNat_error` / `roundDivInt_error` / `*_half_unit` | Half-ULP: `2 * \|n - round(n/d)*d\| ≤ d` |
| `ModelAssetGuard.Token.HashSeeded` | `slotMod_lt` | CHD slot `hash % n < n` |
| `ModelAssetGuard.Token.CHD` | `wellSized_of_displaceCandidatesOk` | Found displace candidates ⇒ wellSized bucket place |
| `ModelAssetGuard.Token.CHD` | `pairwiseNeBool_eq_true_iff_pairwiseDistinct` | Bool pairwise-≠ ↔ Prop pairwiseDistinct |
| `ModelAssetGuard.Token.CHD` | `wellFormed_of_wellFormedBool` / `wellSized_of_wellSizedBool` | Executable gates imply abstract well-formedness |
| `ModelAssetGuard.Token.CHDBuild` | `singleton_wellSized` | Singleton key→slot place is well-sized |
| `ModelAssetGuard.Token.CHDBuild` | `candidateSlots_length` / `candidateSlots_lt` | HashSeeded candidate slots: length + `< n` |
| `ModelAssetGuard.Token.CHDBuild` | `findDisplace_eq_some_imp` | Search return ⇒ `d ≤ maxD` and Bool gate true |
| `ModelAssetGuard.Token.CHDBuild` | `slotMod_one` / `candidateSlots_tableSize_one` | Table size 1 ⇒ every slot is `0` |
| `ModelAssetGuard.Token.CHDBuild` | `displaceCandidatesOk_of_candidatesOkBool` | Bool candidate gate ⇒ Prop `displaceCandidatesOk` |
| `ModelAssetGuard.Token.CHDBuild` | `findDisplace_singleton_freeOccupied` / `findDisplace_nil_keys` | Completeness: singleton or empty keys on free mask ⇒ `d = 0` |
| `ModelAssetGuard.Token.CHDBuild` | `candidatesOkBool_singleton_free` / `slotFreeBool_freeOccupied` | Free-mask Bool gate lemmas |
| `ModelAssetGuard.Token.CHDBuild` | `finalizePlacement_eq_some_imp_wellFormed` / `assembleBucketed_eq_some_imp_wellFormed` | Builder finalize ⇒ abstract `wellFormed` |
| `ModelAssetGuard.Token.CHDBuild` | `assembleBucketed_eq_some_imp` | Assemble success ⇒ `wellFormed` + `nBuckets` / `pilots` fields |
| `ModelAssetGuard.Token.CHDBuild` | `wellSized_of_findDisplace_success` | Compositional gate chain: findDisplace ⇒ wellSized (no mega-unfold) |
| `ModelAssetGuard.Token.CHDBuild` | `wellSized_merge_bucket_places` | Append step preserves wellSized under `BucketPlace.disjoint` |
| `ModelAssetGuard.Token.CHDBuild` | `findDisplace_returns_zero_of_candidatesOk_at_zero` | Restricted ∃d: gate OK at `d = 0` ⇒ search returns `some 0` |
| `ModelAssetGuard.Token.CHDBuild` | `placeBucket_singleton_freeOccupied` | Singleton placeBucket on free table succeeds at `d = 0` |
| `ModelAssetGuard.Quant.Fixed` | `dot_abs_bound_of_entrywise` | Entrywise Int ⇒ `2 * \|e·x\| ≤ scale * \|x\|₁` |
| `ModelAssetGuard.Quant.Fixed` | `rows_dot_abs_bound` | Same entrywise hypothesis on every row |
| `ModelAssetGuard.Quant.LayerBound` | `epsilonBoundSquaredUnits_nonneg` | Nat ε-scale is nonnegative (replaces old Float axiom) |
| `ModelAssetGuard.Quant.Core` | `layer_row_l1_bound` / `layer_rows_l1_bound` | Float L2 layer claim weakened to proven Int L1 forms |
| `ModelAssetGuard.Quant.Core` | `fixed_round_half_ulp` | Alias of Int half-ULP (replaces Float axiom role) |
| `ModelAssetGuard.Token.Tokenizer` | `bpe_string_roundtrip_of_faithful_vocab` | Char-level BPE round-trip under decode-faithful vocab (includes empty) |
| `ModelAssetGuard.Token.Tokenizer` | `sp_string_roundtrip_of_faithful_vocab` | Same for SentencePiece stub |
| `ModelAssetGuard.Token.Tokenizer` | `vocab_decode_faithful_of_forall_single_char` | Nonempty single-char identity vocabs are decode-faithful |
| `ModelAssetGuard.Token.Tokenizer` | `apply_merge_once_length_le` | One BPE merge step never increases token-list length |
| `ModelAssetGuard.Token.Tokenizer` | `apply_merge_once_length_lt_of_occurs` | Merge that fires strictly shortens the list |
| `ModelAssetGuard.Token.Tokenizer` | `apply_merge_once_length_eq_pred_of_occurs` | Firing merge shortens by exactly one |
| `ModelAssetGuard.Token.Tokenizer` | `bpe_encode_with_merge_table_cons_length_of_occurs` | Firing first merge ⇒ encode length ≤ chars − 1 |
| `ModelAssetGuard.Token.Tokenizer` | `apply_merge_once_eq_of_not_occurs` | No-pair merge is identity |
| `ModelAssetGuard.Token.Tokenizer` | `apply_merge_once_length_eq_of_not_occurs` | Absent pair ⇒ length unchanged |
| `ModelAssetGuard.Token.Tokenizer` | `apply_merge_table_length_le` | Multi-merge schedule never lengthens the token list |
| `ModelAssetGuard.Token.Tokenizer` | `apply_merge_table_append` | Merge schedules compose under `++` |
| `ModelAssetGuard.Token.Tokenizer` | `bpe_encode_with_merge_table_append` | Encode-with-merge-table schedules compose under `++` |
| `ModelAssetGuard.Token.Tokenizer` | `perfect_hash_injective_of_unique` | Unique key→token vocab ⇒ token injectivity |
| `ModelAssetGuard.Token.Tokenizer` | `perfect_hash_encode_eq_of_find` / `perfect_hash_key_roundtrip_of_unique` | Assoc-list encode/decode round-trip under uniqueness |
| `ModelAssetGuard.Token.CHD` | `slots_injective_of_wellFormed` / `keys_injective_of_wellFormed` | Abstract MPH placement: unique keys + unique slots ⇒ injective index maps |
| `ModelAssetGuard.Token.CHD` | `slot_eq_implies_index_eq` | Slot equality ↔ index equality under well-formed CHD placement |
| `ModelAssetGuard.Token.CHD` | `pairwiseDistinct_append` | Distinct lists with cross-disjointness concatenate |
| `ModelAssetGuard.Token.CHD` | `BucketPlace.wellSized_append` | Merging cross-disjoint well-sized bucket places stays well-sized |
| `ModelAssetGuard.Token.CHD` | `wellFormed_mk` | Constructor interface for well-formed placements |

## Explicitly not axioms

These used to look like formal claims; they are honest markers only:

- `per_channel_error_bound` — trivial `theorem … : True`
- `verify_128_vectors_sound` — `def … : Prop := True`
- `layer_verification_sound` — `def … : Prop := True`
- `layer_error_bound_float_l2_obligation` — `def … : Prop := True` (former Float L2 axiom; runtime L2 is Rust)
- `round_int8_error_bound_float_obligation` — `def … : Prop := True` (former Float half-ULP axiom; Int form proven)

## Removed / reformulated (no longer axioms)

- `epsilon_bound_positive` — Float ≥ 0 not constructive; replaced by `epsilonBoundSquaredUnits_nonneg`
- `layer_error_bound` — Float L2 layer axiom removed (Wave 5); Int L1 forms proven; Float L2 is an obligation marker only
- `round_int8_error_bound` — Float half-ULP axiom removed (Wave 6); Int half-ULP proven; Float is obligation marker only
- `bpe_deterministic` — false/unprovable for the merge stub; replaced by faithful-vocab round-trip
- `sp_surjective_utf8` — replaced by empty/faithful-vocab forms
- `perfect_hash_injective` — replaced by `perfect_hash_injective_of_unique` (structural)

BPE merge *tables as learned from data* are **not** formalized as correctness of
a trained tokenizer. Lean proves length / faithfulness / composition properties of
the structural merge operators. Runtime tokenization uses Rust `guardd_*`
helpers: CHD MPH hash vocab JSON by default (`kind: "chd_mph"`; legacy open-address
still loads), plus SentencePiece CHAR/BPE/UNIGRAM encode when
the support gate passes (see [`sentencepiece-bpe.md`](sentencepiece-bpe.md)).
Abstract CHD placement lemmas: `Token/CHD.lean` (not a proof of hash/displace search).
Lean `hash_seeded` mirror: `Token/HashSeeded.lean` (executable Nat-mod-2⁶⁴; ASCII goldens vs Rust).
Executable tiny CHD builder + displace search: `Token/CHDBuild.lean` (singleton theorems;
Wave 23 free-occupied completeness for singleton/empty keys; Wave 24 stable bucket order +
λ-plan goldens + compositional wellSized gates; multi-key hash-family ∃d still an obligation).
Lean↔Rust CHD goldens: `goldenBucketedNOk` / `goldenBucketedPlanOk` and Rust
`test_chd_lean_golden_n_buckets_eq_n` / `test_chd_lean_golden_lambda_plans`.
Search budgets: Rust production `MAX_DISPLACE = 4_000_000`; Lean smokes default
`leanSmokeMaxD = 10_000` (builders accept explicit `maxD`).
Float bridges: [`float-obligation.md`](float-obligation.md).

## Policy

1. New `axiom` declarations under `src/lean/` must be added to this table
   in the same change. Prefer obligation markers or proven theorems.
2. CI runs `scripts/check_axioms.py` and fails if:
   - the live axiom count differs from this inventory, or
   - an axiom name appears in Lean but not in this file.
3. Prefer a real theorem over an axiom when the proof is tractable.
4. Prefer an honest obligation marker over a lying theorem when Float/opaque
   ops block a proof.
5. Runtime guarantees come from Rust (`guardd_*`) + bindings; Lean markers
   are not substitutes for those checks.

## Runtime alignment (not formal)

- Quantization error metric: `||W_q x - W x||_2 / ||x||_2`
- int8 ε bound: `0.5 * sqrt(fan_in)` (Lean `compute_epsilon_bound` and Rust `compute_epsilon_bound`)
- Hash vocab: CHD minimal perfect hash by default (`kind: "chd_mph"`); legacy
  open-address JSON still loads (see [`perfect-hash-tokenizer.md`](perfect-hash-tokenizer.md)).
  Lean `Token.CHD` models placement injectivity; not a formal CHD construction proof.
- SentencePiece `.model`: pieces + optional CHAR/BPE/UNIGRAM encode (support matrix in `docs/sentencepiece-bpe.md`)
- Float obligations: `docs/float-obligation.md` (Wave 28 aborted; markers retained; axiom count 0)
