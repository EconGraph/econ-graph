#!/bin/bash

# Debug script to test E2E container and find "No tests found" issue

set -e

echo "=== Debugging E2E Container ==="

# Build the base container first
echo "Building base E2E container..."
docker build -f ci/docker/e2e/Dockerfile -t econ-graph-e2e:debug . --quiet

echo "=== Container Debug Information ==="

# Check working directory
echo "1. Working directory:"
docker run --rm econ-graph-e2e:debug pwd

# Check if tests directory exists
echo "2. Tests directory structure:"
docker run --rm econ-graph-e2e:debug ls -la tests/

echo "3. E2E tests directory:"
docker run --rm econ-graph-e2e:debug ls -la tests/e2e/ | head -10

# Check Playwright configuration
echo "4. Playwright config files:"
docker run --rm econ-graph-e2e:debug ls -la ci/configs/

# Test Playwright directly
echo "5. Testing Playwright test discovery:"
docker run --rm econ-graph-e2e:debug npx playwright test --list --config=ci/configs/playwright-core.config.ts

echo "6. Testing with different working directory:"
docker run --rm -w /app econ-graph-e2e:debug npx playwright test --list --config=ci/configs/playwright-core.config.ts

echo "=== Debug Complete ==="
