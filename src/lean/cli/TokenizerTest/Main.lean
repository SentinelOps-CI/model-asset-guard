import ModelAssetGuard.RustSidecar
import Init.System.IO

open ModelAssetGuard.RustSidecar

/--
TokenizerTest Lean CLI.

Runs real vocab round-trip checks via `guardd_perfect_hash test`.
Supports open-addressed hash vocab JSON/txt and SentencePiece `.model`
(ModelProto pieces; CHAR/BPE/UNIGRAM encode is `guardd_perfect_hash sp-encode`).
Does not claim full official SentencePiece parity.
-/
def main (args : List String) : IO UInt32 := do
  let args := stripLakeDashDash args
  match args with
  | [] => do
    IO.println "Usage: tokenizertest <vocab.json|vocab.txt|.model> [--type <perfecthash|sentencepiece>] [--fuzz <n>]"
    IO.println ""
    IO.println "Runtime checks go through Rust guardd_perfect_hash:"
    IO.println "  - JSON/txt: open-addressed hash vocab round-trip"
    IO.println "  - .model: ModelProto pieces + vocab round-trip (reports encode gate)"
    IO.println "For CHAR/BPE/UNIGRAM segmentation: guardd_perfect_hash sp-encode (docs/sentencepiece-bpe.md)."
    IO.println "Lean structural multi-merge lemmas are separate from SP score-BPE."
    IO.println ""
    IO.println "Build helper:"
    IO.println "  cargo build --release --manifest-path src/rust/guardd/Cargo.toml --bin guardd_perfect_hash"
    return 1
  | path :: rest => do
    let mut tokType : String := "auto"
    let mut fuzz : Option Nat := none
    let mut i : Nat := 0
    while i < rest.length do
      match rest[i]? with
      | some "--type" =>
        match rest[i + 1]? with
        | some t =>
          tokType := t
          i := i + 2
        | none => do
          IO.println "Error: --type requires a value"
          return 1
      | some "--fuzz" =>
        match rest[i + 1]? with
        | some nStr =>
          match nStr.toNat? with
          | some n =>
            fuzz := some n
            i := i + 2
          | none => do
            IO.println "Error: --fuzz requires a non-negative integer"
            return 1
        | none => do
          IO.println "Error: --fuzz requires a value"
          return 1
      | some arg => do
        IO.println s!"Unknown argument: {arg}"
        return 1
      | none => break

    let lower := path.toLower
    let looksModel := lower.endsWith ".model" || lower.endsWith ".proto"
    match tokType with
    | "auto" | "perfecthash" | "perfect-hash" | "mph" | "sentencepiece" | "sp" =>
      if tokType == "sentencepiece" || tokType == "sp" then
        if !looksModel then
          IO.println "Error: --type sentencepiece requires a .model / .proto ModelProto file."
          return 1
      pure ()
    | "bpe" => do
      IO.println "Error: tokenizer type 'bpe' is not a tokenizertest mode."
      IO.println "Structural Lean merges: lake tests. SP CHAR/BPE/UNIGRAM: guardd_perfect_hash sp-encode."
      return 1
    | other => do
      IO.println s!"Error: unknown tokenizer type '{other}'"
      return 1

    let fp : System.FilePath := path
    if !(← fp.pathExists) then
      IO.println s!"Error: vocab file not found: {path}"
      return 1

    match fuzz with
    | some n =>
      if n > 0 then
        IO.println s!"Note: --fuzz {n} requests extra trials; Rust validate_roundtrip covers the vocab."
    | none => pure ()

    runExitCode "guardd_perfect_hash" #["test", path]
