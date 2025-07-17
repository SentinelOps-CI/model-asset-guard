@echo off
REM Model Asset Guard Repository Restructuring Script (Windows)
REM This script migrates the current repository structure to follow modern software engineering standards

echo 🚀 Starting Model Asset Guard repository restructuring...
echo This will reorganize the codebase to follow modern software engineering standards.
echo.

REM Create new directory structure
echo 📁 Creating new directory structure...

REM Main source directories
if not exist "src\lean\cli" mkdir "src\lean\cli"
if not exist "src\rust\guardd" mkdir "src\rust\guardd"
if not exist "src\rust\tools" mkdir "src\rust\tools"
if not exist "src\python\model_asset_guard" mkdir "src\python\model_asset_guard"
if not exist "src\python\scripts" mkdir "src\python\scripts"

REM Test directories
if not exist "tests\unit\lean" mkdir "tests\unit\lean"
if not exist "tests\unit\rust" mkdir "tests\unit\rust"
if not exist "tests\unit\python" mkdir "tests\unit\python"
if not exist "tests\integration\lean" mkdir "tests\integration\lean"
if not exist "tests\integration\rust" mkdir "tests\integration\rust"
if not exist "tests\integration\python" mkdir "tests\integration\python"
if not exist "tests\e2e" mkdir "tests\e2e"
if not exist "tests\performance" mkdir "tests\performance"
if not exist "tests\fixtures\models" mkdir "tests\fixtures\models"
if not exist "tests\fixtures\tokenizers" mkdir "tests\fixtures\tokenizers"
if not exist "tests\fixtures\weights" mkdir "tests\fixtures\weights"
if not exist "tests\harness" mkdir "tests\harness"

REM Bindings directories
if not exist "bindings\python" mkdir "bindings\python"
if not exist "bindings\nodejs" mkdir "bindings\nodejs"
if not exist "bindings\cpp" mkdir "bindings\cpp"

REM Examples directories
if not exist "examples\lean" mkdir "examples\lean"
if not exist "examples\rust" mkdir "examples\rust"
if not exist "examples\python" mkdir "examples\python"

REM Scripts directories
if not exist "scripts\ci" mkdir "scripts\ci"

REM Config directories
if not exist "config\lean" mkdir "config\lean"
if not exist "config\rust" mkdir "config\rust"
if not exist "config\python" mkdir "config\python"

REM Tools directories
if not exist "tools\linting" mkdir "tools\linting"
if not exist "tools\formatting" mkdir "tools\formatting"
if not exist "tools\analysis" mkdir "tools\analysis"

REM Documentation directories
if not exist "docs\api" mkdir "docs\api"
if not exist "docs\guides" mkdir "docs\guides"
if not exist "docs\specs" mkdir "docs\specs"

echo ✅ Directory structure created

REM Move Lean CLI applications
echo 📦 Moving Lean CLI applications...
if exist "VerifyWeights" (
    move "VerifyWeights" "src\lean\cli\"
)
if exist "QuantBound" (
    move "QuantBound" "src\lean\cli\"
)
if exist "TokenizerTest" (
    move "TokenizerTest" "src\lean\cli\"
)
if exist "BitFlipCorpus" (
    move "BitFlipCorpus" "src\lean\cli\"
)
if exist "QuantVerify128" (
    move "QuantVerify128" "src\lean\cli\"
)
if exist "PerfectHash" (
    move "PerfectHash" "src\lean\cli\"
)
if exist "Benchmarks" (
    move "Benchmarks" "src\lean\cli\"
)

REM Move Rust code
echo 📦 Moving Rust code...
if exist "guardd" (
    xcopy "guardd\*" "src\rust\guardd\" /E /I /Y
    rmdir "guardd" /S /Q
)

REM Move Python bindings
echo 📦 Moving Python bindings...
if exist "pytorch_guard.py" (
    move "pytorch_guard.py" "bindings\python\"
)

REM Move Node.js bindings
echo 📦 Moving Node.js bindings...
if exist "node_guard.js" (
    move "node_guard.js" "bindings\nodejs\"
)

REM Move test files
echo 📦 Moving test files...
if exist "Tests" (
    if exist "Tests\Main.lean" (
        move "Tests\Main.lean" "tests\unit\lean\"
    )
    if exist "Tests\integration" (
        xcopy "Tests\integration\*" "tests\integration\lean\" /E /I /Y
        rmdir "Tests\integration" /S /Q
    )
    if exist "Tests\e2e" (
        xcopy "Tests\e2e\*" "tests\e2e\" /E /I /Y
        rmdir "Tests\e2e" /S /Q
    )
    if exist "Tests\performance" (
        xcopy "Tests\performance\*" "tests\performance\" /E /I /Y
        rmdir "Tests\performance" /S /Q
    )
    if exist "Tests\fixtures" (
        xcopy "Tests\fixtures\*" "tests\fixtures\" /E /I /Y
        rmdir "Tests\fixtures" /S /Q
    )
    rmdir "Tests" /S /Q
)

REM Move benchmark files
echo 📦 Moving benchmark files...
if exist "bench" (
    if exist "bench\test_harness.py" (
        move "bench\test_harness.py" "tests\harness\"
    )
    if exist "bench\run.sh" (
        move "bench\run.sh" "tests\harness\"
    )
    if exist "bench\quant_verify_128.py" (
        move "bench\quant_verify_128.py" "tests\performance\"
    )
    if exist "bench\bitflip_corpus.py" (
        move "bench\bitflip_corpus.py" "tests\performance\"
    )
    rmdir "bench" /S /Q
)

REM Move integration test files
echo 📦 Moving integration test files...
if exist "test_huggingface_integration.py" (
    move "test_huggingface_integration.py" "tests\e2e\"
)
if exist "test_perfect_hash_integration.py" (
    move "test_perfect_hash_integration.py" "tests\e2e\"
)

REM Move scripts
echo 📦 Moving scripts...
if exist "bundle.sh" (
    move "bundle.sh" "scripts\"
)

echo ✅ Files moved successfully

echo.
echo 🎉 Repository restructuring completed!
echo.
echo 📋 Summary of changes:
echo   ✅ Created new directory structure
echo   ✅ Moved Lean CLI applications to src\lean\cli\
echo   ✅ Moved Rust code to src\rust\
echo   ✅ Moved Python bindings to bindings\python\
echo   ✅ Moved Node.js bindings to bindings\nodejs\
echo   ✅ Consolidated tests under tests\
echo.
echo 📝 Next steps:
echo   1. Review the new structure in REPOSITORY_STRUCTURE.md
echo   2. Update import paths in your code
echo   3. Test the build system: make build
echo   4. Run tests: make test
echo   5. Update CI/CD workflows if needed
echo.
echo 📚 Documentation:
echo   - REPOSITORY_STRUCTURE.md - New structure overview
echo   - README_NEW_STRUCTURE.md - Detailed usage guide
echo.
pause 