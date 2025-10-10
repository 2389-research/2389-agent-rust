# Multi-stage build for 2389 Agent Protocol
# Using Rust 1.83 for edition2024 support (required by libxml dependency)
FROM rust:1.83-slim-bookworm AS builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy manifest and lock files for better layer caching
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to build dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy source code
COPY src ./src

# Build the application with optimizations
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    python3 \
    python3-pip \
    jq \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1001 agent && \
    mkdir -p /app/config /app/logs && \
    chown -R agent:agent /app

# Copy binary from builder stage
COPY --from=builder /app/target/release/agent2389 /usr/local/bin/agent2389

# Switch to non-root user
USER agent
WORKDIR /app

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD agent2389 health || exit 1

# Expose common ports (none required for MQTT client)
# EXPOSE 8080

# Default environment variables
ENV RUST_LOG=info
ENV AGENT_CONFIG_PATH=/app/config/agent.toml

# Default command
CMD ["agent2389", "run"]