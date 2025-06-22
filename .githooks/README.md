# Git Hooks for Omikuji

This directory contains git hooks to maintain code quality standards.

## Setup

To enable these git hooks in your local repository, run:

```bash
./.githooks/setup.sh
```

## Available Hooks

### pre-commit
- Runs `cargo fmt --check` to verify code formatting
- Runs `cargo clippy -- -D warnings` to check for linting issues
- Prevents commits if either check fails

## Bypassing Hooks

In exceptional cases where you need to commit without running checks:

```bash
git commit --no-verify -m "Your commit message"
```

**Note:** Use this sparingly and ensure CI checks pass.

## Disabling/Re-enabling Hooks

To temporarily disable hooks:
```bash
git config --unset core.hooksPath
```

To re-enable hooks:
```bash
./.githooks/setup.sh
```

## Adding Tests to Pre-commit

The pre-commit hook includes commented-out code to run tests. To enable:

1. Edit `.githooks/pre-commit`
2. Uncomment the test section
3. Save the file

Note that this will make commits slower as tests need to pass.