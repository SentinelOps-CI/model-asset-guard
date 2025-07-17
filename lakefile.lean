import Lake
open Lake DSL

package modelassetguard {
  -- add package configuration options here
}

@[default_target]
lean_lib ModelAssetGuard {
  -- add library configuration options here
}

-- Test suite
lean_exe tests {
  root := `Tests
}

-- Benchmark suite
lean_exe benchmarks {
  root := `Benchmarks
}

-- CLI tools
lean_exe quantbound {
  root := `QuantBound
}

lean_exe verifyweights {
  root := `VerifyWeights
}

lean_exe tokenizertest {
  root := `TokenizerTest
}

-- Bit-flip corpus test (W-4 requirement)
lean_exe bitflipcorpus {
  root := `BitFlipCorpus
}

-- 128 vectors/layer quantization verification (Q-4 requirement)
lean_exe quantverify128 {
  root := `QuantVerify128
}

-- Perfect hash tokenizer CLI (T-3 requirement)
lean_exe perfecthash {
  root := `PerfectHash
}
