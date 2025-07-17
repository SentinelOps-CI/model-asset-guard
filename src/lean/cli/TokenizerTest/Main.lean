import ModelAssetGuard.Token.Tokenizer
import Init.System.IO
import Init.Data.List.Basic
import Init.Data.String.Basic

def main (args : List String) : IO UInt32 := do
  match args with
  | [] => do
    IO.println "Usage: tokenizer-test <tokenizer_path> [--type <bpe|sentencepiece>] [--fuzz <num_tests>]"
    return 1
  | tokenizerPath :: rest => do
    let mut tokenizerType := "bpe"
    let mut fuzzTests := none

    -- Parse arguments
    let mut i := 0
    while i < rest.length do
      match rest.get? i with
      | some "--type" =>
        match rest.get? (i + 1) with
        | some t =>
          tokenizerType := t
          i := i + 2
        | none => do
          IO.println "Error: --type requires a value"
          return 1
      | some "--fuzz" =>
        match rest.get? (i + 1) with
        | some num =>
          match String.toNat? num with
          | some n =>
            fuzzTests := some n
            i := i + 2
          | none => do
            IO.println "Error: --fuzz requires a numeric value"
            return 1
        | none => do
          IO.println "Error: --fuzz requires a value"
          return 1
      | some arg => do
        IO.println s!"Unknown argument: {arg}"
        return 1
      | none => break

    -- Create sample tokenizer (in practice, this would load from file)
    let tokenizer := match tokenizerType with
      | "bpe" => BPETokenizer.mk
        [("hello", 1), ("world", 2), ("test", 3)]
        [("he", "llo", "hello")]
      | "sentencepiece" => SentencePieceTokenizer.mk
        [("hello", 1), ("world", 2), ("test", 3)]
        #[] -- Empty protobuf for demo
      | _ => do
        IO.println s!"Unknown tokenizer type: {tokenizerType}"
        return 1

    -- Test basic determinism
    IO.println s!"Testing {tokenizerType} tokenizer determinism..."
    let testStrings := ["hello world", "test string", "determinism test"]

    let mut allPassed := true
    for testStr in testStrings do
      let tokens := encode tokenizer testStr
      let decoded := decode tokenizer tokens
      let passed := decoded == testStr

      if passed then
        IO.println s!"✓ '{testStr}' → {tokens} → '{decoded}'"
      else
        IO.println s!"✗ '{testStr}' → {tokens} → '{decoded}'"
        allPassed := false

    -- Fuzz testing
    match fuzzTests with
    | some numTests => do
      IO.println s!"Running fuzz test with {numTests} random strings..."
      let fuzzResult ← fuzz_test_tokenizer tokenizer numTests

      if fuzzResult then
        IO.println s!"✓ Fuzz test passed ({numTests} tests)"
      else
        IO.println s!"✗ Fuzz test failed ({numTests} tests)"
        allPassed := false
    | none => pure ()

    -- Final result
    if allPassed then
      IO.println "All tests passed"
      return 0
    else
      IO.println "Some tests failed"
      return 1
