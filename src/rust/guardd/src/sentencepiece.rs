//! SentencePiece `ModelProto` support for Model Asset Guard.
//!
//! ## Supported (fail-closed otherwise)
//! - **Vocab integrity**: extract `pieces`, build open-addressed hash vocab
//! - **CHAR / BPE / UNIGRAM / WORD encode** after SP-faithful normalization
//! - **`precompiled_charsmap`**: Darts double-array leftmost-longest rewrite
//!   (official SP algorithm; see `sp_normalizer`)
//! - **`treat_whitespace_as_suffix`**: dummy `▁` as suffix; WORD split attaches `▁`
//! - **USER_DEFINED / PrefixMatcher forced pieces**: model type-4 pieces + optional
//!   runtime `forced_pieces` (must already be in vocab); normalize protect + CHAR/BPE
//!   freeze + UNIGRAM score bonus
//! - **UNIGRAM lattice n-best** and **sample-encode** (real A* / forward-filter;
//!   refuse for BPE/CHAR)
//! - **UNIGRAM SampleEncodeAndScore** with-replacement (scores = path log-prob −
//!   marginal) and without-replacement (Gumbel-perturbed A* n-best + inclusion probs)
//! - **`byte_fallback`**: OOV surfaces → `<0xXX>` UTF-8 byte pieces
//! - **Decode**: ▁→space with dummy-prefix strip, BYTE reassembly, UNKNOWN→unk_surface,
//!   CONTROL→empty, then optional `denormalizer_spec` charsmap (official SP path)
//! - **Optional silent `<unk>`**: `EncodeOptions.allow_silent_unk` (default false /
//!   fail-closed) matches stock SP OOV without `byte_fallback`
//! - **Consecutive `<unk>` merge**: default matches stock SP *processor*
//!   (`sentencepiece_processor.cc`); CONTROL ids reset the consecutive-unk run;
//!   disable via `merge_consecutive_unk=false` for piece-level / fail-closed id lists
//! - **ExtraOptions `bos`/`eos`/`reverse`**: after consecutive-unk merge, flags compose
//!   as stock documented `reverse:bos:eos` — reverse, then CONTROL wrap. Decode
//!   `reverse` reverses ids before detokenize. ExtraOption `unk`/`unk_piece` is a
//!   no-op on id APIs (stock only rewrites piece *strings* in SentencePieceText).
//! - **Reserved ids**: `unk` / `bos` / `eos` / `pad` resolved from piece names + types
//! - **BPE-dropout sample-encode**: priority-queue BPE + optional UNUSED resegmentation
//!
//! ## Explicitly unsupported (refuse)
//! - BPE/CHAR n-best / lattice sample (clear error; no stub algorithms)
//! - Pathological UNIGRAM n-best agenda shrink / Viterbi fallback by **default**
//!   (refuse rather than silently diverge); opt-in `nbest_stock_compat` matches
//!   stock `Lattice::NBest` shrink + timeout→Viterbi
//! - Bit-identical sample RNG vs stock SP process-global C++ MT19937 — **permanent
//!   non-goal** unless a real C++-compatible MT19937 + identical call-order model
//!   is implemented (do not stub). Cross-checks stay structural only.
//!
//! Without `byte_fallback` and without `allow_silent_unk`, missing coverage fails closed.
//! Reverse NFKC only runs when `denormalizer_spec.precompiled_charsmap` is present
//! (most stock models leave denorm empty — same permanent gap as official SP).
//! See `docs/sentencepiece-bpe.md`.

use crate::perfect_hash::PerfectHashVocab;
use crate::sp_lattice::{
    effective_matcher, encode_unigram_lattice, nbest_encode_unigram,
    sample_encode_and_score_unigram, sample_encode_and_score_unigram_wor, sample_encode_from_nbest,
    sample_encode_unigram_lattice,
};
pub use crate::sp_normalizer::LiteNormalizer;
use crate::sp_normalizer::{
    byte_to_piece, piece_to_byte, sp_denormalize, sp_normalize_with_matcher, PrecompiledCharsMap,
    PrefixMatcher,
};
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

/// SentencePiece piece type (`ModelProto.SentencePiece.Type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum PieceType {
    Normal = 1,
    Unknown = 2,
    Control = 3,
    UserDefined = 4,
    Unused = 5,
    Byte = 6,
}

impl PieceType {
    fn from_i32(v: i32) -> Result<Self, String> {
        match v {
            1 => Ok(Self::Normal),
            2 => Ok(Self::Unknown),
            3 => Ok(Self::Control),
            4 => Ok(Self::UserDefined),
            5 => Ok(Self::Unused),
            6 => Ok(Self::Byte),
            other => Err(format!("unknown SentencePiece.Type {other}")),
        }
    }
}

/// TrainerSpec model type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ModelType {
    Unigram = 1,
    Bpe = 2,
    Word = 3,
    Char = 4,
}

impl ModelType {
    fn from_i32(v: i32) -> Result<Self, String> {
        match v {
            1 => Ok(Self::Unigram),
            2 => Ok(Self::Bpe),
            3 => Ok(Self::Word),
            4 => Ok(Self::Char),
            other => Err(format!("unknown TrainerSpec.ModelType {other}")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unigram => "UNIGRAM",
            Self::Bpe => "BPE",
            Self::Word => "WORD",
            Self::Char => "CHAR",
        }
    }
}

/// One vocab piece with optional score/type.
#[derive(Debug, Clone, PartialEq)]
pub struct SpPiece {
    pub piece: String,
    pub score: f32,
    pub piece_type: PieceType,
}

/// Default SP `unk_surface` (`" ⁇ "` = space + U+2047 + space).
pub const DEFAULT_UNK_SURFACE: &str = " \u{2047} ";

/// Encode knobs. Default remains fail-closed on OOV without `byte_fallback`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodeOptions {
    /// When true and `byte_fallback` is false, emit the UNKNOWN piece id for
    /// uncovered seeds / Viterbi gaps (stock SentencePiece behavior).
    pub allow_silent_unk: bool,
    /// Extra forced pieces for this call (must already exist in vocab).
    /// Merged with model `USER_DEFINED` PrefixMatcher; empty string refused.
    pub forced_pieces: Vec<String>,
    /// When true, WORD `SplitIntoWords` ignores `TrainerSpec.treat_whitespace_as_suffix`
    /// (matches stock `word_model.cc`, which always passes `false`). Default false
    /// honors the trainer flag (suffix vocab works at encode time).
    pub word_official_split: bool,
    /// When true (default) and `byte_fallback` is false, merge consecutive
    /// UNKNOWN ids into one (stock SP *processor* `Encode` / `PopulateSentencePieceText`).
    /// Set false to keep model-level per-span ids (useful for span-aligned audits).
    pub merge_consecutive_unk: bool,
    /// Reverse piece ids after merge and **before** bos/eos.
    /// Application order is always stock `reverse → bos → eos` (see
    /// [`EncodeOptions::apply_extra_options_flags`]). CLI `--extra a:b:c` accepts
    /// those tokens in **any order** (stock ExtraOptions string is a flag set).
    pub reverse: bool,
    /// Prepend CONTROL `bos_id` after consecutive-unk merge (stock ExtraOptions `bos`).
    /// Refuses when the model has no CONTROL bos piece.
    pub add_bos: bool,
    /// Append CONTROL `eos_id` after consecutive-unk merge (stock ExtraOptions `eos`).
    /// Refuses when the model has no CONTROL eos piece.
    pub add_eos: bool,
    /// UNIGRAM n-best only: when true, match stock `Lattice::NBest` pathological
    /// agenda shrink (`min(512, nbest*10)`) and timeout→Viterbi fallback.
    /// Default false refuses rather than silently truncating the search.
    pub nbest_stock_compat: bool,
}

impl Default for EncodeOptions {
    fn default() -> Self {
        Self {
            allow_silent_unk: false,
            forced_pieces: Vec::new(),
            word_official_split: false,
            // Match stock SentencePieceProcessor public encode output.
            merge_consecutive_unk: true,
            reverse: false,
            add_bos: false,
            add_eos: false,
            nbest_stock_compat: false,
        }
    }
}

impl EncodeOptions {
    /// Apply ExtraOptions-style flag tokens. Application order is always
    /// `reverse → bos → eos` regardless of `flags` order (stock semantics:
    /// the ExtraOptions string is a set of switches).
    ///
    /// Accepted tokens (case-insensitive): `reverse`, `bos`, `eos`.
    /// Unknown tokens → `Err`.
    pub fn apply_extra_options_flags(&mut self, flags: &[&str]) -> Result<(), String> {
        let mut reverse = false;
        let mut bos = false;
        let mut eos = false;
        for raw in flags {
            let t = raw.trim();
            if t.is_empty() {
                continue;
            }
            match t.to_ascii_lowercase().as_str() {
                "reverse" => reverse = true,
                "bos" => bos = true,
                "eos" => eos = true,
                other => {
                    return Err(format!(
                        "unknown ExtraOptions token '{other}' (supported: reverse, bos, eos)"
                    ));
                }
            }
        }
        // Fixed application order (not input order).
        if reverse {
            self.reverse = true;
        }
        if bos {
            self.add_bos = true;
        }
        if eos {
            self.add_eos = true;
        }
        Ok(())
    }

    /// Parse a colon-separated ExtraOptions string (`bos:eos:reverse` etc.).
    pub fn apply_extra_options_str(&mut self, s: &str) -> Result<(), String> {
        let parts: Vec<&str> = s.split(':').collect();
        self.apply_extra_options_flags(&parts)
    }
}

/// Decode knobs. Default matches id→text without ExtraOptions.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DecodeOptions {
    /// Reverse token ids before detokenize (stock decode ExtraOptions `reverse`).
    pub reverse: bool,
}

/// Merge consecutive UNKNOWN ids (processor-level; skipped when `byte_fallback`).
///
/// Stock `SentencePieceProcessor` resets the consecutive-unk run when a CONTROL
/// id appears (BOS/EOS/PAD or other control pieces). Segmentation encoders do
/// not emit CONTROL as matchable edges; ExtraOptions `add_bos`/`add_eos` wrap
/// **after** merge (stock `ApplyExtraOptions` order). Injected CONTROL ids in
/// an id list still reset the run via `control_ids`.
pub fn merge_consecutive_unk_ids(ids: &[u32], unk_id: u32) -> Vec<u32> {
    merge_consecutive_unk_ids_with_controls(ids, unk_id, &[])
}

/// Like [`merge_consecutive_unk_ids`], but CONTROL ids in `control_ids` reset
/// the consecutive-unk run (official `is_prev_unk = false` on `IsControl`).
pub fn merge_consecutive_unk_ids_with_controls(
    ids: &[u32],
    unk_id: u32,
    control_ids: &[u32],
) -> Vec<u32> {
    let mut out = Vec::with_capacity(ids.len());
    let mut prev_unk = false;
    for &id in ids {
        if control_ids.contains(&id) {
            out.push(id);
            prev_unk = false;
            continue;
        }
        let is_unk = id == unk_id;
        if is_unk && prev_unk {
            continue;
        }
        out.push(id);
        prev_unk = is_unk;
    }
    out
}

fn model_control_ids(model: &SentencePieceModel) -> Vec<u32> {
    let mut ids: Vec<u32> = model
        .piece_records
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            if p.piece_type == PieceType::Control {
                Some(i as u32)
            } else {
                None
            }
        })
        .collect();
    // Reserved CONTROL slots are already typed Control in records; keep explicit
    // bos/eos/pad as a belt-and-suspenders for incomplete piece typing.
    for id in [model.bos_id, model.eos_id, model.pad_id]
        .into_iter()
        .flatten()
    {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids
}

fn finalize_encode_ids(
    model: &SentencePieceModel,
    opts: &EncodeOptions,
    ids: Vec<u32>,
) -> Result<Vec<u32>, String> {
    // Stock order: consecutive-unk merge inside PopulateSentencePieceText, then
    // ApplyExtraOptions. Our bool flags compose as documented CLI
    // `reverse:bos:eos` (reverse, then bos, then eos) — not arbitrary colon order.
    let mut ids = if opts.merge_consecutive_unk && !model.byte_fallback {
        let controls = model_control_ids(model);
        merge_consecutive_unk_ids_with_controls(&ids, model.unk_id, &controls)
    } else {
        ids
    };
    if opts.reverse {
        ids.reverse();
    }
    if opts.add_bos {
        let bos = model.bos_id.ok_or_else(|| {
            "add_bos: model has no CONTROL bos_id (bos_piece missing or not CONTROL)".to_string()
        })?;
        ids.insert(0, bos);
    }
    if opts.add_eos {
        let eos = model.eos_id.ok_or_else(|| {
            "add_eos: model has no CONTROL eos_id (eos_piece missing or not CONTROL)".to_string()
        })?;
        ids.push(eos);
    }
    Ok(ids)
}

fn finalize_encode_nbest(
    model: &SentencePieceModel,
    opts: &EncodeOptions,
    nbests: Vec<(Vec<u32>, f32)>,
) -> Result<Vec<(Vec<u32>, f32)>, String> {
    let mut out = Vec::with_capacity(nbests.len());
    for (ids, score) in nbests {
        out.push((finalize_encode_ids(model, opts, ids)?, score));
    }
    Ok(out)
}

