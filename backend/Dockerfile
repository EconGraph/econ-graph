# Multi-stage Dockerfile for EconGraph Backend
FROM rust:1.88-slim AS builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy Cargo files
COPY Cargo.toml ./
COPY Cargo.lock* ./

# Copy crates directory (needed for workspace dependencies)
COPY crates ./crates

# Copy migrations directory (needed for embed_migrations! macro)
COPY migrations ./migrations

# Create a dummy main.rs to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies (this layer will be cached)
RUN cargo build --release --bin econ-graph-backend

# Remove dummy main.rs and copy actual source code
RUN rm src/main.rs
COPY src ./src

# Build the actual application
RUN cargo build --release --bin econ-graph-backend

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies and update packages to fix zlib vulnerability
RUN apt-get update && apt-get upgrade -y && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -r -s /bin/false econuser

# Set working directory
WORKDIR /app

# Copy binary from builder stage
COPY --from=builder /app/target/release/econ-graph-backend /app/econ-graph-backend

# Change ownership to non-root user
RUN chown econuser:econuser /app/econ-graph-backend

# Switch to non-root user
USER econuser

# Expose port (default, can be overridden at runtime)
EXPOSE 9876

# Health check (uses default port, should be overridden for different ports)
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:${BACKEND_PORT:-9876}/health || exit 1

# Run the application
CMD ["./econ-graph-backend"]
