import Init.System.IO
import Init.System.FilePath

namespace ModelAssetGuard.RustSidecar

/-- `lake exe foo -- args` may forward a literal `--` as argv[0]; drop it. -/
def stripLakeDashDash (args : List String) : List String :=
  match args with
  | "--" :: rest => rest
  | other => other

/-- Candidate paths for a Rust `guardd_*` helper binary. -/
def candidates (binName : String) : List System.FilePath :=
  [ s!"src/rust/guardd/target/release/{binName}",
    s!"src/rust/guardd/target/debug/{binName}",
    s!"src/rust/guardd/target/release/{binName}.exe",
    s!"src/rust/guardd/target/debug/{binName}.exe" ]

/-- Locate a built helper binary by name, if present. -/
def findBin (binName : String) : IO (Option System.FilePath) := do
  for candidate in candidates binName do
    if (← candidate.pathExists) then
      return some candidate
  return none

/-- Run a Rust helper; returns stdout on exit 0, else none. -/
def runCapturing (binName : String) (args : Array String) : IO (Option String) := do
  match (← findBin binName) with
  | none => return none
  | some bin => do
    let out ← IO.Process.output { cmd := bin.toString, args }
    if out.exitCode != 0 then
      if out.stderr.trim != "" then
        IO.eprintln out.stderr.trim
      if out.stdout.trim != "" then
        IO.eprintln out.stdout.trim
      return none
    return some out.stdout.trim

/-- Run a Rust helper and return its exit code (255 if binary missing). -/
def runExitCode (binName : String) (args : Array String) : IO UInt32 := do
  match (← findBin binName) with
  | none => do
    IO.eprintln s!"Error: Rust helper `{binName}` not found."
    IO.eprintln "Build with:"
    IO.eprintln s!"  cargo build --release --manifest-path src/rust/guardd/Cargo.toml --bin {binName}"
    return 255
  | some bin => do
    let out ← IO.Process.output { cmd := bin.toString, args }
    if out.stdout.trim != "" then
      IO.println out.stdout.trim
    if out.stderr.trim != "" then
      IO.eprintln out.stderr.trim
    return out.exitCode

/-- Print a missing-binary hint for `binName`. -/
def missingBinHint (binName : String) : IO Unit := do
  IO.println s!"Error: Rust helper `{binName}` not found or failed."
  IO.println "Build with:"
  IO.println s!"  cargo build --release --manifest-path src/rust/guardd/Cargo.toml --bin {binName}"

end ModelAssetGuard.RustSidecar
