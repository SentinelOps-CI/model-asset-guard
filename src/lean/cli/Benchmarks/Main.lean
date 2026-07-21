import ModelAssetGuard.RustSidecar
import Init.System.IO

open ModelAssetGuard.RustSidecar

/--
Measure real SHA-256 throughput via `guardd_sha256` on a temporary file.

Does not print invented MB/s targets. Exits non-zero if the helper is missing.
-/
def main (_args : List String) : IO UInt32 := do
  IO.println "Model Asset Guard benchmarks"
  IO.println "----------------------------"

  match (← findBin "guardd_sha256") with
  | none => do
    missingBinHint "guardd_sha256"
    return 1
  | some bin => do
    let tmp := "bench_guardd_sha256.tmp"
    -- 256 KiB deterministic payload (honest timing, not a claimed target)
    let body := String.mk (List.replicate (256 * 1024) 'x')
    IO.FS.writeFile tmp body
    let t0 ← IO.monoMsNow
    let out ← IO.Process.output { cmd := bin.toString, args := #[tmp] }
    let t1 ← IO.monoMsNow
    try IO.FS.removeFile tmp catch _ => pure ()
    if out.exitCode != 0 then
      IO.println "Error: guardd_sha256 failed during benchmark"
      return 1
    let elapsedMs := t1 - t0
    let bytes := body.length
    let mb := bytes.toFloat / (1024.0 * 1024.0)
    let secs := elapsedMs.toFloat / 1000.0
    let mbs := if secs > 0.0 then mb / secs else 0.0
    IO.println s!"op: sha256 via {bin}"
    IO.println s!"bytes: {bytes}"
    IO.println s!"elapsed_ms: {elapsedMs}"
    IO.println s!"throughput_MBps: {mbs}"
    IO.println s!"digest: {out.stdout.trim}"
    IO.println "status: measured_ok"
    return 0
