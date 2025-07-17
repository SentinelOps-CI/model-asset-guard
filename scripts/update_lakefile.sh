#!/bin/bash

# Update lakefile.lean for new repository structure

echo "📝 Updating lakefile.lean for new repository structure..."

# Backup original lakefile
cp lakefile.lean lakefile.lean.backup

# Create new lakefile.lean
cat > lakefile.lean << 'EOF'
import Lake
open Lake DSL

package model_asset_guard {
  -- add package configuration options here
}

-- Lean specifications
@[default_target]
lean_lib ModelAssetGuard {
  roots := #[`ModelAssetGuard]
}

-- CLI applications
lean_exe verifyweights {
  root := `VerifyWeights
}

lean_exe quantbound {
  root := `QuantBound
}

lean_exe tokenizertest {
  root := `TokenizerTest
}

lean_exe bitflipcorpus {
  root := `BitFlipCorpus
}

lean_exe quantverify128 {
  root := `QuantVerify128
}

lean_exe perfecthash {
  root := `PerfectHash
}

lean_exe benchmarks {
  root := `Benchmarks
}

-- Tests
lean_exe tests {
  root := `Tests
}

-- Development dependencies
require std from git "https://github.com/leanprover/std4" @ "main"

-- Rust sidecar library (optional dependency)
-- This would be built separately with Cargo
-- lean_exe rust_sidecar {
--   root := `RustSidecar
-- }
EOF

echo "✅ lakefile.lean updated for new structure"
echo "📋 Changes made:"
echo "  - Updated paths for CLI applications"
echo "  - Organized into logical sections"
echo "  - Added comments for clarity"
echo "  - Prepared for future Rust integration"
echo ""
echo "⚠️  Note: You may need to update import paths in your Lean files"
echo "   to match the new directory structure." 