//! SentencePiece `precompiled_charsmap` + whitespace normalization.
//!
//! Matches the official SP `Normalizer` algorithm in `normalizer.cc`:
//! - Decode blob: `u32 le trie_bytes_len` + Darts double-array units + `\0`-delimited
//!   replacement string pool
//! - `NormalizePrefix`: leftmost-**longest** trie match over the UTF-8 byte stream
//! - Whitespace: ASCII-space collapse / dummy prefix / `▁` escape
//!
//! This is **not** a claim of grapheme-cluster parity with HuggingFace `tokenizers`
//! (which uses a different walk); cross-checks target the `sentencepiece` package.

use std::borrow::Cow;
use std::collections::HashMap;

const SP_SPACE_STR: &str = "\u{2581}";
const REPLACEMENT_CHAR: &str = "\u{FFFD}";
const K_MAX_TRIE_RESULTS: usize = 32;

/// Leftmost-longest prefix matcher over USER_DEFINED / forced pieces.
///
/// Matches official SP `normalizer::PrefixMatcher`: at each position, take the
/// longest dictionary entry that is a prefix; if none, consume one Unicode char.
#[derive(Debug, Clone, Default)]
pub struct PrefixMatcher {
    /// Pieces sorted by descending byte length (stable among equal lengths).
    entries: Vec<String>,
}

impl PrefixMatcher {
    /// Build from dictionary pieces. Empty strings and NULs are refused.
    pub fn from_pieces<I, S>(pieces: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut entries: Vec<String> = Vec::new();
        let mut seen = HashMap::new();
        for p in pieces {
            let s = p.into();
            if s.is_empty() {
                return Err("PrefixMatcher: empty piece not allowed".into());
            }
            if s.as_bytes().contains(&0) {
                return Err("PrefixMatcher: piece must not contain NUL".into());
            }
            if seen.insert(s.clone(), ()).is_none() {
                entries.push(s);
            }
        }
        entries.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        Ok(Self { entries })
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    /// Merge additional pieces into a new matcher (fail-closed on invalid).
    pub fn with_extra<I, S>(&self, extra: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut all: Vec<String> = self.entries.clone();
        all.extend(extra.into_iter().map(Into::into));
        Self::from_pieces(all)
    }

    /// Returns `(consume_bytes, found)`.
    pub fn prefix_match(&self, input: &str) -> (usize, bool) {
        if input.is_empty() {
            return (0, false);
        }
        for e in &self.entries {
            if input.starts_with(e.as_str()) {
                return (e.len(), true);
            }
        }
        let n = input
            .chars()
            .next()
            .map(|c| c.len_utf8())
            .unwrap_or(0)
            .min(input.len());
        (n, false)
    }
}

/// Lite knobs from `NormalizerSpec` (always applied after charsmap rewrite).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteNormalizer {
    pub add_dummy_prefix: bool,
    pub remove_extra_whitespaces: bool,
    pub escape_whitespaces: bool,
}

impl Default for LiteNormalizer {
    fn default() -> Self {
        Self {
            add_dummy_prefix: true,
            remove_extra_whitespaces: true,
            escape_whitespaces: true,
        }
    }
}

/// Parsed `precompiled_charsmap` (Darts double-array + replacement pool).
#[derive(Debug, Clone)]
pub struct PrecompiledCharsMap {
    trie: Vec<u32>,
    normalized: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
struct ArrayUnit(u32);

impl ArrayUnit {
    fn has_leaf(self) -> bool {
        ((self.0 >> 8) & 1) == 1
    }

    fn value(self) -> u32 {
        self.0 & ((1u32 << 31) - 1)
    }

    fn label(self) -> u32 {
        self.0 & ((1u32 << 31) | 0xFF)
    }

    fn offset(self) -> usize {
        let shifted = (self.0 >> 10) << ((self.0 & (1u32 << 9)) >> 6);
        shifted as usize
    }
}

impl PrecompiledCharsMap {
    /// Decode and validate an official SP charsmap blob.
    pub fn parse(blob: &[u8]) -> Result<Self, String> {
        if blob.len() < 4 {
            return Err("precompiled_charsmap: truncated header".into());
        }
        let trie_blob_size = u32::from_le_bytes(blob[0..4].try_into().unwrap()) as usize;
        if trie_blob_size >= blob.len().saturating_sub(4) {
            return Err("precompiled_charsmap: trie size exceeds blob".into());
        }
        // Official SP: unit_size=4 and size must be a multiple of 1024.
        if trie_blob_size < 1024 || (trie_blob_size & 0x3FF) != 0 {
            return Err(
                "precompiled_charsmap: trie size must be >= 1024 and divisible by 1024".into(),
            );
        }
        if trie_blob_size % 4 != 0 {
            return Err("precompiled_charsmap: trie size not divisible by 4".into());
        }
        let trie_bytes = &blob[4..4 + trie_blob_size];
        let normalized = blob[4 + trie_blob_size..].to_vec();
        if normalized.is_empty() || *normalized.last().unwrap() != 0 {
            return Err("precompiled_charsmap: normalized pool must be NUL-terminated".into());
        }
        let mut trie = Vec::with_capacity(trie_blob_size / 4);
        for chunk in trie_bytes.chunks_exact(4) {
            trie.push(u32::from_le_bytes(chunk.try_into().unwrap()));
        }
        let map = Self { trie, normalized };
        map.validate()?;
        Ok(map)
    }