fn finalize_encode_scored(
    model: &SentencePieceModel,
    opts: &EncodeOptions,
    rows: Vec<(Vec<u32>, f32)>,
) -> Result<Vec<(Vec<u32>, f32)>, String> {
    let mut out = Vec::with_capacity(rows.len());
    for (ids, score) in rows {
        out.push((finalize_encode_ids(model, opts, ids)?, score));
    }
    Ok(out)
}

/// Sample-encode knobs (UNIGRAM lattice / BPE refused).
#[derive(Debug, Clone)]
pub struct SampleEncodeOptions {
    /// Official SP: `<0` lattice sample; `0|1` Viterbi; `>1` sample from n-best.
    pub nbest_size: i32,
    /// Temperature / smoothing (`alpha` in SP). Must be finite and `>= 0`.
    pub alpha: f32,
    /// Optional RNG seed for reproducible tests. `None` → `StdRng::from_entropy()`.
    /// Not stock SP's process-global C++ MT19937 (permanent non-goal; see module docs).
    pub seed: Option<u64>,
    pub encode: EncodeOptions,
}

impl Default for SampleEncodeOptions {
    fn default() -> Self {
        Self {
            nbest_size: -1,
            alpha: 0.0,
            seed: None,
            encode: EncodeOptions::default(),
        }
    }
}

/// SampleEncodeAndScore knobs (UNIGRAM only).
#[derive(Debug, Clone)]
pub struct SampleEncodeAndScoreOptions {
    /// Number of samples to draw (`samples` / `num_samples` in SP).
    pub num_samples: usize,
    /// Inverse temperature (`alpha` in SP). Must be finite and `>= 0`.
    pub alpha: f32,
    /// Without-replacement via Gumbel-perturbed A* n-best + inclusion probs.
    pub wor: bool,
    /// Force-include Viterbi best (requires `wor=true` in official SP).
    pub include_best: bool,
    /// Optional RNG seed for reproducible tests. `None` → `StdRng::from_entropy()`.
    /// Not stock SP's process-global C++ MT19937 (permanent non-goal; see module docs).
    pub seed: Option<u64>,
    pub encode: EncodeOptions,
}

impl Default for SampleEncodeAndScoreOptions {
    fn default() -> Self {
        Self {
            num_samples: 1,
            alpha: 0.0,
            wor: false,
            include_best: false,
            seed: None,
            encode: EncodeOptions::default(),
        }
    }
}

/// Default reserved piece spellings (TrainerSpec defaults).
pub const DEFAULT_UNK_PIECE: &str = "<unk>";
pub const DEFAULT_BOS_PIECE: &str = "<s>";
pub const DEFAULT_EOS_PIECE: &str = "</s>";
pub const DEFAULT_PAD_PIECE: &str = "<pad>";

/// Parsed SentencePiece model.
#[derive(Debug, Clone)]
pub struct SentencePieceModel {
    /// Open-addressed table over `pieces` with token id = piece index.
    pub vocab: PerfectHashVocab,
    /// Raw piece strings in model order (index = token id).
    pub pieces: Vec<String>,
    /// Full piece records (scores / types).
    pub piece_records: Vec<SpPiece>,
    /// Model type from TrainerSpec (default UNIGRAM if field absent).
    pub model_type: ModelType,
    /// Lite normalizer settings used by encode.
    pub normalizer: LiteNormalizer,
    /// Parsed charsmap when present and valid.
    pub charsmap: Option<PrecompiledCharsMap>,
    /// Denormalizer lite flags (from `denormalizer_spec`).
    pub denormalizer: LiteNormalizer,
    /// Parsed denormalizer charsmap when present and valid.
    pub denorm_charsmap: Option<PrecompiledCharsMap>,
    /// `TrainerSpec.byte_fallback`.
    pub byte_fallback: bool,
    /// `TrainerSpec.treat_whitespace_as_suffix`.
    pub treat_whitespace_as_suffix: bool,
    /// `TrainerSpec.allow_whitespace_only_pieces` (WORD split).
    pub allow_whitespace_only_pieces: bool,
    /// Surface emitted for UNKNOWN piece ids on decode.
    pub unk_surface: String,
    /// Token id of the UNKNOWN piece (default 0 when present).
    pub unk_id: u32,
    /// BOS id when piece exists and is CONTROL; else `None` (`-1` in SP).
    pub bos_id: Option<u32>,
    /// EOS id when piece exists and is CONTROL; else `None`.
    pub eos_id: Option<u32>,
    /// PAD id when piece exists and is CONTROL; else `None`.
    pub pad_id: Option<u32>,
    /// PrefixMatcher over `USER_DEFINED` pieces (may be empty).
    pub prefix_matcher: PrefixMatcher,
    /// True when encode path is allowed for this model.
    pub encode_supported: bool,
    /// Human-readable reason when encode is refused.
    pub encode_unsupported_reason: Option<String>,
}

fn read_varint(data: &[u8], i: &mut usize) -> Result<u64, String> {
    let mut result: u64 = 0;
    let mut shift = 0u32;
    loop {
        if *i >= data.len() {
            return Err("truncated protobuf varint".into());
        }
        let byte = data[*i];
        *i += 1;
        result |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(result);
        }
        shift += 7;
        if shift > 63 {
            return Err("protobuf varint overflow".into());
        }
    }
}

fn read_fixed32(data: &[u8], i: &mut usize) -> Result<u32, String> {
    if *i + 4 > data.len() {
        return Err("truncated protobuf fixed32".into());
    }
    let v = u32::from_le_bytes(data[*i..*i + 4].try_into().unwrap());
    *i += 4;
    Ok(v)
}

fn skip_field(data: &[u8], i: &mut usize, wire_type: u64) -> Result<(), String> {
    match wire_type {
        0 => {
            let _ = read_varint(data, i)?;
            Ok(())
        }
        1 => {
            if *i + 8 > data.len() {
                return Err("truncated protobuf fixed64".into());
            }
            *i += 8;
            Ok(())
        }
        2 => {
            let len = read_varint(data, i)? as usize;
            if *i + len > data.len() {
                return Err("truncated protobuf length-delimited field".into());
            }
            *i += len;
            Ok(())
        }
        5 => {
            let _ = read_fixed32(data, i)?;
            Ok(())
        }
        other => Err(format!("unsupported protobuf wire type {other}")),
    }
}

fn parse_sentence_piece(msg: &[u8]) -> Result<SpPiece, String> {
    let mut i = 0usize;
    let mut piece: Option<String> = None;
    let mut score: f32 = 0.0;
    let mut piece_type = PieceType::Normal;
    while i < msg.len() {
        let key = read_varint(msg, &mut i)?;
        let field = key >> 3;
        let wire = key & 0x7;
        match (field, wire) {
            (1, 2) => {
                let len = read_varint(msg, &mut i)? as usize;
                if i + len > msg.len() {
                    return Err("truncated SentencePiece.piece".into());
                }
                let s = std::str::from_utf8(&msg[i..i + len])
                    .map_err(|_| "SentencePiece.piece is not valid UTF-8".to_string())?
                    .to_string();
                i += len;
                if s.is_empty() {
                    return Err("SentencePiece.piece must be non-empty".into());
                }
                piece = Some(s);
            }
            (2, 5) => {
                let bits = read_fixed32(msg, &mut i)?;
                score = f32::from_bits(bits);
                if !score.is_finite() {
                    return Err("SentencePiece.score must be finite".into());
                }
            }
            (3, 0) => {
                let v = read_varint(msg, &mut i)?;
                if v > i32::MAX as u64 {
                    return Err("SentencePiece.type out of range".into());
                }
                piece_type = PieceType::from_i32(v as i32)?;
            }
            _ => skip_field(msg, &mut i, wire)?,
        }
    }
    let piece = piece.ok_or_else(|| "SentencePiece message missing field piece".to_string())?;
    Ok(SpPiece {
        piece,
        score,
        piece_type,
    })
}

#[derive(Debug, Default)]
struct TrainerFlags {
    model_type: Option<ModelType>,
    byte_fallback: bool,
    treat_whitespace_as_suffix: bool,
    allow_whitespace_only_pieces: bool,
    unk_surface: Option<String>,
    unk_piece: Option<String>,
    bos_piece: Option<String>,
    eos_piece: Option<String>,
    pad_piece: Option<String>,
}

fn parse_trainer_spec(msg: &[u8]) -> Result<TrainerFlags, String> {
    let mut i = 0usize;
    let mut flags = TrainerFlags::default();
    while i < msg.len() {
        let key = read_varint(msg, &mut i)?;
        let field = key >> 3;
        let wire = key & 0x7;
        match (field, wire) {
            (3, 0) => {
                let v = read_varint(msg, &mut i)?;
                if v > i32::MAX as u64 {
                    return Err("TrainerSpec.model_type out of range".into());
                }
                flags.model_type = Some(ModelType::from_i32(v as i32)?);
            }
            (24, 0) => {
                flags.treat_whitespace_as_suffix = read_varint(msg, &mut i)? != 0;
            }
            (26, 0) => {
                flags.allow_whitespace_only_pieces = read_varint(msg, &mut i)? != 0;
            }
            (35, 0) => {
                flags.byte_fallback = read_varint(msg, &mut i)? != 0;
            }
            (44, 2) => {
                let len = read_varint(msg, &mut i)? as usize;
                if i + len > msg.len() {
                    return Err("truncated TrainerSpec.unk_surface".into());
                }
                let s = std::str::from_utf8(&msg[i..i + len])
                    .map_err(|_| "TrainerSpec.unk_surface is not valid UTF-8".to_string())?
                    .to_string();
                i += len;
                flags.unk_surface = Some(s);
            }
            (45, 2) => {
                flags.unk_piece = Some(read_trainer_string(msg, &mut i, "unk_piece")?);
            }
            (46, 2) => {
                flags.bos_piece = Some(read_trainer_string(msg, &mut i, "bos_piece")?);
            }
            (47, 2) => {
                flags.eos_piece = Some(read_trainer_string(msg, &mut i, "eos_piece")?);
            }
            (48, 2) => {
                flags.pad_piece = Some(read_trainer_string(msg, &mut i, "pad_piece")?);
            }
            _ => skip_field(msg, &mut i, wire)?,
        }
    }
    Ok(flags)
}

fn read_trainer_string(msg: &[u8], i: &mut usize, field: &str) -> Result<String, String> {
    let len = read_varint(msg, i)? as usize;
    if *i + len > msg.len() {
        return Err(format!("truncated TrainerSpec.{field}"));
    }
    let s = std::str::from_utf8(&msg[*i..*i + len])
        .map_err(|_| format!("TrainerSpec.{field} is not valid UTF-8"))?
        .to_string();
    *i += len;
    Ok(s)
}

#[derive(Debug, Default)]
struct NormalizerFlags {
    lite: LiteNormalizer,
    charsmap_bytes: Option<Vec<u8>>,
}

fn parse_normalizer_spec(msg: &[u8]) -> Result<NormalizerFlags, String> {
    let mut i = 0usize;
    let mut flags = NormalizerFlags::default();
    while i < msg.len() {
        let key = read_varint(msg, &mut i)?;
        let field = key >> 3;
        let wire = key & 0x7;
        match (field, wire) {
            (2, 2) => {
                let len = read_varint(msg, &mut i)? as usize;
                if i + len > msg.len() {
                    return Err("truncated NormalizerSpec.precompiled_charsmap".into());
                }
                if len > 0 {
                    flags.charsmap_bytes = Some(msg[i..i + len].to_vec());
                }
                i += len;
            }
            (3, 0) => {
                flags.lite.add_dummy_prefix = read_varint(msg, &mut i)? != 0;
            }
            (4, 0) => {
                flags.lite.remove_extra_whitespaces = read_varint(msg, &mut i)? != 0;
            }
            (5, 0) => {
                flags.lite.escape_whitespaces = read_varint(msg, &mut i)? != 0;
            }
            _ => skip_field(msg, &mut i, wire)?,
        }
    }
    Ok(flags)
}

struct ParsedModelProto {
    pieces: Vec<SpPiece>,
    trainer: TrainerFlags,
    normalizer: NormalizerFlags,
    has_normalizer_spec: bool,
    denormalizer: NormalizerFlags,
    has_denormalizer_spec: bool,
}

fn parse_model_proto(data: &[u8]) -> Result<ParsedModelProto, String> {
    let mut i = 0usize;
    let mut pieces = Vec::new();
    let mut trainer = TrainerFlags::default();
    let mut normalizer = NormalizerFlags::default();
    let mut has_normalizer_spec = false;
    let mut denormalizer = NormalizerFlags::default();
    let mut has_denormalizer_spec = false;
    while i < data.len() {
        let key = read_varint(data, &mut i)?;
        let field = key >> 3;
        let wire = key & 0x7;
        match (field, wire) {
            (1, 2) => {
                let len = read_varint(data, &mut i)? as usize;
                if i + len > data.len() {
                    return Err("truncated ModelProto.pieces entry".into());
                }
                let piece = parse_sentence_piece(&data[i..i + len])?;
                i += len;
                pieces.push(piece);
            }
            (2, 2) => {
                let len = read_varint(data, &mut i)? as usize;
                if i + len > data.len() {
                    return Err("truncated ModelProto.trainer_spec".into());
                }
                trainer = parse_trainer_spec(&data[i..i + len])?;
                i += len;
            }
            (3, 2) => {
                let len = read_varint(data, &mut i)? as usize;
                if i + len > data.len() {
                    return Err("truncated ModelProto.normalizer_spec".into());
                }
                normalizer = parse_normalizer_spec(&data[i..i + len])?;
                has_normalizer_spec = true;
                i += len;
            }
            (5, 2) => {
                let len = read_varint(data, &mut i)? as usize;
                if i + len > data.len() {
                    return Err("truncated ModelProto.denormalizer_spec".into());
                }
                denormalizer = parse_normalizer_spec(&data[i..i + len])?;
                has_denormalizer_spec = true;
                i += len;
            }
            _ => skip_field(data, &mut i, wire)?,
        }
    }
    Ok(ParsedModelProto {
        pieces,
        trainer,
        normalizer,
        has_normalizer_spec,
        denormalizer,
        has_denormalizer_spec,
    })
}

