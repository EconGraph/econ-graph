# CI Optimization Notes

## Current CI Performance Issues

Our current CI pipeline has significant setup overhead due to repeatedly installing the same dependencies across multiple jobs:

- **Rust toolchain installation**: ~30-45 seconds per job
- **System build tools installation**: ~15-30 seconds per job  
- **Shared tools installation** (diesel_cli, cargo-audit, cargo-deny): ~60-90 seconds per job
- **Total setup time per job**: ~2-3 minutes

With 14 parallel backend test jobs, this represents ~28-42 minutes of redundant setup time.

## Optimization Strategies

### 1. Custom Docker Images (Recommended)

Create pre-built Docker images with all dependencies installed:

```dockerfile
# .github/docker/rust-backend.Dockerfile
FROM ubuntu:22.04

# Install system dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    postgresql-client \
    && rm -rf /var/lib/apt/lists/*

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:$PATH"

# Install common Rust tools
RUN cargo install diesel_cli cargo-audit cargo-deny

# Set working directory
WORKDIR /workspace
```

Usage in workflows:
```yaml
jobs:
  backend-tests:
    runs-on: ubuntu-latest
    container:
      image: ghcr.io/your-org/rust-backend:latest
      credentials:
        username: ${{ github.actor }}
        password: ${{ secrets.GITHUB_TOKEN }}
```

**Benefits**: Eliminates all setup time, reduces CI time by 2-3 minutes per job.

### 2. Composite Actions

Create reusable composite actions for common setup steps:

```yaml
# .github/actions/setup-rust-backend/action.yml
name: 'Setup Rust Backend Environment'
description: 'Install Rust, build tools, and common dependencies'
runs:
  using: 'composite'
  steps:
    - name: Install system build tools
      shell: bash
      run: sudo apt-get update && sudo apt-get install -y build-essential
    
    - name: Install Rust toolchain
      uses: dtolnay/rust-toolchain@1.89.0
      with:
        components: rustfmt, clippy
    
    - name: Install shared tools
      shell: bash
      run: |
        cargo install diesel_cli --no-default-features --features postgres
        cargo install cargo-audit cargo-deny
```

Usage:
```yaml
- name: Setup Rust Backend
  uses: ./.github/actions/setup-rust-backend
```

**Benefits**: Reduces setup time from ~2-3 minutes to ~30 seconds, improves maintainability.

### 3. Self-Hosted Runners

Set up persistent self-hosted runners with pre-configured environments:

```yaml
runs-on: [self-hosted, linux, x64]
```

**Benefits**: No setup time, fastest possible execution, but requires infrastructure management.

### 4. GitHub Container Registry (GHCR) with Automated Builds

Automated Docker image builds and pushes:

```yaml
# .github/workflows/build-images.yml
name: Build Custom Images

on:
  push:
    paths:
      - '.github/docker/**'
  schedule:
    - cron: '0 2 * * 0'  # Weekly rebuild

jobs:
  build-rust-image:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Build and push Rust image
        uses: docker/build-push-action@v5
        with:
          context: .github/docker
          file: .github/docker/rust-backend.Dockerfile
          push: true
          tags: |
            ghcr.io/${{ github.repository }}/rust-backend:latest
            ghcr.io/${{ github.repository }}/rust-backend:${{ github.sha }}
```

### 5. Optimized Caching Strategy

Enhanced caching for system packages and Rust toolchain:

```yaml
- name: Cache system packages
  uses: actions/cache@v3
  with:
    path: /var/cache/apt
    key: ${{ runner.os }}-apt-${{ hashFiles('**/apt-packages.txt') }}
    restore-keys: |
      ${{ runner.os }}-apt-

- name: Cache Rust toolchain
  uses: actions/cache@v3
  with:
    path: |
      ~/.rustup
      ~/.cargo
    key: ${{ runner.os }}-rust-${{ hashFiles('rust-toolchain.toml') }}
```

## Implementation Roadmap

### Phase 1: Quick Wins (1-2 hours)
- [ ] Create composite action for common setup steps
- [ ] Implement optimized caching strategy
- [ ] **Expected improvement**: Reduce setup time from 2-3 minutes to 30 seconds per job

### Phase 2: Medium-term Optimization (1-2 days)
- [ ] Build custom Docker images for backend testing
- [ ] Set up automated image builds via GHCR
- [ ] Migrate backend test jobs to use custom images
- [ ] **Expected improvement**: Eliminate setup time entirely, reduce total CI time by 20-30 minutes

### Phase 3: Advanced Optimization (1 week)
- [ ] Evaluate self-hosted runners for high-volume scenarios
- [ ] Implement multi-stage Docker builds for different job types
- [ ] Add image vulnerability scanning and updates
- [ ] **Expected improvement**: Fastest possible CI execution with minimal maintenance

## Current CI Performance Baseline

- **Total CI time**: ~35-40 minutes
- **Setup overhead**: ~28-42 minutes (across 14 backend jobs)
- **Actual test execution**: ~7-12 minutes
- **Parallelization efficiency**: ~70% (due to setup bottlenecks)

## Target Performance Goals

- **Phase 1 target**: ~25-30 minutes total CI time
- **Phase 2 target**: ~15-20 minutes total CI time  
- **Phase 3 target**: ~10-15 minutes total CI time

## Notes

- Custom Docker images provide the best ROI for our current setup
- Composite actions are a good intermediate step that requires minimal infrastructure changes
- Self-hosted runners should only be considered if CI volume increases significantly
- All optimizations should maintain the current parallelization benefits we've achieved

## Related Files

- `.github/workflows/ci.yml` - Current CI configuration
- `backend/Cargo.toml` - Rust dependencies
- `frontend/package.json` - Node.js dependencies
