//! CLI helper: 128-vector quantization verification from a LayerSpec JSON file
//! (used by Lean quantverify128 / quantbound).

use guardd::quant_verification::{verify_model_128_vectors, LayerSpec};
use std::env;
use std::fs;
use std::process;

fn print_usage() {
    eprintln!(
        "Usage: guardd_quant128 <layers.json> [--output <file>]
layers.json must be a JSON array of objects:
  [{{\"name\",\"weights\",\"fan_in\",\"fan_out\",\"quant_type\"}}, ...]
Exit 0 only if every layer passes."
    );
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        print_usage();
        process::exit(2);
    }

    let mut layers_path: Option<String> = None;
    let mut output_path: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--output" | "-o" => match args.get(i + 1) {
                Some(p) => {
                    output_path = Some(p.clone());
                    i += 2;
                }
                None => {
                    eprintln!("Error: --output requires a path");
                    process::exit(2);
                }
            },
            "-h" | "--help" => {
                print_usage();
                process::exit(0);
            }
            other if !other.starts_with('-') && layers_path.is_none() => {
                layers_path = Some(other.to_string());
                i += 1;
            }
            other => {
                eprintln!("Error: unknown argument '{other}'");
                print_usage();
                process::exit(2);
            }
        }
    }

    let layers_path = match layers_path {
        Some(p) => p,
        None => {
            eprintln!("Error: missing layers.json path");
            print_usage();
            process::exit(2);
        }
    };

    let content = match fs::read_to_string(&layers_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: read '{layers_path}': {e}");
            process::exit(1);
        }
    };

    let specs: Vec<LayerSpec> = match serde_json::from_str(&content) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: invalid layers JSON: {e}");
            process::exit(1);
        }
    };

    if specs.is_empty() {
        eprintln!("Error: layers JSON array is empty");
        process::exit(1);
    }

    let layers: Vec<_> = specs.into_iter().map(LayerSpec::into_tuple).collect();
    let verification = verify_model_128_vectors(layers);
    let json = match serde_json::to_string_pretty(&verification) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("Error: serialize result: {e}");
            process::exit(1);
        }
    };

    if let Some(path) = output_path {
        if let Err(e) = fs::write(&path, &json) {
            eprintln!("Error: write '{path}': {e}");
            process::exit(1);
        }
        println!("Wrote verification results to {path}");
    } else {
        println!("{json}");
    }

    let all_passed =
        verification.passed_layers == verification.total_layers && verification.total_layers > 0;
    process::exit(if all_passed { 0 } else { 1 });
}
