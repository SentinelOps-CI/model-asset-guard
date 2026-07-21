//! CLI helper: print lowercase hex SHA-256 of a file (used by Lean verifyweights).

use sha2::{Digest, Sha256};
use std::env;
use std::fs::File;
use std::io::Read;
use std::process;

fn main() {
    let mut args = env::args().skip(1);
    let path = match args.next() {
        Some(p) => p,
        None => {
            eprintln!("Usage: guardd_sha256 <path>");
            process::exit(2);
        }
    };

    let mut file = match File::open(&path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Error: cannot open '{path}': {e}");
            process::exit(1);
        }
    };

    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1024 * 64];
    loop {
        match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => hasher.update(&buf[..n]),
            Err(e) => {
                eprintln!("Error: read failed for '{path}': {e}");
                process::exit(1);
            }
        }
    }

    let digest = hasher.finalize();
    println!("{}", hex::encode(digest));
}
