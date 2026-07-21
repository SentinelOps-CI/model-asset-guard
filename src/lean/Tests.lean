import ModelAssetGuard.Weights
import ModelAssetGuard.Quant.Core
import ModelAssetGuard.Quant.LayerBound
import ModelAssetGuard.Token.Tokenizer
import ModelAssetGuard.Token.CHD
import ModelAssetGuard.Token.HashSeeded
import ModelAssetGuard.Token.CHDBuild
import ModelAssetGuard.RustSidecar
import Init.System.IO

open ModelAssetGuard
open ModelAssetGuard.Quant
open ModelAssetGuard.Quant.Fixed
open ModelAssetGuard.Token
open ModelAssetGuard.Token.CHD
open ModelAssetGuard.Token.HashSeeded
open ModelAssetGuard.Token.CHDBuild
open ModelAssetGuard.RustSidecar

/-- Fail the harness with a message. -/
def fail (msg : String) : IO Bool := do
  IO.println s!"FAIL: {msg}"
  return false

/-- Pass with a message. -/
def pass (msg : String) : IO Bool := do
  IO.println s!"PASS: {msg}"
  return true

/-- simpleDigest must be deterministic for the same input. -/
def testSimpleDigestDeterministic : IO Bool := do
  let a := simpleDigest "checkpoint-bytes"
  let b := simpleDigest "checkpoint-bytes"
  let c := simpleDigest "other-bytes"
  if a != b then
    fail "simpleDigest not deterministic"
  else if a == c then
    fail "simpleDigest collided on distinct inputs (unlikely but treated as fail)"
  else
    pass "simpleDigest deterministic"

/-- F015: quantization_error_matrix must be Wq - W, not dequantized Wq. -/
def testQuantizationErrorMatrix : IO Bool := do
  let W : WeightMatrix := #[#[0.4, -0.6]]
  let Wq := dequantize_matrix (quantize_matrix W)
  let E := quantization_error_matrix W
  -- Element (0,0): error = dequant(round(0.4)) - 0.4
  let e00 := E[0]![0]!
  let expected := Wq[0]![0]! - W[0]![0]!
  if Float.abs (e00 - expected) > 1e-9 then
    fail s!"quantization_error_matrix[0][0]={e00} expected {expected}"
  else if Float.abs e00 < 1e-12 && Float.abs (W[0]![0]! - Wq[0]![0]!) > 1e-9 then
    -- If there is a nonzero quant error, E must not be identical to Wq alone
    -- (this branch is a secondary sanity check).
    fail "quantization_error_matrix looks like dequantized weights"
  else
    pass "quantization_error_matrix is Wq - W"

/-- F027: vocab_size must reflect vocab length. -/
def testVocabSize : IO Bool := do
  let t : PerfectHashTokenizer := {
    vocab := [("hello", 1), ("world", 2)]
    hash_table := []
  }
  let n := Tokenizer.vocab_size t
  if n != 2 then
    fail s!"PerfectHashTokenizer.vocab_size={n} expected 2"
  else
    pass "Tokenizer.vocab_size uses vocab.length"

/-- Optional: if guardd_sha256 is built, a tiny file must hash successfully. -/
def testGuarddSha256IfPresent : IO Bool := do
  match (← findBin "guardd_sha256") with
  | none => do
    IO.println "SKIP: guardd_sha256 not built (optional)"
    return true
  | some _ => do
    let path := "lean_tests_sha256.tmp"
    IO.FS.writeFile path "model-asset-guard-test"
    match (← sha256HexViaRust path) with
    | none => do
      try IO.FS.removeFile path catch _ => pure ()
      fail "guardd_sha256 present but sha256HexViaRust failed"
    | some hex => do
      try IO.FS.removeFile path catch _ => pure ()
      if hex.length != 64 then
        fail s!"unexpected digest length {hex.length}"
      else
        pass s!"guardd_sha256 digest len=64"

