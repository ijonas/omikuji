# Git Hooks Guide

This guide explains how to use and configure git hooks for maintaining code quality in Omikuji development.

## Overview

Git hooks are scripts that run automatically at certain points in the git workflow. Omikuji uses pre-commit hooks to ensure code quality before changes are committed.

## Setup

### Quick Setup

Run the setup script from the project root:

```bash
./.githooks/setup.sh
```

This configures git to use the hooks in `.githooks/` directory.

### Manual Setup

If you prefer manual configuration:

```bash
git config core.hooksPath .githooks
```

### Verify Setup

Check that hooks are configured:

```bash
git config --get core.hooksPath
# Should output: .githooks
```

## Available Hooks

### pre-commit

The pre-commit hook runs before each commit and performs:

1. **Format Check** (`cargo fmt --check`)
   - Ensures code follows Rust formatting standards
   - Fails if any files need formatting

2. **Lint Check** (`cargo clippy -- -D warnings`)
   - Runs Clippy static analysis
   - Fails on any warnings or errors

Example output:

```
Running pre-commit checks...
Checking code formatting...
✅ Code formatting check passed!

Running clippy linter...
✅ Clippy check passed!
```

## Hook Implementation

The pre-commit hook script (`.githooks/pre-commit`):

```bash
#!/bin/sh
set -e

echo "Running pre-commit checks..."

# Check if cargo is available
if ! command -v cargo &> /dev/null; then
    echo "❌ Error: cargo is not installed or not in PATH"
    exit 1
fi

# Check formatting
echo "Checking code formatting..."
if ! cargo fmt -- --check; then
    echo "❌ Code formatting check failed!"
    echo "Please run 'cargo fmt' to fix formatting issues."
    exit 1
fi
echo "✅ Code formatting check passed!"

# Run clippy
echo "Running clippy linter..."
if ! cargo clippy -- -D warnings; then
    echo "❌ Clippy check failed!"
    echo "Please fix the issues reported by clippy."
    exit 1
fi
echo "✅ Clippy check passed!"

# Optional: Run tests (uncomment to enable)
# echo "Running tests..."
# if ! cargo test --quiet; then
#     echo "❌ Tests failed!"
#     exit 1
# fi
# echo "✅ All tests passed!"

echo "✅ All pre-commit checks passed!"
```

## Customizing Hooks

### Adding Test Runs

To run tests before every commit:

1. Edit `.githooks/pre-commit`
2. Uncomment the test section:

```bash
echo "Running tests..."
if ! cargo test --quiet; then
    echo "❌ Tests failed!"
    exit 1
fi
echo "✅ All tests passed!"
```

**Note**: This will slow down commits significantly.

### Adding Custom Checks

Add your own checks to the pre-commit hook:

```bash
# Example: Check for TODO comments
echo "Checking for TODO comments..."
if git diff --cached --name-only | xargs grep -l "TODO" 2>/dev/null; then
    echo "⚠️  Warning: TODO comments found in staged files"
    # Change to 'exit 1' to make this a hard failure
fi
```

### Skip Specific Files

Modify checks to exclude certain files:

```bash
# Example: Skip formatting check for generated files
if ! cargo fmt -- --check --skip-children src/generated/; then
    echo "❌ Code formatting check failed!"
    exit 1
fi
```

## Bypassing Hooks

### One-time Bypass

In exceptional cases, bypass hooks for a single commit:

```bash
git commit --no-verify -m "Emergency fix: bypass checks"
```

**⚠️ Warning**: Only use when absolutely necessary. Ensure CI checks pass.

### Temporary Disable

Disable hooks for current session:

```bash
git config --unset core.hooksPath
```

Re-enable with:

```bash
./.githooks/setup.sh
```

## Troubleshooting

### Hook Not Running

1. Check hook is executable:
   ```bash
   ls -la .githooks/pre-commit
   # Should show execute permissions (x)
   ```

2. Make executable if needed:
   ```bash
   chmod +x .githooks/pre-commit
   ```

3. Verify git configuration:
   ```bash
   git config --get core.hooksPath
   ```

### Hook Fails Incorrectly

1. Run checks manually:
   ```bash
   cargo fmt --check
   cargo clippy -- -D warnings
   ```

2. Check Rust toolchain:
   ```bash
   rustc --version
   cargo --version
   ```

3. Update dependencies:
   ```bash
   cargo update
   ```

### Performance Issues

If hooks are too slow:

1. Run checks in parallel:
   ```bash
   cargo fmt --check & 
   cargo clippy -- -D warnings &
   wait
   ```

2. Skip expensive checks:
   - Comment out test runs
   - Use `--quick` flags where available

3. Use cargo-watch during development:
   ```bash
   cargo watch -x fmt -x clippy
   ```

## Best Practices

### For Developers

1. **Run checks before committing**: Don't rely solely on hooks
2. **Fix issues immediately**: Don't bypass and "fix later"
3. **Keep hooks fast**: Long-running hooks discourage use
4. **Test hooks locally**: Ensure they work on your machine

### For Maintainers

1. **Document hook requirements**: List all tools needed
2. **Provide bypass instructions**: For emergency situations
3. **Keep hooks simple**: Complex hooks are fragile
4. **Version control hooks**: Track changes to hook scripts

## CI Integration

Git hooks complement CI but don't replace it:

- **Hooks**: Fast, local checks
- **CI**: Comprehensive validation

Ensure CI runs the same checks:

```yaml
# .github/workflows/ci.yml
- name: Check formatting
  run: cargo fmt -- --check

- name: Run clippy
  run: cargo clippy -- -D warnings
```

## Advanced Configuration

### Multiple Hook Directories

Use global and local hooks:

```bash
# Global hooks (all projects)
git config --global core.hooksPath ~/.githooks

# Project-specific hooks
cd /path/to/omikuji
git config core.hooksPath .githooks
```

### Shared Team Hooks

1. Commit hooks to repository (already done)
2. Document setup in README
3. Add setup to onboarding process
4. Consider automatic setup scripts

### Hook Templates

Create templates for new hooks:

```bash
# .githooks/template
#!/bin/sh
set -e

echo "Running [HOOK NAME]..."

# Add checks here

echo "✅ [HOOK NAME] passed!"
```

## See Also

- [Git Hooks Documentation](https://git-scm.com/book/en/v2/Customizing-Git-Git-Hooks)
- [Contributing Guide](contributing.md)
- [Testing Guide](testing.md)