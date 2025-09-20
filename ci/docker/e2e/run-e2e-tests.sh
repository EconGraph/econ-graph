#!/bin/bash

# Run E2E Tests with Pre-built Containers
# This script runs E2E tests using pre-built Docker containers
# to avoid the overhead of installing dependencies and building frontend

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
CONTAINER_TYPE="standard"
TEST_TYPE="core"
BACKEND_URL="http://host.docker.internal:8080"
FRONTEND_URL="http://host.docker.internal:3000"

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --mobile)
            CONTAINER_TYPE="mobile"
            shift
            ;;
        --test-type)
            TEST_TYPE="$2"
            shift 2
            ;;
        --backend-url)
            BACKEND_URL="$2"
            shift 2
            ;;
        --frontend-url)
            FRONTEND_URL="$2"
            shift 2
            ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo "Options:"
            echo "  --mobile              Use mobile E2E container"
            echo "  --test-type TYPE      Test type: core, comprehensive, mobile (default: core)"
            echo "  --backend-url URL     Backend URL (default: http://host.docker.internal:8080)"
            echo "  --frontend-url URL    Frontend URL (default: http://host.docker.internal:3000)"
            echo "  --help                Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Determine container image
if [ "$CONTAINER_TYPE" = "mobile" ]; then
    CONTAINER_IMAGE="econ-graph-e2e-mobile:latest"
else
    CONTAINER_IMAGE="econ-graph-e2e:latest"
fi

echo -e "${BLUE}Running E2E Tests with Pre-built Container${NC}"
echo -e "${YELLOW}Container: $CONTAINER_IMAGE${NC}"
echo -e "${YELLOW}Test Type: $TEST_TYPE${NC}"
echo -e "${YELLOW}Backend URL: $BACKEND_URL${NC}"
echo -e "${YELLOW}Frontend URL: $FRONTEND_URL${NC}"

# Check if container exists
if ! docker image inspect "$CONTAINER_IMAGE" >/dev/null 2>&1; then
    echo -e "${RED}Error: Container $CONTAINER_IMAGE not found${NC}"
    echo -e "${YELLOW}Please run ./build-containers.sh first${NC}"
    exit 1
fi

# Determine the test command based on test type
case $TEST_TYPE in
    "core")
        TEST_CMD="npm run test:e2e:core"
        ;;
    "comprehensive")
        TEST_CMD="npm run test:e2e:comprehensive"
        ;;
    "mobile")
        TEST_CMD="npm run test:e2e:mobile"
        ;;
    *)
        echo -e "${RED}Error: Unknown test type: $TEST_TYPE${NC}"
        echo -e "${YELLOW}Valid test types: core, comprehensive, mobile${NC}"
        exit 1
        ;;
esac

# Run the E2E tests in the container
echo -e "${GREEN}Starting E2E tests...${NC}"
docker run --rm \
    --network host \
    -e BACKEND_URL="$BACKEND_URL" \
    -e FRONTEND_URL="$FRONTEND_URL" \
    -e BASE_URL="$FRONTEND_URL" \
    -v "$(pwd)/frontend/tests:/app/tests" \
    -v "$(pwd)/frontend/ci:/app/ci" \
    -v "$(pwd)/frontend/playwright.config.ts:/app/playwright.config.ts" \
    -v "$(pwd)/frontend/playwright.mobile.config.ts:/app/playwright.mobile.config.ts" \
    "$CONTAINER_IMAGE" \
    bash -c "$TEST_CMD"

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ E2E tests completed successfully${NC}"
else
    echo -e "${RED}✗ E2E tests failed${NC}"
    exit 1
fi
