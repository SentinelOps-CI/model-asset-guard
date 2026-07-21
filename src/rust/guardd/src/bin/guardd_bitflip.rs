//! CLI helper for bit-flip corpus tests (used by Lean bitflipcorpus).
//!
//! Defaults are small (MB-scale). GB-scale sizes require `--allow-large`.

use guardd::bitflip_corpus::{BitFlipCorpusTester, MAX_DEFAULT_CORPUS_BYTES};
use std::env;
use std::process;

fn print_usage() {
    eprintln!(
        "Usage: guardd_bitflip [--size-mb <n>] [--size-gb <n>] [--corruptions <n>] [--temp-dir <path>] [--allow-large]
Defaults: --size-mb 1, --corruptions 5
Without --allow-large, size is capped at {} bytes ({} MB).",
        MAX_DEFAULT_CORPUS_BYTES,
        MAX_DEFAULT_CORPUS_BYTES / (1024 * 1024)
    );
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut size_mb: Option<u64> = None;
    let mut size_gb: Option<u64> = None;
    let mut corruptions: usize = 5;
    let mut temp_dir: Option<String> = None;
    let mut allow_large = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--size-mb" => {
                let v = args.get(i + 1).and_then(|s| s.parse().ok());
                match v {
                    Some(n) => {
                        size_mb = Some(n);
                        i += 2;
                    }
                    None => {
                        eprintln!("Error: --size-mb requires a non-negative integer");
                        process::exit(2);
                    }
                }
            }
            "--size-gb" => {
                let v = args.get(i + 1).and_then(|s| s.parse().ok());
                match v {
                    Some(n) => {
                        size_gb = Some(n);
                        i += 2;
                    }
                    None => {
                        eprintln!("Error: --size-gb requires a non-negative integer");
                        process::exit(2);
                    }
                }
            }
            "--corruptions" => {
                let v = args.get(i + 1).and_then(|s| s.parse().ok());
                match v {
                    Some(n) => {
                        corruptions = n;
                        i += 2;
                    }
                    None => {
                        eprintln!("Error: --corruptions requires a positive integer");
                        process::exit(2);
                    }
                }
            }
            "--temp-dir" => match args.get(i + 1) {
                Some(p) => {
                    temp_dir = Some(p.clone());
                    i += 2;
                }
                None => {
                    eprintln!("Error: --temp-dir requires a path");
                    process::exit(2);
                }
            },
            "--allow-large" => {
                allow_large = true;
                i += 1;
            }
            "-h" | "--help" => {
                print_usage();
                process::exit(0);
            }
            other => {
                eprintln!("Error: unknown argument '{other}'");
                print_usage();
                process::exit(2);
            }
        }
    }

    if size_mb.is_some() && size_gb.is_some() {
        eprintln!("Error: specify only one of --size-mb or --size-gb");
        process::exit(2);
    }

    let size_bytes = if let Some(gb) = size_gb {
        if gb == 0 {
            1024 * 1024
        } else {
            gb.saturating_mul(1024 * 1024 * 1024)
        }
    } else {
        let mb = size_mb.unwrap_or(1);
        mb.saturating_mul(1024 * 1024).max(1)
    };

    let temp = temp_dir.unwrap_or_else(|| {
        std::env::temp_dir()
            .join("bitflip_corpus")
            .to_string_lossy()
            .into_owned()
    });
    if let Err(e) = std::fs::create_dir_all(&temp) {
        eprintln!("Error: cannot create temp dir '{temp}': {e}");
        process::exit(1);
    }

    let tester = BitFlipCorpusTester::new(1, Some(temp));
    match tester.run_bitflip_test_bytes(size_bytes, corruptions, allow_large) {
        Ok(result) => match serde_json::to_string_pretty(&result) {
            Ok(json) => {
                println!("{json}");
                process::exit(if result.test_passed { 0 } else { 1 });
            }
            Err(e) => {
                eprintln!("Error: serialize result: {e}");
                process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("Error: bitflip corpus failed: {e}");
            process::exit(1);
        }
    }
}
