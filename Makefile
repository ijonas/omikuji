.PHONY: build run test clean fmt lint check doc release install help

# Default target
all: build

# Build the project
build:
	cargo build

# Run the project with default config
run:
	cargo run

# Run with custom config file
run-config:
	cargo run -- -c config.yaml

# Run all tests
test:
	cargo test

# Run tests with output
test-verbose:
	cargo test -- --nocapture

# Clean build artifacts
clean:
	cargo clean

# Format code
fmt:
	cargo fmt

# Check code formatting
fmt-check:
	cargo fmt --check

# Run clippy linter
lint:
	cargo clippy

# Run clippy with warnings as errors
lint-strict:
	cargo clippy -- -D warnings

# Run all checks (format, lint, test)
check: fmt-check lint test

# Build optimized release version
release:
	cargo build --release

# Run release version
run-release:
	cargo run --release

# Generate documentation
doc:
	cargo doc --open

# Install the binary locally
install:
	cargo install --path .

# Update dependencies
update:
	cargo update

# Check for outdated dependencies
outdated:
	cargo outdated

# Run security audit
audit:
	cargo audit

# Development mode with auto-reload (requires cargo-watch)
watch:
	cargo watch -x run

# Run with debug logging
debug:
	RUST_LOG=debug cargo run

# Run with trace logging
trace:
	RUST_LOG=trace cargo run

# Help target
help:
	@echo "Available targets:"
	@echo "  make build        - Build the project"
	@echo "  make run          - Run with default config"
	@echo "  make run-config   - Run with config.yaml"
	@echo "  make test         - Run all tests"
	@echo "  make test-verbose - Run tests with output"
	@echo "  make clean        - Clean build artifacts"
	@echo "  make fmt          - Format code"
	@echo "  make fmt-check    - Check code formatting"
	@echo "  make lint         - Run clippy linter"
	@echo "  make lint-strict  - Run clippy with strict warnings"
	@echo "  make check        - Run all checks (format, lint, test)"
	@echo "  make release      - Build optimized release"
	@echo "  make run-release  - Run release version"
	@echo "  make doc          - Generate documentation"
	@echo "  make install      - Install binary locally"
	@echo "  make update       - Update dependencies"
	@echo "  make outdated     - Check for outdated dependencies"
	@echo "  make audit        - Run security audit"
	@echo "  make watch        - Run with auto-reload (needs cargo-watch)"
	@echo "  make debug        - Run with debug logging"
	@echo "  make trace        - Run with trace logging"
	@echo "  make help         - Show this help message"