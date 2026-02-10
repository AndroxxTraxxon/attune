# Multi-stage Dockerfile for Attune Rust services
# This Dockerfile can build any of the Attune services by specifying a build argument
# Usage: DOCKER_BUILDKIT=1 docker build --build-arg SERVICE=api -f docker/Dockerfile -t attune-api .
#
# BuildKit cache mounts are used to speed up incremental builds by persisting:
# - Cargo registry and git cache (with sharing=locked to prevent race conditions)
# - Rust incremental compilation artifacts
#
# This dramatically reduces rebuild times from ~5 minutes to ~30 seconds for code-only changes.

ARG RUST_VERSION=1.92
ARG DEBIAN_VERSION=bookworm

# ============================================================================
# Stage 1: Builder - Compile the Rust services
# ============================================================================
FROM rust:${RUST_VERSION}-${DEBIAN_VERSION} AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy workspace manifests and source code
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
COPY migrations/ ./migrations/
COPY .sqlx/ ./.sqlx/

# Build argument to specify which service to build
ARG SERVICE=api

# Build the specified service with BuildKit cache mounts
# Cache mount sharing modes prevent race conditions during parallel builds:
# - sharing=locked: Only one build can access the cache at a time (prevents file conflicts)
# - cargo registry/git: Locked to prevent "File exists" errors when extracting dependencies
# - target: Locked to prevent compilation artifact conflicts
#
# This is slower than parallel builds but eliminates race conditions.
# Alternative: Use docker-compose --build with --no-parallel flag, or build sequentially.
#
# First build: ~5-6 minutes
# Incremental builds (code changes only): ~30-60 seconds
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/build/target,sharing=locked \
    cargo build --release --bin attune-${SERVICE} && \
    cp /build/target/release/attune-${SERVICE} /build/attune-service-binary

# ============================================================================
# Stage 2: Pack Binaries Builder - Build native pack binaries with GLIBC 2.36
# ============================================================================
FROM rust:${RUST_VERSION}-${DEBIAN_VERSION} AS pack-builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
COPY .sqlx/ ./.sqlx/

# Build pack binaries (sensors, etc.) with GLIBC 2.36 for maximum compatibility
# These binaries will work on any system with GLIBC 2.36 or newer
# IMPORTANT: Copy binaries WITHIN the cache mount, before it's unmounted
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/build/target,sharing=locked \
    mkdir -p /build/pack-binaries && \
    cargo build --release --bin attune-core-timer-sensor && \
    cp /build/target/release/attune-core-timer-sensor /build/pack-binaries/attune-core-timer-sensor && \
    ls -lh /build/pack-binaries/

# Verify binaries were copied successfully (after cache unmount)
RUN ls -lah /build/pack-binaries/ && \
    test -f /build/pack-binaries/attune-core-timer-sensor && \
    echo "Timer sensor binary built successfully"

# ============================================================================
# Stage 3: Runtime - Create minimal runtime image
# ============================================================================
FROM debian:${DEBIAN_VERSION}-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 attune && \
    mkdir -p /opt/attune/packs /opt/attune/logs && \
    chown -R attune:attune /opt/attune

WORKDIR /opt/attune

# Copy the service binary from builder
# Note: We copy from /build/attune-service-binary because the cache mount is not available in COPY
COPY --from=builder /build/attune-service-binary /usr/local/bin/attune-service

# Copy configuration for Docker Compose development
# Production: mount config files as a volume instead of baking them into the image
COPY config.docker.yaml ./config.yaml

# Copy migrations for services that need them
COPY migrations/ ./migrations/

# Copy packs directory (excluding binaries that will be overwritten)
COPY packs/ ./packs/

# Overwrite pack binaries with ones built with compatible GLIBC from pack-builder stage
# Copy individual files to ensure they overwrite existing ones
COPY --from=pack-builder /build/pack-binaries/attune-core-timer-sensor ./packs/core/sensors/attune-core-timer-sensor

# Make binaries executable and set ownership
RUN chmod +x ./packs/core/sensors/attune-core-timer-sensor && \
    chown -R attune:attune /opt/attune

# Switch to non-root user
USER attune

# Environment variables (can be overridden at runtime)
ENV RUST_LOG=info
ENV ATTUNE_CONFIG=/opt/attune/config.yaml

# Health check (will be overridden per service in docker-compose)
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Expose default port (override per service)
EXPOSE 8080

# Run the service
CMD ["/usr/local/bin/attune-service"]
