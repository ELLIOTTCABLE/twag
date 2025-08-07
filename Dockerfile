# Use the official Rust image with a pinned version
FROM rust:1.83-slim AS builder

# Install build dependencies including OpenSSL
RUN apt-get update && apt-get install -y \
   pkg-config \
   libssl-dev \
   ca-certificates \
   && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app

# Copy manifests for dependency caching
COPY Cargo.toml Cargo.lock ./

# Copy source and build
COPY src ./src
COPY templates ./templates
COPY migrations ./migrations

# Copy SQLx offline compilation data
COPY .sqlx ./.sqlx

# Fetch dependencies first for better layer caching
RUN cargo fetch

# Build with SQLx offline mode
ENV SQLX_OFFLINE=true
RUN cargo build --release

# Runtime stage - use debian-slim with OpenSSL
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
   ca-certificates \
   libssl3 \
   && rm -rf /var/lib/apt/lists/*

# Copy the binary from builder stage
COPY --from=builder /usr/src/app/target/release/twag /usr/local/bin/twag

# Expose port
EXPOSE 3000

# Run the binary
ENTRYPOINT ["/usr/local/bin/twag"]
