//! CLI helper for perfect-hash encode/decode/test/generate (used by Lean perfecthash / tokenizertest).
//!
//! Also accepts SentencePiece `.model` (`ModelProto`) files:
//! - vocab piece extraction + hash-vocab round-trip
//! - real CHAR/BPE/UNIGRAM encode when the model passes the fail-closed support gate
//!   (`sp-encode` / `sp-decode` / `sp-nbest` / `sp-sample`)

use guardd::perfect_hash::PerfectHashVocab;
use guardd::sentencepiece::{
    self, EncodeOptions, SampleEncodeAndScoreOptions, SampleEncodeOptions, SentencePieceModel,
};
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::Path;
use std::process;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum VocabFile {
    Pairs(Vec<(String, u32)>),
    Words { words: Vec<String> },
    Table(PerfectHashVocab),
}

fn print_usage() {
    eprintln!(
        "Usage:
  guardd_perfect_hash encode <vocab.json|vocab.txt|.model> <text>
  guardd_perfect_hash decode <vocab.json|vocab.txt|.model> <token>
  guardd_perfect_hash test <vocab.json|vocab.txt|.model>
  guardd_perfect_hash generate <vocab.txt|vocab.json|.model> <output.json>
  guardd_perfect_hash sp-encode <.model> <text> [--silent-unk] [--no-merge-unk] [--reverse] [--bos] [--eos] [--extra reverse:bos:eos] [--forced <piece>]...
  guardd_perfect_hash sp-decode <.model> [--reverse] <token_id> [token_id...]
  guardd_perfect_hash sp-normalize <.model> <text>
  guardd_perfect_hash sp-nbest <.model> <text> <n> [--silent-unk] [--no-merge-unk] [--reverse] [--bos] [--eos] [--extra ...] [--nbest-stock-compat]
  guardd_perfect_hash sp-sample <.model> <text> [--nbest N] [--alpha A] [--seed S] [--silent-unk] [--no-merge-unk] [--reverse] [--bos] [--eos] [--extra ...]
  guardd_perfect_hash sp-sample-score <.model> <text> [--samples N] [--alpha A] [--seed S] [--silent-unk] [--no-merge-unk] [--reverse] [--bos] [--eos] [--extra ...] [--wor] [--include-best]

SentencePiece .model:
  - encode/decode/test/generate: piece extraction → hash vocab (whitespace-split encode)
  - sp-encode/sp-decode: CHAR, BPE, UNIGRAM, or WORD segmentation when supported; else fail closed
  - sp-normalize: charsmap + whitespace (+ treat_whitespace_as_suffix) + USER_DEFINED PrefixMatcher
  - sp-nbest: UNIGRAM lattice only
  - sp-sample: UNIGRAM lattice sample or BPE-dropout (alpha = dropout prob)
  - sp-sample-score: UNIGRAM SampleEncodeAndScore (WR or WOR / Gumbel inclusion)
  See docs/sentencepiece-bpe.md for the support matrix."
    );
}

fn is_model_proto_path(path: &str) -> bool {
    let p = Path::new(path);
    matches!(
        p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()),
        Some(ext) if ext == "model" || ext == "proto"
    )
}

fn load_vocab(path: &str) -> Result<PerfectHashVocab, String> {
    if is_model_proto_path(path) {
        let model = sentencepiece::load_model_proto_file(path)?;
        return Ok(model.vocab);
    }

    let content = fs::read_to_string(path).map_err(|e| format!("read '{path}': {e}"))?;
    if path.ends_with(".json") {
        let parsed: VocabFile =
            serde_json::from_str(&content).map_err(|e| format!("invalid vocab JSON: {e}"))?;
        Ok(match parsed {
            VocabFile::Pairs(pairs) => PerfectHashVocab::build(pairs),
            VocabFile::Words { words } => PerfectHashVocab::from_words(words),
            VocabFile::Table(table) => table,
        })
    } else {
        let words: Vec<String> = content
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect();
        Ok(PerfectHashVocab::from_words(words))
    }
}

