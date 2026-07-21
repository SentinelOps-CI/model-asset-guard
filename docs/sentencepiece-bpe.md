# SentencePiece / BPE support matrix

Honest status for Model Asset Guard Wave 18. This is **not** a claim of full
SentencePiece compatibility or formal completeness.

## Runtime (Rust `guardd`)

| Capability | Status |
| --- | --- |
| Parse `ModelProto` pieces (UTF-8, scores, types) | **Yes** — minimal wire parser; skips unknown fields |
| Open-addressed hash vocab from pieces | **Yes** — token id = piece index; fail on duplicates / empty |
| `guardd_perfect_hash test` on `.model` | **Yes** — vocab round-trip + reports `model_type` / encode gate |
| CHAR encode (`TrainerSpec.model_type=CHAR`) | **Yes** — SP normalize → PrefixMatcher seeds (USER_DEFINED) |
| BPE encode (`model_type=BPE`) | **Yes** — PrefixMatcher freeze + priority-queue merges (official `Encode` ≡ `SampleEncode(α=0)`) |
| UNIGRAM encode (`model_type=UNIGRAM`, default if trainer absent) | **Yes** — lattice Viterbi; USER_DEFINED score bonus |
| WORD encode | **Yes** — `SplitIntoWords` + vocab lookup (fail-closed OOV) |
| `precompiled_charsmap` (Darts double-array) | **Yes** — official leftmost-longest rewrite + whitespace policy |
| Full Unicode NFKC corpus parity | **Not claimed** — curated goldens (221/rule, Wave 27) + integration cross-check; not exhaustive |
| `byte_fallback` | **Yes** — when vocab contains all 256 `<0xXX>` pieces; else refuse |
| `treat_whitespace_as_suffix` | **Yes** — dummy `▁` as suffix; WORD split attaches `▁` as suffix |
| `allow_whitespace_only_pieces` | **Yes** — parsed; affects WORD `SplitIntoWords` |
| USER_DEFINED / PrefixMatcher forced pieces | **Yes** — model type-4 + optional runtime `forced_pieces` (must be in vocab) |
| Unsupported forced-piece shapes | **Refuse** — empty / NUL / not-in-vocab |
| Reserved ids (`unk` / `bos` / `eos` / `pad`) | **Yes** — name + CONTROL/UNKNOWN type checks (SP `PieceToId` rules) |
| CONTROL pieces in segmentation | **Excluded** — not matchable edges (reserved); decode surface empty |
| ExtraOptions `bos` / `eos` / `reverse` | **Yes** — application order is always `reverse → bos → eos` (stock). CLI `--extra bos:eos:reverse` accepts those tokens in **any order** (flag set). Decode `reverse` reverses ids before detokenize |
| ExtraOptions `unk` / `unk_piece` | **N/A on id APIs** — stock only rewrites piece *strings* in `SentencePieceText`; our encode/decode return ids / detokenized text |
| Consecutive `<unk>` merge (processor) | **Yes (default)** — stock SP processor merge when not `byte_fallback`; CONTROL ids reset the run; `merge_consecutive_unk=false` / `--no-merge-unk` keeps piece-level ids |
| UNIGRAM lattice n-best | **Yes** — A* over lattice (cap 1024); pathological agenda/timeout **refuse** by default |
| UNIGRAM n-best stock agenda policy | **Opt-in** — `nbest_stock_compat` / `--nbest-stock-compat`: shrink to `min(512, n*10)` when agenda ≥ 10 000; timeout under overflow → Viterbi (matches `unigram_model.cc`) |
| UNIGRAM sample-encode | **Yes** — lattice sample (`nbest_size<0`) or sample-from-nbest (`nbest_size>1`) |
| UNIGRAM SampleEncodeAndScore (with-replacement) | **Yes** — scores = path log-prob − marginal |
| SampleEncodeAndScore WOR / `include_best` | **Yes** — Gumbel-perturbed A* n-best + log inclusion probs |
| BPE/CHAR n-best or lattice sample | **No** — clear error (no stub algorithms) |
| BPE-dropout sample-encode | **Yes** — priority-queue BPE; `alpha` = skip-merge probability; UNUSED reseg |
| Decode (ids → text) | **Yes** — ▁ dummy-prefix strip, BYTE→UTF-8, UNKNOWN→`unk_surface`, CONTROL→empty |
| Denormalizer / reverse charsmap | **Yes** — when `denormalizer_spec.precompiled_charsmap` non-empty |
| Reverse NFKC without denorm blob | **Permanent gap** — same as stock SP (most models leave denorm empty) |
| Silent `<unk>` without `byte_fallback` | **Opt-in** — `EncodeOptions.allow_silent_unk` / CLI `--silent-unk`; **default fail-closed** |
| WORD official flag-blind split | **Opt-in** — `word_official_split` matches stock `word_model.cc` (ignores trainer suffix flag) |
| Bit-identical sample RNG vs stock SP | **Permanent non-goal** — structural cross-check only; see MT19937 note below |
| Structural Lean-aligned merge table | **Separate** — `guardd::bpe` matches Lean `apply_merge_*`, not SP scores |
| Safe Rust API (`sp_encode` / `sp_decode` / …) | **Yes** — thin wrappers over the same paths as the C ABI |

