@echo off
REM Script to set the MAG_QUANT_MAX_ERROR GitHub secret
REM This script helps configure the quantization error threshold for CI/CD

echo 🔧 Setting up quantization error threshold for Model Asset Guard CI/CD
echo.

REM Check if gh CLI is installed
gh --version >nul 2>&1
if errorlevel 1 (
    echo ❌ Error: GitHub CLI (gh) is not installed
    echo Please install it from: https://cli.github.com/
    pause
    exit /b 1
)

REM Check if user is authenticated
gh auth status >nul 2>&1
if errorlevel 1 (
    echo ❌ Error: Not authenticated with GitHub CLI
    echo Please run: gh auth login
    pause
    exit /b 1
)

REM Get the error threshold from user input or use default
set DEFAULT_THRESHOLD=0.025
echo Enter the maximum quantization error threshold (default: %DEFAULT_THRESHOLD%):
set /p USER_THRESHOLD=

REM Use default if no input provided
if "%USER_THRESHOLD%"=="" set USER_THRESHOLD=%DEFAULT_THRESHOLD%

echo.
echo 📋 Summary:
for /f "tokens=*" %%i in ('gh repo view --json nameWithOwner -q .nameWithOwner') do set REPO_NAME=%%i
echo   Repository: %REPO_NAME%
echo   Secret: MAG_QUANT_MAX_ERROR
echo   Value: %USER_THRESHOLD%
echo.

REM Confirm the action
echo Do you want to set this secret? (y/N):
set /p CONFIRM=

if /i "%CONFIRM%"=="y" (
    echo Setting GitHub secret...
    gh secret set MAG_QUANT_MAX_ERROR -b "%USER_THRESHOLD%"
    echo ✅ Secret set successfully!
    echo.
    echo The quantization verification job will now use this threshold.
    echo You can verify it's set by running: gh secret list
) else (
    echo ❌ Secret not set. Exiting.
    pause
    exit /b 1
)

echo.
echo 📝 Next steps:
echo   1. The secret will be used in the next CI/CD run
echo   2. You can monitor the quantization verification job in GitHub Actions
echo   3. To update the threshold later, run this script again
echo.
echo 🔍 To verify the secret is working:
echo   1. Push a commit to trigger CI/CD
echo   2. Check the 'quantization-verification' job in GitHub Actions
echo   3. Look for the threshold value in the job logs
echo.
pause 