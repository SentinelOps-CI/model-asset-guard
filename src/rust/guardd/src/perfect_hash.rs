use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Hash vocabulary used by FFI encode/decode.
///
/// Two on-disk forms (detected at load):
/// 1. **CHD minimal perfect hash** (`kind: "chd_mph"`) — default for new builds.
///    Fields: `vocab`, `pilots`, `n_buckets`, `indices`, `seed0`, `seed1`.
/// 2. **Legacy open-addressing** — `vocab`, `hash_table`, `table_size`
///    (linear probing). Still loaded for older JSON; not produced by
///    [`PerfectHashVocab::build`] anymore.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PerfectHashVocab {
    pub vocab: Vec<(String, u32)>,
    /// `"chd_mph"` for CHD; omitted / `"open_address"` for legacy tables.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Per-bucket displace values (CHD).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pilots: Vec<u32>,
    /// CHD bucket count.
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub n_buckets: usize,
    /// `indices[slot] = vocab_index` for CHD slot in `0..n`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub indices: Vec<usize>,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub seed0: u64,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub seed1: u64,
    /// Legacy open-addressed slots (`usize::MAX` = empty).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hash_table: Vec<usize>,
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub table_size: usize,
}

fn is_zero_usize(v: &usize) -> bool {
    *v == 0
}
fn is_zero_u64(v: &u64) -> bool {
    *v == 0
}

const KIND_CHD: &str = "chd_mph";
const KIND_OPEN: &str = "open_address";

/// Default CHD seeds (fixed for deterministic cross-platform tables).
const DEFAULT_SEED0: u64 = 0x9e37_79b9_7f4a_7c15;
const DEFAULT_SEED1: u64 = 0xbf58_476d_1ce4_e5b9;

impl PerfectHashVocab {
    /// Build a **CHD minimal perfect hash** for the given vocab.
    ///
    /// Maps `n` keys injectively into `0..n` slots (true MPH). Lookup always
    /// verifies the stored string so unknown keys still map to `0`.
    pub fn build(vocab: Vec<(String, u32)>) -> Self {
        match Self::build_chd_mph(vocab.clone()) {
            Ok(v) => v,
            Err(_) => {
                // Extremely pathological key sets: fall back to open-addressing
                // with an honest kind tag (not claimed as MPH).
                let mut open = Self::build_open_address(vocab);
                open.kind = Some(KIND_OPEN.to_string());
                open
            }
        }
    }

    /// Explicit open-addressed table (legacy / fallback). Not an MPH.
    pub fn build_open_address(vocab: Vec<(String, u32)>) -> Self {
        if vocab.is_empty() {
            return Self::empty_open();
        }

        let table_size = (vocab.len() * 2).next_power_of_two().max(4);
        let mut hash_table = vec![usize::MAX; table_size];

        for (i, (word, _)) in vocab.iter().enumerate() {
            let mut slot = Self::hash(word) % table_size;
            for _ in 0..table_size {
                if hash_table[slot] == usize::MAX {
                    hash_table[slot] = i;
                    break;
                }
                slot = (slot + 1) % table_size;
            }
        }

        Self {
            vocab,
            kind: Some(KIND_OPEN.to_string()),
            pilots: Vec::new(),
            n_buckets: 0,
            indices: Vec::new(),
            seed0: 0,
            seed1: 0,
            hash_table,
            table_size,
        }
    }

    fn empty_open() -> Self {
        Self {
            vocab: Vec::new(),
            kind: Some(KIND_OPEN.to_string()),
            pilots: Vec::new(),
            n_buckets: 0,
            indices: Vec::new(),
            seed0: 0,
            seed1: 0,
            hash_table: Vec::new(),
            table_size: 0,
        }
    }

    fn empty_mph() -> Self {
        Self {
            vocab: Vec::new(),
            kind: Some(KIND_CHD.to_string()),
            pilots: Vec::new(),
            n_buckets: 0,
            indices: Vec::new(),
            seed0: DEFAULT_SEED0,
            seed1: DEFAULT_SEED1,
            hash_table: Vec::new(),
            table_size: 0,
        }
    }

