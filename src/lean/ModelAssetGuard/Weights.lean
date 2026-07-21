import Init.System.IO
import Init.Data.String.Basic
import Init.System.FilePath

namespace ModelAssetGuard

/-- Checkpoint type representing a model weight file with integrity metadata -/
structure Checkpoint where
  path : String
  size : Nat
  digest : String
  deriving Repr

/--
Lean-only placeholder digest (NOT SHA-256).

Used only for in-process Lean unit experiments. Production / CLI verification
must use the Rust `guardd_sha256` binary (real SHA-256).
-/
def simpleDigest (content : String) : String :=
  let folded := content.foldl (fun acc c => (acc * 131 + c.toNat) % 1000000007) 0
  toString folded

/-- Candidate locations for the Rust SHA-256 helper. -/
def guarddSha256Candidates : List System.FilePath :=
  [ "src/rust/guardd/target/release/guardd_sha256",
    "src/rust/guardd/target/debug/guardd_sha256",
    "src/rust/guardd/target/release/guardd_sha256.exe",
    "src/rust/guardd/target/debug/guardd_sha256.exe" ]

/-- Locate a built `guardd_sha256` binary, if present. -/
def findGuarddSha256 : IO (Option System.FilePath) := do
  for candidate in guarddSha256Candidates do
    if (← candidate.pathExists) then
      return some candidate
  return none

/--
Compute lowercase hex SHA-256 via the Rust sidecar CLI.

Returns `none` if the binary is missing or the process fails.
-/
def sha256HexViaRust (path : String) : IO (Option String) := do
  match (← findGuarddSha256) with
  | none => return none
  | some bin => do
    let out ← IO.Process.output {
      cmd := bin.toString
      args := #[path]
    }
    if out.exitCode != 0 then
      return none
    return some out.stdout.trim

/-- Verify that a checkpoint's digest matches its file content (placeholder digest). -/
def verify (checkpoint : Checkpoint) : IO Bool := do
  let content ← IO.FS.readFile checkpoint.path
  let computedDigest := simpleDigest content
  return computedDigest == checkpoint.digest

/-- Create a checkpoint from a file path using the Lean placeholder digest. -/
def createCheckpoint (path : String) : IO Checkpoint := do
  let content ← IO.FS.readFile path
  let digest := simpleDigest content
  let size := content.length
  return { path, size, digest }

/-- Verify checkpoint with size validation (placeholder digest). -/
def verifyWithSize (checkpoint : Checkpoint) : IO Bool := do
  let content ← IO.FS.readFile checkpoint.path
  let computedDigest := simpleDigest content
  let size := content.length
  return computedDigest == checkpoint.digest && size == checkpoint.size

end ModelAssetGuard