    fn validate(&self) -> Result<(), String> {
        if self.trie.is_empty() {
            return Err("precompiled_charsmap: empty trie".into());
        }
        // Soft bounds check: every leaf value must index into the pool.
        for &unit in &self.trie {
            let u = ArrayUnit(unit);
            if u.has_leaf() {
                // value is stored at the node reached after offset jump; we cannot
                // cheaply enumerate leaves without walking. Skip deep validate —
                // corrupt maps fail closed at normalize time via bounds checks.
                let _ = u.value();
            }
        }
        Ok(())
    }

    /// Darts `commonPrefixSearch`: `(replacement_offset, match_byte_len)` pairs.
    fn common_prefix_search(&self, key: &[u8]) -> Vec<(u32, usize)> {
        let mut results = Vec::new();
        if self.trie.is_empty() || key.is_empty() {
            return results;
        }
        let mut node_pos: usize = 0;
        let mut unit = ArrayUnit(self.trie[node_pos]);
        node_pos ^= unit.offset();
        for (i, &c) in key.iter().enumerate() {
            if c == 0 {
                break;
            }
            if node_pos >= self.trie.len() {
                break;
            }
            node_pos ^= c as usize;
            if node_pos >= self.trie.len() {
                break;
            }
            unit = ArrayUnit(self.trie[node_pos]);
            if unit.label() != u32::from(c) {
                return results;
            }
            node_pos ^= unit.offset();
            if unit.has_leaf() {
                if node_pos >= self.trie.len() {
                    break;
                }
                let value = ArrayUnit(self.trie[node_pos]).value();
                results.push((value, i + 1));
                if results.len() >= K_MAX_TRIE_RESULTS {
                    break;
                }
            }
        }
        results
    }

    fn replacement_at(&self, offset: u32) -> Result<&str, String> {
        let start = offset as usize;
        if start >= self.normalized.len() {
            return Err("precompiled_charsmap: replacement offset out of range".into());
        }
        let rest = &self.normalized[start..];
        let end = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| "precompiled_charsmap: replacement missing NUL".to_string())?;
        std::str::from_utf8(&rest[..end])
            .map_err(|_| "precompiled_charsmap: replacement is not UTF-8".into())
    }
}

/// One UTF-8 character length at the start of `input`, or `None` if empty/invalid.
fn utf8_char_len(input: &[u8]) -> Option<usize> {
    if input.is_empty() {
        return None;
    }
    let b0 = input[0];
    let want = if b0 < 0x80 {
        1
    } else if b0 & 0xE0 == 0xC0 {
        2
    } else if b0 & 0xF0 == 0xE0 {
        3
    } else if b0 & 0xF8 == 0xF0 {
        4
    } else {
        return None;
    };
    if input.len() < want {
        return None;
    }
    if std::str::from_utf8(&input[..want]).is_err() {
        return None;
    }
    Some(want)
}

/// `NormalizePrefix`: `(rewritten, consume_bytes)`.
///
/// Official order: PrefixMatcher (USER_DEFINED) wins over charsmap rewrite.
fn normalize_prefix<'a>(
    charsmap: Option<&'a PrecompiledCharsMap>,
    matcher: Option<&PrefixMatcher>,
    input: &'a [u8],
) -> Result<(Cow<'a, str>, usize), String> {
    if input.is_empty() {
        return Ok((Cow::Borrowed(""), 0));
    }

    if let Some(m) = matcher {
        if let Ok(s) = std::str::from_utf8(input) {
            let (mblen, found) = m.prefix_match(s);
            if found {
                return Ok((Cow::Borrowed(&s[..mblen]), mblen));
            }
        }
    }

    let mut longest_length = 0usize;
    let mut longest_value = 0u32;
    if let Some(map) = charsmap {
        let matches = map.common_prefix_search(input);
        for (value, length) in matches {
            if longest_length == 0 || length > longest_length {
                longest_length = length;
                longest_value = value;
            }
        }
    }

    if longest_length == 0 || longest_length > input.len() {
        match utf8_char_len(input) {
            Some(n) => {
                let s = std::str::from_utf8(&input[..n])
                    .map_err(|_| "normalize: invalid UTF-8".to_string())?;
                Ok((Cow::Borrowed(s), n))
            }
            None => {
                // Official SP: consume 1 byte, emit U+FFFD.
                Ok((Cow::Borrowed(REPLACEMENT_CHAR), 1))
            }
        }
    } else {
        let map = charsmap.expect("longest match implies charsmap");
        let repl = map.replacement_at(longest_value)?;
        Ok((Cow::Borrowed(repl), longest_length))
    }
}

