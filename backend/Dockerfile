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

# Create a dummy main.rs to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies (this layer will be cached)
RUN cargo build --release --bin econ-graph-backend

# Remove dummy main.rs and copy actual source code
RUN rm src/main.rs
COPY src ./src
COPY migrations ./migrations

# Build the actual application
RUN cargo build --release --bin econ-graph-backend

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
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

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Run the application
CMD ["./econ-graph-backend"]
