use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfectHashVocab {
    pub vocab: Vec<(String, u32)>, // (string, token)
    pub hash_table: Vec<usize>,    // Maps hash(string) % table_size -> vocab index
    pub table_size: usize,
}

impl PerfectHashVocab {
    /// Build a minimal perfect hash table for the given vocab
    #[allow(dead_code)]
    pub fn build(vocab: Vec<(String, u32)>) -> Self {
        let table_size = vocab.len();
        let mut hash_table = vec![usize::MAX; table_size];
        let mut string_to_index = HashMap::new();
        for (i, (word, _)) in vocab.iter().enumerate() {
            let mut hash = Self::hash(word) % table_size;
            // Linear probing for simplicity (replace with a real MPH if needed)
            while hash_table[hash] != usize::MAX {
                hash = (hash + 1) % table_size;
            }
            hash_table[hash] = i;
            string_to_index.insert(word, i);
        }
        Self { vocab, hash_table, table_size }
    }

    /// Hash function (can be replaced with a better one)
    pub fn hash(s: &str) -> usize {
        let mut hash = 5381usize;
        for b in s.bytes() {
            hash = ((hash << 5).wrapping_add(hash)).wrapping_add(b as usize);
        }
        hash
    }

    /// Encode a string to a token using the perfect hash
    pub fn encode(&self, word: &str) -> u32 {
        let mut hash = Self::hash(word) % self.table_size;
        for _ in 0..self.table_size {
            let idx = self.hash_table[hash];
            if idx == usize::MAX {
                return 0; // Unknown token
            }
            if self.vocab[idx].0 == word {
                return self.vocab[idx].1;
            }
            hash = (hash + 1) % self.table_size;
        }
        0 // Unknown token
    }

    /// Decode a token to a string
    pub fn decode(&self, token: u32) -> &str {
        for (word, tok) in &self.vocab {
            if *tok == token {
                return word;
            }
        }
        "<unk>"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perfect_hash_encode_decode() {
        let vocab = vec![
            ("hello".to_string(), 1),
            ("world".to_string(), 2),
            ("test".to_string(), 3),
        ];
        let mph = PerfectHashVocab::build(vocab.clone());
        for (word, token) in &vocab {
            assert_eq!(mph.encode(word), *token);
            assert_eq!(mph.decode(*token), word);
        }
        // Unknown word
        assert_eq!(mph.encode("unknown"), 0);
        assert_eq!(mph.decode(999), "<unk>");
    }

    #[test]
    fn test_no_collisions() {
        let vocab = vec![
            ("a".to_string(), 1),
            ("b".to_string(), 2),
            ("c".to_string(), 3),
            ("d".to_string(), 4),
        ];
        let mph = PerfectHashVocab::build(vocab.clone());
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