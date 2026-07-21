import ModelAssetGuard.RustSidecar
import Init.System.IO

open ModelAssetGuard.RustSidecar

/--
BitFlipCorpus Lean CLI.

Delegates to Rust `guardd_bitflip`. Default size is 1 MiB; GB-scale requires
`--allow-large` (never invents a fake rejection rate).
-/
def main (args : List String) : IO UInt32 := do
  let args := stripLakeDashDash args
  match args with
  | ["--help"] | ["-h"] => do
    IO.println "Usage: bitflipcorpus [--size-mb <n>] [--size-gb <n>] [--corruptions <n>] [--temp-dir <path>] [--allow-large]"
    IO.println ""
    IO.println "Runs the Rust bit-flip corpus via guardd_bitflip."
    IO.println "Default (no flags): --size-mb 1 --corruptions 5"
    IO.println "Sizes above 64 MiB require --allow-large."
    IO.println ""
    IO.println "Build helper:"
    IO.println "  cargo build --release --manifest-path src/rust/guardd/Cargo.toml --bin guardd_bitflip"
    return 0
  | [] =>
    -- Safe default: 1 MiB corpus (real work, not a simulated pass).
    runExitCode "guardd_bitflip" #["--size-mb", "1", "--corruptions", "5"]
  | _ =>
    runExitCode "guardd_bitflip" args.toArray
