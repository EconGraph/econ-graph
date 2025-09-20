#!/bin/bash

# Build E2E Test Containers
# This script builds Docker containers with pre-installed Playwright browsers
# to speed up E2E test execution in CI

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Building E2E Test Containers...${NC}"

# Get the project root directory (assuming this script is in ci/docker/e2e/)
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$PROJECT_ROOT"

# Build standard E2E container
echo -e "${YELLOW}Building standard E2E container...${NC}"
echo -e "${YELLOW}This may take several minutes as it installs Node.js, npm dependencies, and Playwright browsers...${NC}"
echo -e "${YELLOW}Starting Docker build...${NC}"
# Use script to create a PTY for unbuffered output
script -q /dev/null docker build --progress=plain -f ci/docker/e2e/Dockerfile -t econ-graph-e2e:latest . &
BUILD_PID=$!
echo -e "${YELLOW}Docker build started with PID: $BUILD_PID${NC}"

# Show progress while building
while kill -0 $BUILD_PID 2>/dev/null; do
    echo -e "${YELLOW}Still building... (PID: $BUILD_PID)${NC}"
    sleep 10
done

# Wait for the build to complete
wait $BUILD_PID
BUILD_EXIT_CODE=$?

if [ $BUILD_EXIT_CODE -eq 0 ]; then
    echo -e "${GREEN}✓ Standard E2E container built successfully${NC}"
else
    echo -e "${RED}✗ Failed to build standard E2E container${NC}"
    exit 1
fi

# Build mobile E2E container
echo -e "${YELLOW}Building mobile E2E container...${NC}"
echo -e "${YELLOW}This builds on the standard container and adds mobile-specific dependencies...${NC}"
echo -e "${YELLOW}Starting mobile Docker build...${NC}"
# Use script to create a PTY for unbuffered output
script -q /dev/null docker build --progress=plain -f ci/docker/e2e/Dockerfile.mobile -t econ-graph-e2e-mobile:latest . &
MOBILE_BUILD_PID=$!
echo -e "${YELLOW}Mobile Docker build started with PID: $MOBILE_BUILD_PID${NC}"

# Show progress while building
while kill -0 $MOBILE_BUILD_PID 2>/dev/null; do
    echo -e "${YELLOW}Still building mobile container... (PID: $MOBILE_BUILD_PID)${NC}"
    sleep 10
done

# Wait for the build to complete
wait $MOBILE_BUILD_PID
MOBILE_BUILD_EXIT_CODE=$?

if [ $MOBILE_BUILD_EXIT_CODE -eq 0 ]; then
    echo -e "${GREEN}✓ Mobile E2E container built successfully${NC}"
else
    echo -e "${RED}✗ Failed to build mobile E2E container${NC}"
    exit 1
fi

echo -e "${GREEN}All E2E containers built successfully!${NC}"
echo -e "${YELLOW}Available containers:${NC}"
echo "  - econ-graph-e2e:latest (standard E2E tests)"
echo "  - econ-graph-e2e-mobile:latest (mobile E2E tests)"
