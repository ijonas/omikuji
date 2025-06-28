#!/bin/bash
# Run linting checks that match GitHub Actions CI

set -e

echo "=== Running Omikuji Linting (GitHub Actions CI Settings) ==="
echo

echo "1. Checking code formatting..."
if cargo fmt -- --check; then
    echo "✓ Code formatting is correct"
else
    echo "✗ Code formatting issues found!"
    echo "  Run 'cargo fmt' to fix formatting"
    exit 1
fi
echo

echo "2. Running clippy with CI settings..."
if cargo clippy -- -D warnings -D clippy::uninlined_format_args; then
    echo "✓ Clippy checks passed"
else
    echo "✗ Clippy found issues!"
    echo "  Run 'cargo clippy --fix -- -D warnings -D clippy::uninlined_format_args' to fix some issues automatically"
    exit 1
fi
echo

echo "3. Running clippy on all targets..."
if cargo clippy --all-targets --all-features -- -D warnings -D clippy::uninlined_format_args; then
    echo "✓ All target checks passed"
else
    echo "✗ Clippy found issues in tests or examples!"
    exit 1
fi
echo

echo "=== All linting checks passed! ✓ ==="
echo "Your code is ready for GitHub Actions CI"