    /// CHD (Compress, Hash, Displace) construction.
    ///
    /// Tries compact bucket counts first, then `n_buckets = n` if needed.
    pub fn build_chd_mph(vocab: Vec<(String, u32)>) -> Result<Self, String> {
        let n = vocab.len();
        if n == 0 {
            return Ok(Self::empty_mph());
        }

        // Reject duplicate keys — MPH cannot exist for multisets.
        {
            let mut seen = HashSet::new();
            for (w, _) in &vocab {
                if !seen.insert(w.as_str()) {
                    return Err(format!("duplicate vocab key {w:?}"));
                }
            }
        }

        let seed0 = DEFAULT_SEED0;
        let seed1 = DEFAULT_SEED1;

        // λ ≈ 5, λ ≈ 2, then `n` (always constructible for distinct keys with enough tries).
        for n_buckets in Self::rust_bucket_plans(n) {
            if let Ok(v) = Self::try_chd(&vocab, n_buckets, seed0, seed1) {
                return Ok(v);
            }
        }
        Err("CHD could not find displace values for this key set".into())
    }

    /// Bucket processing order: descending length, then ascending bucket id.
    ///
    /// Shared with Lean `CHDBuild.bucketProcessOrder` (Wave 24).
    fn bucket_process_order(buckets: &[Vec<usize>]) -> Vec<usize> {
        let mut order: Vec<usize> = (0..buckets.len()).collect();
        order.sort_by_key(|&b| (std::cmp::Reverse(buckets[b].len()), b));
        order
    }

    /// Rust λ-plans: `((n+4)/5).max(1)`, `((n+1)/2).max(1)`, then `n` (deduped).
    fn rust_bucket_plans(n: usize) -> Vec<usize> {
        if n == 0 {
            return vec![0];
        }
        let a = ((n + 4) / 5).max(1);
        let b = ((n + 1) / 2).max(1);
        let mut out = Vec::with_capacity(3);
        for x in [a, b, n] {
            if !out.contains(&x) {
                out.push(x);
            }
        }
        out
    }