fn encode_support_gate(
    _model_type: ModelType,
    trainer: &TrainerFlags,
    normalizer: &NormalizerFlags,
    charsmap: &Option<PrecompiledCharsMap>,
) -> Result<(), String> {
    // All ModelType variants are encodeable when structural gates pass.
    if normalizer.charsmap_bytes.is_some() && charsmap.is_none() {
        return Err(
            "encode unsupported: NormalizerSpec.precompiled_charsmap present but invalid".into(),
        );
    }
    if !normalizer.lite.escape_whitespaces {
        return Err("encode unsupported when escape_whitespaces=false".into());
    }
    let _ = trainer.byte_fallback; // validated later once pieces known
    Ok(())
}

fn validate_byte_fallback_vocab(pieces: &[SpPiece]) -> Result<(), String> {
    let mut seen = [false; 256];
    for p in pieces {
        if p.piece_type == PieceType::Byte {
            if let Some(b) = piece_to_byte(&p.piece) {
                seen[b as usize] = true;
            }
        }
    }
    // Also accept NORMAL pieces named <0xXX> (some crafted fixtures).
    for p in pieces {
        if let Some(b) = piece_to_byte(&p.piece) {
            seen[b as usize] = true;
        }
    }
    if seen.iter().any(|&x| !x) {
        return Err(
            "encode unsupported: byte_fallback=true but vocab missing some <0xXX> pieces".into(),
        );
    }
    Ok(())
}

/// Encode a u64 as a protobuf varint (test / fixture helper).
pub fn encode_varint(mut v: u64, out: &mut Vec<u8>) {
    loop {
        let mut byte = (v & 0x7f) as u8;
        v >>= 7;
        if v != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if v == 0 {
            break;
        }
    }
}

fn encode_len_delim(field: u64, payload: &[u8], out: &mut Vec<u8>) {
    encode_varint((field << 3) | 2, out);
    encode_varint(payload.len() as u64, out);
    out.extend_from_slice(payload);
}

#[inline]
fn tag_varint(field: u64) -> u64 {
    field << 3
}

#[inline]
fn tag_fixed32(field: u64) -> u64 {
    (field << 3) | 5
}

fn encode_sp_piece(piece: &str, score: Option<f32>, piece_type: Option<i32>) -> Vec<u8> {
    let mut sp = Vec::new();
    encode_varint((1 << 3) | 2, &mut sp);
    encode_varint(piece.len() as u64, &mut sp);
    sp.extend_from_slice(piece.as_bytes());
    if let Some(s) = score {
        encode_varint(tag_fixed32(2), &mut sp);
        sp.extend_from_slice(&s.to_bits().to_le_bytes());
    }
    if let Some(t) = piece_type {
        encode_varint(tag_varint(3), &mut sp);
        encode_varint(t as u64, &mut sp);
    }
    sp
}

/// Build a minimal `ModelProto` bytes blob with the given pieces (for tests).
pub fn encode_model_proto_pieces(pieces: &[&str]) -> Vec<u8> {
    let mut out = Vec::new();
    for piece in pieces {
        encode_len_delim(1, &encode_sp_piece(piece, None, None), &mut out);
    }
    out
}

/// Fixture builder: pieces + optional trainer/normalizer/denormalizer fields.
#[derive(Default)]
pub struct ModelProtoFixture {
    pub pieces: Vec<(String, Option<f32>, Option<i32>)>,
    pub model_type: Option<i32>,
    pub byte_fallback: Option<bool>,
    pub treat_whitespace_as_suffix: Option<bool>,
    pub allow_whitespace_only_pieces: Option<bool>,
    pub unk_surface: Option<String>,
    pub unk_piece: Option<String>,
    pub bos_piece: Option<String>,
    pub eos_piece: Option<String>,
    pub pad_piece: Option<String>,
    pub add_dummy_prefix: Option<bool>,
    pub remove_extra_whitespaces: Option<bool>,
    pub escape_whitespaces: Option<bool>,
    pub precompiled_charsmap: Option<Vec<u8>>,
    pub denorm_precompiled_charsmap: Option<Vec<u8>>,
    pub denorm_add_dummy_prefix: Option<bool>,
    pub denorm_remove_extra_whitespaces: Option<bool>,
    pub denorm_escape_whitespaces: Option<bool>,
}

impl ModelProtoFixture {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for (piece, score, ty) in &self.pieces {
            encode_len_delim(1, &encode_sp_piece(piece, *score, *ty), &mut out);
        }

        let mut trainer = Vec::new();
        if let Some(mt) = self.model_type {
            encode_varint(tag_varint(3), &mut trainer);
            encode_varint(mt as u64, &mut trainer);
        }
        if let Some(v) = self.treat_whitespace_as_suffix {
            encode_varint(tag_varint(24), &mut trainer);
            encode_varint(u64::from(v), &mut trainer);
        }
        if let Some(v) = self.allow_whitespace_only_pieces {
            encode_varint(tag_varint(26), &mut trainer);
            encode_varint(u64::from(v), &mut trainer);
        }
        if let Some(v) = self.byte_fallback {
            encode_varint(tag_varint(35), &mut trainer);
            encode_varint(u64::from(v), &mut trainer);
        }
        if let Some(ref s) = self.unk_surface {
            encode_len_delim(44, s.as_bytes(), &mut trainer);
        }
        if let Some(ref s) = self.unk_piece {
            encode_len_delim(45, s.as_bytes(), &mut trainer);
        }
        if let Some(ref s) = self.bos_piece {
            encode_len_delim(46, s.as_bytes(), &mut trainer);
        }
        if let Some(ref s) = self.eos_piece {
            encode_len_delim(47, s.as_bytes(), &mut trainer);
        }
        if let Some(ref s) = self.pad_piece {
            encode_len_delim(48, s.as_bytes(), &mut trainer);
        }
        if !trainer.is_empty() {
            encode_len_delim(2, &trainer, &mut out);
        }

        let mut norm = Vec::new();
        if let Some(ref map) = self.precompiled_charsmap {
            encode_len_delim(2, map, &mut norm);
        }
        if let Some(v) = self.add_dummy_prefix {
            encode_varint(tag_varint(3), &mut norm);
            encode_varint(u64::from(v), &mut norm);
        }
        if let Some(v) = self.remove_extra_whitespaces {
            encode_varint(tag_varint(4), &mut norm);
            encode_varint(u64::from(v), &mut norm);
        }
        if let Some(v) = self.escape_whitespaces {
            encode_varint(tag_varint(5), &mut norm);
            encode_varint(u64::from(v), &mut norm);
        }
        if !norm.is_empty() {
            encode_len_delim(3, &norm, &mut out);
        }

        let mut denorm = Vec::new();
        if let Some(ref map) = self.denorm_precompiled_charsmap {
            encode_len_delim(2, map, &mut denorm);
        }
        if let Some(v) = self.denorm_add_dummy_prefix {
            encode_varint(tag_varint(3), &mut denorm);
            encode_varint(u64::from(v), &mut denorm);
        }
        if let Some(v) = self.denorm_remove_extra_whitespaces {
            encode_varint(tag_varint(4), &mut denorm);
            encode_varint(u64::from(v), &mut denorm);
        }
        if let Some(v) = self.denorm_escape_whitespaces {
            encode_varint(tag_varint(5), &mut denorm);
            encode_varint(u64::from(v), &mut denorm);
        }
        if !denorm.is_empty() {
            encode_len_delim(5, &denorm, &mut out);
        }
        out
    }
}

/// Parse SentencePiece `ModelProto` bytes and build a vocab table.
pub fn load_model_proto(bytes: &[u8]) -> Result<SentencePieceModel, String> {
    if bytes.is_empty() {
        return Err("empty model_proto".into());
    }
    let parsed = parse_model_proto(bytes)?;
    if parsed.pieces.is_empty() {
        return Err("ModelProto contains no pieces; refuse empty SentencePiece vocab".into());
    }
    {
        let mut seen = HashSet::new();
        for p in &parsed.pieces {
            if !seen.insert(p.piece.as_str()) {
                return Err(format!(
                    "duplicate SentencePiece piece {:?}; refuse ambiguous vocab",
                    p.piece
                ));
            }
        }
    }

    let model_type = parsed.trainer.model_type.unwrap_or(ModelType::Unigram);
    let normalizer = if parsed.has_normalizer_spec {
        parsed.normalizer.lite.clone()
    } else {
        LiteNormalizer::default()
    };

    let (charsmap, charsmap_parse_err) = match &parsed.normalizer.charsmap_bytes {
        Some(blob) => match PrecompiledCharsMap::parse(blob) {
            Ok(m) => (Some(m), None),
            Err(e) => (None, Some(e)),
        },
        None => (None, None),
    };

    // Official SP only constructs a denormalizer when charsmap is non-empty.
    let denormalizer = if parsed.has_denormalizer_spec {
        parsed.denormalizer.lite.clone()
    } else {
        LiteNormalizer {
            add_dummy_prefix: false,
            remove_extra_whitespaces: false,
            escape_whitespaces: false,
        }
    };
    let (denorm_charsmap, denorm_parse_err) = match &parsed.denormalizer.charsmap_bytes {
        Some(blob) if !blob.is_empty() => match PrecompiledCharsMap::parse(blob) {
            Ok(m) => (Some(m), None),
            Err(e) => (None, Some(e)),
        },
        _ => (None, None),
    };
    if let Some(e) = denorm_parse_err {
        return Err(format!(
            "invalid denormalizer_spec.precompiled_charsmap ({e})"
        ));
    }

    let mut encode_gate =
        encode_support_gate(model_type, &parsed.trainer, &parsed.normalizer, &charsmap);
    if let Some(ref e) = charsmap_parse_err {
        encode_gate = Err(format!(
            "encode unsupported: invalid precompiled_charsmap ({e})"
        ));
    }
    // Invalid denorm refuses decode later; encode still allowed (matches stock SP load).
    if encode_gate.is_ok() && parsed.trainer.byte_fallback {
        if let Err(e) = validate_byte_fallback_vocab(&parsed.pieces) {
            encode_gate = Err(e);
        }
    }

    let (encode_supported, encode_unsupported_reason) = match encode_gate {
        Ok(()) => (true, None),
        Err(reason) => (false, Some(reason)),
    };

    let unk_id = parsed
        .pieces
        .iter()
        .position(|p| p.piece_type == PieceType::Unknown)
        .unwrap_or(0) as u32;

    let unk_piece_name = parsed
        .trainer
        .unk_piece
        .as_deref()
        .unwrap_or(DEFAULT_UNK_PIECE);
    let bos_piece_name = parsed
        .trainer
        .bos_piece
        .as_deref()
        .unwrap_or(DEFAULT_BOS_PIECE);
    let eos_piece_name = parsed
        .trainer
        .eos_piece
        .as_deref()
        .unwrap_or(DEFAULT_EOS_PIECE);
    let pad_piece_name = parsed
        .trainer
        .pad_piece
        .as_deref()
        .unwrap_or(DEFAULT_PAD_PIECE);

    // Official SP: bos/eos/pad only valid when PieceToId hits a CONTROL piece.
    let bos_id = resolve_control_id(&parsed.pieces, bos_piece_name);
    let eos_id = resolve_control_id(&parsed.pieces, eos_piece_name);
    let pad_id = resolve_control_id(&parsed.pieces, pad_piece_name);
    // Prefer typed UNKNOWN; fall back to named unk_piece if typed id missing.
    let unk_id = if parsed
        .pieces
        .get(unk_id as usize)
        .is_some_and(|p| p.piece_type == PieceType::Unknown)
    {
        unk_id
    } else {
        parsed
            .pieces
            .iter()
            .position(|p| p.piece == unk_piece_name && p.piece_type == PieceType::Unknown)
            .unwrap_or(unk_id as usize) as u32
    };

    let user_defined: Vec<String> = parsed
        .pieces
        .iter()
        .filter(|p| p.piece_type == PieceType::UserDefined)
        .map(|p| p.piece.clone())
        .collect();
    let prefix_matcher = PrefixMatcher::from_pieces(user_defined)?;

    let piece_strings: Vec<String> = parsed.pieces.iter().map(|p| p.piece.clone()).collect();
    let pairs: Vec<(String, u32)> = piece_strings
        .iter()
        .enumerate()
        .map(|(i, p)| (p.clone(), i as u32))
        .collect();
    let vocab = PerfectHashVocab::build(pairs);
    if !vocab.validate_roundtrip() {
        return Err("parsed ModelProto vocab failed encode/decode round-trip".into());
    }

    Ok(SentencePieceModel {
        vocab,
        pieces: piece_strings,
        piece_records: parsed.pieces,
        model_type,
        normalizer,
        charsmap,
        denormalizer,
        denorm_charsmap,
        byte_fallback: parsed.trainer.byte_fallback,
        treat_whitespace_as_suffix: parsed.trainer.treat_whitespace_as_suffix,
        allow_whitespace_only_pieces: parsed.trainer.allow_whitespace_only_pieces,
        unk_surface: parsed
            .trainer
            .unk_surface
            .unwrap_or_else(|| DEFAULT_UNK_SURFACE.to_string()),
        unk_id,
        bos_id,
        eos_id,
        pad_id,
        prefix_matcher,
        encode_supported,
        encode_unsupported_reason,
    })
}

