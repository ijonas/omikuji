.PHONY: build run test clean fmt lint check doc release install help tag docker docker-build docker-run

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

# Tag a new release
tag:
ifndef VERSION
	$(error VERSION is not set. Usage: make tag VERSION=x.y.z)
endif
	@echo "Updating version to $(VERSION) in Cargo.toml..."
	@# Update version in Cargo.toml (works on both macOS and Linux)
	@if [ "$$(uname)" = "Darwin" ]; then \
		sed -i '' 's/^version = ".*"/version = "$(VERSION)"/' Cargo.toml; \
	else \
		sed -i 's/^version = ".*"/version = "$(VERSION)"/' Cargo.toml; \
	fi
	@echo "Running cargo check to update Cargo.lock..."
	@cargo check --quiet
	@echo "Committing version changes..."
	@git add Cargo.toml Cargo.lock
	@git commit -m "chore: bump version to $(VERSION)" || echo "No changes to commit"
	@echo "Creating tag v$(VERSION)..."
	@git tag -a v$(VERSION) -m "Release v$(VERSION)"
	@echo "Version updated and tag created!"
	@echo "Push with: git push && git push origin v$(VERSION)"

# Build Docker image locally
docker-build:
	docker build -t omikuji:latest .

# Run Docker container
docker-run:
	docker run -v $(PWD)/config.yaml:/config/config.yaml omikuji:latest

# Build multi-platform Docker image
docker-buildx:
	docker buildx build --platform linux/amd64,linux/arm64 -t omikuji:latest .

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
	@echo "  make tag VERSION=x.y.z - Tag a new release"
	@echo "  make docker-build - Build Docker image locally"
	@echo "  make docker-run   - Run Docker container"
	@echo "  make docker-buildx - Build multi-platform Docker image"
	@echo "  make help         - Show this help message"