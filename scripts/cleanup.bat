@echo off
REM Model Asset Guard - Cleanup Script (Windows)
REM This script removes redundant files and directories after restructuring

echo 🧹 Starting cleanup of redundant files...
echo This will remove files that are no longer needed in the new structure.
echo.

REM First, let's move any remaining files that should be preserved
echo 📦 Moving remaining files to new locations...

REM Move test files from old Tests directory
if exist "Tests\Main.lean" (
    echo Moving Tests\Main.lean to tests\unit\lean\
    move "Tests\Main.lean" "tests\unit\lean\"
)

if exist "Tests\e2e" (
    echo Moving Tests\e2e contents to tests\e2e\
    xcopy "Tests\e2e\*" "tests\e2e\" /E /I /Y
)

if exist "Tests\integration" (
    echo Moving Tests\integration contents to tests\integration\lean\
    xcopy "Tests\integration\*" "tests\integration\lean\" /E /I /Y
)

if exist "Tests\performance" (
    echo Moving Tests\performance contents to tests\performance\
    xcopy "Tests\performance\*" "tests\performance\" /E /I /Y
)

if exist "Tests\fixtures" (
    echo Moving Tests\fixtures contents to tests\fixtures\
    xcopy "Tests\fixtures\*" "tests\fixtures\" /E /I /Y
)

REM Move benchmark files from old bench directory
if exist "bench\test_harness.py" (
    echo Moving bench\test_harness.py to tests\harness\
    move "bench\test_harness.py" "tests\harness\"
)

if exist "bench\run.sh" (
    echo Moving bench\run.sh to tests\harness\
    move "bench\run.sh" "tests\harness\"
)

if exist "bench\quant_verify_128.py" (
    echo Moving bench\quant_verify_128.py to tests\performance\
    move "bench\quant_verify_128.py" "tests\performance\"
)

if exist "bench\bitflip_corpus.py" (
    echo Moving bench\bitflip_corpus.py to tests\performance\
    move "bench\bitflip_corpus.py" "tests\performance\"
)

REM Move CLI applications to new structure
if exist "VerifyWeights" (
    echo Moving VerifyWeights to src\lean\cli\
    move "VerifyWeights" "src\lean\cli\"
)

if exist "QuantBound" (
    echo Moving QuantBound to src\lean\cli\
    move "QuantBound" "src\lean\cli\"
)

if exist "TokenizerTest" (
    echo Moving TokenizerTest to src\lean\cli\
    move "TokenizerTest" "src\lean\cli\"
)

if exist "BitFlipCorpus" (
    echo Moving BitFlipCorpus to src\lean\cli\
    move "BitFlipCorpus" "src\lean\cli\"
)

if exist "QuantVerify128" (
    echo Moving QuantVerify128 to src\lean\cli\
    move "QuantVerify128" "src\lean\cli\"
)

if exist "PerfectHash" (
    echo Moving PerfectHash to src\lean\cli\
    move "PerfectHash" "src\lean\cli\"
)

if exist "Benchmarks" (
    echo Moving Benchmarks to src\lean\cli\
    move "Benchmarks" "src\lean\cli\"
)

REM Move Rust code
if exist "guardd" (
    echo Moving guardd contents to src\rust\guardd\
    xcopy "guardd\*" "src\rust\guardd\" /E /I /Y
)

REM Move Python bindings
if exist "pytorch_guard.py" (
    echo Moving pytorch_guard.py to bindings\python\
    move "pytorch_guard.py" "bindings\python\"
)

REM Move Node.js bindings
if exist "node_guard.js" (
    echo Moving node_guard.js to bindings\nodejs\
    move "node_guard.js" "bindings\nodejs\"
)

REM Move integration test files
if exist "test_huggingface_integration.py" (
    echo Moving test_huggingface_integration.py to tests\e2e\
    move "test_huggingface_integration.py" "tests\e2e\"
)

if exist "test_perfect_hash_integration.py" (
    echo Moving test_perfect_hash_integration.py to tests\e2e\
    move "test_perfect_hash_integration.py" "tests\e2e\"
)

REM Move scripts
if exist "bundle.sh" (
    echo Moving bundle.sh to scripts\
    move "bundle.sh" "scripts\"
)

echo ✅ Files moved successfully

REM Now remove redundant directories
echo 🗑️ Removing redundant directories...

if exist "Tests" (
    echo Removing old Tests directory...
    rmdir "Tests" /S /Q
)

if exist "bench" (
    echo Removing old bench directory...
    rmdir "bench" /S /Q
)

if exist "guardd" (
    echo Removing old guardd directory...
    rmdir "guardd" /S /Q
)

REM Remove Python cache directories
if exist "__pycache__" (
    echo Removing Python cache...
    rmdir "__pycache__" /S /Q
)

REM Remove test result files that will be regenerated
if exist "perfect_hash_test_results.json" (
    echo Removing old test results...
    del "perfect_hash_test_results.json"
)

REM Remove redundant documentation files (keep only the new ones)
if exist "REPOSITORY_STRUCTURE.md" (
    echo Keeping REPOSITORY_STRUCTURE.md for reference...
)

if exist "README_NEW_STRUCTURE.md" (
    echo Keeping README_NEW_STRUCTURE.md for reference...
)

if exist "RESTRUCTURING_SUMMARY.md" (
    echo Keeping RESTRUCTURING_SUMMARY.md for reference...
)

echo ✅ Redundant directories removed

REM Create a summary of what was cleaned up
echo.
echo 🎉 Cleanup completed successfully!
echo.
echo 📋 Summary of cleanup:
echo   ✅ Moved remaining test files to new structure
echo   ✅ Moved CLI applications to src\lean\cli\
echo   ✅ Moved Rust code to src\rust\guardd\
echo   ✅ Moved language bindings to bindings\
echo   ✅ Removed old Tests directory
echo   ✅ Removed old bench directory
echo   ✅ Removed old guardd directory
echo   ✅ Cleaned up Python cache
echo   ✅ Removed old test result files
echo.
echo 📝 Next steps:
echo   1. Review the new structure
echo   2. Update any remaining import paths
echo   3. Test the build system: make build
echo   4. Run tests: make test
echo.
echo 📚 Documentation preserved:
echo   - REPOSITORY_STRUCTURE.md
echo   - README_NEW_STRUCTURE.md
echo   - RESTRUCTURING_SUMMARY.md
echo.
pause 