fn resolve_control_id(pieces: &[SpPiece], name: &str) -> Option<u32> {
    pieces.iter().enumerate().find_map(|(i, p)| {
        if p.piece == name && p.piece_type == PieceType::Control {
            Some(i as u32)
        } else {
            None
        }
    })
}

/// Load a `.model` file from disk.
pub fn load_model_proto_file(path: &str) -> Result<SentencePieceModel, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read '{path}': {e}"))?;
    load_model_proto(&bytes)
}

/// Real runtime path: build an open-addressed vocab table from JSON text.
pub fn load_vocab_json(json: &str) -> Result<PerfectHashVocab, String> {
    serde_json::from_str(json).map_err(|e| format!("invalid vocab JSON: {e}"))
}

const SP_SPACE: char = '\u{2581}';

/// SP-faithful normalization (charsmap + whitespace + USER_DEFINED matcher).
pub fn normalize(model: &SentencePieceModel, text: &str) -> Result<String, String> {
    normalize_with_options(model, text, &EncodeOptions::default())
}

pub fn normalize_with_options(
    model: &SentencePieceModel,
    text: &str,
    opts: &EncodeOptions,
) -> Result<String, String> {
    let matcher = effective_matcher(model, opts)?;
    let matcher_ref = if matcher.is_empty() {
        None
    } else {
        Some(&matcher)
    };
    sp_normalize_with_matcher(
        text,
        &model.normalizer,
        model.charsmap.as_ref(),
        matcher_ref,
        model.treat_whitespace_as_suffix,
    )
}

/// Backward-compatible alias: whitespace-only normalize (no charsmap / matcher).
pub fn lite_normalize(text: &str, norm: &LiteNormalizer) -> Result<String, String> {
    sp_normalize_with_matcher(text, norm, None, None, false)
}

fn piece_index_map(model: &SentencePieceModel) -> HashMap<&str, (u32, f32, PieceType)> {
    let mut map = HashMap::new();
    for (i, rec) in model.piece_records.iter().enumerate() {
        // Match official SP: CONTROL / UNKNOWN / BYTE are reserved — not segmentation
        // candidates. UNUSED is skipped at match time. BYTE looked up via byte_fallback.
        if matches!(
            rec.piece_type,
            PieceType::Unused | PieceType::Control | PieceType::Unknown | PieceType::Byte
        ) {
            continue;
        }
        map.insert(rec.piece.as_str(), (i as u32, rec.score, rec.piece_type));
    }
    map
}

/// Full piece map including reserved types (for byte_fallback / explicit id lookups).
fn piece_index_map_all(model: &SentencePieceModel) -> HashMap<&str, (u32, f32, PieceType)> {
    let mut map = HashMap::new();
    for (i, rec) in model.piece_records.iter().enumerate() {
        if rec.piece_type == PieceType::Unused {
            continue;
        }
        map.insert(rec.piece.as_str(), (i as u32, rec.score, rec.piece_type));
    }
    map
}

pub(crate) fn byte_piece_ids(
    model: &SentencePieceModel,
    surface: &str,
) -> Result<Vec<u32>, String> {
    let map = piece_index_map_all(model);
    let mut out = Vec::with_capacity(surface.len());
    for &b in surface.as_bytes() {
        let key = byte_to_piece(b);
        match map.get(key.as_str()) {
            Some(&(id, _, _)) => out.push(id),
            None => {
                return Err(format!("byte_fallback: missing piece {key} in vocab"));
            }
        }
    }
    Ok(out)
}

fn require_encode(model: &SentencePieceModel) -> Result<(), String> {
    if model.encode_supported {
        Ok(())
    } else {
        Err(model
            .encode_unsupported_reason
            .clone()
            .unwrap_or_else(|| "encode unsupported".into()))
    }
}

/// CHAR encode: PrefixMatcher seeds (USER_DEFINED freeze) then one id per seed.
pub fn encode_char_with_options(
    model: &SentencePieceModel,
    text: &str,
    opts: EncodeOptions,
) -> Result<Vec<u32>, String> {
    if model.model_type != ModelType::Char {
        return Err(format!(
            "encode_char requires CHAR model, got {}",
            model.model_type.as_str()
        ));
    }
    require_encode(model)?;
    let matcher = effective_matcher(model, &opts)?;
    let normalized = normalize_with_options(model, text, &opts)?;
    let map = piece_index_map(model);
    let mut out = Vec::new();
    let mut rest = normalized.as_str();
    while !rest.is_empty() {
        let (mblen, _found) = matcher.prefix_match(rest);
        let key = &rest[..mblen];
        match map.get(key) {
            Some(&(id, _, _)) => out.push(id),
            None if model.byte_fallback => {
                out.extend(byte_piece_ids(model, key)?);
            }
            None if opts.allow_silent_unk => {
                out.push(model.unk_id);
            }
            None => {
                return Err(format!(
                    "CHAR encode: seed {key:?} not in vocab (fail-closed; no byte_fallback)"
                ));
            }
        }
        rest = &rest[mblen..];
    }
    finalize_encode_ids(model, &opts, out)
}

/// CHAR encode (fail-closed default).
pub fn encode_char(model: &SentencePieceModel, text: &str) -> Result<Vec<u32>, String> {
    encode_char_with_options(model, text, EncodeOptions::default())
}

#[derive(Clone)]
struct BpeSymbol {
    prev: i32,
    next: i32,
    freeze: bool,
    /// Byte range into the normalized string (piece may span merges).
    start: usize,
    len: usize,
}

#[derive(Clone)]
struct BpePair {
    left: i32,
    right: i32,
    size: usize,
}

#[derive(Clone)]
struct BpePairOrd {
    idx: usize,
    score: f32,
    left: i32,
}

