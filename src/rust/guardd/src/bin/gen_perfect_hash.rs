use std::env;
use std::fs;
use std::io::{self, Write};

// Define the PerfectHashVocab structure locally for the binary
#[derive(serde::Serialize, serde::Deserialize)]
struct PerfectHashVocab {
    words: Vec<String>,
    hash_table: Vec<u32>,
}

impl PerfectHashVocab {
    fn build(vocab: Vec<(String, u32)>) -> Self {
        let words: Vec<String> = vocab.iter().map(|(word, _)| word.clone()).collect();
        let hash_table: Vec<u32> = vocab.iter().map(|(_, token)| *token).collect();
        PerfectHashVocab { words, hash_table }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: gen_perfect_hash <vocab.txt|vocab.json> [output.json]");
        std::process::exit(1);
    }
    let vocab_path = &args[1];
    let output_path = if args.len() > 2 { Some(&args[2]) } else { None };

    // Check if file exists
    if fs::metadata(vocab_path).is_err() {
        eprintln!("Error: File '{}' not found", vocab_path);
        std::process::exit(1);
    }

    // Read vocab file
    let content = match fs::read_to_string(vocab_path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Failed to read vocab file '{}': {}", vocab_path, e);
            std::process::exit(1);
        }
    };
    let vocab: Vec<(String, u32)> = if vocab_path.ends_with(".json") {
        serde_json::from_str(&content).expect("Invalid vocab JSON format")
    } else {
        // Assume newline-separated, assign tokens 1..N
        content
            .lines()
            .enumerate()
            .map(|(i, line)| (line.trim().to_string(), (i + 1) as u32))
            .collect()
    };

    let mph = PerfectHashVocab::build(vocab);
    let json = serde_json::to_string_pretty(&mph).expect("Failed to serialize perfect hash vocab");

    match output_path {
        Some(path) => {
            fs::write(path, &json).expect("Failed to write output file");
            println!("Perfect hash vocab written to {}", path);
        }
        None => {
            io::stdout().write_all(json.as_bytes()).unwrap();
        }
    }
} 