### What HF / SP `.model` files can encode now

| Model characteristic | Encode? |
| --- | --- |
| CHAR / BPE / UNIGRAM / WORD, `identity` normalizer (empty charsmap) | **Yes** (if vocab covers the normalized string, or `byte_fallback` / silent-unk) |
| Same + non-empty `precompiled_charsmap` (custom / `nfkc` / `nfkc_cf` blobs) | **Yes** — charsmap applied; OOV still fail-closed unless BF / silent-unk |
| `treat_whitespace_as_suffix=true` | **Yes** — normalize + WORD split |
| `USER_DEFINED` pieces in the model | **Yes** — PrefixMatcher protect + CHAR/BPE freeze + UNIGRAM bonus |
| `byte_fallback=true` with full `<0xXX>` set (common modern LLM SP models) | **Yes** |
| Non-empty `denormalizer_spec` charsmap | **Decode applies** reverse rewrite after piece surfaces |
| Invalid charsmap / incomplete byte set | **Refuse** |
| Models relying on silent `<unk>` for OOV (no byte_fallback) | **Refuse** by default; pass `allow_silent_unk` / `--silent-unk` |

Crafted / cached fixtures under `tests/fixtures/`:

| File | Role |
| --- | --- |
| `sp_tiny_identity_unigram.model` | Empty charsmap, UNIGRAM |
| `sp_tiny_custom_norm_unigram.model` | Tiny custom charsmap (`FF21→A`, `2122→TM`) |
| `sp_custom_charsmap.bin` | Raw charsmap blob for unit tests |
| `sp_denorm_charsmap.bin` | Denorm charsmap (`TM→™`) for unit tests |
| `sp_tiny_identity_denorm_unigram.model` | Identity encode + denorm charsmap on decode |
| `sp_tiny_identity_bpe_bytefallback.model` | BPE + `byte_fallback` |
| `sp_nfkc_normalize_goldens.json` | Curated identity/nfkc/nfkc_cf normalize goldens (221 each; compat + NFD/NFC edges; not exhaustive) |

### Normalization (supported subset)

Implemented to match official SP `normalizer.cc` (not the HuggingFace grapheme walk):

1. Optional PrefixMatcher (USER_DEFINED / forced) leftmost-longest — **wins over** charsmap
2. Optional `precompiled_charsmap` leftmost-longest rewrite over the UTF-8 stream
3. ASCII-space collapse (`remove_extra_whitespaces`)
4. Optional leading **or trailing** `▁` (`add_dummy_prefix` × `treat_whitespace_as_suffix`)
5. Spaces → `▁` (`U+2581`) when `escape_whitespaces` is true (encode path requires this)

Malformed / undecodable charsmap blobs refuse encode. We do **not** claim exhaustive
NFKC / `nfkc_cf` equivalence for every Unicode edge case; see the curated golden file
and optional integration tests that load the live SP normalizer blob.

### USER_DEFINED / PrefixMatcher notes

- Model pieces with type `USER_DEFINED` (4) build the default PrefixMatcher at load.
- Runtime `forced_pieces` must already appear in the vocab; empty / NUL / missing → refuse.
- CHAR: seeds via `PrefixMatch` (longest forced piece, else one Unicode scalar).
- BPE: matched forced regions are **frozen** (no merge across / into them).
- UNIGRAM: forced / USER_DEFINED edges use score `0.1 * (char_len - 1)` (official bonus).

