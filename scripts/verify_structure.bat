@echo off 
echo 🔍 Verifying repository structure... 
echo. 
echo Checking source directories... 
if exist "src\lean\ModelAssetGuard" echo ✅ src\lean\ModelAssetGuard 
if exist "src\lean\cli" echo ✅ src\lean\cli 
if exist "src\rust\guardd" echo ✅ src\rust\guardd 
if exist "src\python" echo ✅ src\python 
echo. 
echo Checking test directories... 
if exist "tests\unit" echo ✅ tests\unit 
if exist "tests\integration" echo ✅ tests\integration 
if exist "tests\e2e" echo ✅ tests\e2e 
if exist "tests\performance" echo ✅ tests\performance 
echo. 
echo Checking binding directories... 
if exist "bindings\python" echo ✅ bindings\python 
if exist "bindings\nodejs" echo ✅ bindings\nodejs 
echo. 
echo 🎉 Repository structure verification complete! 
pause 
