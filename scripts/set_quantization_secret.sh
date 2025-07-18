#!/bin/bash

# Script to set the MAG_QUANT_MAX_ERROR GitHub secret
# This script helps configure the quantization error threshold for CI/CD

set -e

echo "🔧 Setting up quantization error threshold for Model Asset Guard CI/CD"
echo ""

# Check if gh CLI is installed
if ! command -v gh &> /dev/null; then
    echo "❌ Error: GitHub CLI (gh) is not installed"
    echo "Please install it from: https://cli.github.com/"
    exit 1
fi

# Check if user is authenticated
if ! gh auth status &> /dev/null; then
    echo "❌ Error: Not authenticated with GitHub CLI"
    echo "Please run: gh auth login"
    exit 1
fi

# Get the error threshold from user input or use default
DEFAULT_THRESHOLD="0.025"
echo "Enter the maximum quantization error threshold (default: $DEFAULT_THRESHOLD):"
read -r USER_THRESHOLD

# Use default if no input provided
THRESHOLD=${USER_THRESHOLD:-$DEFAULT_THRESHOLD}

# Validate the threshold is a valid number
if ! [[ "$THRESHOLD" =~ ^[0-9]+\.?[0-9]*$ ]] || (( $(echo "$THRESHOLD <= 0" | bc -l) )); then
    echo "❌ Error: Invalid threshold value. Must be a positive number."
    exit 1
fi

echo ""
echo "📋 Summary:"
echo "  Repository: $(gh repo view --json nameWithOwner -q .nameWithOwner)"
echo "  Secret: MAG_QUANT_MAX_ERROR"
echo "  Value: $THRESHOLD"
echo ""

# Confirm the action
echo "Do you want to set this secret? (y/N):"
read -r CONFIRM

if [[ "$CONFIRM" =~ ^[Yy]$ ]]; then
    echo "Setting GitHub secret..."
    gh secret set MAG_QUANT_MAX_ERROR -b "$THRESHOLD"
    echo "✅ Secret set successfully!"
    echo ""
    echo "The quantization verification job will now use this threshold."
    echo "You can verify it's set by running: gh secret list"
else
    echo "❌ Secret not set. Exiting."
    exit 1
fi

echo ""
echo "📝 Next steps:"
echo "  1. The secret will be used in the next CI/CD run"
echo "  2. You can monitor the quantization verification job in GitHub Actions"
echo "  3. To update the threshold later, run this script again"
echo ""
echo "🔍 To verify the secret is working:"
echo "  1. Push a commit to trigger CI/CD"
echo "  2. Check the 'quantization-verification' job in GitHub Actions"
echo "  3. Look for the threshold value in the job logs" 