### WORD notes

- After normalize, split on `▁` meta-whitespace via official `SplitIntoWords` rules
  (prefix vs suffix mode; optional whitespace-only pieces).
- Each word must exist in vocab (NORMAL / USER_DEFINED); else fail-closed unless
  `byte_fallback` / `allow_silent_unk`.
- **Official divergence (default):** stock `word_model.cc` calls `SplitIntoWords(normalized)`
  with default `treat_ws_as_suffix=false`, so WORD *inference* ignores
  `TrainerSpec.treat_whitespace_as_suffix`. The trainer *does* pass the flag.
  We honor the trainer flag at encode time by default (suffix vocab works).
- **Compat:** `EncodeOptions.word_official_split=true` / Python
  `word_official_split=True` / Node `wordOfficialSplit: true` forces the flag-blind
  stock split (use with `allow_silent_unk` when suffix vocab becomes OOV under
  prefix splitting).

### Consecutive `<unk>` merge (processor)

Stock `SentencePieceProcessor::Encode` / `PopulateSentencePieceText` merges a
continuous run of UNKNOWN piece ids into a single id when `byte_fallback` is
**disabled**. CONTROL ids (`IsControl`) reset the run (`is_prev_unk = false`) so
unks on either side of BOS/EOS/PAD do not merge across the control. Segmentation
encoders do not emit CONTROL as matchable edges; ExtraOptions `add_bos` /
`add_eos` wrap CONTROL ids **after** this merge (stock `ApplyExtraOptions`
order). Injected CONTROL ids in an id list still reset the run. With
`byte_fallback`, UNK surfaces expand to `<0xXX>` bytes instead — no merge.

| Knob | Default | Effect |
| --- | --- | --- |
| `merge_consecutive_unk` / `mergeConsecutiveUnk` | **true** | Match stock processor public encode output |
| `false` / CLI `--no-merge-unk` | opt-out | Keep model-level per-span UNKNOWN ids |
| `add_bos` / `addBos` / CLI `--bos` | **false** | Prepend CONTROL `bos_id` after optional reverse |
| `add_eos` / `addEos` / CLI `--eos` | **false** | Append CONTROL `eos_id` after optional reverse |
| `reverse` / CLI `--reverse` | **false** | Reverse ids after merge and **before** bos/eos (documented stock `reverse:bos:eos`) |
| Decode `reverse` | **false** | Reverse ids before detokenize |

Silent-unk remains **opt-in** (`allow_silent_unk`); without it, OOV still fails closed
and merge never observes UNKNOWN ids.

ExtraOption `unk`/`unk_piece` is intentionally **not** exposed on id APIs (stock
no-op for integer encode/decode).

### UNIGRAM lattice notes

- Operates on Unicode character positions of the normalized string.
- Piece types marked `UNUSED` / `CONTROL` / `UNKNOWN` / `BYTE` are not segmentation
  candidates (reserved); BYTE used only via `byte_fallback` expansion.
- 1-best is lattice Viterbi; n-best is A* from EOS using Viterbi backtrace heuristics.
- Sample-encode: `nbest_size < 0` → forward-filter / backward sample; `>1` → discrete
  sample over lattice n-best with temperature `alpha`; `0|1` → Viterbi.
- SampleEncodeAndScore (`wor=false`): repeated lattice samples; score =
  `Σ α·node.score − marginal`.
- SampleEncodeAndScore (`wor=true`): Gumbel-perturbed A* n-best of size `k+1`;
  κ = (k+1)-th perturbed fx; returned scores are log inclusion probabilities
  (`log Gumbel survival`). `include_best` prepends Viterbi with score `0` and
  removes it from the Gumbel draw (official constraint: requires `wor=true`).
  **MT19937 permanent non-goal:** stock SP uses a process-global C++ MT19937
  stream (`random::GetRandomGenerator()`). We use an explicit per-call seedable
  `StdRng`. Bit-identical draws would require a real C++-compatible MT19937 plus
  an identical call-order model — **not planned** and must not be stubbed.
  Cross-checks assert structural properties (uniqueness, n-best membership,
  `include_best`), not byte-identical samples.
