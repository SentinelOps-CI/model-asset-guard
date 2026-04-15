import Lake
open Lake DSL

package modelassetguard {
  -- CI policy: warnings fail the build (same as `lean -DwarningAsError=true`).
  leanOptions := #[⟨`warningAsError, true⟩]
}

@[default_target]
lean_lib ModelAssetGuard {
  srcDir := "src/lean"
}

-- Test suite
lean_exe tests {
  root := `Tests
  srcDir := "src/lean"
}

-- Benchmark suite
lean_exe benchmarks {
  root := `cli.Benchmarks.Main
  srcDir := "src/lean"
}

-- CLI tools
lean_exe quantbound {
  root := `cli.QuantBound.Main
  srcDir := "src/lean"
}

lean_exe verifyweights {
  root := `cli.VerifyWeights.Main
  srcDir := "src/lean"
}

lean_exe tokenizertest {
  root := `cli.TokenizerTest.Main
  srcDir := "src/lean"
}

-- Bit-flip corpus test (W-4 requirement)
lean_exe bitflipcorpus {
  root := `cli.BitFlipCorpus.Main
  srcDir := "src/lean"
}

-- 128 vectors/layer quantization verification (Q-4 requirement)
lean_exe quantverify128 {
  root := `cli.QuantVerify128.Main
  srcDir := "src/lean"
}

-- Perfect hash tokenizer CLI (T-3 requirement)
lean_exe perfecthash {
  root := `cli.PerfectHash.Main
  srcDir := "src/lean"
}