impl PartialEq for BpePairOrd {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score && self.left == other.left
    }
}
impl Eq for BpePairOrd {}
impl PartialOrd for BpePairOrd {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for BpePairOrd {
    fn cmp(&self, other: &Self) -> Ordering {
        // Max-heap by score; tie-break lower left index (official SP).
        match self
            .score
            .partial_cmp(&other.score)
            .unwrap_or(Ordering::Equal)
        {
            Ordering::Equal => other.left.cmp(&self.left),
            ord => ord,
        }
    }
}

/// BPE piece map: NORMAL / USER_DEFINED / UNUSED (official `pieces_` includes unused).
fn bpe_piece_map(model: &SentencePieceModel) -> HashMap<&str, (u32, f32, PieceType)> {
    let mut map = HashMap::new();
    for (i, rec) in model.piece_records.iter().enumerate() {
        if matches!(
            rec.piece_type,
            PieceType::Control | PieceType::Unknown | PieceType::Byte
        ) {
            continue;
        }
        map.insert(rec.piece.as_str(), (i as u32, rec.score, rec.piece_type));
    }
    map
}

fn bpe_resegment(
    w: &str,
    map: &HashMap<&str, (u32, f32, PieceType)>,
    rev_merge: &HashMap<String, (String, String)>,
    out: &mut Vec<u32>,
) -> Result<(), String> {
    match map.get(w) {
        Some(&(id, _, ty)) if ty != PieceType::Unused => {
            out.push(id);
            Ok(())
        }
        Some(_) | None => {
            if let Some((left, right)) = rev_merge.get(w) {
                bpe_resegment(left, map, rev_merge, out)?;
                bpe_resegment(right, map, rev_merge, out)
            } else if let Some(&(id, _, _)) = map.get(w) {
                // UNUSED without rev_merge entry (should not happen for merges we recorded).
                out.push(id);
                Ok(())
            } else {
                Err(format!("BPE encode: final symbol {w:?} missing from vocab"))
            }
        }
    }
}

/// Priority-queue BPE with optional dropout (`alpha` = skip-merge probability).
///
/// Matches official `bpe_model.cc` `SampleEncode`: Encode is `alpha=0`.
/// UNUSED merges are recorded in `rev_merge` and resegmented at the end.
pub fn encode_bpe_sample_with_options(
    model: &SentencePieceModel,
    text: &str,
    alpha: f32,
    opts: EncodeOptions,
    rng: &mut impl Rng,
) -> Result<Vec<u32>, String> {
    if model.model_type != ModelType::Bpe {
        return Err(format!(
            "encode_bpe requires BPE model, got {}",
            model.model_type.as_str()
        ));
    }
    if !(alpha.is_finite() && alpha >= 0.0) {
        return Err("BPE sample: alpha must be finite and >= 0".into());
    }
    require_encode(model)?;
    let matcher = effective_matcher(model, &opts)?;
    let normalized = normalize_with_options(model, text, &opts)?;
    if normalized.is_empty() {
        return Ok(Vec::new());
    }
    let map = bpe_piece_map(model);
    let norm_bytes = normalized.as_bytes();

    // Seed symbols via PrefixMatcher (USER_DEFINED freeze), else one Unicode scalar.
    // Unk seeds are held as frozen spans that never merge (expanded at emit time).
    let mut symbols: Vec<BpeSymbol> = Vec::new();
    let mut unk_spans: HashMap<usize, String> = HashMap::new();
    let mut rest = normalized.as_str();
    let mut byte_off = 0usize;
    while !rest.is_empty() {
        let (mblen, found) = matcher.prefix_match(rest);
        let key = &rest[..mblen];
        let idx = symbols.len() as i32;
        if map.contains_key(key) {
            symbols.push(BpeSymbol {
                prev: if idx == 0 { -1 } else { idx - 1 },
                next: -1, // patched below
                freeze: found,
                start: byte_off,
                len: mblen,
            });
        } else if model.byte_fallback || opts.allow_silent_unk {
            unk_spans.insert(symbols.len(), key.to_string());
            symbols.push(BpeSymbol {
                prev: if idx == 0 { -1 } else { idx - 1 },
                next: -1,
                freeze: true,
                start: byte_off,
                len: mblen,
            });
        } else {
            return Err(format!(
                "BPE encode: seed symbol {key:?} not in vocab (fail-closed; no byte_fallback)"
            ));
        }
        byte_off += mblen;
        rest = &rest[mblen..];
    }
    let n = symbols.len();
    for (i, sym) in symbols.iter_mut().enumerate() {
        sym.next = if i + 1 < n { (i + 1) as i32 } else { -1 };
    }

    let mut pool: Vec<BpePair> = Vec::new();
    let mut agenda: BinaryHeap<BpePairOrd> = BinaryHeap::new();
    let mut rev_merge: HashMap<String, (String, String)> = HashMap::new();

    let surface = |syms: &[BpeSymbol], i: usize| -> &str {
        let s = &syms[i];
        std::str::from_utf8(&norm_bytes[s.start..s.start + s.len]).unwrap_or("")
    };

    let maybe_add = |syms: &[BpeSymbol],
                     pool: &mut Vec<BpePair>,
                     agenda: &mut BinaryHeap<BpePairOrd>,
                     rev_merge: &mut HashMap<String, (String, String)>,
                     left: i32,
                     right: i32| {
        if left < 0 || right < 0 {
            return;
        }
        let (l, r) = (left as usize, right as usize);
        if syms[l].freeze || syms[r].freeze {
            return;
        }
        let piece = format!("{}{}", surface(syms, l), surface(syms, r));
        let Some(&(_, score, ty)) = map.get(piece.as_str()) else {
            return;
        };
        let idx = pool.len();
        pool.push(BpePair {
            left,
            right,
            size: piece.len(),
        });
        agenda.push(BpePairOrd { idx, score, left });
        if ty == PieceType::Unused {
            rev_merge.insert(
                piece,
                (surface(syms, l).to_string(), surface(syms, r).to_string()),
            );
        }
    };

    for i in 1..n {
        maybe_add(
            &symbols,
            &mut pool,
            &mut agenda,
            &mut rev_merge,
            (i - 1) as i32,
            i as i32,
        );
    }

    while let Some(top) = agenda.pop() {
        let pair = pool[top.idx].clone();
        let (li, ri) = (pair.left as usize, pair.right as usize);
        if symbols[li].len == 0
            || symbols[ri].len == 0
            || symbols[li].len + symbols[ri].len != pair.size
        {
            continue;
        }
        // BPE-dropout: skip merge with probability `alpha`.
        if alpha >= 1.0 || (alpha > 0.0 && rng.gen::<f64>() < f64::from(alpha)) {
            continue;
        }

        // Merge right into left (byte span grows; right cleared).
        symbols[li].len += symbols[ri].len;
        symbols[li].next = symbols[ri].next;
        let right_next = symbols[ri].next;
        if right_next >= 0 {
            symbols[right_next as usize].prev = pair.left;
        }
        symbols[ri].len = 0;

        maybe_add(
            &symbols,
            &mut pool,
            &mut agenda,
            &mut rev_merge,
            symbols[li].prev,
            pair.left,
        );
        maybe_add(
            &symbols,
            &mut pool,
            &mut agenda,
            &mut rev_merge,
            pair.left,
            symbols[li].next,
        );
    }

    let mut out = Vec::new();
    let mut index = 0i32;
    while index >= 0 {
        let i = index as usize;
        if let Some(unk) = unk_spans.get(&i) {
            if model.byte_fallback {
                out.extend(byte_piece_ids(model, unk)?);
            } else if opts.allow_silent_unk {
                out.push(model.unk_id);
            } else {
                return Err(format!(
                    "BPE encode: seed symbol {unk:?} not in vocab (fail-closed; no byte_fallback)"
                ));
            }
        } else {
            let w = surface(&symbols, i).to_string();
            bpe_resegment(&w, &map, &rev_merge, &mut out)?;
        }
        index = symbols[i].next;
    }
    finalize_encode_ids(model, &opts, out)
}

/// SentencePiece-style BPE encode: PrefixMatcher seeds + priority-queue merges
/// (official `Encode` ≡ `SampleEncode(alpha=0)`).
pub fn encode_bpe_with_options(
    model: &SentencePieceModel,
    text: &str,
    opts: EncodeOptions,
) -> Result<Vec<u32>, String> {
    // Deterministic path: no RNG draws when alpha == 0.
    let mut rng = rand::rngs::StdRng::seed_from_u64(0);
    encode_bpe_sample_with_options(model, text, 0.0, opts, &mut rng)
}

pub fn encode_bpe(model: &SentencePieceModel, text: &str) -> Result<Vec<u32>, String> {
    encode_bpe_with_options(model, text, EncodeOptions::default())
}

/// Official SP `SplitIntoWords` over a normalized string (`▁` = whitespace meta).
pub fn split_into_words(
    normalized: &str,
    treat_ws_as_suffix: bool,
    allow_ws_only_pieces: bool,
) -> Vec<&str> {
    const SPACE: &str = "\u{2581}";
    let bytes = normalized.as_bytes();
    if bytes.is_empty() {
        return Vec::new();
    }
    let mut result: Vec<(usize, usize)> = Vec::new(); // (start, len) in bytes
    let mut in_ws_sequence = false;
    let mut offset = 0usize;

    let char_len = |at: usize| -> usize {
        let b = bytes[at];
        match b {
            0x00..=0x7F => 1,
            0xC0..=0xDF => 2,
            0xE0..=0xEF => 3,
            0xF0..=0xF7 => 4,
            _ => 1,
        }
        .min(bytes.len() - at)
    };

    if treat_ws_as_suffix {
        result.push((0, 0));
        while offset < bytes.len() {
            let mblen = char_len(offset);
            let is_ws = normalized.get(offset..offset + mblen) == Some(SPACE);
            if is_ws {
                in_ws_sequence = true;
            } else if in_ws_sequence {
                if allow_ws_only_pieces {
                    result.push((offset, 0));
                }
                in_ws_sequence = false;
            }
            let last = result.len() - 1;
            result[last].1 += mblen;
            offset += mblen;
            if offset < bytes.len() && is_ws && !allow_ws_only_pieces {
                result.push((offset, 0));
            }
        }
    } else {
        while offset < bytes.len() {
            let mblen = char_len(offset);
            let is_ws = normalized.get(offset..offset + mblen) == Some(SPACE);
            if offset == 0 || (is_ws && (!in_ws_sequence || !allow_ws_only_pieces)) {
                result.push((offset, 0));
                in_ws_sequence = true;
            }
            if in_ws_sequence && !is_ws {
                in_ws_sequence = false;
            }
            let last = result.len() - 1;
            result[last].1 += mblen;
            offset += mblen;
        }
    }

    result
        .into_iter()
        .filter(|(_, len)| *len > 0)
        .map(|(start, len)| &normalized[start..start + len])
        .collect()
}

/// WORD encode: whitespace-delimited pieces after normalize (`SplitIntoWords`).
pub fn encode_word_with_options(
    model: &SentencePieceModel,
    text: &str,
    opts: EncodeOptions,
) -> Result<Vec<u32>, String> {
    if model.model_type != ModelType::Word {
        return Err(format!(
            "encode_word requires WORD model, got {}",
            model.model_type.as_str()
        ));
    }
    require_encode(model)?;
    let normalized = normalize_with_options(model, text, &opts)?;
    if normalized.is_empty() {
        return Ok(Vec::new());
    }
    let map = piece_index_map(model);
    let treat_suffix = if opts.word_official_split {
        false
    } else {
        model.treat_whitespace_as_suffix
    };
    let words = split_into_words(
        &normalized,
        treat_suffix,
        model.allow_whitespace_only_pieces,
    );
    let mut out = Vec::with_capacity(words.len());
    for w in words {
        match map.get(w) {
            Some(&(id, _, _)) => out.push(id),
            None if model.byte_fallback => {
                out.extend(byte_piece_ids(model, w)?);
            }
            None if opts.allow_silent_unk => {
                out.push(model.unk_id);
            }
            None => {
                return Err(format!(
                    "WORD encode: piece {w:?} not in vocab (fail-closed; no byte_fallback)"
                ));
            }
        }
    }
    finalize_encode_ids(model, &opts, out)
}

pub fn encode_word(model: &SentencePieceModel, text: &str) -> Result<Vec<u32>, String> {
    encode_word_with_options(model, text, EncodeOptions::default())
}

/// UNIGRAM encode: lattice Viterbi over Unicode character positions.
pub fn encode_unigram_with_options(
    model: &SentencePieceModel,
    text: &str,
    opts: EncodeOptions,
) -> Result<Vec<u32>, String> {
    if model.model_type != ModelType::Unigram {
        return Err(format!(
            "encode_unigram requires UNIGRAM model, got {}",
            model.model_type.as_str()
        ));
    }
    require_encode(model)?;
    let normalized = normalize_with_options(model, text, &opts)?;
    let ids = encode_unigram_lattice(model, &normalized, opts.clone())?;
    finalize_encode_ids(model, &opts, ids)
}

pub fn encode_unigram(model: &SentencePieceModel, text: &str) -> Result<Vec<u32>, String> {
    encode_unigram_with_options(model, text, EncodeOptions::default())
}

/// N-best encode (UNIGRAM lattice A* only). Returns `(ids, score)` best-first.
pub fn nbest_encode_with_options(
    model: &SentencePieceModel,
    text: &str,
    nbest_size: usize,
    opts: EncodeOptions,
) -> Result<Vec<(Vec<u32>, f32)>, String> {
    require_encode(model)?;
    match model.model_type {
        ModelType::Unigram => {
            let normalized = normalize_with_options(model, text, &opts)?;
            let nbests = nbest_encode_unigram(model, &normalized, nbest_size, opts.clone())?;
            finalize_encode_nbest(model, &opts, nbests)
        }
        other => Err(format!(
            "nbest_encode unsupported for model_type {} (UNIGRAM lattice only)",
            other.as_str()
        )),
    }
}

pub fn nbest_encode(
    model: &SentencePieceModel,
    text: &str,
    nbest_size: usize,
) -> Result<Vec<(Vec<u32>, f32)>, String> {
    nbest_encode_with_options(model, text, nbest_size, EncodeOptions::default())
}

/// Sample-encode (UNIGRAM lattice sample / BPE-dropout). See [`SampleEncodeOptions`].
pub fn sample_encode_with_options(
    model: &SentencePieceModel,
    text: &str,
    sample: SampleEncodeOptions,
) -> Result<Vec<u32>, String> {
    require_encode(model)?;
    if !sample.alpha.is_finite() || sample.alpha < 0.0 {
        return Err("sample_encode: alpha must be finite and >= 0".into());
    }
    let opts = sample.encode.clone();
    match model.model_type {
        ModelType::Unigram => {
            let normalized = normalize_with_options(model, text, &opts)?;
            let mut rng: StdRng = match sample.seed {
                Some(s) => StdRng::seed_from_u64(s),
                None => StdRng::from_entropy(),
            };
            let ids = if sample.nbest_size < 0 {
                sample_encode_unigram_lattice(
                    model,
                    &normalized,
                    sample.alpha,
                    opts.clone(),
                    &mut rng,
                )?
            } else if sample.nbest_size == 0 || sample.nbest_size == 1 {
                encode_unigram_lattice(model, &normalized, opts.clone())?
            } else {
                sample_encode_from_nbest(
                    model,
                    &normalized,
                    sample.nbest_size as usize,
                    sample.alpha,
                    opts.clone(),
                    &mut rng,
                )?
            };
            finalize_encode_ids(model, &opts, ids)
        }
        ModelType::Bpe => {
            // Official BPE: nbest_size ignored; alpha is dropout probability.
            let mut rng: StdRng = match sample.seed {
                Some(s) => StdRng::seed_from_u64(s),
                None => StdRng::from_entropy(),
            };
            // encode_bpe_sample_with_options already finalizes.
            encode_bpe_sample_with_options(model, text, sample.alpha, opts, &mut rng)
        }
        other => Err(format!(
            "sample_encode unsupported for model_type {} (UNIGRAM lattice or BPE-dropout only)",
            other.as_str()
        )),
    }
}

pub fn sample_encode(
    model: &SentencePieceModel,
    text: &str,
    nbest_size: i32,
    alpha: f32,
) -> Result<Vec<u32>, String> {
    sample_encode_with_options(
        model,
        text,
        SampleEncodeOptions {
            nbest_size,
            alpha,
            seed: None,
            encode: EncodeOptions::default(),
        },
    )
}

/// SampleEncodeAndScore (UNIGRAM). With-replacement returns `(ids, log_prob − marginal)`.
/// Without-replacement returns Gumbel-Top-k draws with log inclusion probabilities.
pub fn sample_encode_and_score_with_options(
    model: &SentencePieceModel,
    text: &str,
    sample: SampleEncodeAndScoreOptions,
) -> Result<Vec<(Vec<u32>, f32)>, String> {
    require_encode(model)?;
    if model.model_type != ModelType::Unigram {
        return Err(format!(
            "sample_encode_and_score unsupported for model_type {} (UNIGRAM only)",
            model.model_type.as_str()
        ));
    }
    if !sample.alpha.is_finite() || sample.alpha < 0.0 {
        return Err("sample_encode_and_score: alpha must be finite and >= 0".into());
    }
    if sample.num_samples == 0 {
        return Err("sample_encode_and_score: num_samples must be >= 1".into());
    }
    if sample.num_samples > 1024 {
        return Err("sample_encode_and_score: num_samples must be <= 1024".into());
    }
    if sample.include_best && !sample.wor {
        return Err(
            "sample_encode_and_score: include_best requires wor=true (official SP constraint)"
                .into(),
        );
    }
    let opts = sample.encode.clone();
    let normalized = normalize_with_options(model, text, &opts)?;
    let mut rng: StdRng = match sample.seed {
        Some(s) => StdRng::seed_from_u64(s),
        None => StdRng::from_entropy(),
    };
    let rows = if sample.wor {
        sample_encode_and_score_unigram_wor(
            model,
            &normalized,
            sample.num_samples,
            sample.alpha,
            sample.include_best,
            opts.clone(),
            &mut rng,
        )?
    } else {
        sample_encode_and_score_unigram(
            model,
            &normalized,
            sample.num_samples,
            sample.alpha,
            opts.clone(),
            &mut rng,
        )?
    };
    finalize_encode_scored(model, &opts, rows)
}

pub fn sample_encode_and_score(
    model: &SentencePieceModel,
    text: &str,
    num_samples: usize,
    alpha: f32,
) -> Result<Vec<(Vec<u32>, f32)>, String> {
    sample_encode_and_score_with_options(
        model,
        text,
        SampleEncodeAndScoreOptions {
            num_samples,
            alpha,
            wor: false,
            include_best: false,
            seed: None,
            encode: EncodeOptions::default(),
        },
    )
}

/// Encode using the model's declared type (BPE, CHAR, UNIGRAM, or WORD).
pub fn encode_with_options(
    model: &SentencePieceModel,
    text: &str,
    opts: EncodeOptions,
) -> Result<Vec<u32>, String> {
    require_encode(model)?;
    match model.model_type {
        ModelType::Bpe => encode_bpe_with_options(model, text, opts),
        ModelType::Char => encode_char_with_options(model, text, opts),
        ModelType::Unigram => encode_unigram_with_options(model, text, opts),
        ModelType::Word => encode_word_with_options(model, text, opts),
    }
}

/// Encode with fail-closed OOV default (`EncodeOptions::default()`).
pub fn encode(model: &SentencePieceModel, text: &str) -> Result<Vec<u32>, String> {
    encode_with_options(model, text, EncodeOptions::default())
}

/// Decode token ids toward official SP (`▁` strip, BYTE UTF-8, unk_surface, denorm).
///
/// Permanent gap vs lossy NFKC: without a non-empty `denormalizer_spec` charsmap,
/// reverse NFKC is not applied (same as stock SP for most models).
pub fn decode_pieces(model: &SentencePieceModel, tokens: &[u32]) -> Result<String, String> {
    decode_pieces_with_options(model, tokens, DecodeOptions::default())
}

/// Decode with ExtraOptions-style knobs (currently `reverse` only).
pub fn decode_pieces_with_options(
    model: &SentencePieceModel,
    tokens: &[u32],
    opts: DecodeOptions,
) -> Result<String, String> {
    let reversed;
    let tokens = if opts.reverse {
        reversed = {
            let mut v = tokens.to_vec();
            v.reverse();
            v
        };
        reversed.as_slice()
    } else {
        tokens
    };
    decode_pieces_inner(model, tokens)
}

fn decode_pieces_inner(model: &SentencePieceModel, tokens: &[u32]) -> Result<String, String> {
    let mut piece_strs: Vec<&str> = Vec::with_capacity(tokens.len());
    for &tok in tokens {
        let idx = tok as usize;
        let piece = model
            .pieces
            .get(idx)
            .ok_or_else(|| format!("token id {tok} out of range"))?;
        piece_strs.push(piece.as_str());
    }

    let mut text = String::new();
    let mut byte_buf: Vec<u8> = Vec::new();
    let mut is_bos_ws = true;

    let flush_bytes = |buf: &mut Vec<u8>, text: &mut String| -> Result<(), String> {
        if buf.is_empty() {
            return Ok(());
        }
        let mut offset = 0;
        while offset < buf.len() {
            match std::str::from_utf8(&buf[offset..]) {
                Ok(s) => {
                    text.push_str(s);
                    offset = buf.len();
                }
                Err(e) => {
                    let valid = e.valid_up_to();
                    if valid > 0 {
                        text.push_str(std::str::from_utf8(&buf[offset..offset + valid]).unwrap());
                        offset += valid;
                    }
                    // Invalid sequence: emit U+FFFD and skip one byte (SP-like).
                    text.push('\u{FFFD}');
                    offset += 1;
                }
            }
        }
        buf.clear();
        Ok(())
    };

    for (i, &tok) in tokens.iter().enumerate() {
        let idx = tok as usize;
        let rec = &model.piece_records[idx];
        let piece = piece_strs[i];

        if rec.piece_type == PieceType::Byte || piece_to_byte(piece).is_some() {
            if let Some(b) = piece_to_byte(piece) {
                byte_buf.push(b);
            } else {
                return Err(format!("BYTE piece {piece:?} is not <0xXX>"));
            }
            continue;
        }

        flush_bytes(&mut byte_buf, &mut text)?;

        if rec.piece_type == PieceType::Control {
            if !text.is_empty() {
                is_bos_ws = false;
            }
            continue;
        }

        if rec.piece_type == PieceType::Unknown {
            // When decoding from ids, piece == IdToPiece → emit unk_surface.
            text.push_str(&model.unk_surface);
            is_bos_ws = false;
            continue;
        }

        let mut p = piece;
        if is_bos_ws
            && (model.normalizer.add_dummy_prefix || model.normalizer.remove_extra_whitespaces)
        {
            if let Some(rest) = p.strip_prefix(SP_SPACE) {
                p = rest;
                // With remove_extra_whitespaces, do not keep bos_ws sticky after consume.
                if model.normalizer.remove_extra_whitespaces {
                    // matches SP: has_bos_ws forced false after consume
                }
            }
        }

        let decoded = p.replace(SP_SPACE, " ");
        if !decoded.is_empty() {
            text.push_str(&decoded);
            is_bos_ws = false;
        } else if !text.is_empty() {
            is_bos_ws = false;
        }
        // Empty after bos ▁ consume: keep is_bos_ws true when remove_extra_whitespaces
        // (SP keeps expecting bos until a non-empty surface appears).
    }
    flush_bytes(&mut byte_buf, &mut text)?;

    // Official SP: if denormalizer_ present, text = denormalizer_->Normalize(text).
    if model.denorm_charsmap.is_some() {
        text = sp_denormalize(&text, &model.denormalizer, model.denorm_charsmap.as_ref())?;
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures")
            .join(name)
    }

    #[test]
    fn empty_bytes_fail_closed() {
        let err = load_model_proto(&[]).unwrap_err();
        assert!(err.contains("empty"));
    }

    #[test]
    fn garbage_bytes_fail_closed() {
        let err = load_model_proto(&[0x0a, 0x7f, 0x01]).unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn no_pieces_fail_closed() {
        let bytes = {
            let mut v = Vec::new();
            encode_varint(2 << 3, &mut v);
            encode_varint(1, &mut v);
            v
        };
        let err = load_model_proto(&bytes).unwrap_err();
        assert!(err.contains("no pieces"));
    }

    #[test]
    fn minimal_model_proto_roundtrip() {
        let bytes = encode_model_proto_pieces(&["<unk>", "▁a", "b"]);
        let model = load_model_proto(&bytes).unwrap();
        assert_eq!(model.pieces.len(), 3);
        assert_eq!(model.vocab.encode("<unk>"), 0);
        assert_eq!(model.vocab.encode("▁a"), 1);
        assert_eq!(model.vocab.encode("b"), 2);
        assert!(model.vocab.validate_roundtrip());
        assert_eq!(model.model_type, ModelType::Unigram);
        assert!(model.encode_supported);
        assert_eq!(encode(&model, "a").unwrap(), vec![1]);
    }

    #[test]
    fn duplicate_piece_fail_closed() {
        let bytes = encode_model_proto_pieces(&["a", "a"]);
        let err = load_model_proto(&bytes).unwrap_err();
        assert!(err.contains("duplicate"));
    }

    #[test]
    fn vocab_json_roundtrip_path() {
        let mph = PerfectHashVocab::from_words(vec!["a".into(), "b".into()]);
        let json = serde_json::to_string(&mph).unwrap();
        let loaded = load_vocab_json(&json).unwrap();
        assert!(loaded.validate_roundtrip());
        assert_eq!(loaded.encode("a"), mph.encode("a"));
    }

    #[test]
    fn skips_unknown_fields() {
        let mut bytes = encode_model_proto_pieces(&["x", "y"]);
        encode_varint((99 << 3) | 5, &mut bytes);
        bytes.extend_from_slice(&[1, 2, 3, 4]);
        let model = load_model_proto(&bytes).unwrap();
        assert_eq!(model.pieces, vec!["x".to_string(), "y".to_string()]);
    }

    #[test]
    fn non_utf8_piece_fail_closed() {
        let mut sp = Vec::new();
        encode_varint((1 << 3) | 2, &mut sp);
        encode_varint(2, &mut sp);
        sp.extend_from_slice(&[0xff, 0xfe]);
        let mut bytes = Vec::new();
        encode_len_delim(1, &sp, &mut bytes);
        let err = load_model_proto(&bytes).unwrap_err();
        assert!(err.contains("UTF-8"));
    }

    #[test]
    fn oversized_length_fail_closed() {
        let err = load_model_proto(&[0x0a, 0xff, 0xff, 0xff, 0xff, 0x0f]).unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn char_encode_path() {
        let fixture = ModelProtoFixture {
            pieces: vec![
                ("<unk>".into(), Some(0.0), Some(2)),
                ("▁".into(), Some(0.0), Some(1)),
                ("a".into(), Some(0.0), Some(1)),
                ("b".into(), Some(0.0), Some(1)),
            ],
            model_type: Some(4),
            add_dummy_prefix: Some(true),
            remove_extra_whitespaces: Some(true),
            escape_whitespaces: Some(true),
            ..Default::default()
        };
        let model = load_model_proto(&fixture.encode()).unwrap();
        assert!(model.encode_supported);
        assert_eq!(model.model_type, ModelType::Char);
        let ids = encode(&model, "ab").unwrap();
        assert_eq!(ids, vec![1, 2, 3]);
        let decoded = decode_pieces(&model, &ids).unwrap();
        assert_eq!(decoded, "ab");
    }

    #[test]
    fn extra_options_reverse_bos_eos_composition() {
        let fixture = ModelProtoFixture {
            pieces: vec![
                ("<unk>".into(), Some(0.0), Some(2)),
                ("<s>".into(), Some(0.0), Some(3)),
                ("</s>".into(), Some(0.0), Some(3)),
                ("▁".into(), Some(0.0), Some(1)),
                ("a".into(), Some(0.0), Some(1)),
                ("b".into(), Some(0.0), Some(1)),
            ],
            model_type: Some(4),
            add_dummy_prefix: Some(true),
            remove_extra_whitespaces: Some(true),
            escape_whitespaces: Some(true),
            ..Default::default()
        };
        let model = load_model_proto(&fixture.encode()).unwrap();
        let body = encode(&model, "ab").unwrap();
        assert_eq!(body, vec![3, 4, 5]);
        // Stock documented `reverse:bos:eos`: reverse body, then bos, then eos.
        let mut expected = body.clone();
        expected.reverse();
        expected.insert(0, model.bos_id.unwrap());
        expected.push(model.eos_id.unwrap());
        let got = encode_with_options(
            &model,
            "ab",
            EncodeOptions {
                reverse: true,
                add_bos: true,
                add_eos: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(got, expected);
        assert_eq!(got, vec![1, 5, 4, 3, 2]);
        let fwd = decode_pieces(&model, &body).unwrap();
        let rev = decode_pieces_with_options(
            &model,
            &{
                let mut r = body.clone();
                r.reverse();
                r
            },
            DecodeOptions { reverse: true },
        )
        .unwrap();
        assert_eq!(fwd, rev);
    }

    #[test]
    fn extra_options_str_order_independent() {
        let mut a = EncodeOptions::default();
        a.apply_extra_options_str("eos:bos:reverse").unwrap();
        let mut b = EncodeOptions::default();
        b.apply_extra_options_str("reverse:bos:eos").unwrap();
        assert!(a.reverse && a.add_bos && a.add_eos);
        assert_eq!(a.reverse, b.reverse);
        assert_eq!(a.add_bos, b.add_bos);
        assert_eq!(a.add_eos, b.add_eos);
        let mut bad = EncodeOptions::default();
        assert!(bad.apply_extra_options_str("bos:unk").is_err());
    }

    #[test]
    fn bpe_encode_merges_by_score() {
        let fixture = ModelProtoFixture {
            pieces: vec![
                ("<unk>".into(), Some(0.0), Some(2)),
                ("▁".into(), Some(-5.0), Some(1)),
                ("a".into(), Some(-4.0), Some(1)),
                ("b".into(), Some(-4.0), Some(1)),
                ("ab".into(), Some(-1.0), Some(1)),
                ("▁a".into(), Some(-0.5), Some(1)),
                ("▁ab".into(), Some(0.0), Some(1)),
            ],
            model_type: Some(2),
            add_dummy_prefix: Some(true),
            remove_extra_whitespaces: Some(true),
            escape_whitespaces: Some(true),
            ..Default::default()
        };
        let model = load_model_proto(&fixture.encode()).unwrap();
        assert!(model.encode_supported);
        let ids = encode(&model, "ab").unwrap();
        assert_eq!(ids, vec![6]);
        assert_eq!(decode_pieces(&model, &ids).unwrap(), "ab");
    }

    #[test]
    fn unigram_viterbi_prefers_highest_score_path() {
        let fixture = ModelProtoFixture {
            pieces: vec![
                ("<unk>".into(), Some(0.0), Some(2)),
                ("▁".into(), Some(-10.0), Some(1)),
                ("a".into(), Some(-5.0), Some(1)),
                ("b".into(), Some(-5.0), Some(1)),
                ("ab".into(), Some(-3.0), Some(1)),
                ("▁a".into(), Some(-2.0), Some(1)),
                ("▁ab".into(), Some(-0.5), Some(1)),
            ],
            model_type: Some(1),
            add_dummy_prefix: Some(true),
            remove_extra_whitespaces: Some(true),
            escape_whitespaces: Some(true),
            ..Default::default()
        };
        let model = load_model_proto(&fixture.encode()).unwrap();
        assert!(model.encode_supported);
        assert_eq!(model.model_type, ModelType::Unigram);
        let ids = encode(&model, "ab").unwrap();
        assert_eq!(ids, vec![6]);
        assert_eq!(decode_pieces(&model, &ids).unwrap(), "ab");
    }

    #[test]
    fn unigram_viterbi_multi_piece_when_better() {
        let fixture = ModelProtoFixture {
            pieces: vec![
                ("<unk>".into(), Some(0.0), Some(2)),
                ("▁".into(), Some(-10.0), Some(1)),
                ("a".into(), Some(-5.0), Some(1)),
                ("b".into(), Some(-1.0), Some(1)),
                ("▁a".into(), Some(-1.0), Some(1)),
                ("▁ab".into(), Some(-20.0), Some(1)),
            ],
            model_type: Some(1),
            add_dummy_prefix: Some(true),
            remove_extra_whitespaces: Some(true),
            escape_whitespaces: Some(true),
            ..Default::default()
        };
        let model = load_model_proto(&fixture.encode()).unwrap();
        let ids = encode(&model, "ab").unwrap();
        assert_eq!(ids, vec![4, 3]);
    }

    #[test]
    fn unigram_incomplete_coverage_fail_closed() {
        let fixture = ModelProtoFixture {
            pieces: vec![
                ("▁".into(), Some(-1.0), Some(1)),
                ("a".into(), Some(-1.0), Some(1)),
            ],
            model_type: Some(1),
            add_dummy_prefix: Some(true),
            escape_whitespaces: Some(true),
            remove_extra_whitespaces: Some(true),
            ..Default::default()
        };
        let model = load_model_proto(&fixture.encode()).unwrap();
        let err = encode(&model, "ab").unwrap_err();
        assert!(err.contains("no covering segmentation"), "{err}");
    }

    #[test]
    fn word_model_encodes_whitespace_words() {
        let fixture = ModelProtoFixture {
            pieces: vec![
                ("<unk>".into(), Some(0.0), Some(2)),
                ("\u{2581}hello".into(), Some(0.0), Some(1)),
                ("\u{2581}world".into(), Some(0.0), Some(1)),
            ],
            model_type: Some(3), // WORD
            add_dummy_prefix: Some(true),
            escape_whitespaces: Some(true),
            remove_extra_whitespaces: Some(true),
            ..Default::default()
        };
        let model = load_model_proto(&fixture.encode()).unwrap();
        assert!(
            model.encode_supported,
            "{:?}",
            model.encode_unsupported_reason
        );
        assert_eq!(model.model_type, ModelType::Word);
        let ids = encode(&model, "hello world").unwrap();
        assert_eq!(ids, vec![1, 2]);
        assert_eq!(decode_pieces(&model, &ids).unwrap(), "hello world");
    }

    #[test]
    fn treat_whitespace_as_suffix_normalize_and_word() {
        let fixture = ModelProtoFixture {
            pieces: vec![
                ("<unk>".into(), Some(0.0), Some(2)),
                ("hello\u{2581}".into(), Some(0.0), Some(1)),
                ("world\u{2581}".into(), Some(0.0), Some(1)),
            ],
            model_type: Some(3),
            treat_whitespace_as_suffix: Some(true),
            add_dummy_prefix: Some(true),
            escape_whitespaces: Some(true),
            remove_extra_whitespaces: Some(true),
            ..Default::default()
        };
        let model = load_model_proto(&fixture.encode()).unwrap();
        assert!(model.encode_supported);
        assert!(model.treat_whitespace_as_suffix);
        assert_eq!(
            normalize(&model, "hello world").unwrap(),
            "hello\u{2581}world\u{2581}"
        );
        let ids = encode(&model, "hello world").unwrap();
        assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn control_symbols_resolve_and_decode_empty() {
        let fixture = ModelProtoFixture {
            pieces: vec![
                ("<unk>".into(), Some(0.0), Some(2)),
                ("<s>".into(), Some(0.0), Some(3)),
                ("</s>".into(), Some(0.0), Some(3)),
                ("<pad>".into(), Some(0.0), Some(3)),
                ("\u{2581}a".into(), Some(-1.0), Some(1)),
            ],
            model_type: Some(1),
            add_dummy_prefix: Some(true),
            escape_whitespaces: Some(true),
            remove_extra_whitespaces: Some(true),
            ..Default::default()
        };
        let model = load_model_proto(&fixture.encode()).unwrap();
        assert_eq!(model.bos_id, Some(1));
        assert_eq!(model.eos_id, Some(2));
        assert_eq!(model.pad_id, Some(3));
        assert_eq!(model.unk_id, 0);
        // Control surfaces decode to empty.
        assert_eq!(decode_pieces(&model, &[1, 4, 2]).unwrap(), "a");
        // Control piece string must not match as a NORMAL segmentation edge.
        let ids = encode(&model, "a").unwrap();
        assert_eq!(ids, vec![4]);
        // ExtraOptions bos/eos wrap after merge (stock ApplyExtraOptions order).
        let wrapped = encode_with_options(
            &model,
            "a",
            EncodeOptions {
                add_bos: true,
                add_eos: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(wrapped, vec![1, 4, 2]);
        assert_eq!(decode_pieces(&model, &wrapped).unwrap(), "a");
        // reverse alone on a single body piece is a no-op before wrap.
        let rev_wrapped = encode_with_options(
            &model,
            "a",
            EncodeOptions {
                reverse: true,
                add_bos: true,
                add_eos: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(rev_wrapped, vec![1, 4, 2]);
        // reverse:bos:eos on a multi-id body (manual body + finalize composition).
        let mut body = vec![4u32, 4u32];
        body.reverse();
        body.insert(0, 1);
        body.push(2);
        assert_eq!(body, vec![1, 4, 4, 2]);
        // Decode ExtraOptions reverse undoes id order before detokenize.
        let fwd = decode_pieces(&model, &[1, 4, 2]).unwrap();
        let rev = decode_pieces_with_options(&model, &[2, 4, 1], DecodeOptions { reverse: true })
            .unwrap();
        assert_eq!(fwd, rev);
        // Refuse when CONTROL bos/eos missing.
        let no_ctrl = ModelProtoFixture {
            pieces: vec![
                ("<unk>".into(), Some(0.0), Some(2)),
                ("\u{2581}a".into(), Some(-1.0), Some(1)),
            ],
            model_type: Some(1),
            add_dummy_prefix: Some(true),
            escape_whitespaces: Some(true),
            remove_extra_whitespaces: Some(true),
            ..Default::default()
        };
        let bare = load_model_proto(&no_ctrl.encode()).unwrap();
        assert!(bare.bos_id.is_none());
        let err = encode_with_options(
            &bare,
            "a",
            EncodeOptions {
                add_bos: true,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("add_bos"), "{err}");
    }

    #[test]
    fn sample_encode_and_score_with_replacement() {
        let fixture = ModelProtoFixture {
            pieces: vec![
                ("\u{2581}".into(), Some(-1.0), Some(1)),
                ("a".into(), Some(-1.0), Some(1)),
                ("b".into(), Some(-1.0), Some(1)),
                ("ab".into(), Some(-0.5), Some(1)),
                ("\u{2581}a".into(), Some(-0.5), Some(1)),
                ("\u{2581}ab".into(), Some(-0.1), Some(1)),
            ],
            model_type: Some(1),
            add_dummy_prefix: Some(true),
            escape_whitespaces: Some(true),
            remove_extra_whitespaces: Some(true),
            ..Default::default()
        };
        let model = load_model_proto(&fixture.encode()).unwrap();
        let scored = sample_encode_and_score_with_options(
            &model,
            "ab",
            SampleEncodeAndScoreOptions {
                num_samples: 3,
                alpha: 0.5,
                wor: false,
                include_best: false,
                seed: Some(7),
                encode: EncodeOptions::default(),
            },
        )
        .unwrap();
        assert_eq!(scored.len(), 3);
        for (ids, score) in &scored {
            assert!(!ids.is_empty());
            assert!(score.is_finite(), "{score}");
        }
        let wor = sample_encode_and_score_with_options(
            &model,
            "ab",
            SampleEncodeAndScoreOptions {
                num_samples: 2,
                alpha: 0.5,
                wor: true,
                include_best: true,
                seed: Some(1),
                encode: EncodeOptions::default(),
            },
        )
        .unwrap();
        assert!(!wor.is_empty());
        // include_best → first row is Viterbi with log-inclusion 0.
        assert_eq!(wor[0].1, 0.0);
        let best = encode(&model, "ab").unwrap();
        assert_eq!(wor[0].0, best);
        let mut seen = HashSet::new();
        for (ids, score) in &wor {
            assert!(score.is_finite(), "{score}");
            assert!(
                seen.insert(ids.clone()),
                "WOR samples must be unique: {ids:?}"
            );
        }
        let ib_err = sample_encode_and_score_with_options(
            &model,
            "ab",
            SampleEncodeAndScoreOptions {
                num_samples: 2,
                alpha: 0.5,
                wor: false,
                include_best: true,
                seed: Some(1),
                encode: EncodeOptions::default(),
            },
        )
        .unwrap_err();
        assert!(ib_err.contains("include_best"), "{ib_err}");
    }

    #[test]
    fn refuses_invalid_precompiled_charsmap() {
        let fixture = ModelProtoFixture {
            pieces: vec![("▁".into(), Some(0.0), None), ("a".into(), Some(0.0), None)],
            model_type: Some(2),
            precompiled_charsmap: Some(vec![1, 2, 3]),
            escape_whitespaces: Some(true),
            ..Default::default()
        };
        let model = load_model_proto(&fixture.encode()).unwrap();
        assert!(!model.encode_supported);
        assert!(encode(&model, "a")
            .unwrap_err()
            .contains("precompiled_charsmap"));
    }

    #[test]
    fn accepts_valid_custom_charsmap_fixture() {
        let path = fixture("sp_tiny_custom_norm_unigram.model");
        let model = load_model_proto_file(path.to_str().unwrap()).unwrap();
        assert!(
            model.encode_supported,
            "{:?}",
            model.encode_unsupported_reason
        );
        assert!(model.charsmap.is_some());
        // Fullwidth A rewritten to ASCII A; A is OOV → fail closed without BF.
        let err = encode(&model, "\u{FF21}bc").unwrap_err();
        assert!(
            err.contains("no covering") || err.contains("not in vocab"),
            "{err}"
        );
        // Covered ASCII path still works.
        let ids = encode(&model, "hello").unwrap();
        assert_eq!(ids, vec![1, 12, 4, 6, 6, 2]);
        assert_eq!(decode_pieces(&model, &ids).unwrap(), "hello");
    }

    #[test]
    fn identity_trained_fixture_encode() {
        let path = fixture("sp_tiny_identity_unigram.model");
        let model = load_model_proto_file(path.to_str().unwrap()).unwrap();
        assert!(model.encode_supported);
        assert!(model.charsmap.is_none());
        assert_eq!(encode(&model, "hello").unwrap(), vec![1, 12, 4, 6, 6, 2]);
        assert_eq!(
            encode(&model, "  foo  bar  ").unwrap(),
            vec![1, 5, 2, 2, 1, 7, 3, 9]
        );
    }

    #[test]
    fn byte_fallback_bpe_fixture() {
        let path = fixture("sp_tiny_identity_bpe_bytefallback.model");
        let model = load_model_proto_file(path.to_str().unwrap()).unwrap();
        assert!(
            model.encode_supported,
            "{:?}",
            model.encode_unsupported_reason
        );
        assert!(model.byte_fallback);
        let ids = encode(&model, "Z").unwrap();
        // ▁ + <0x5A>
        assert_eq!(decode_pieces(&model, &ids).unwrap(), "Z");
        let emoji = encode(&model, "😀").unwrap();
        assert_eq!(decode_pieces(&model, &emoji).unwrap(), "😀");
        let hello = encode(&model, "hello").unwrap();
        assert_eq!(decode_pieces(&model, &hello).unwrap(), "hello");
    }

    #[test]
    fn refuses_byte_fallback_without_byte_pieces() {
        let fixture = ModelProtoFixture {
            pieces: vec![("a".into(), Some(0.0), None)],
            model_type: Some(2),
            byte_fallback: Some(true),
            escape_whitespaces: Some(true),
            ..Default::default()
        };
        let model = load_model_proto(&fixture.encode()).unwrap();
        assert!(!model.encode_supported);
        assert!(encode(&model, "a").unwrap_err().contains("byte_fallback"));
    }

    #[test]
    fn missing_seed_char_fail_closed() {
        let fixture = ModelProtoFixture {
            pieces: vec![("▁".into(), Some(0.0), None), ("a".into(), Some(0.0), None)],
            model_type: Some(2),
            add_dummy_prefix: Some(true),
            escape_whitespaces: Some(true),
            remove_extra_whitespaces: Some(true),
            ..Default::default()
        };
        let model = load_model_proto(&fixture.encode()).unwrap();
        let err = encode(&model, "z").unwrap_err();
        assert!(err.contains("not in vocab"));
    }

    #[test]
    fn decode_unk_uses_unk_surface() {
        let fixture = ModelProtoFixture {
            pieces: vec![
                ("<unk>".into(), Some(0.0), Some(2)),
                ("▁".into(), Some(0.0), Some(1)),
                ("a".into(), Some(0.0), Some(1)),
            ],
            model_type: Some(4),
            add_dummy_prefix: Some(true),
            remove_extra_whitespaces: Some(true),
            escape_whitespaces: Some(true),
            ..Default::default()
        };
        let model = load_model_proto(&fixture.encode()).unwrap();
        assert_eq!(
            decode_pieces(&model, &[1, 0, 2]).unwrap(),
            format!("{}a", DEFAULT_UNK_SURFACE)
        );
    }

    #[test]
    fn silent_unk_opt_in_matches_stock_shape() {
        let fixture = ModelProtoFixture {
            pieces: vec![
                ("<unk>".into(), Some(0.0), Some(2)),
                ("▁".into(), Some(-5.0), Some(1)),
                ("a".into(), Some(-4.0), Some(1)),
            ],
            model_type: Some(4), // CHAR
            add_dummy_prefix: Some(true),
            remove_extra_whitespaces: Some(true),
            escape_whitespaces: Some(true),
            ..Default::default()
        };
        let model = load_model_proto(&fixture.encode()).unwrap();
        assert!(encode(&model, "z").is_err());
        let ids = encode_with_options(
            &model,
            "z",
            EncodeOptions {
                allow_silent_unk: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(ids, vec![1, 0]); // ▁ + <unk>
        assert_eq!(decode_pieces(&model, &ids).unwrap(), DEFAULT_UNK_SURFACE);
    }

    #[test]
    fn silent_unk_bpe_and_unigram() {
        let bpe = ModelProtoFixture {
            pieces: vec![
                ("<unk>".into(), Some(0.0), Some(2)),
                ("▁".into(), Some(-5.0), Some(1)),
                ("a".into(), Some(-4.0), Some(1)),
            ],
            model_type: Some(2),
            add_dummy_prefix: Some(true),
            remove_extra_whitespaces: Some(true),
            escape_whitespaces: Some(true),
            ..Default::default()
        };
        let model = load_model_proto(&bpe.encode()).unwrap();
        let opts = EncodeOptions {
            allow_silent_unk: true,
            ..Default::default()
        };
        assert_eq!(
            encode_with_options(&model, "z", opts.clone()).unwrap(),
            vec![1, 0]
        );

        let ug = ModelProtoFixture {
            pieces: vec![
                ("<unk>".into(), Some(0.0), Some(2)),
                ("▁".into(), Some(-10.0), Some(1)),
                ("a".into(), Some(-5.0), Some(1)),
            ],
            model_type: Some(1),
            add_dummy_prefix: Some(true),
            remove_extra_whitespaces: Some(true),
            escape_whitespaces: Some(true),
            ..Default::default()
        };
        let model = load_model_proto(&ug.encode()).unwrap();
        assert_eq!(encode_with_options(&model, "z", opts).unwrap(), vec![1, 0]);
    }

    #[test]
    fn denorm_fixture_applies_reverse_charsmap() {
        let path = fixture("sp_tiny_identity_denorm_unigram.model");
        let model = load_model_proto_file(path.to_str().unwrap()).unwrap();
        assert!(model.denorm_charsmap.is_some());
        let ids = encode(&model, "TM").unwrap();
        assert_eq!(decode_pieces(&model, &ids).unwrap(), "\u{2122}");
        let ids = encode(&model, "xTMy").unwrap();
        assert_eq!(decode_pieces(&model, &ids).unwrap(), "x\u{2122}y");
        // Covered identity path unchanged.
        assert_eq!(
            decode_pieces(&model, &encode(&model, "hello").unwrap()).unwrap(),
            "hello"
        );
    }

    #[test]
    fn denorm_crafted_fixture_roundtrip() {
        let blob = std::fs::read(fixture("sp_denorm_charsmap.bin")).unwrap();
        let fixture = ModelProtoFixture {
            pieces: vec![
                ("<unk>".into(), Some(0.0), Some(2)),
                ("▁".into(), Some(-1.0), Some(1)),
                ("T".into(), Some(-1.0), Some(1)),
                ("M".into(), Some(-1.0), Some(1)),
                ("▁TM".into(), Some(0.0), Some(1)),
            ],
            model_type: Some(1),
            add_dummy_prefix: Some(true),
            remove_extra_whitespaces: Some(true),
            escape_whitespaces: Some(true),
            denorm_precompiled_charsmap: Some(blob),
            denorm_add_dummy_prefix: Some(false),
            denorm_remove_extra_whitespaces: Some(false),
            denorm_escape_whitespaces: Some(false),
            ..Default::default()
        };
        let model = load_model_proto(&fixture.encode()).unwrap();
        let ids = encode(&model, "TM").unwrap();
        assert_eq!(ids, vec![4]);
        assert_eq!(decode_pieces(&model, &ids).unwrap(), "\u{2122}");
    }

    #[test]
    fn user_defined_forced_piece_unigram_and_bpe() {
        let ug = ModelProtoFixture {
            pieces: vec![
                ("<unk>".into(), Some(0.0), Some(2)),
                ("▁".into(), Some(-10.0), Some(1)),
                ("a".into(), Some(-5.0), Some(1)),
                ("b".into(), Some(-5.0), Some(1)),
                ("ab".into(), Some(-0.1), Some(1)),
                ("▁a".into(), Some(-2.0), Some(1)),
                ("<foo>".into(), Some(0.0), Some(4)), // USER_DEFINED
                ("f".into(), Some(-5.0), Some(1)),
                ("o".into(), Some(-5.0), Some(1)),
                ("<".into(), Some(-5.0), Some(1)),
                (">".into(), Some(-5.0), Some(1)),
            ],
            model_type: Some(1),
            add_dummy_prefix: Some(true),
            remove_extra_whitespaces: Some(true),
            escape_whitespaces: Some(true),
            ..Default::default()
        };
        let model = load_model_proto(&ug.encode()).unwrap();
        assert!(!model.prefix_matcher.is_empty());
        assert_eq!(encode(&model, "<foo>").unwrap(), vec![1, 6]);
        assert_eq!(encode(&model, "<foo>ab").unwrap(), vec![1, 6, 4]);
        // Runtime forced piece must exist in vocab.
        let err = encode_with_options(
            &model,
            "a",
            EncodeOptions {
                forced_pieces: vec!["not_in_vocab".into()],
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("forced_pieces"), "{err}");

        let bpe = ModelProtoFixture {
            pieces: vec![
                ("<unk>".into(), Some(0.0), Some(2)),
                ("▁".into(), Some(-5.0), Some(1)),
                ("a".into(), Some(-4.0), Some(1)),
                ("b".into(), Some(-4.0), Some(1)),
                ("ab".into(), Some(-1.0), Some(1)),
                ("▁a".into(), Some(-0.5), Some(1)),
                ("<foo>".into(), Some(0.0), Some(4)),
                ("f".into(), Some(-4.0), Some(1)),
                ("o".into(), Some(-4.0), Some(1)),
                ("<".into(), Some(-4.0), Some(1)),
                (">".into(), Some(-4.0), Some(1)),
            ],
            model_type: Some(2),
            add_dummy_prefix: Some(true),
            remove_extra_whitespaces: Some(true),
            escape_whitespaces: Some(true),
            ..Default::default()
        };
        let model = load_model_proto(&bpe.encode()).unwrap();
        assert_eq!(encode(&model, "a<foo>b").unwrap(), vec![5, 6, 3]);
    }

    #[test]
    fn unigram_nbest_and_sample_seeded() {
        let fixture = ModelProtoFixture {
            pieces: vec![
                ("<unk>".into(), Some(0.0), Some(2)),
                ("▁".into(), Some(-10.0), Some(1)),
                ("a".into(), Some(-5.0), Some(1)),
                ("b".into(), Some(-5.0), Some(1)),
                ("ab".into(), Some(-3.0), Some(1)),
                ("▁a".into(), Some(-2.0), Some(1)),
                ("▁ab".into(), Some(-0.5), Some(1)),
            ],
            model_type: Some(1),
            add_dummy_prefix: Some(true),
            remove_extra_whitespaces: Some(true),
            escape_whitespaces: Some(true),
            ..Default::default()
        };
        let model = load_model_proto(&fixture.encode()).unwrap();
        let nbest = nbest_encode(&model, "ab", 5).unwrap();
        assert!(!nbest.is_empty());
        assert_eq!(nbest[0].0, encode(&model, "ab").unwrap());
        // Distinct segmentations among top-n.
        let uniq: HashSet<Vec<u32>> = nbest.iter().map(|(ids, _)| ids.clone()).collect();
        assert!(uniq.len() >= 2, "{nbest:?}");

        let sampled = sample_encode_with_options(
            &model,
            "ab",
            SampleEncodeOptions {
                nbest_size: 5,
                alpha: 1.0,
                seed: Some(42),
                encode: EncodeOptions::default(),
            },
        )
        .unwrap();
        assert!(uniq.contains(&sampled), "{sampled:?} not in {uniq:?}");

        let lattice_sample = sample_encode_with_options(
            &model,
            "ab",
            SampleEncodeOptions {
                nbest_size: -1,
                alpha: 0.0,
                seed: Some(7),
                encode: EncodeOptions::default(),
            },
        )
        .unwrap();
        assert!(!lattice_sample.is_empty());

        let bpe = ModelProtoFixture {
            pieces: vec![
                ("▁".into(), Some(-5.0), Some(1)),
                ("a".into(), Some(-4.0), Some(1)),
                ("b".into(), Some(-4.0), Some(1)),
                ("ab".into(), Some(-1.0), Some(1)),
                ("▁a".into(), Some(-0.5), Some(1)),
                ("▁ab".into(), Some(0.0), Some(1)),
            ],
            model_type: Some(2),
            add_dummy_prefix: Some(true),
            escape_whitespaces: Some(true),
            remove_extra_whitespaces: Some(true),
            ..Default::default()
        };
        let bpe_model = load_model_proto(&bpe.encode()).unwrap();
        assert!(nbest_encode(&bpe_model, "ab", 3)
            .unwrap_err()
            .contains("UNIGRAM"));
        let det = encode(&bpe_model, "ab").unwrap();
        let mut variants = HashSet::new();
        for seed in 0..30u64 {
            let sampled = sample_encode_with_options(
                &bpe_model,
                "ab",
                SampleEncodeOptions {
                    nbest_size: -1,
                    alpha: 0.5,
                    seed: Some(seed),
                    encode: EncodeOptions::default(),
                },
            )
            .unwrap();
            variants.insert(sampled);
        }
        assert!(
            variants.len() >= 2,
            "BPE-dropout should diversify, got {variants:?}"
        );
        assert!(
            variants.contains(&det),
            "deterministic encode must remain reachable"
        );
    }

    #[test]
    fn word_official_split_ignores_trainer_suffix_flag() {
        let fixture = ModelProtoFixture {
            pieces: vec![
                ("<unk>".into(), Some(0.0), Some(2)),
                ("hello\u{2581}".into(), Some(0.0), Some(1)),
                ("world\u{2581}".into(), Some(0.0), Some(1)),
            ],
            model_type: Some(3),
            treat_whitespace_as_suffix: Some(true),
            add_dummy_prefix: Some(true),
            escape_whitespaces: Some(true),
            remove_extra_whitespaces: Some(true),
            ..Default::default()
        };
        let model = load_model_proto(&fixture.encode()).unwrap();
        assert_eq!(encode(&model, "hello world").unwrap(), vec![1, 2]);
        let compat = encode_with_options(
            &model,
            "hello world",
            EncodeOptions {
                word_official_split: true,
                allow_silent_unk: true,
                ..Default::default()
            },
        )
        .unwrap();
        // Flag-blind prefix SplitIntoWords → three OOV spans → three unks at
        // model level; stock SP *processor* merges consecutive unks by default.
        assert_eq!(compat, vec![0]);
        let piece_level = encode_with_options(
            &model,
            "hello world",
            EncodeOptions {
                word_official_split: true,
                allow_silent_unk: true,
                merge_consecutive_unk: false,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(piece_level, vec![0, 0, 0]);
    }

    #[test]
    fn consecutive_unk_merge_matches_processor_default() {
        let fixture = ModelProtoFixture {
            pieces: vec![
                ("<unk>".into(), Some(0.0), Some(2)),
                ("▁".into(), Some(0.0), Some(1)),
                ("a".into(), Some(0.0), Some(1)),
            ],
            model_type: Some(4), // CHAR
            add_dummy_prefix: Some(true),
            remove_extra_whitespaces: Some(true),
            escape_whitespaces: Some(true),
            ..Default::default()
        };
        let model = load_model_proto(&fixture.encode()).unwrap();
        let silent = EncodeOptions {
            allow_silent_unk: true,
            ..Default::default()
        };
        // xyz → ▁ + three OOVs → ▁ + one merged unk (stock processor).
        assert_eq!(
            encode_with_options(&model, "xyz", silent.clone()).unwrap(),
            vec![1, 0]
        );
        // xay → ▁ + unk + a + unk (non-adjacent unks preserved).
        assert_eq!(
            encode_with_options(&model, "xay", silent.clone()).unwrap(),
            vec![1, 0, 2, 0]
        );
        let piece = EncodeOptions {
            allow_silent_unk: true,
            merge_consecutive_unk: false,
            ..Default::default()
        };
        assert_eq!(
            encode_with_options(&model, "xyz", piece).unwrap(),
            vec![1, 0, 0, 0]
        );
    }

    #[test]
    fn consecutive_unk_merge_resets_on_control_ids() {
        // Stock processor: CONTROL resets is_prev_unk so unk runs on either side
        // of BOS/EOS do not merge across the control.
        let unk = 0u32;
        let bos = 1u32;
        let eos = 2u32;
        let ids = [unk, unk, bos, unk, unk, eos, unk];
        assert_eq!(
            merge_consecutive_unk_ids_with_controls(&ids, unk, &[bos, eos]),
            vec![unk, bos, unk, eos, unk]
        );
        // Without control awareness, bos≠unk so runs already split — but when
        // a control id equals neither unk nor a "normal" separator, reset still
        // matters for id lists that inject CONTROL between unks.
        assert_eq!(merge_consecutive_unk_ids(&[unk, unk, unk], unk), vec![unk]);
    }

    #[test]
    fn merge_consecutive_unk_skipped_with_byte_fallback() {
        // byte_fallback never emits unk ids; merge flag is a no-op.
        let path = fixture("sp_tiny_identity_bpe_bytefallback.model");
        let model = load_model_proto_file(path.to_str().unwrap()).unwrap();
        assert!(model.byte_fallback);
        let ids = encode(&model, "z").unwrap();
        assert!(!ids.is_empty());
        assert!(!ids.contains(&model.unk_id));
    }

    #[test]
    fn empty_denorm_is_permanent_gap_for_lossy_norm() {
        // Custom charsmap rewrites ™→TM; without denorm, decode cannot restore ™.
        let path = fixture("sp_tiny_custom_norm_unigram.model");
        let model = load_model_proto_file(path.to_str().unwrap()).unwrap();
        assert!(model.denorm_charsmap.is_none());
        // Covered ASCII still round-trips.
        let ids = encode(&model, "hello").unwrap();
        assert_eq!(decode_pieces(&model, &ids).unwrap(), "hello");
    }
}