/// Full SP normalize (charsmap + whitespace policy + optional PrefixMatcher).
///
/// Encode path uses `escape_whitespaces=true`. Denormalizer specs typically set
/// `escape_whitespaces=false`, `add_dummy_prefix=false`, and
/// `remove_extra_whitespaces=false` (charsmap rewrite only).
pub fn sp_normalize(
    text: &str,
    lite: &LiteNormalizer,
    charsmap: Option<&PrecompiledCharsMap>,
    treat_whitespace_as_suffix: bool,
) -> Result<String, String> {
    sp_normalize_with_matcher(text, lite, charsmap, None, treat_whitespace_as_suffix)
}

/// Like [`sp_normalize`], with USER_DEFINED / forced-piece PrefixMatcher.
pub fn sp_normalize_with_matcher(
    text: &str,
    lite: &LiteNormalizer,
    charsmap: Option<&PrecompiledCharsMap>,
    matcher: Option<&PrefixMatcher>,
    treat_whitespace_as_suffix: bool,
) -> Result<String, String> {
    let mut input = text.as_bytes();
    if input.is_empty() {
        return Ok(String::new());
    }

    // Ignore heading ASCII spaces (after charsmap rewrite of each prefix).
    if lite.remove_extra_whitespaces {
        while !input.is_empty() {
            let (sp, n) = normalize_prefix(charsmap, matcher, input)?;
            if sp.as_ref() != " " {
                break;
            }
            input = &input[n..];
        }
    }
    if input.is_empty() {
        return Ok(String::new());
    }

    let space_sym: &str = if lite.escape_whitespaces {
        SP_SPACE_STR
    } else {
        " "
    };

    let mut out = String::with_capacity(input.len().saturating_mul(2));

    // Official SP: dummy whitespace is a prefix unless treat_whitespace_as_suffix.
    if !treat_whitespace_as_suffix && lite.add_dummy_prefix {
        out.push_str(space_sym);
    }

    let mut is_prev_space = lite.remove_extra_whitespaces;
    while !input.is_empty() {
        let (sp_cow, n) = normalize_prefix(charsmap, matcher, input)?;
        let mut sp = sp_cow.as_ref();
        while is_prev_space && sp.starts_with(' ') {
            sp = &sp[1..];
        }
        if !sp.is_empty() {
            for ch in sp.chars() {
                if ch == ' ' {
                    out.push_str(space_sym);
                } else {
                    out.push(ch);
                }
            }
            is_prev_space = sp.ends_with(' ');
        }
        input = &input[n..];
        if !lite.remove_extra_whitespaces {
            is_prev_space = false;
        }
    }

    if lite.remove_extra_whitespaces {
        let trail = if lite.escape_whitespaces {
            SP_SPACE_STR
        } else {
            " "
        };
        while out.ends_with(trail) {
            out.truncate(out.len() - trail.len());
        }
    }

    // Official SP: with treat_whitespace_as_suffix, dummy whitespace is a suffix.
    if treat_whitespace_as_suffix && lite.add_dummy_prefix {
        out.push_str(space_sym);
    }

    Ok(out)
}

/// Apply denormalizer charsmap (official SP: `denormalizer_->Normalize` after piece decode).
pub fn sp_denormalize(
    text: &str,
    lite: &LiteNormalizer,
    charsmap: Option<&PrecompiledCharsMap>,
) -> Result<String, String> {
    sp_normalize(text, lite, charsmap, false)
}

/// Convert a byte to the SentencePiece byte-piece spelling `<0xXX>` (uppercase hex).
pub fn byte_to_piece(b: u8) -> String {
    format!("<0x{b:02X}>")
}