- Pathological n-best: agenda ≥ 10 000 or (default) wall-clock >30 s → **refuse**.
  Stock `Lattice::NBest` instead shrinks to `min(512, nbest_size*10)` when the
  agenda overflows, and on timeout **inside that overflow branch** falls back to
  Viterbi. Opt-in `nbest_stock_compat` / `--nbest-stock-compat` / JSON
  `nbest_stock_compat=true` matches that policy. Default stays refuse so
  pathological lattices never silently return a truncated n-best.
- With `byte_fallback`, single-char OOV edges use `min_score - 10` (SP `kUnkPenalty`)
  then expand to `<0xXX>` pieces.
- With `allow_silent_unk` (and no BF), the same edge emits `unk_id`.
- Default without BF / silent-unk: incomplete coverage fails closed.

### BPE notes

- Deterministic encode uses the same priority-queue algorithm as official
  `SampleEncode(α=0)` (score-desc, lower-left tie-break).
- Sample-encode / BPE-dropout: `alpha` is the skip-merge probability
  ([BPE-dropout](https://arxiv.org/abs/1910.13267)); `nbest_size` ignored.
- `UNUSED` merge targets are recorded in a reverse-merge map and resegmented at
  emit time (official `rev_merge` path). Typical trained models have no UNUSED
  pieces; the path is still exercised when present.
- Not a claim of bit-identical RNG vs stock SP (MT19937 permanent non-goal).

### Decode notes

| Step | Status |
| --- | --- |
| Concatenate `IdToPiece` | Yes |
| Strip one leading `▁` (dummy prefix / whitespace policy) | Yes |
| `▁` → space elsewhere | Yes |
| BYTE runs → UTF-8 (invalid → U+FFFD) | Yes |
| UNKNOWN id → `TrainerSpec.unk_surface` (default `" ⁇ "`) | Yes |
| Control pieces | Empty surface |
| Decode ExtraOptions `reverse` | Yes — reverses ids before detokenize |
| `denormalizer_spec` charsmap | Applied when present (official SP order) |
| Implicit reverse NFKC without denorm | **No** — permanent gap (documented + tested) |

### CLI

```bash
cargo run --manifest-path src/rust/guardd/Cargo.toml --bin guardd_perfect_hash -- \
  sp-encode path/to/model.model "hello"

cargo run --manifest-path src/rust/guardd/Cargo.toml --bin guardd_perfect_hash -- \
  sp-encode path/to/model.model "z" --silent-unk

cargo run --manifest-path src/rust/guardd/Cargo.toml --bin guardd_perfect_hash -- \
  sp-encode path/to/model.model "xyz" --silent-unk --no-merge-unk

cargo run --manifest-path src/rust/guardd/Cargo.toml --bin guardd_perfect_hash -- \
  sp-encode path/to/model.model "hello" --reverse --bos --eos

cargo run --manifest-path src/rust/guardd/Cargo.toml --bin guardd_perfect_hash -- \
  sp-nbest path/to/model.model "hello" 5 --nbest-stock-compat

cargo run --manifest-path src/rust/guardd/Cargo.toml --bin guardd_perfect_hash -- \
  sp-sample path/to/model.model "hello" --nbest 5 --alpha 0.5 --seed 1

cargo run --manifest-path src/rust/guardd/Cargo.toml --bin guardd_perfect_hash -- \
  sp-sample-score path/to/model.model "hello" --samples 3 --alpha 0.5 --seed 1

cargo run --manifest-path src/rust/guardd/Cargo.toml --bin guardd_perfect_hash -- \
  sp-sample-score path/to/model.model "hello" --samples 3 --alpha 0.5 --seed 1 --wor --include-best

cargo run --manifest-path src/rust/guardd/Cargo.toml --bin guardd_perfect_hash -- \
  sp-decode path/to/model.model 1 2 3

cargo run --manifest-path src/rust/guardd/Cargo.toml --bin guardd_perfect_hash -- \
  sp-decode path/to/model.model --reverse 3 2 1

# Vocab integrity only (whitespace-split hash encode — not SP segmentation):
cargo run --manifest-path src/rust/guardd/Cargo.toml --bin guardd_perfect_hash -- \
  test path/to/model.model
```

### FFI / bindings

| Surface | Status |
| --- | --- |
| `guardd_sp_encode` / `guardd_sp_encode_with_options` / `guardd_sp_decode` / `guardd_sp_decode_with_options` | **Yes** |
| `guardd_sp_nbest_encode` / `guardd_sp_sample_encode` | **Yes** — UNIGRAM lattice; BPE-dropout via sample_encode |
| `guardd_sp_sample_encode_and_score` | **Yes** — UNIGRAM WR + WOR |
| Options: `allow_silent_unk`, `forced_pieces`, `word_official_split`, `merge_consecutive_unk`, `reverse`, `add_bos`, `add_eos`, `nbest_stock_compat` | **Yes** — silent-unk fail-closed; merge default on; reverse/bos/eos opt-in; n-best stock-compat opt-in |
| Sample opts: `nbest_size`, `alpha`, `seed` | **Yes** |
| Score opts: `num_samples`, `alpha`, `wor`, `include_best`, `seed` | **Yes** |
| Python `sp_encode` / `sp_nbest_encode` / `sp_sample_encode` / `sp_sample_encode_and_score` / `sp_decode` | **Yes** |
| Node `spEncode` / `spNbestEncode` / `spSampleEncode` / `spSampleEncodeAndScore` / `spDecode` | **Yes** |

## Formal (Lean)

| Claim | Status |
| --- | --- |
| Char-level stub encode/decode round-trip under faithful vocab | Proven |
| `apply_merge_once` / `apply_merge_table` length bounds | Proven |
| Merge identity when pair absent | Proven (`apply_merge_once_eq_of_not_occurs`) |
| Absent merge ⇒ length unchanged | Proven (`apply_merge_once_length_eq_of_not_occurs`) |
| Merge schedule composition under `++` | Proven (`apply_merge_table_append`) |
| Encode-with-merge-table append composition | Proven (`bpe_encode_with_merge_table_append`) |
| Encode length ≤ character count | Proven |
| Firing one-step merge shortens by exactly 1 | Proven (`apply_merge_once_length_eq_pred_of_occurs`) |
| Trained BPE merge tables / SP score BPE / UNIGRAM Viterbi | **Not** formalized |
| Full SP ModelProto semantics | **Not** formalized |
| Charsmap / denormalizer / PrefixMatcher | **Not** formalized (runtime + docs only; Darts not tractable at 0 axioms) |
| Minimal perfect hash cutover | **Yes (runtime)** — CHD MPH default via `PerfectHashVocab::build`; Lean proves injectivity under unique keys only, not CHD correctness |

## Float obligations

Unrelated to tokenization; see [`float-obligation.md`](float-obligation.md).
Axiom count remains **0**.

## Cross-check

- Offline golden vectors: crafted BPE/UNIGRAM/WORD ModelProto fixtures in Rust unit
  tests and `tests/e2e/test_offline_guard.py`.
- Cached tiny trained models in `tests/fixtures/` (identity / custom charsmap /
  denorm / byte_fallback).
- Curated NFKC normalize goldens in `sp_nfkc_normalize_goldens.json` (221 rows per
  identity / nfkc / nfkc_cf rule; Wave 27 expanded with compat forms and
  composed/decomposed pairs; size-aware; not a full Unicode audit). Offline Rust
  hygiene covers identity rows with charsmap-only knobs; live full nfkc blobs stay
  integration-only.
- Optional integration (`pytest -m integration`): compare against the
  `sentencepiece` Python package when installed (covered strings / unique-optimum
  crafted fixtures / WORD / `treat_whitespace_as_suffix` / denorm / silent-unk /
  consecutive-unk merge / USER_DEFINED / n-best / SampleEncodeAndScore WR+WOR /
  BPE-dropout / `word_official_split` / live nfkc blob + expanded goldens).

## Related paths

| Area | Path |
| --- | --- |
| SP parser + encode/decode | `src/rust/guardd/src/sentencepiece.rs` |
| UNIGRAM lattice n-best / sample / WOR | `src/rust/guardd/src/sp_lattice.rs` |
| Charsmap + PrefixMatcher + normalize / denormalize | `src/rust/guardd/src/sp_normalizer.rs` |
| Lean-aligned BPE merges | `src/rust/guardd/src/bpe.rs` |
| Lean specs | `src/lean/ModelAssetGuard/Token/Tokenizer.lean` |
| CLI | `src/rust/guardd/src/bin/guardd_perfect_hash.rs` |
| C ABI | `guardd_sp_encode*` / `guardd_sp_nbest_encode` / `guardd_sp_sample_encode` / `guardd_sp_sample_encode_and_score` / `guardd_sp_decode` |
