@echo off
REM Model Asset Guard - Final Cleanup Script
REM This script completes the repository restructuring and cleans up reconfiguration files

echo 🧹 Final cleanup and path correction...
echo.

REM Create missing directories
echo 📁 Creating missing directories...

if not exist "tests" mkdir "tests"
if not exist "tests\unit" mkdir "tests\unit"
if not exist "tests\unit\lean" mkdir "tests\unit\lean"
if not exist "tests\unit\rust" mkdir "tests\unit\rust"
if not exist "tests\unit\python" mkdir "tests\unit\python"
if not exist "tests\integration" mkdir "tests\integration"
if not exist "tests\integration\lean" mkdir "tests\integration\lean"
if not exist "tests\integration\rust" mkdir "tests\integration\rust"
if not exist "tests\integration\python" mkdir "tests\integration\python"
if not exist "tests\e2e" mkdir "tests\e2e"
if not exist "tests\performance" mkdir "tests\performance"
if not exist "tests\harness" mkdir "tests\harness"
if not exist "tests\fixtures" mkdir "tests\fixtures"

if not exist "bindings" mkdir "bindings"
if not exist "bindings\python" mkdir "bindings\python"
if not exist "bindings\nodejs" mkdir "bindings\nodejs"

if not exist "src\lean\cli\tests" mkdir "src\lean\cli\tests"

echo ✅ Directories created

REM Move remaining files to their correct locations
echo 📦 Moving remaining files...

REM Move language bindings
if exist "pytorch_guard.py" (
    echo Moving pytorch_guard.py to bindings\python\
    move "pytorch_guard.py" "bindings\python\"
)

if exist "node_guard.js" (
    echo Moving node_guard.js to bindings\nodejs\
    move "node_guard.js" "bindings\nodejs\"
)

REM Create placeholder files for empty directories (Git requirement)
echo 📝 Creating placeholder files for empty directories...
echo # This directory contains tests > tests\unit\lean\.gitkeep
echo # This directory contains tests > tests\unit\rust\.gitkeep
echo # This directory contains tests > tests\unit\python\.gitkeep
echo # This directory contains tests > tests\integration\lean\.gitkeep
echo # This directory contains tests > tests\integration\rust\.gitkeep
echo # This directory contains tests > tests\integration\python\.gitkeep
echo # This directory contains tests > tests\e2e\.gitkeep
echo # This directory contains tests > tests\performance\.gitkeep
echo # This directory contains tests > tests\harness\.gitkeep
echo # This directory contains tests > tests\fixtures\.gitkeep

echo ✅ Files moved and placeholders created

REM Clean up reconfiguration files (they're no longer needed)
echo 🗑️ Cleaning up reconfiguration files...

if exist "RESTRUCTURING_SUMMARY.md" (
    echo Removing RESTRUCTURING_SUMMARY.md...
    del "RESTRUCTURING_SUMMARY.md"
)

if exist "README_NEW_STRUCTURE.md" (
    echo Removing README_NEW_STRUCTURE.md...
    del "README_NEW_STRUCTURE.md"
)

if exist "REPOSITORY_STRUCTURE.md" (
    echo Removing REPOSITORY_STRUCTURE.md...
    del "REPOSITORY_STRUCTURE.md"
)

echo ✅ Reconfiguration files removed

REM Update the main README to reflect the new structure
echo 📝 Updating main README...
if exist "README.md" (
    echo Updating README.md with new structure information...
    copy "README.md" "README.md.backup"
)

echo ✅ README backed up

REM Create a final verification script
echo 📋 Creating verification script...
echo @echo off > scripts\verify_structure.bat
echo echo 🔍 Verifying repository structure... >> scripts\verify_structure.bat
echo echo. >> scripts\verify_structure.bat
echo echo Checking source directories... >> scripts\verify_structure.bat
echo if exist "src\lean\ModelAssetGuard" echo ✅ src\lean\ModelAssetGuard >> scripts\verify_structure.bat
echo if exist "src\lean\cli" echo ✅ src\lean\cli >> scripts\verify_structure.bat
echo if exist "src\rust\guardd" echo ✅ src\rust\guardd >> scripts\verify_structure.bat
echo if exist "src\python" echo ✅ src\python >> scripts\verify_structure.bat
echo echo. >> scripts\verify_structure.bat
echo echo Checking test directories... >> scripts\verify_structure.bat
echo if exist "tests\unit" echo ✅ tests\unit >> scripts\verify_structure.bat
echo if exist "tests\integration" echo ✅ tests\integration >> scripts\verify_structure.bat
echo if exist "tests\e2e" echo ✅ tests\e2e >> scripts\verify_structure.bat
echo if exist "tests\performance" echo ✅ tests\performance >> scripts\verify_structure.bat
echo echo. >> scripts\verify_structure.bat
echo echo Checking binding directories... >> scripts\verify_structure.bat
echo if exist "bindings\python" echo ✅ bindings\python >> scripts\verify_structure.bat
echo if exist "bindings\nodejs" echo ✅ bindings\nodejs >> scripts\verify_structure.bat
echo echo. >> scripts\verify_structure.bat
echo echo 🎉 Repository structure verification complete! >> scripts\verify_structure.bat
echo pause >> scripts\verify_structure.bat

echo ✅ Verification script created

echo.
echo 🎉 Final cleanup completed successfully!
echo.
echo 📋 Summary:
echo   ✅ Created all missing directories
echo   ✅ Moved remaining files to correct locations
echo   ✅ Created placeholder files for empty directories
echo   ✅ Removed reconfiguration files
echo   ✅ Created verification script
echo.
echo 📝 Next steps:
echo   1. Run: scripts\verify_structure.bat
echo   2. Test the build system
echo   3. Run all tests
echo   4. Update any import paths if needed
echo.
pause 