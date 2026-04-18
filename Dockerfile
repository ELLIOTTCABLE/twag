# Use the official Rust image with a pinned version
FROM rust:1.94-slim AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
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

# Runtime stage
FROM debian:trixie-slim

RUN apt-get update && apt-get install -y \
   ca-certificates \
   && rm -rf /var/lib/apt/lists/*

# Copy the binary from builder stage
COPY --from=builder /usr/src/app/target/release/twag /usr/local/bin/twag

# Expose port
EXPOSE 3000

# Run the binary
ENTRYPOINT ["/usr/local/bin/twag"]
