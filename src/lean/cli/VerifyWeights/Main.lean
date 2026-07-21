import ModelAssetGuard.Weights
import ModelAssetGuard.RustSidecar
import Init.System.IO
import Init.Data.List.Basic

open ModelAssetGuard
open ModelAssetGuard.RustSidecar

/--
VerifyWeights CLI.

When `--digest` is provided, verification uses real SHA-256 via the Rust
`guardd_sha256` binary. If that binary is unavailable, the process exits
non-zero instead of claiming success with the Lean placeholder digest.
-/
def main (args : List String) : IO UInt32 := do
  let args := stripLakeDashDash args
  match args with
  | [] => do
    IO.println "Usage: verifyweights <path> [--digest <hex_sha256>]"
    IO.println ""
    IO.println "SHA-256 is computed by the Rust helper `guardd_sha256`."
    IO.println "Build it with:"
    IO.println "  cargo build --release --manifest-path src/rust/guardd/Cargo.toml --bin guardd_sha256"
    return 1
  | path :: rest => do
    let mut expectedDigest : Option String := none
    let mut i : Nat := 0
    while i < rest.length do
      match rest[i]? with
      | some "--digest" =>
        match rest[i + 1]? with
        | some digest =>
          expectedDigest := some digest
          i := i + 2
        | none => do
          IO.println "Error: --digest requires a value"
          return 1
      | some arg => do
        IO.println s!"Unknown argument: {arg}"
        return 1
      | none => break

    match (← sha256HexViaRust path) with
    | none => do
      IO.println "Error: could not compute SHA-256 via Rust `guardd_sha256`."
      IO.println "Build the helper and retry:"
      IO.println "  cargo build --release --manifest-path src/rust/guardd/Cargo.toml --bin guardd_sha256"
      IO.println "Lean `simpleDigest` is a placeholder and is NOT used for CLI verification."
      return 1
    | some actual => do
      match expectedDigest with
      | none => do
        IO.println "SHA-256 (via Rust guardd_sha256):"
        IO.println s!"  Path: {path}"
        IO.println s!"  Digest: {actual}"
        return 0
      | some expected => do
        if actual == expected then
          IO.println "Checkpoint SHA-256 verified successfully"
          IO.println s!"  Path: {path}"
          IO.println s!"  Digest: {actual}"
          return 0
        else
          IO.println "Checkpoint SHA-256 verification failed"
          IO.println s!"  Path: {path}"
          IO.println s!"  Expected: {expected}"
          IO.println s!"  Actual:   {actual}"
          return 1
