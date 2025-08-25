# Stage 1: Build the Rust application
FROM rust:1.89-slim-bookworm AS builder

# Set the working directory inside the container
WORKDIR /app

# Install common build dependencies required by many Rust projects that link to C libraries.
# pkg-config is often needed for finding libraries.
# build-essential provides essential build tools like gcc.
# libssl-dev is for OpenSSL development headers, commonly used for TLS.
# clang and cmake are added for broader compatibility with C/C++ dependencies and build systems.
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    pkg-config \
    build-essential \
    libssl-dev && \
    rm -rf /var/lib/apt/lists/*

# Copy only Cargo.toml and Cargo.lock first to leverage Docker's layer caching
# This step helps to re-use previous layers if only source code changes
COPY Cargo.toml Cargo.lock ./

# Build dependencies, but do not generate a binary yet
# This warms up the cache for dependencies
RUN mkdir -p src

# Copy the rest of the application source code
COPY . /app

# Build the Rust application in release mode
# --locked ensures that Cargo.lock is respected, preventing unexpected dependency changes
# --target ensures cross-compilation if needed, though for standard Linux it's often not explicit
RUN cargo build --release --locked

# Stage 2: Create the final runtime image
# Use a slim Debian base image for a small footprint
FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    openssl \
    ca-certificates \
    neovim && \
    rm -rf /var/lib/apt/lists/*

# Copy the compiled binary from the builder stage to a PATH directory
# The 'ratatai' binary is typically found in target/release/
COPY --from=builder /app/target/release/ratatai /usr/local/bin

# Ensure the binary has execute permissions
RUN chmod +x /usr/local/bin/ratatai

# Set working directory
WORKDIR /app

# Create a directory for logs with proper permissions for any user
RUN mkdir -p /app/logs && chmod 777 /app/logs

# Set the entrypoint for the container to directly execute 'ratatai'
# Any arguments passed to 'docker run' will be appended to this entrypoint
ENTRYPOINT ["/usr/local/bin/ratatai"]

# Optional: Add metadata for documentation or image registries
LABEL org.opencontainers.image.source="https://github.com/uggla/ratatai"
LABEL org.opencontainers.image.description="TUI tool for managing responses to OpenStack Nova bugs."
LABEL org.opencontainers.image.licenses="Apache v2"
