import ModelAssetGuard.RustSidecar
import Init.System.IO

open ModelAssetGuard.RustSidecar

/--
Perfect-hash Lean CLI.

Encode/decode/test/generate delegate to the Rust `guardd_perfect_hash` sidecar.
Missing binary or sidecar failure exits non-zero (no simulated success).
-/
def main (args : List String) : IO UInt32 := do
  let args := stripLakeDashDash args
  match args with
  | [] => do
    IO.println "Usage: perfecthash <command> [options]"
    IO.println ""
    IO.println "Commands (via Rust guardd_perfect_hash):"
    IO.println "  encode <vocab_file> <text>     - Encode text using perfect hash tokenizer"
    IO.println "  decode <vocab_file> <token>    - Decode token using perfect hash tokenizer"
    IO.println "  test <vocab_file>              - Validate encode/decode round-trips"
    IO.println "  generate <vocab_file> <output> - Generate perfect hash vocabulary JSON"
    IO.println ""
    IO.println "Build helper:"
    IO.println "  cargo build --release --manifest-path src/rust/guardd/Cargo.toml --bin guardd_perfect_hash"
    return 1
  | "encode" :: vocab :: text :: _ =>
    runExitCode "guardd_perfect_hash" #["encode", vocab, text]
  | "encode" :: _ => do
    IO.println "Error: encode requires <vocab_file> <text>"
    return 1
  | "decode" :: vocab :: token :: _ =>
    runExitCode "guardd_perfect_hash" #["decode", vocab, token]
  | "decode" :: _ => do
    IO.println "Error: decode requires <vocab_file> <token>"
    return 1
  | "test" :: vocab :: _ =>
    runExitCode "guardd_perfect_hash" #["test", vocab]
  | "test" :: _ => do
    IO.println "Error: test requires <vocab_file>"
    return 1
  | "generate" :: vocab :: output :: _ =>
    runExitCode "guardd_perfect_hash" #["generate", vocab, output]
  | "generate" :: _ => do
    IO.println "Error: generate requires <vocab_file> <output>"
    return 1
  | command :: _ => do
    IO.println s!"Unknown command: {command}"
    IO.println "Use 'perfecthash' without arguments for help"
    return 1