/// Parse `<0xXX>` → byte, or `None`.
pub fn piece_to_byte(piece: &str) -> Option<u8> {
    let rest = piece.strip_prefix("<0x")?.strip_suffix('>')?;
    if rest.len() != 2 {
        return None;
    }
    u8::from_str_radix(rest, 16).ok()
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
    fn parses_custom_charsmap_blob() {
        let blob = std::fs::read(fixture("sp_custom_charsmap.bin")).expect("fixture");
        let map = PrecompiledCharsMap::parse(&blob).expect("parse");
        // Fullwidth A (U+FF21) → "A"
        let ff21 = "\u{FF21}".as_bytes();
        let matches = map.common_prefix_search(ff21);
        assert!(!matches.is_empty());
        let (off, len) = matches.iter().max_by_key(|(_, l)| *l).copied().unwrap();
        assert_eq!(len, ff21.len());
        assert_eq!(map.replacement_at(off).unwrap(), "A");
    }

    #[test]
    fn normalize_applies_charsmap_and_dummy_prefix() {
        let blob = std::fs::read(fixture("sp_custom_charsmap.bin")).expect("fixture");
        let map = PrecompiledCharsMap::parse(&blob).unwrap();
        let lite = LiteNormalizer::default();
        let out = sp_normalize("\u{FF21}bc", &lite, Some(&map), false).unwrap();
        assert_eq!(out, "\u{2581}Abc");
        let tm = sp_normalize("x\u{2122}y", &lite, Some(&map), false).unwrap();
        assert_eq!(tm, "\u{2581}xTMy");
    }

    #[test]
    fn identity_normalize_matches_whitespace_policy() {
        let lite = LiteNormalizer::default();
        let out = sp_normalize("  foo  bar  ", &lite, None, false).unwrap();
        assert_eq!(out, "\u{2581}foo\u{2581}bar");
        let hello = sp_normalize("hello", &lite, None, false).unwrap();
        assert_eq!(hello, "\u{2581}hello");
    }

    #[test]
    fn treat_whitespace_as_suffix_moves_dummy_ws() {
        let lite = LiteNormalizer::default();
        let out = sp_normalize("hello world", &lite, None, true).unwrap();
        assert_eq!(out, "hello\u{2581}world\u{2581}");
        let single = sp_normalize("hello", &lite, None, true).unwrap();
        assert_eq!(single, "hello\u{2581}");
    }

    #[test]
    fn rejects_truncated_charsmap() {
        let err = PrecompiledCharsMap::parse(&[1, 2, 3]).unwrap_err();
        assert!(err.contains("truncated") || err.contains("trie"));
    }

    #[test]
    fn byte_piece_roundtrip() {
        for b in [0u8, 0x5A, 0xF0, 0xFF] {
            let p = byte_to_piece(b);
            assert_eq!(piece_to_byte(&p), Some(b));
        }
        assert_eq!(piece_to_byte("<0xGG>"), None);
        assert_eq!(piece_to_byte("<unk>"), None);
    }

    #[test]
    fn denormalize_applies_reverse_charsmap() {
        let blob = std::fs::read(fixture("sp_denorm_charsmap.bin")).expect("fixture");
        let map = PrecompiledCharsMap::parse(&blob).unwrap();
        let lite = LiteNormalizer {
            add_dummy_prefix: false,
            remove_extra_whitespaces: false,
            escape_whitespaces: false,
        };
        assert_eq!(
            sp_denormalize("xTMy", &lite, Some(&map)).unwrap(),
            "x\u{2122}y"
        );
        assert_eq!(sp_denormalize("TM", &lite, Some(&map)).unwrap(), "\u{2122}");
        // No match → identity.
        assert_eq!(sp_denormalize("hello", &lite, Some(&map)).unwrap(), "hello");
    }

    /// Offline hygiene for Wave 27 curated goldens: identity rows use charsmap-only
    /// knobs (no dummy-prefix / ▁ escape). Full nfkc / nfkc_cf blobs stay
    /// integration-only — not an exhaustive Unicode NFKC claim.
    #[test]
    fn curated_nfkc_identity_goldens_charsmap_only() {
        let raw = std::fs::read_to_string(fixture("sp_nfkc_normalize_goldens.json"))
            .expect("sp_nfkc_normalize_goldens.json");
        let v: serde_json::Value = serde_json::from_str(&raw).expect("golden json");
        let identity = v["rule"]["identity"]
            .as_array()
            .expect("rule.identity array");
        assert!(
            identity.len() > 100,
            "Wave 27 expects curated identity goldens expanded beyond 100/rule, got {}",
            identity.len()
        );
        for rule in ["nfkc", "nfkc_cf"] {
            let n = v["rule"][rule]
                .as_array()
                .unwrap_or_else(|| panic!("rule.{rule} array"))
                .len();
            assert_eq!(
                n,
                identity.len(),
                "golden rule lengths must stay aligned ({rule}={n}, identity={})",
                identity.len()
            );
            assert!(n > 100, "Wave 27: {rule} goldens beyond 100/rule, got {n}");
        }

        let lite = LiteNormalizer {
            add_dummy_prefix: false,
            remove_extra_whitespaces: false,
            escape_whitespaces: false,
        };
        for row in identity {
            let inp = row["in"].as_str().expect("in");
            let expected = row["out"].as_str().expect("out");
            let out = sp_normalize(inp, &lite, None, false).expect("normalize");
            assert_eq!(out, expected, "identity golden mismatch for {inp:?}");
        }
    }
}