    fn try_chd(
        vocab: &[(String, u32)],
        n_buckets: usize,
        seed0: u64,
        seed1: u64,
    ) -> Result<Self, String> {
        let n = vocab.len();
        let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); n_buckets];
        for (i, (word, _)) in vocab.iter().enumerate() {
            let b = (Self::hash_seeded(word, seed0) as usize) % n_buckets;
            buckets[b].push(i);
        }

        // Largest bucket first; equal lengths tie-break by ascending bucket id.
        // Stable + explicit key so Lean `bucketProcessOrder` matches exactly.
        let order = Self::bucket_process_order(&buckets);

        let mut occupied = vec![false; n];
        let mut indices = vec![usize::MAX; n];
        let mut pilots = vec![0u32; n_buckets];

        // Production displace search budget (Lean smokes may use smaller `maxD`).
        const MAX_DISPLACE: u32 = 4_000_000;

        for &b in &order {
            let keys = &buckets[b];
            if keys.is_empty() {
                continue;
            }
            let mut placed = false;
            for d in 0..=MAX_DISPLACE {
                let mut slots: Vec<usize> = Vec::with_capacity(keys.len());
                let mut local = HashSet::with_capacity(keys.len());
                let mut ok = true;
                for &ki in keys {
                    let slot = (Self::hash_seeded(&vocab[ki].0, seed1.wrapping_add(u64::from(d)))
                        as usize)
                        % n;
                    if occupied[slot] || !local.insert(slot) {
                        ok = false;
                        break;
                    }
                    slots.push(slot);
                }
                if !ok {
                    continue;
                }
                for (j, &slot) in slots.iter().enumerate() {
                    occupied[slot] = true;
                    indices[slot] = keys[j];
                }
                pilots[b] = d;
                placed = true;
                break;
            }
            if !placed {
                return Err(format!(
                    "no displace for bucket {b} (size {}) with {n_buckets} buckets",
                    keys.len()
                ));
            }
        }

        if indices.iter().any(|&i| i == usize::MAX) {
            return Err("CHD left unfilled slots".into());
        }

        Ok(Self {
            vocab: vocab.to_vec(),
            kind: Some(KIND_CHD.to_string()),
            pilots,
            n_buckets,
            indices,
            seed0,
            seed1,
            hash_table: Vec::new(),
            table_size: 0,
        })
    }

    /// Build from a word list, assigning tokens `1..N` in order (CHD MPH).
    pub fn from_words(words: Vec<String>) -> Self {
        let vocab = words
            .into_iter()
            .enumerate()
            .map(|(i, w)| (w, (i + 1) as u32))
            .collect();
        Self::build(vocab)
    }

    /// djb2-style hash (legacy open-addressing; stable across platforms).
    pub fn hash(s: &str) -> usize {
        let mut hash = 5381usize;
        for b in s.bytes() {
            hash = ((hash << 5).wrapping_add(hash)).wrapping_add(b as usize);
        }
        hash
    }

    /// Seeded 64-bit hash for CHD (mix after djb2-style accumulation).
    pub fn hash_seeded(s: &str, seed: u64) -> u64 {
        let mut h = seed.wrapping_add(5381);
        for b in s.bytes() {
            h = h.wrapping_mul(33).wrapping_add(u64::from(b));
        }
        // SplitMix64 finalizer
        h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        h = (h ^ (h >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        h ^ (h >> 31)
    }

    pub fn is_chd_mph(&self) -> bool {
        self.kind.as_deref() == Some(KIND_CHD)
            || (!self.pilots.is_empty() && !self.indices.is_empty() && self.n_buckets > 0)
    }

    pub fn is_open_address(&self) -> bool {
        !self.is_chd_mph() && self.table_size > 0 && !self.hash_table.is_empty()
    }

    /// Encode a string to a token. Unknown words map to `0`.
    pub fn encode(&self, word: &str) -> u32 {
        if self.vocab.is_empty() {
            return 0;
        }
        if self.is_chd_mph() {
            return self.encode_chd(word);
        }
        self.encode_open(word)
    }

    fn encode_chd(&self, word: &str) -> u32 {
        let n = self.vocab.len();
        if n == 0 || self.n_buckets == 0 || self.indices.len() != n {
            return 0;
        }
        let b = (Self::hash_seeded(word, self.seed0) as usize) % self.n_buckets;
        let d = match self.pilots.get(b) {
            Some(&d) => d,
            None => return 0,
        };
        let slot = (Self::hash_seeded(word, self.seed1.wrapping_add(u64::from(d))) as usize) % n;
        let idx = match self.indices.get(slot).copied() {
            Some(i) if i < n => i,
            _ => return 0,
        };
        match self.vocab.get(idx) {
            Some((w, tok)) if w == word => *tok,
            _ => 0,
        }
    }

    fn encode_open(&self, word: &str) -> u32 {
        if self.table_size == 0 || self.vocab.is_empty() {
            return 0;
        }

        let mut slot = Self::hash(word) % self.table_size;
        for _ in 0..self.table_size {
            let idx = self.hash_table[slot];
            if idx == usize::MAX {
                return 0;
            }
            if let Some((w, tok)) = self.vocab.get(idx) {
                if w == word {
                    return *tok;
                }
            }
            slot = (slot + 1) % self.table_size;
        }
        0
    }

    /// Decode a token to a string. Unknown tokens map to `"<unk>"`.
    pub fn decode(&self, token: u32) -> &str {
        for (word, tok) in &self.vocab {
            if *tok == token {
                return word;
            }
        }
        "<unk>"
    }

    /// Validate that every vocab entry round-trips through the table.
    pub fn validate_roundtrip(&self) -> bool {
        if self.is_chd_mph() {
            // Structural MPH checks.
            let n = self.vocab.len();
            if self.indices.len() != n || self.n_buckets == 0 || self.pilots.len() != self.n_buckets
            {
                return false;
            }
            let mut seen_slots = HashSet::new();
            for &idx in &self.indices {
                if idx >= n || !seen_slots.insert(idx) {
                    return false;
                }
            }
        }
        let mut seen_tokens = HashMap::new();
        for (word, token) in &self.vocab {
            if self.encode(word) != *token {
                return false;
            }
            if self.decode(*token) != word {
                return false;
            }
            if seen_tokens.insert(*token, word).is_some() {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_seeded_goldens_wave20() {
        // Shared with Lean `Token.HashSeeded` / Python mirror.
        const SEED0: u64 = 0x9e37_79b9_7f4a_7c15;
        assert_eq!(PerfectHashVocab::hash_seeded("", SEED0), 9862407849072414892);
        assert_eq!(PerfectHashVocab::hash_seeded("a", SEED0), 13358874647675253293);
        assert_eq!(PerfectHashVocab::hash_seeded("ab", SEED0), 10513868738868360859);
        assert_eq!(PerfectHashVocab::hash_seeded("hello", SEED0), 5468378884496047854);
        const SEED1: u64 = 0xbf58_476d_1ce4_e5b9;
        assert_eq!(PerfectHashVocab::hash_seeded("", SEED1), 2747279002426353127);
        assert_eq!(PerfectHashVocab::hash_seeded("a", SEED1), 13232201968939760250);
    }

    #[test]
    fn test_perfect_hash_encode_decode() {
        let vocab = vec![
            ("hello".to_string(), 1),
            ("world".to_string(), 2),
            ("test".to_string(), 3),
        ];
        let mph = PerfectHashVocab::build(vocab.clone());
        assert!(mph.is_chd_mph(), "default build must be CHD MPH");
        assert!(mph.validate_roundtrip());
        for (word, token) in &vocab {
            assert_eq!(mph.encode(word), *token);
            assert_eq!(mph.decode(*token), word.as_str());
        }
        assert_eq!(mph.encode("unknown"), 0);
        assert_eq!(mph.decode(999), "<unk>");
    }

    #[test]
    fn test_json_roundtrip_schema() {
        let mph = PerfectHashVocab::from_words(vec!["hello".into(), "world".into(), "test".into()]);
        assert!(mph.is_chd_mph());
        let json = serde_json::to_string(&mph).unwrap();
        assert!(json.contains("chd_mph"));
        assert!(!json.contains("hash_table") || !json.contains("\"hash_table\":["));
        let loaded: PerfectHashVocab = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded, mph);
        assert!(loaded.validate_roundtrip());
        assert_eq!(loaded.encode("hello"), 1);
        assert_eq!(loaded.decode(2), "world");
    }

    #[test]
    fn test_chd_is_minimal() {
        let words: Vec<String> = (0..200).map(|i| format!("w{i}")).collect();
        let mph = PerfectHashVocab::from_words(words);
        assert!(mph.is_chd_mph());
        assert_eq!(mph.indices.len(), mph.vocab.len());
        assert!(mph.validate_roundtrip());
        // Minimal: exactly n filled slots, no open-address over-allocation.
        assert!(mph.hash_table.is_empty());
        assert_eq!(mph.table_size, 0);
    }

    #[test]
    fn test_legacy_open_address_still_loads() {
        let open =
            PerfectHashVocab::build_open_address(vec![("hello".into(), 1), ("world".into(), 2)]);
        assert!(open.is_open_address());
        assert!(!open.is_chd_mph());
        let json = serde_json::to_string(&open).unwrap();
        let loaded: PerfectHashVocab = serde_json::from_str(&json).unwrap();
        assert!(loaded.validate_roundtrip());
        assert_eq!(loaded.encode("hello"), 1);

        // Untagged legacy JSON without kind field.
        let legacy =
            r#"{"vocab":[["a",1],["b",2]],"hash_table":[0,18446744073709551615,1],"table_size":3}"#;
        // table_size 3 may not match — build a real open table and strip kind.
        let mut legacy_v =
            PerfectHashVocab::build_open_address(vec![("a".into(), 1), ("b".into(), 2)]);
        legacy_v.kind = None;
        let legacy_json = serde_json::to_string(&legacy_v).unwrap();
        let loaded2: PerfectHashVocab = serde_json::from_str(&legacy_json).unwrap();
        assert!(loaded2.is_open_address());
        assert_eq!(loaded2.encode("a"), 1);
        let _ = legacy; // silence if unused shape
    }

    #[test]
    fn test_mph_slots_are_bijective() {
        let vocab: Vec<(String, u32)> = (0..64).map(|i| (format!("k{i}"), i as u32 + 1)).collect();
        let mph = PerfectHashVocab::build(vocab);
        assert!(mph.is_chd_mph());
        let mut seen = HashSet::new();
        for (word, _) in &mph.vocab {
            let b = (PerfectHashVocab::hash_seeded(word, mph.seed0) as usize) % mph.n_buckets;
            let d = mph.pilots[b];
            let slot = (PerfectHashVocab::hash_seeded(word, mph.seed1.wrapping_add(u64::from(d)))
                as usize)
                % mph.vocab.len();
            assert!(seen.insert(slot), "slot collision for {word}");
        }
        assert_eq!(seen.len(), mph.vocab.len());
    }

    fn slots_by_key(mph: &PerfectHashVocab) -> Vec<usize> {
        let n = mph.vocab.len();
        let mut slots = vec![0usize; n];
        for (slot, &ki) in mph.indices.iter().enumerate() {
            slots[ki] = slot;
        }
        slots
    }

    /// Shared with Lean `CHDBuild.golden*` under forced `n_buckets = n` (Wave 23).
    #[test]
    fn test_chd_lean_golden_n_buckets_eq_n() {
        let cases: &[(&[&str], &[usize], &[u32])] = &[
            (&["a", "b", "c"], &[0, 2, 1], &[0, 0, 0]),
            (&["hello", "world", "test"], &[0, 2, 1], &[2, 0, 1]),
            (&["a", "b", "c", "d"], &[2, 1, 0, 3], &[0, 3, 0, 1]),
        ];
        for &(words, expect_slots, expect_pilots) in cases {
            let vocab: Vec<(String, u32)> = words
                .iter()
                .enumerate()
                .map(|(i, w)| ((*w).to_string(), (i + 1) as u32))
                .collect();
            let n = vocab.len();
            let mph = PerfectHashVocab::try_chd(&vocab, n, DEFAULT_SEED0, DEFAULT_SEED1)
                .unwrap_or_else(|e| panic!("try_chd failed for {words:?}: {e}"));
            assert_eq!(mph.n_buckets, n, "words={words:?}");
            assert_eq!(&mph.pilots[..], expect_pilots, "pilots words={words:?}");
            assert_eq!(slots_by_key(&mph), expect_slots, "slots words={words:?}");
            assert!(mph.validate_roundtrip());
        }
    }

    /// Wave 24: equal-length buckets process in ascending bucket-id order.
    #[test]
    fn test_chd_bucket_order_equal_length_tiebreak() {
        // Artificial equal lengths: three buckets of size 2.
        let buckets = vec![vec![0, 1], vec![2, 3], vec![4, 5]];
        assert_eq!(
            PerfectHashVocab::bucket_process_order(&buckets),
            vec![0, 1, 2],
            "equal lengths ⇒ ascending bucket id"
        );
        // Mixed: size 3, size 1, size 3, size 1 → [0, 2, 1, 3]
        let buckets2 = vec![vec![0, 1, 2], vec![3], vec![4, 5, 6], vec![7]];
        assert_eq!(
            PerfectHashVocab::bucket_process_order(&buckets2),
            vec![0, 2, 1, 3]
        );
        // Empty buckets sort last among equals by id among zeros.
        let buckets3 = vec![vec![], vec![0, 1], vec![], vec![2, 3]];
        assert_eq!(
            PerfectHashVocab::bucket_process_order(&buckets3),
            vec![1, 3, 0, 2]
        );
    }

    /// Shared with Lean `CHDBuild.goldenPlan*` — first successful λ-plan (Wave 24).
    #[test]
    fn test_chd_lean_golden_lambda_plans() {
        type LambdaPlanCase<'a> = (&'a [&'a str], usize, &'a [usize], &'a [u32]);
        let cases: &[LambdaPlanCase<'_>] = &[
            (&["a", "b", "c"], 1, &[0, 2, 1], &[0]),
            (&["hello", "world", "test"], 1, &[1, 2, 0], &[5]),
            (&["a", "b", "c", "d"], 1, &[2, 3, 1, 0], &[22]),
            (
                &["w0", "w1", "w2", "w3", "w4", "w5", "w6", "w7"],
                2,
                &[1, 3, 2, 0, 5, 4, 7, 6],
                &[1, 92],
            ),
            (
                &["x", "y", "z", "w", "v", "u"],
                2,
                &[0, 4, 3, 5, 1, 2],
                &[41, 5],
            ),
            (&["one", "two", "three", "four"], 1, &[0, 1, 3, 2], &[5]),
        ];
        for &(words, expect_nb, expect_slots, expect_pilots) in cases {
            let vocab: Vec<(String, u32)> = words
                .iter()
                .enumerate()
                .map(|(i, w)| ((*w).to_string(), (i + 1) as u32))
                .collect();
            let n = vocab.len();
            let plans = PerfectHashVocab::rust_bucket_plans(n);
            let mut mph = None;
            for &nb in &plans {
                if let Ok(v) = PerfectHashVocab::try_chd(&vocab, nb, DEFAULT_SEED0, DEFAULT_SEED1) {
                    mph = Some(v);
                    break;
                }
            }
            let mph = mph.unwrap_or_else(|| panic!("no λ-plan for {words:?}"));
            assert_eq!(mph.n_buckets, expect_nb, "n_buckets words={words:?}");
            assert_eq!(&mph.pilots[..], expect_pilots, "pilots words={words:?}");
            assert_eq!(slots_by_key(&mph), expect_slots, "slots words={words:?}");
            assert!(mph.validate_roundtrip());
            // Cross-check: `build_chd_mph` selects the same first plan.
            let built = PerfectHashVocab::build_chd_mph(vocab).expect("build_chd_mph");
            assert_eq!(built.n_buckets, expect_nb);
            assert_eq!(built.pilots, mph.pilots);
            assert_eq!(built.indices, mph.indices);
        }
    }

    /// Wave 24: real key sets where multiple buckets share a length.
    #[test]
    fn test_chd_bucket_order_parity_stress() {
        // nb=2: both buckets length 2 → order [0, 1] by ascending id.
        let words = ["one", "two", "three", "four"];
        let nb = 2usize;
        let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); nb];
        for (i, w) in words.iter().enumerate() {
            let b = (PerfectHashVocab::hash_seeded(w, DEFAULT_SEED0) as usize) % nb;
            buckets[b].push(i);
        }
        assert_eq!(buckets.iter().map(|b| b.len()).collect::<Vec<_>>(), vec![2, 2]);
        assert_eq!(PerfectHashVocab::bucket_process_order(&buckets), vec![0, 1]);

        // nb=4: lengths [2,0,4,2] → process size-4 then the two size-2 (ids 0 before 3).
        let words8 = ["w0", "w1", "w2", "w3", "w4", "w5", "w6", "w7"];
        let nb4 = 4usize;
        let mut buckets4: Vec<Vec<usize>> = vec![Vec::new(); nb4];
        for (i, w) in words8.iter().enumerate() {
            let b = (PerfectHashVocab::hash_seeded(w, DEFAULT_SEED0) as usize) % nb4;
            buckets4[b].push(i);
        }
        assert_eq!(
            buckets4.iter().map(|b| b.len()).collect::<Vec<_>>(),
            vec![2, 0, 4, 2]
        );
        assert_eq!(
            PerfectHashVocab::bucket_process_order(&buckets4),
            vec![2, 0, 3, 1]
        );
    }

    #[test]
    fn test_no_collisions_resolve_open() {
        let vocab = vec![
            ("a".to_string(), 1),
            ("b".to_string(), 2),
            ("c".to_string(), 3),
            ("d".to_string(), 4),
        ];
        let mph = PerfectHashVocab::build_open_address(vocab.clone());
        let mut seen = std::collections::HashSet::new();
        for (word, _) in &vocab {
            let mut hash = PerfectHashVocab::hash(word) % mph.table_size;
            let mut found = false;
            for _ in 0..mph.table_size {
                let idx = mph.hash_table[hash];
                if idx != usize::MAX && mph.vocab[idx].0 == *word {
                    assert!(seen.insert(hash));
                    found = true;
                    break;
                }
                hash = (hash + 1) % mph.table_size;
            }
            assert!(found);
        }
    }
}
