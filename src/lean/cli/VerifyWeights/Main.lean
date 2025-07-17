import ModelAssetGuard.Weights
import Init.System.IO
import Init.Data.List.Basic

def main (args : List String) : IO UInt32 := do
  match args with
  | [] => do
    IO.println "Usage: verify-weights <path> [--digest <digest>]"
    return 1
  | path :: rest => do
    let mut expectedDigest := none

    -- Parse arguments
    let mut i := 0
    while i < rest.length do
      match rest.get? i with
      | some "--digest" =>
        match rest.get? (i + 1) with
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

    -- Create checkpoint
    let checkpoint ← createCheckpoint path

    -- Verify checkpoint
    let isValid ← verify checkpoint

    if isValid then
      IO.println s!"✓ Checkpoint verified successfully"
      IO.println s!"  Path: {checkpoint.path}"
      IO.println s!"  Size: {checkpoint.size} bytes"
      IO.println s!"  Digest: {checkpoint.digest}"
      return 0
    else
      IO.println s!"✗ Checkpoint verification failed"
      IO.println s!"  Path: {checkpoint.path}"
      IO.println s!"  Expected digest: {expectedDigest}"
      IO.println s!"  Actual digest: {checkpoint.digest}"
      return 1
