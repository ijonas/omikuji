# Multi-stage build for minimal final image
# Builder stage
FROM rust:1.82-alpine AS builder

# Install build dependencies
RUN apk add --no-cache \
    musl-dev \
    openssl-dev \
    openssl-libs-static \
    pkgconfig \
    git

# Create app directory
WORKDIR /build

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src

# Build the application
RUN cargo build --release --target x86_64-unknown-linux-musl && \
    strip target/x86_64-unknown-linux-musl/release/omikuji

# Runtime stage
FROM alpine:3.19

# Install runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    tzdata \
    && rm -rf /var/cache/apk/*

# Create non-root user
RUN addgroup -g 1000 omikuji && \
    adduser -D -u 1000 -G omikuji omikuji

# Copy binary from builder
COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/omikuji /usr/local/bin/omikuji

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