/-- Shared fixture formula: int8 ε = 0.5 * sqrt(fan_in); fan_in=16 → 2.0. -/
def testEpsilonBoundMatchesSharedFixture : IO Bool := do
  let config : LayerConfig := { fan_in := 16, fan_out := 4, quant_type := "int8" }
  let eps := compute_epsilon_bound config
  let expected : Float := 2.0
  if Float.abs (eps - expected) > 1e-6 then
    fail s!"int8 epsilon for fan_in=16 is {eps}, expected {expected} (shared fixture)"
  else
    pass "compute_epsilon_bound matches shared int8 fixture (0.5*sqrt(16)=2)"

/-- Fixed-point half-ULP smoke: round(5/2)=3 and error form holds. -/
def testFixedRoundHalfUlp : IO Bool := do
  let hd : (2 : Nat) > 0 := by decide
  let q := roundDivNat 5 2 hd
  let err := 2 * Int.natAbs ((5 : Int) - (q : Int) * (2 : Int))
  -- theorem roundDivNat_error guarantees err ≤ 2
  if q != 3 then
    fail s!"roundDivNat 5/2 = {q}, expected 3"
  else if err > 2 then
    fail s!"half-ULP residual {err} > 2"
  else
    pass "Fixed.roundDivNat half-ULP smoke (5/2 → 3)"

/-- Empty-vocab BPE stub round-trips a simple string. -/
def testBpeCharLevelRoundtrip : IO Bool := do
  let t : BPETokenizer := { vocab := [], merges := [] }
  let s := "ab"
  let tokens := bpe_encode t s
  let decoded := bpe_decode t tokens
  if decoded != s then
    fail s!"bpe roundtrip {s} → {tokens} → {decoded}"
  else
    pass "bpe_char_level empty-vocab roundtrip"

/-- Nonempty faithful single-char vocab still round-trips (merges unused). -/
def testBpeFaithfulNonemptyRoundtrip : IO Bool := do
  let t : BPETokenizer := {
    vocab := [("a", 'a'.toNat), ("b", 'b'.toNat)]
    merges := []
  }
  let s := "ab"
  let tokens := bpe_encode t s
  let decoded := bpe_decode t tokens
  if decoded != s then
    fail s!"bpe faithful nonempty roundtrip {s} → {tokens} → {decoded}"
  else
    pass "bpe_faithful nonempty single-char vocab roundtrip"

/-- Merge step shortens when the pair occurs. -/
def testBpeMergeOnceShortens : IO Bool := do
  let tokens : List Token := [1, 2, 3]
  let merged := apply_merge_once tokens 1 2 9
  if merged != [9, 3] then
    fail s!"apply_merge_once expected [9,3], got {merged}"
  else if merged.length >= tokens.length then
    fail "apply_merge_once should shorten on hit"
  else
    pass "apply_merge_once shortens on adjacent pair"

/-- Multi-merge table applies left-to-right and never lengthens. -/
def testBpeMergeTable : IO Bool := do
  let tokens : List Token := [1, 2, 3, 4]
  let merges : List (Token × Token × Token) := [(1, 2, 9), (9, 3, 8)]
  let merged := apply_merge_table tokens merges
  if merged != [8, 4] then
    fail s!"apply_merge_table expected [8,4], got {merged}"
  else if merged.length > tokens.length then
    fail "apply_merge_table lengthened tokens"
  else
    pass "apply_merge_table multi-merge shortens stepwise"

/-- No-pair merge is identity; append composes schedules. -/
def testBpeMergeIdempotentAndCompose : IO Bool := do
  let tokens : List Token := [1, 3, 2]
  let once := apply_merge_once tokens 1 2 9
  if once != tokens then
    fail s!"expected identity when pair absent, got {once}"
  else
    let m₁ : List (Token × Token × Token) := [(1, 3, 7)]
    let m₂ : List (Token × Token × Token) := [(7, 2, 8)]
    let composed := apply_merge_table tokens (m₁ ++ m₂)
    let stepwise := apply_merge_table (apply_merge_table tokens m₁) m₂
    if composed != stepwise then
      fail s!"append composition mismatch {composed} vs {stepwise}"
    else if composed != [8] then
      fail s!"expected [8], got {composed}"
    else
      pass "apply_merge_once identity + apply_merge_table append compose"

