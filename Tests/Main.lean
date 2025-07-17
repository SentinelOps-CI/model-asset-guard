import ModelAssetGuard.Weights
import ModelAssetGuard.Quant.Core
import ModelAssetGuard.Quant.LayerBound
import ModelAssetGuard.Token.Tokenizer
import Init.System.IO
import Init.Data.List.Basic

def testWeights : IO Bool := do
  IO.println "Testing weight integrity module..."

  -- Create a test file
  let testContent := "test checkpoint data"
  let testPath := "test_checkpoint.bin"
  IO.FS.writeFile testPath testContent

  -- Test checkpoint creation
  let checkpoint ← createCheckpoint testPath
  let expectedSize := testContent.length
  let expectedDigest := Crypto.SHA256.hash testContent

  let sizeOk := checkpoint.size == expectedSize
  let digestOk := checkpoint.digest == expectedDigest

  IO.println s!"  Size check: {sizeOk}"
  IO.println s!"  Digest check: {digestOk}"

  -- Test verification
  let isValid ← verify checkpoint
  IO.println s!"  Verification: {isValid}"

  -- Cleanup
  IO.FS.removeFile testPath

  return sizeOk && digestOk && isValid

def testQuantization : IO Bool := do
  IO.println "Testing quantization module..."

  -- Test rounding function
  let testValues := [0.5, 1.5, -0.5, -1.5]
  let mut allOk := true

  for x in testValues do
    let rounded := Quant.round_int8 x
    let error := Quant.quant_error x
    let boundOk := |error| ≤ 0.5 * Quant.ulp_int8

    IO.println s!"  x={x}, rounded={rounded}, error={error}, bound_ok={boundOk}"
    allOk := allOk && boundOk

  -- Test layer configuration
  let config := LayerConfig.mk 512 512 "int8"
  let epsilon := compute_epsilon_bound config
  let epsilonOk := epsilon > 0

  IO.println s!"  Epsilon bound: {epsilon}, positive: {epsilonOk}"

  return allOk && epsilonOk

def testTokenizer : IO Bool := do
  IO.println "Testing tokenizer module..."

  -- Test BPE tokenizer
  let bpeTokenizer := BPETokenizer.mk
    [("hello", 1), ("world", 2)]
    [("he", "llo", "hello")]

  let testString := "hello world"
  let tokens := encode bpeTokenizer testString
  let decoded := decode bpeTokenizer tokens

  let determinismOk := decoded == testString
  IO.println s!"  BPE determinism: {determinismOk}"
  IO.println s!"    '{testString}' → {tokens} → '{decoded}'"

  -- Test SentencePiece tokenizer
  let spTokenizer := SentencePieceTokenizer.mk
    [("hello", 1), ("world", 2)]
    #[]

  let spTokens := encode spTokenizer testString
  let spDecoded := decode spTokenizer spTokens

  let spDeterminismOk := spDecoded == testString
  IO.println s!"  SentencePiece determinism: {spDeterminismOk}"
  IO.println s!"    '{testString}' → {spTokens} → '{spDecoded}'"

  return determinismOk && spDeterminismOk

def main (args : List String) : IO UInt32 := do
  IO.println "Running Model Asset Guard tests..."
  IO.println ""

  let weightsOk ← testWeights
  IO.println ""

  let quantOk ← testQuantization
  IO.println ""

  let tokenizerOk ← testTokenizer
  IO.println ""

  let allOk := weightsOk && quantOk && tokenizerOk

  if allOk then
    IO.println "✓ All tests passed"
    return 0
  else
    IO.println "✗ Some tests failed"
    return 1
