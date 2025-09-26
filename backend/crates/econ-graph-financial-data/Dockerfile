# Multi-stage build for EconGraph Financial Data Service
# This Dockerfile creates an optimized production image for the financial data service

# Stage 1: Build stage
FROM rust:1.75-slim as builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/

# Build the financial data service
RUN cargo build --release --bin econ_graph_financial_data

# Stage 2: Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -r econ && useradd -r -g econ econ

# Set working directory
WORKDIR /app

# Copy the binary from builder stage
COPY --from=builder /app/target/release/econ_graph_financial_data /app/

# Create data directory for Parquet files
RUN mkdir -p /app/data && chown -R econ:econ /app

# Switch to non-root user
USER econ

# Expose port
EXPOSE 3001

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3001/health || exit 1

# Set environment variables
ENV RUST_LOG=info
ENV RUST_BACKTRACE=1
ENV DATA_DIR=/app/data

# Run the service
CMD ["./econ_graph_financial_data"]