/-- Encode-with-merge-table append composition (Wave 15). -/
def testBpeEncodeMergeTableAppend : IO Bool := do
  let text := "ab"
  let m₁ : List (Token × Token × Token) := [('a'.toNat, 'b'.toNat, 9)]
  let m₂ : List (Token × Token × Token) := []
  let composed := bpe_encode_with_merge_table text (m₁ ++ m₂)
  let stepwise := apply_merge_table (bpe_encode_with_merge_table text m₁) m₂
  if composed != stepwise then
    fail s!"encode merge append mismatch {composed} vs {stepwise}"
  else if composed != [9] then
    fail s!"expected [9], got {composed}"
  else
    pass "bpe_encode_with_merge_table append compose"

/-- Wave 18: abstract CHD placement well-formedness Bool check. -/
def testChdPlacementWellFormed : IO Bool := do
  if !Placement.wellFormedBool Placement.exampleTwo then
    fail "exampleTwo should be well-formed"
  else if !Placement.wellFormedBool Placement.exampleThree then
    fail "exampleThree should be well-formed"
  else if Placement.wellFormedBool Placement.exampleCollision then
    fail "exampleCollision should not be well-formed"
  else
    pass "CHD Placement.wellFormedBool accepts MPH examples, rejects collision"

/-- Wave 19: bucket-place merge preserves wellSized under disjointness. -/
def testChdBucketPlaceMerge : IO Bool := do
  let p : BucketPlace := { keyIndices := [0], slots := [1] }
  let q : BucketPlace := { keyIndices := [1], slots := [0] }
  if !BucketPlace.wellSizedBool p 2 then
    fail "p should be wellSized"
  else if !BucketPlace.wellSizedBool q 2 then
    fail "q should be wellSized"
  else if !BucketPlace.disjointBool p q then
    fail "p,q should be disjoint"
  else
    let m := BucketPlace.append p q
    if !BucketPlace.wellSizedBool m 2 then
      fail "merged place should be wellSized"
    else if m.keyIndices != [0, 1] || m.slots != [1, 0] then
      fail s!"unexpected merge {m.keyIndices} / {m.slots}"
    else
      pass "CHD BucketPlace.append merges disjoint wellSized places"

/-- Wave 20: Lean hashSeeded matches Rust/Python goldens (ASCII). -/
def testHashSeededGoldens : IO Bool := do
  let h0 := hashSeededAscii defaultSeed0 ""
  let ha := hashSeededAscii defaultSeed0 "a"
  let hab := hashSeededAscii defaultSeed0 "ab"
  let hh := hashSeededAscii defaultSeed0 "hello"
  if h0 != goldenEmptySeed0 then
    fail s!"empty hash {h0} expected {goldenEmptySeed0}"
  else if ha != goldenASeed0 then
    fail s!"'a' hash {ha} expected {goldenASeed0}"
  else if hab != goldenAbSeed0 then
    fail s!"'ab' hash {hab} expected {goldenAbSeed0}"
  else if hh != goldenHelloSeed0 then
    fail s!"'hello' hash {hh} expected {goldenHelloSeed0}"
  else if !(slotMod ha 7 (by decide : 7 > 0) < 7) then
    fail "slotMod not < n"
  else
    pass "HashSeeded goldens match Rust/Python (seed0 ASCII)"

/-- Wave 21: executable ASCII MPH builder finds a well-formed placement. -/
def testChdBuildAsciiMph : IO Bool := do
  if !buildAsciiMphOk ["a", "b", "c"] then
    fail "buildAsciiMphOk failed for [a,b,c]"
  else if !buildAsciiMphOk ["hello", "world"] then
    fail "buildAsciiMphOk failed for [hello,world]"
  else if !buildAsciiMphOk [] then
    fail "empty build should succeed"
  else
    match buildAsciiMph ["x", "y"] with
    | none => fail "expected some placement"
    | some p =>
        if p.keys != ["x", "y"] then
          fail "keys mismatch"
        else if p.slots.length != 2 then
          fail "slots length mismatch"
        else if Placement.pairwiseNeBool p.slots != true then
          fail "slots not distinct"
        else
          pass "CHDBuild.buildAsciiMph places ASCII keys injectively"

