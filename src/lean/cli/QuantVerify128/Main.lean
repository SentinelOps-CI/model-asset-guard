import ModelAssetGuard.RustSidecar
import Init.System.IO

open ModelAssetGuard.RustSidecar

/--
QuantVerify128 Lean CLI.

Requires a LayerSpec JSON file and delegates to Rust `guardd_quant128`.
Does not invent sample layers or print fake pass rates.
-/
def main (args : List String) : IO UInt32 := do
  let args := stripLakeDashDash args
  match args with
  | [] => do
    IO.println "Usage: quantverify128 <layers.json> [--output <file>]"
    IO.println ""
    IO.println "layers.json: JSON array of {name, weights, fan_in, fan_out, quant_type}"
    IO.println "Verification uses Rust 128-vector sampling (vector error ||Wq x - W x||/||x||)."
    IO.println ""
    IO.println "Build helper:"
    IO.println "  cargo build --release --manifest-path src/rust/guardd/Cargo.toml --bin guardd_quant128"
    return 1
  | path :: rest => do
    let mut outArgs : Array String := #[path]
    let mut i : Nat := 0
    while i < rest.length do
      match rest[i]? with
      | some "--output" | some "-o" =>
        match rest[i + 1]? with
        | some p =>
          outArgs := outArgs.push "--output" |>.push p
          i := i + 2
        | none => do
          IO.println "Error: --output requires a path"
          return 1
      | some arg => do
        IO.println s!"Unknown argument: {arg}"
        return 1
      | none => break
    runExitCode "guardd_quant128" outArgs