fn load_sp_model(path: &str) -> Result<SentencePieceModel, String> {
    if !is_model_proto_path(path) {
        return Err("sp-* commands require a .model / .proto ModelProto file".into());
    }
    sentencepiece::load_model_proto_file(path)
}

fn parse_sp_encode_flags(args: &[String]) -> Result<(EncodeOptions, usize), String> {
    // Returns (opts, index of first non-flag after model+text consumed by caller).
    // Caller passes args starting at the first optional flag (after text).
    let mut opts = EncodeOptions::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--silent-unk" => {
                opts.allow_silent_unk = true;
                i += 1;
            }
            "--no-merge-unk" => {
                opts.merge_consecutive_unk = false;
                i += 1;
            }
            "--reverse" => {
                opts.reverse = true;
                i += 1;
            }
            "--bos" => {
                opts.add_bos = true;
                i += 1;
            }
            "--eos" => {
                opts.add_eos = true;
                i += 1;
            }
            "--extra" => {
                i += 1;
                let spec = args.get(i).ok_or_else(|| {
                    "--extra requires a colon-separated ExtraOptions string".to_string()
                })?;
                opts.apply_extra_options_str(spec)?;
                i += 1;
            }
            "--nbest-stock-compat" => {
                opts.nbest_stock_compat = true;
                i += 1;
            }
            "--forced" => {
                i += 1;
                let piece = args
                    .get(i)
                    .ok_or_else(|| "--forced requires a piece argument".to_string())?;
                opts.forced_pieces.push(piece.clone());
                i += 1;
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown flag '{other}'"));
            }
            _ => break,
        }
    }
    Ok((opts, i))
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        process::exit(2);
    }

    match args[1].as_str() {
        "encode" => {
            if args.len() < 4 {
                eprintln!("Error: encode requires <vocab> <text>");
                print_usage();
                process::exit(2);
            }
            let vocab = match load_vocab(&args[2]) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("Error: {e}");
                    process::exit(1);
                }
            };
            let text = &args[3];
            let tokens: Vec<u32> = text.split_whitespace().map(|w| vocab.encode(w)).collect();
            match serde_json::to_string(&tokens) {
                Ok(j) => println!("{j}"),
                Err(e) => {
                    eprintln!("Error: serialize tokens: {e}");
                    process::exit(1);
                }
            }
        }
        "decode" => {
            if args.len() < 4 {
                eprintln!("Error: decode requires <vocab> <token>");
                print_usage();
                process::exit(2);
            }
            let vocab = match load_vocab(&args[2]) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("Error: {e}");
                    process::exit(1);
                }
            };
            let token: u32 = match args[3].parse() {
                Ok(t) => t,
                Err(_) => {
                    eprintln!("Error: token must be a non-negative integer");
                    process::exit(2);
                }
            };
            println!("{}", vocab.decode(token));
        }
        "sp-encode" => {
            if args.len() < 4 {
                eprintln!(
                    "Error: sp-encode requires <.model> <text> [--silent-unk] [--forced p]..."
                );
                print_usage();
                process::exit(2);
            }
            let model = match load_sp_model(&args[2]) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("Error: {e}");
                    process::exit(1);
                }
            };
            let opts = match parse_sp_encode_flags(&args[4..]) {
                Ok((o, consumed)) if consumed == args[4..].len() => o,
                Ok(_) => {
                    eprintln!("Error: unexpected positional args after flags");
                    process::exit(2);
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    process::exit(2);
                }
            };
            let tokens = match sentencepiece::encode_with_options(&model, &args[3], opts) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("Error: {e}");
                    process::exit(1);
                }
            };
            match serde_json::to_string(&tokens) {
                Ok(j) => println!("{j}"),
                Err(e) => {
                    eprintln!("Error: serialize tokens: {e}");
                    process::exit(1);
                }
            }
        }
        "sp-decode" => {
            if args.len() < 4 {
                eprintln!(
                    "Error: sp-decode requires <.model> [--reverse] <token_id> [token_id...]"
                );
                print_usage();
                process::exit(2);
            }
            let model = match load_sp_model(&args[2]) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("Error: {e}");
                    process::exit(1);
                }
            };
            let mut reverse = false;
            let mut tokens = Vec::new();
            for arg in &args[3..] {
                if arg == "--reverse" {
                    reverse = true;
                    continue;
                }
                match arg.parse::<u32>() {
                    Ok(t) => tokens.push(t),
                    Err(_) => {
                        eprintln!("Error: token ids must be non-negative integers (got '{arg}')");
                        process::exit(2);
                    }
                }
            }
            if tokens.is_empty() {
                eprintln!("Error: sp-decode requires at least one token id");
                process::exit(2);
            }
            match sentencepiece::decode_pieces_with_options(
                &model,
                &tokens,
                sentencepiece::DecodeOptions { reverse },
            ) {
                Ok(s) => println!("{s}"),
                Err(e) => {
                    eprintln!("Error: {e}");
                    process::exit(1);
                }
            }
        }
        "sp-normalize" => {
            if args.len() < 4 {
                eprintln!("Error: sp-normalize requires <.model> <text>");
                print_usage();
                process::exit(2);
            }
            let model = match load_sp_model(&args[2]) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("Error: {e}");
                    process::exit(1);
                }
            };
            match sentencepiece::normalize(&model, &args[3]) {
                Ok(s) => println!("{s}"),
                Err(e) => {
                    eprintln!("Error: {e}");
                    process::exit(1);
                }
            }
        }
        "sp-nbest" => {
            if args.len() < 5 {
                eprintln!("Error: sp-nbest requires <.model> <text> <n>");
                print_usage();
                process::exit(2);
            }
            let model = match load_sp_model(&args[2]) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("Error: {e}");
                    process::exit(1);
                }
            };
            let n: usize = match args[4].parse() {
                Ok(v) if v >= 1 => v,
                _ => {
                    eprintln!("Error: n must be an integer >= 1");
                    process::exit(2);
                }
            };
            let opts = match parse_sp_encode_flags(&args[5..]) {
                Ok((o, consumed)) if consumed == args[5..].len() => o,
                Ok(_) => {
                    eprintln!("Error: unexpected positional args after flags");
                    process::exit(2);
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    process::exit(2);
                }
            };
            match sentencepiece::nbest_encode_with_options(&model, &args[3], n, opts) {
                Ok(nbests) => {
                    let rows: Vec<serde_json::Value> = nbests
                        .iter()
                        .map(
                            |(tokens, score)| serde_json::json!({"tokens": tokens, "score": score}),
                        )
                        .collect();
                    println!("{}", serde_json::json!(rows));
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    process::exit(1);
                }
            }
        }
        "sp-sample" => {
            if args.len() < 4 {
                eprintln!("Error: sp-sample requires <.model> <text> [flags]");
                print_usage();
                process::exit(2);
            }
            let model = match load_sp_model(&args[2]) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("Error: {e}");
                    process::exit(1);
                }
            };
            let mut sample = SampleEncodeOptions::default();
            let mut i = 4;
            while i < args.len() {
                match args[i].as_str() {
                    "--silent-unk" => {
                        sample.encode.allow_silent_unk = true;
                        i += 1;
                    }
                    "--no-merge-unk" => {
                        sample.encode.merge_consecutive_unk = false;
                        i += 1;
                    }
                    "--reverse" => {
                        sample.encode.reverse = true;
                        i += 1;
                    }
                    "--bos" => {
                        sample.encode.add_bos = true;
                        i += 1;
                    }
                    "--eos" => {
                        sample.encode.add_eos = true;
                        i += 1;
                    }
                    "--extra" => {
                        i += 1;
                        let spec = match args.get(i) {
                            Some(s) => s,
                            None => {
                                eprintln!(
                                    "Error: --extra requires a colon-separated ExtraOptions string"
                                );
                                process::exit(2);
                            }
                        };
                        if let Err(e) = sample.encode.apply_extra_options_str(spec) {
                            eprintln!("Error: {e}");
                            process::exit(2);
                        }
                        i += 1;
                    }
                    "--nbest" => {
                        i += 1;
                        let v: i32 = match args.get(i).and_then(|s| s.parse().ok()) {
                            Some(v) => v,
                            None => {
                                eprintln!("Error: --nbest requires an integer");
                                process::exit(2);
                            }
                        };
                        sample.nbest_size = v;
                        i += 1;
                    }
                    "--alpha" => {
                        i += 1;
                        let v: f32 = match args.get(i).and_then(|s| s.parse().ok()) {
                            Some(v) => v,
                            None => {
                                eprintln!("Error: --alpha requires a float");
                                process::exit(2);
                            }
                        };
                        sample.alpha = v;
                        i += 1;
                    }
                    "--seed" => {
                        i += 1;
                        let v: u64 = match args.get(i).and_then(|s| s.parse().ok()) {
                            Some(v) => v,
                            None => {
                                eprintln!("Error: --seed requires a u64");
                                process::exit(2);
                            }
                        };
                        sample.seed = Some(v);
                        i += 1;
                    }
                    other => {
                        eprintln!("Error: unknown flag '{other}'");
                        process::exit(2);
                    }
                }
            }
            match sentencepiece::sample_encode_with_options(&model, &args[3], sample) {
                Ok(tokens) => println!("{}", serde_json::json!(tokens)),
                Err(e) => {
                    eprintln!("Error: {e}");
                    process::exit(1);
                }
            }
        }
        "sp-sample-score" => {
            if args.len() < 4 {
                eprintln!("Error: sp-sample-score requires <.model> <text> [flags]");
                print_usage();
                process::exit(2);
            }
            let model = match load_sp_model(&args[2]) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("Error: {e}");
                    process::exit(1);
                }
            };
            let mut sample = SampleEncodeAndScoreOptions::default();
            let mut i = 4;
            while i < args.len() {
                match args[i].as_str() {
                    "--silent-unk" => {
                        sample.encode.allow_silent_unk = true;
                        i += 1;
                    }
                    "--no-merge-unk" => {
                        sample.encode.merge_consecutive_unk = false;
                        i += 1;
                    }
                    "--reverse" => {
                        sample.encode.reverse = true;
                        i += 1;
                    }
                    "--bos" => {
                        sample.encode.add_bos = true;
                        i += 1;
                    }
                    "--eos" => {
                        sample.encode.add_eos = true;
                        i += 1;
                    }
                    "--extra" => {
                        i += 1;
                        let spec = match args.get(i) {
                            Some(s) => s,
                            None => {
                                eprintln!(
                                    "Error: --extra requires a colon-separated ExtraOptions string"
                                );
                                process::exit(2);
                            }
                        };
                        if let Err(e) = sample.encode.apply_extra_options_str(spec) {
                            eprintln!("Error: {e}");
                            process::exit(2);
                        }
                        i += 1;
                    }
                    "--samples" => {
                        i += 1;
                        let v: usize = match args.get(i).and_then(|s| s.parse().ok()) {
                            Some(v) if v >= 1 => v,
                            _ => {
                                eprintln!("Error: --samples requires an integer >= 1");
                                process::exit(2);
                            }
                        };
                        sample.num_samples = v;
                        i += 1;
                    }
                    "--alpha" => {
                        i += 1;
                        let v: f32 = match args.get(i).and_then(|s| s.parse().ok()) {
                            Some(v) => v,
                            None => {
                                eprintln!("Error: --alpha requires a float");
                                process::exit(2);
                            }
                        };
                        sample.alpha = v;
                        i += 1;
                    }
                    "--seed" => {
                        i += 1;
                        let v: u64 = match args.get(i).and_then(|s| s.parse().ok()) {
                            Some(v) => v,
                            None => {
                                eprintln!("Error: --seed requires a u64");
                                process::exit(2);
                            }
                        };
                        sample.seed = Some(v);
                        i += 1;
                    }
                    "--wor" => {
                        sample.wor = true;
                        i += 1;
                    }
                    "--include-best" => {
                        sample.include_best = true;
                        i += 1;
                    }
                    other => {
                        eprintln!("Error: unknown flag '{other}'");
                        process::exit(2);
                    }
                }
            }
            match sentencepiece::sample_encode_and_score_with_options(&model, &args[3], sample) {
                Ok(rows) => {
                    let out: Vec<serde_json::Value> = rows
                        .iter()
                        .map(
                            |(tokens, score)| serde_json::json!({"tokens": tokens, "score": score}),
                        )
                        .collect();
                    println!("{}", serde_json::json!(out));
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    process::exit(1);
                }
            }
        }
        "test" => {
            if args.len() < 3 {
                eprintln!("Error: test requires <vocab>");
                print_usage();
                process::exit(2);
            }
            let path = &args[2];
            if is_model_proto_path(path) {
                let model = match sentencepiece::load_model_proto_file(path) {
                    Ok(m) => m,
                    Err(e) => {
                        eprintln!("Error: {e}");
                        process::exit(1);
                    }
                };
                if model.vocab.vocab.is_empty() {
                    eprintln!("Error: empty vocabulary");
                    process::exit(1);
                }
                if !model.vocab.validate_roundtrip() {
                    eprintln!("Error: round-trip validation failed");
                    process::exit(1);
                }
                let encode_status = if model.encode_supported {
                    "supported"
                } else {
                    "unsupported"
                };
                println!(
                    "{{\"ok\":true,\"kind\":\"sentencepiece_model_proto\",\"model_type\":\"{}\",\
                     \"encode\":\"{}\",\"vocab_size\":{},\"table_size\":{}}}",
                    model.model_type.as_str(),
                    encode_status,
                    model.vocab.vocab.len(),
                    model.vocab.table_size
                );
            } else {
                let vocab = match load_vocab(path) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("Error: {e}");
                        process::exit(1);
                    }
                };
                if vocab.vocab.is_empty() {
                    eprintln!("Error: empty vocabulary");
                    process::exit(1);
                }
                if !vocab.validate_roundtrip() {
                    eprintln!("Error: round-trip validation failed");
                    process::exit(1);
                }
                println!(
                    "{{\"ok\":true,\"kind\":\"hash_vocab\",\"vocab_size\":{},\"table_size\":{}}}",
                    vocab.vocab.len(),
                    vocab.table_size
                );
            }
        }
        "generate" => {
            if args.len() < 4 {
                eprintln!("Error: generate requires <vocab_in> <output.json>");
                print_usage();
                process::exit(2);
            }
            let vocab = match load_vocab(&args[2]) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("Error: {e}");
                    process::exit(1);
                }
            };
            if !vocab.validate_roundtrip() {
                eprintln!("Error: generated vocabulary failed encode/decode validation");
                process::exit(1);
            }
            let json = match serde_json::to_string_pretty(&vocab) {
                Ok(j) => j,
                Err(e) => {
                    eprintln!("Error: serialize: {e}");
                    process::exit(1);
                }
            };
            if let Err(e) = fs::write(&args[3], &json) {
                eprintln!("Error: write '{}': {e}", args[3]);
                process::exit(1);
            }
            println!("Perfect hash vocab written to {}", args[3]);
        }
        other => {
            eprintln!("Error: unknown command '{other}'");
            print_usage();
            process::exit(2);
        }
    }
}
