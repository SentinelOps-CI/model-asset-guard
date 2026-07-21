import ModelAssetGuard.RustSidecar
import Init.System.IO

open ModelAssetGuard.RustSidecar

/--
QuantBound Lean CLI.

Requires a real LayerSpec JSON describing model layers. Delegates to
`guardd_quant128` — does not substitute hardcoded sample layers.
-/
def main (args : List String) : IO UInt32 := do
  let args := stripLakeDashDash args
  match args with
  | [] => do
    IO.println "Usage: quantbound <layers.json> [--output <file>] [--verify]"
    IO.println ""
    IO.println "layers.json must contain real layer payloads:"
    IO.println "  [{\"name\", \"weights\", \"fan_in\", \"fan_out\", \"quant_type\"}, ...]"
    IO.println ""
    IO.println "This CLI no longer invents sample layers. Provide weights from your model"
    IO.println "(e.g. export via Python ModelAssetGuard / state_dict dump)."
    IO.println ""
    IO.println "Build helper:"
    IO.println "  cargo build --release --manifest-path src/rust/guardd/Cargo.toml --bin guardd_quant128"
    return 1
  | path :: rest => do
    -- Accept and ignore legacy --verify / --tolerance flags after validating them.
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
      | some "--verify" =>
        i := i + 1
      | some "--tolerance" =>
        match rest[i + 1]? with
        | some _ =>
          IO.println "Warning: --tolerance is ignored; Rust epsilon bounds are used."
          i := i + 2
        | none => do
          IO.println "Error: --tolerance requires a value"
          return 1
      | some arg => do
        IO.println s!"Unknown argument: {arg}"
        return 1
      | none => break

    -- Fail closed if the layers file is missing (do not fall back to samples).
    let fp : System.FilePath := path
    if !(← fp.pathExists) then
      IO.println s!"Error: layers file not found: {path}"
      return 1
    runExitCode "guardd_quant128" outArgs
