# Build stage
FROM rust:1.93.1-slim as builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    llvm \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Copy manifest files
COPY Cargo.toml ./

# Create dummy main.rs to cache dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy source code
COPY src ./src
COPY templates ./templates

# Build actual binary
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    gosu \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /build/target/release/LangTrans /app/langtrans

# Create directories for data
RUN mkdir -p /app/model /app/data

# Default environment variables
ENV LANGTRANS_PORT=8080 \
    LANGTRANS_MODEL_PATH=/app/model \
    LANGTRANS_APIKEYS_PATH=/app/data/api_keys.json \
    RUST_LOG=info

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=120s --retries=3 \
    CMD curl -f http://localhost:8080/admin/login || exit 1

# Create non-root user and set ownership
RUN useradd -m -u 1000 langtrans && \
    chown -R langtrans:langtrans /app

# Copy entrypoint script (runs as root to fix volume permissions, then drops to langtrans)
COPY entrypoint.sh /app/entrypoint.sh
RUN chmod +x /app/entrypoint.sh

ENTRYPOINT ["/app/entrypoint.sh"]
CMD ["/app/langtrans"]
