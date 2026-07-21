use guardd::perfect_hash::PerfectHashVocab;
use serde::Deserialize;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::process;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum VocabFile {
    /// List of `[word, token]` pairs.
    Pairs(Vec<(String, u32)>),
    /// Object with a `words` array (tokens assigned 1..N).
    Words { words: Vec<String> },
}

fn print_usage() {
    eprintln!(
        "Usage: gen_perfect_hash <vocab.txt|vocab.json> [<output.json>|--output <output.json>]"
    );
}

fn parse_args(args: &[String]) -> Result<(String, Option<String>), String> {
    if args.len() < 2 {
        return Err("missing vocab path".into());
    }
    let vocab_path = args[1].clone();
    let mut output_path = None;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--output" | "-o" => {
                let path = args
                    .get(i + 1)
                    .ok_or_else(|| "--output requires a path".to_string())?;
                output_path = Some(path.clone());
                i += 2;
            }
            other if !other.starts_with('-') && output_path.is_none() => {
                output_path = Some(other.to_string());
                i += 1;
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok((vocab_path, output_path))
}

fn load_vocab(vocab_path: &str, content: &str) -> Result<PerfectHashVocab, String> {
    if vocab_path.ends_with(".json") {
        let parsed: VocabFile =
            serde_json::from_str(content).map_err(|e| format!("invalid vocab JSON: {e}"))?;
        let mph = match parsed {
            VocabFile::Pairs(pairs) => PerfectHashVocab::build(pairs),
            VocabFile::Words { words } => PerfectHashVocab::from_words(words),
        };
        Ok(mph)
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

fn main() {
    let args: Vec<String> = env::args().collect();
    let (vocab_path, output_path) = match parse_args(&args) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error: {e}");
            print_usage();
            process::exit(1);
        }
    };

    if fs::metadata(&vocab_path).is_err() {
        eprintln!("Error: File '{vocab_path}' not found");
        process::exit(1);
    }

    let content = match fs::read_to_string(&vocab_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to read vocab file '{vocab_path}': {e}");
            process::exit(1);
        }
    };

    let mph = match load_vocab(&vocab_path, &content) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    };

    if !mph.validate_roundtrip() {
        eprintln!("Error: generated vocabulary failed encode/decode validation");
        process::exit(1);
    }

    let json = match serde_json::to_string_pretty(&mph) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("Failed to serialize perfect hash vocab: {e}");
            process::exit(1);
        }
    };

    match output_path {
        Some(path) => {
            if let Err(e) = fs::write(&path, &json) {
                eprintln!("Failed to write output file '{path}': {e}");
                process::exit(1);
            }
            println!("CHD perfect-hash vocab written to {path}");
        }
        None => {
            if let Err(e) = io::stdout().write_all(json.as_bytes()) {
                eprintln!("Failed to write to stdout: {e}");
                process::exit(1);
            }
        }
    }
}
