# Multi-stage build for minimal final image
# Builder stage
FROM rust:1.82-bookworm AS builder

# Create app directory
WORKDIR /build

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src

# Build the application
RUN cargo build --release && \
    strip target/release/omikuji

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    tzdata \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -r -g 1000 omikuji && \
    useradd -r -u 1000 -g omikuji omikuji

# Copy binary from builder
COPY --from=builder /build/target/release/omikuji /usr/local/bin/omikuji

# Create config directory
RUN mkdir -p /config && chown omikuji:omikuji /config

# Switch to non-root user
USER omikuji

# Set working directory
WORKDIR /config

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:8080/metrics || exit 1

# Expose metrics port
EXPOSE 8080

# Default command
ENTRYPOINT ["/usr/local/bin/omikuji"]
CMD ["-c", "/config/config.yaml"]