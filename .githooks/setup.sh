#!/bin/bash
# Setup script for git hooks

echo "Setting up git hooks for Omikuji..."

# Get the git hooks directory
HOOKS_DIR=".githooks"

# Check if we're in a git repository
if [ ! -d ".git" ]; then
    echo "Error: Not in a git repository"
    exit 1
fi

# Configure git to use our hooks directory
git config core.hooksPath "$HOOKS_DIR"

echo "✅ Git hooks configured successfully!"
echo ""
echo "The following hooks are now active:"
echo "  - pre-commit: Runs 'cargo fmt --check' and 'cargo clippy' before each commit"
echo ""
echo "To bypass the pre-commit hook in exceptional cases, use:"
echo "  git commit --no-verify"
echo ""
echo "To disable hooks temporarily:"
echo "  git config --unset core.hooksPath"
echo ""
echo "To re-enable hooks:"
echo "  ./$HOOKS_DIR/setup.sh"