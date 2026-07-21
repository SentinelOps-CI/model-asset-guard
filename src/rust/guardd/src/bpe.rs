//! Lean-aligned structural BPE merge operators.
//!
//! These match `ModelAssetGuard.Token.apply_merge_once` /
//! `apply_merge_table` in `Tokenizer.lean`: leftmost first-occurrence merge,
//! then a left-to-right merge schedule. This is **not** SentencePiece score-
//! based BPE (see `sentencepiece::encode_bpe`); it is the formal/runtime twin
//! of the Lean structural lemmas.

/// One merge step: replace the first adjacent pair `(a, b)` with `m`.
pub fn apply_merge_once(tokens: &[u32], a: u32, b: u32, m: u32) -> Vec<u32> {
    if tokens.is_empty() {
        return Vec::new();
    }
    if tokens.len() == 1 {
        return vec![tokens[0]];
    }
    let mut out = Vec::with_capacity(tokens.len());
    let mut i = 0usize;
    let mut merged = false;
    while i < tokens.len() {
        if !merged && i + 1 < tokens.len() && tokens[i] == a && tokens[i + 1] == b {
            out.push(m);
            i += 2;
            merged = true;
        } else {
            out.push(tokens[i]);
            i += 1;
        }
    }
    out
}

/// Apply a token-level merge schedule left-to-right.
pub fn apply_merge_table(tokens: &[u32], merges: &[(u32, u32, u32)]) -> Vec<u32> {
    let mut cur = tokens.to_vec();
    for &(a, b, m) in merges {
        cur = apply_merge_once(&cur, a, b, m);
    }
    cur
}

/// Char-level seed encode then structural merge table (Lean `bpe_encode_with_merge_table`).
pub fn encode_with_merge_table(text: &str, merges: &[(u32, u32, u32)]) -> Vec<u32> {
    let seeds: Vec<u32> = text.chars().map(|c| c as u32).collect();
    apply_merge_table(&seeds, merges)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_once_shortens_on_hit() {
        assert_eq!(apply_merge_once(&[1, 2, 3], 1, 2, 9), vec![9, 3]);
    }

    #[test]
    fn merge_once_idempotent_when_pair_absent() {
        let tokens = vec![1u32, 3, 2];
        assert_eq!(apply_merge_once(&tokens, 1, 2, 9), tokens);
    }

    #[test]
    fn merge_table_multi_step() {
        let tokens = [1u32, 2, 3, 4];
        let merges = [(1, 2, 9), (9, 3, 8)];
        assert_eq!(apply_merge_table(&tokens, &merges), vec![8, 4]);
    }

    #[test]
    fn merge_table_never_lengthens() {
        let tokens = [5u32, 5, 5, 5];
        let merges = [(5, 5, 1), (1, 5, 2), (2, 5, 3)];
        let out = apply_merge_table(&tokens, &merges);
        assert!(out.len() <= tokens.len());
    }

    #[test]
    fn encode_with_merge_table_char_seed() {
        // 'a'=97, 'b'=98 → merge to 1
        let merges = [(97u32, 98, 1)];
        assert_eq!(encode_with_merge_table("ab", &merges), vec![1]);
        assert_eq!(encode_with_merge_table("a", &merges), vec![97]);
    }
}
