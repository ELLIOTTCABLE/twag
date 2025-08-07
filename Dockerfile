# Use the official Rust image with a pinned version
FROM rust:1.83-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
   pkg-config \
   libssl-dev \
   && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app

# Copy manifests for dependency caching
COPY Cargo.toml Cargo.lock ./

# Copy SQLx offline compilation data (required for SQLX_OFFLINE=true)
COPY .sqlx ./.sqlx

# Fetch dependencies first for better layer caching
RUN cargo fetch

# Copy source and build
COPY src ./src
COPY templates ./templates
COPY migrations ./migrations

# Build with SQLx offline mode
ENV SQLX_OFFLINE=true
RUN cargo build --release

# Runtime stage - use Chainguard for better security
FROM cgr.dev/chainguard/glibc-dynamic:latest

# Copy the binary from builder stage
COPY --from=builder /usr/src/app/target/release/twag /usr/local/bin/twag

# Expose port
EXPOSE 3000

# Run the binary
ENTRYPOINT ["/usr/local/bin/twag"]
