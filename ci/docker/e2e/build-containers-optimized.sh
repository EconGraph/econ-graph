#!/bin/bash

# Build Optimized E2E Test Containers
# This script builds Docker containers with pre-built backend and frontend
# to speed up E2E test execution in CI

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${GREEN}Building Optimized E2E Test Containers...${NC}"
echo -e "${BLUE}This will build both backend and frontend in parallel for faster startup times${NC}"

# Get the project root directory (assuming this script is in ci/docker/e2e/)
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$PROJECT_ROOT"

# Build optimized E2E container
echo -e "${YELLOW}Building optimized E2E container with pre-built backend and frontend...${NC}"
echo -e "${YELLOW}This may take several minutes as it builds Rust backend, Node.js frontend, and installs Playwright browsers...${NC}"
echo -e "${YELLOW}Starting optimized Docker build...${NC}"

# Use script to create a PTY for unbuffered output
script -q /dev/null docker build --progress=plain -f ci/docker/e2e/Dockerfile.optimized -t econ-graph-e2e-optimized:latest . &
BUILD_PID=$!
echo -e "${YELLOW}Optimized Docker build started with PID: $BUILD_PID${NC}"

# Show progress while building
while kill -0 $BUILD_PID 2>/dev/null; do
    echo -e "${YELLOW}Still building optimized container... (PID: $BUILD_PID)${NC}"
    sleep 15
done

# Wait for the build to complete
wait $BUILD_PID
BUILD_EXIT_CODE=$?

if [ $BUILD_EXIT_CODE -eq 0 ]; then
    echo -e "${GREEN}✓ Optimized E2E container built successfully${NC}"
else
    echo -e "${RED}✗ Failed to build optimized E2E container${NC}"
    exit 1
fi

# Build optimized mobile E2E container
echo -e "${YELLOW}Building optimized mobile E2E container...${NC}"
echo -e "${YELLOW}This builds on the optimized container and adds mobile-specific dependencies...${NC}"
echo -e "${YELLOW}Starting optimized mobile Docker build...${NC}"

# Use script to create a PTY for unbuffered output
script -q /dev/null docker build --progress=plain -f ci/docker/e2e/Dockerfile.mobile.optimized -t econ-graph-e2e-mobile-optimized:latest . &
MOBILE_BUILD_PID=$!
echo -e "${YELLOW}Optimized mobile Docker build started with PID: $MOBILE_BUILD_PID${NC}"

# Show progress while building
while kill -0 $MOBILE_BUILD_PID 2>/dev/null; do
    echo -e "${YELLOW}Still building optimized mobile container... (PID: $MOBILE_BUILD_PID)${NC}"
    sleep 10
done

# Wait for the build to complete
wait $MOBILE_BUILD_PID
MOBILE_BUILD_EXIT_CODE=$?

if [ $MOBILE_BUILD_EXIT_CODE -eq 0 ]; then
    echo -e "${GREEN}✓ Optimized mobile E2E container built successfully${NC}"
else
    echo -e "${RED}✗ Failed to build optimized mobile E2E container${NC}"
    exit 1
fi

echo -e "${GREEN}All optimized E2E containers built successfully!${NC}"
echo -e "${YELLOW}Available optimized containers:${NC}"
echo "  - econ-graph-e2e-optimized:latest (optimized E2E tests with pre-built backend)"
echo "  - econ-graph-e2e-mobile-optimized:latest (optimized mobile E2E tests with pre-built backend)"
echo -e "${BLUE}These containers have the backend binary pre-built, which should significantly speed up E2E test startup times.${NC}"
