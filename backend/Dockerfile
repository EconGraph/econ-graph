# syntax=docker/dockerfile:1.6
#
# Multi-stage Dockerfile for the EconGraph Rust workspace (build context: backend/).
#
# Targets:
#   (default, last stage)      econ-graph-backend   -> docker build -t econ-graph-backend .
#   --target crawler-worker    crawler-worker       -> docker build --target crawler-worker -t econ-graph-crawler-worker .
#
# Both images come from ONE cargo invocation in the `builder` stage, so building both
# targets compiles the workspace once.
#
# Only the workspace crates (crates/*) and migrations/ are copied in. backend/src/ is a
# stale, uncompiled copy of the crates and is intentionally NOT used.
#
# Dependency caching uses BuildKit cache mounts (cargo registry, git checkouts and the
# target dir) instead of a dummy-main.rs skeleton, which never worked for this workspace
# (many crates, several [[bin]] paths). Requires BuildKit (default since Docker 23;
# docker/build-push-action always uses it).
#
# Cargo.lock: /backend/Cargo.lock is gitignored, so CI/clean checkouts resolve
# dependencies fresh (exact `=` pins in the workspace Cargo.toml keep the risky ones
# stable). If a local Cargo.lock exists it IS copied in and honoured.

# Keep in sync with rust-toolchain.toml at the repository root (channel = "1.89.0").
# Bookworm to match the runtime image's glibc/OpenSSL/libpq.
ARG RUST_VERSION=1.89.0

############################
# Build stage
############################
FROM rust:${RUST_VERSION}-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config \
        libssl-dev \
        libpq-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Cargo.lock* is optional; pairing it with Cargo.toml keeps COPY valid when it's absent.
COPY Cargo.toml Cargo.lock* ./
COPY crates ./crates
# Needed at compile time by embed_migrations!() in econ-graph-core.
COPY migrations ./migrations

# Build by binary name (not crate path): the crawler-worker binary may move between crates.
# The target dir is a cache mount, so copy the binaries out within the same RUN.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release --bin econ-graph-backend --bin crawler-worker \
    && mkdir -p /out \
    && cp target/release/econ-graph-backend target/release/crawler-worker /out/

############################
# Shared runtime base
############################
FROM debian:bookworm-slim AS runtime-base

RUN apt-get update && apt-get upgrade -y && apt-get install -y --no-install-recommends \
        ca-certificates \
        libssl3 \
        libpq5 \
    && rm -rf /var/lib/apt/lists/*

# UID/GID 1000 matches runAsUser/fsGroup in the k8s manifests.
RUN groupadd -g 1000 econuser && useradd -r -u 1000 -g econuser -s /bin/false econuser

WORKDIR /app

############################
# crawler-worker image (--target crawler-worker)
############################
FROM runtime-base AS crawler-worker

COPY --from=builder --chown=econuser:econuser /out/crawler-worker /app/crawler-worker

USER econuser

# No ports and no HTTP endpoint: the worker only talks to Postgres and upstream APIs.
# It does not run migrations (the backend applies them at startup).
# SIGTERM: stops claiming new jobs and finishes in-flight ones.
STOPSIGNAL SIGTERM
ENV RUST_LOG=info
CMD ["/app/crawler-worker"]

############################
# Backend image (default target; keep this stage LAST)
############################
FROM runtime-base AS backend

# curl is only for the HEALTHCHECK below.
RUN apt-get update && apt-get install -y --no-install-recommends curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder --chown=econuser:econuser /out/econ-graph-backend /app/econ-graph-backend

USER econuser

# Default port; the application reads BACKEND_PORT at runtime.
EXPOSE 9876

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:${BACKEND_PORT:-9876}/health || exit 1

CMD ["./econ-graph-backend"]