/-- Wave 22: n=1 displace completeness smoke + bucketed builder. -/
def testChdBuildWave22 : IO Bool := do
  if !findDisplace_n1_ok defaultSeed1 [97] then
    fail "findDisplace n=1 should return some 0"
  else if !buildAsciiMphBucketedOk ["a", "b", "c", "d"] then
    fail "buildAsciiMphBucketedOk failed"
  else if !buildAsciiMphBucketedOk ["hello", "world", "test"] then
    fail "buildAsciiMphBucketedOk failed for 3 words"
  else
    pass "CHDBuild Wave22 n=1 smoke + bucketed ASCII MPH"

/-- Wave 23: free-occupied completeness smoke + Lean/Rust CHD goldens. -/
def testChdBuildWave23 : IO Bool := do
  if !findDisplace_n1_ok defaultSeed1 [97] then
    fail "n=1 smoke still required"
  else if findDisplace defaultSeed1 3 (by decide) [[97]] (freeOccupied 3) 8 != some 0 then
    fail "singleton freeOccupied should return d=0 for any n>0"
  else if findDisplace defaultSeed1 2 (by decide) [] (freeOccupied 2) 4 != some 0 then
    fail "empty key list should return d=0"
  else if candidatesOkBool [0, 0] 1 (freeOccupied 1) != false then
    fail "tableSize=1 multi-key gate should fail"
  else if !goldenBucketedNOk then
    fail "Lean/Rust goldenBucketedNOk mismatch (slots/pilots)"
  else if !buildAsciiMphBucketedOk ["x", "y", "z", "w"] then
    fail "bucketed plans failed for 4 words"
  else
    match buildAsciiMphBucketedN ["a", "b", "c"] with
    | none => fail "expected bucketed-N build"
    | some b =>
        if !Placement.wellFormedBool b.placement then
          fail "golden placement not wellFormedBool"
        else if b.nBuckets != 3 then
          fail "expected n_buckets = 3"
        else
          pass "CHDBuild Wave23 freeOccupied completeness + Lean/Rust goldens"

/-- Wave 24: stable bucket order, λ-plan goldens, 2-key / occupied smokes. -/
def testChdBuildWave24 : IO Bool := do
  if !bucketOrderEqualLengthOk then
    fail "bucketProcessOrder equal-length tie-break mismatch"
  else if !bucketOrderStressOk then
    fail "bucket order stress (one/two/…, w0..w7) mismatch vs Rust"
  else if !goldenBucketedPlanOk then
    fail "λ-plan goldenBucketedPlanOk mismatch (slots/pilots/n_buckets)"
  else if !findDisplace_twoKey_free_smoke then
    fail "two-key free-table findDisplace smoke failed"
  else if !findDisplace_singleton_occupied_smoke then
    fail "singleton nonempty-occupied findDisplace smoke failed"
  else if !wave24ChdOk then
    fail "wave24ChdOk aggregate failed"
  else
    pass "CHDBuild Wave24 stable order + λ-plans + restricted ∃d smokes"

def main (_args : List String) : IO UInt32 := do
  IO.println "Model Asset Guard Lean tests"
  let r1 ← testSimpleDigestDeterministic
  let r2 ← testQuantizationErrorMatrix
  let r3 ← testVocabSize
  let r4 ← testGuarddSha256IfPresent
  let r5 ← testEpsilonBoundMatchesSharedFixture
  let r6 ← testFixedRoundHalfUlp
  let r7 ← testBpeCharLevelRoundtrip
  let r8 ← testBpeFaithfulNonemptyRoundtrip
  let r9 ← testBpeMergeOnceShortens
  let r10 ← testBpeMergeTable
  let r11 ← testBpeMergeIdempotentAndCompose
  let r12 ← testBpeEncodeMergeTableAppend
  let r13 ← testChdPlacementWellFormed
  let r14 ← testChdBucketPlaceMerge
  let r15 ← testHashSeededGoldens
  let r16 ← testChdBuildAsciiMph
  let r17 ← testChdBuildWave22
  let r18 ← testChdBuildWave23
  let r19 ← testChdBuildWave24
  let ok := r1 && r2 && r3 && r4 && r5 && r6 && r7 && r8 && r9 && r10 && r11 && r12 && r13 && r14 && r15 && r16 && r17 && r18 && r19
  if ok then
    IO.println "All Lean tests passed"
    return 0
  else
    IO.println "Lean tests failed"
    return 1
