#!/bin/bash

# Local Docker Testing Script
# This script provides faster ways to test Docker builds locally

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}Docker Local Testing Script${NC}"
echo "================================"

# Function to show usage
show_usage() {
    echo "Usage: $0 [OPTION]"
    echo ""
    echo "Options:"
    echo "  quick-test    - Quick test with minimal output (fastest)"
    echo "  verbose-test  - Full verbose output (slower but shows progress)"
    echo "  test-base     - Test only the base E2E container"
    echo "  test-mobile   - Test only the mobile container (requires base)"
    echo "  clean         - Clean up Docker images and containers"
    echo "  help          - Show this help message"
}

# Function for quick testing
quick_test() {
    echo -e "${YELLOW}Quick Docker Test (minimal output)${NC}"
    echo "This will build with minimal output for speed..."

    # Test base container
    echo -e "${BLUE}Building base E2E container...${NC}"
    docker build -f ci/docker/e2e/Dockerfile -t econ-graph-e2e:test . --quiet

    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✓ Base container built successfully${NC}"
    else
        echo -e "${RED}✗ Base container build failed${NC}"
        return 1
    fi

    # Test mobile container
    echo -e "${BLUE}Building mobile E2E container...${NC}"
    docker build -f ci/docker/e2e/Dockerfile.mobile -t econ-graph-e2e-mobile:test . --quiet

    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✓ Mobile container built successfully${NC}"
        echo -e "${GREEN}All containers built successfully!${NC}"
    else
        echo -e "${RED}✗ Mobile container build failed${NC}"
        return 1
    fi
}

# Function for verbose testing
verbose_test() {
    echo -e "${YELLOW}Verbose Docker Test (full output)${NC}"
    echo "This will show all build output..."

    # Test base container
    echo -e "${BLUE}Building base E2E container...${NC}"
    docker build -f ci/docker/e2e/Dockerfile -t econ-graph-e2e:test . --progress=plain

    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✓ Base container built successfully${NC}"
    else
        echo -e "${RED}✗ Base container build failed${NC}"
        return 1
    fi

    # Test mobile container
    echo -e "${BLUE}Building mobile E2E container...${NC}"
    docker build -f ci/docker/e2e/Dockerfile.mobile -t econ-graph-e2e-mobile:test . --progress=plain

    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✓ Mobile container built successfully${NC}"
        echo -e "${GREEN}All containers built successfully!${NC}"
    else
        echo -e "${RED}✗ Mobile container build failed${NC}"
        return 1
    fi
}

# Function to test only base container
test_base() {
    echo -e "${YELLOW}Testing Base E2E Container Only${NC}"
    docker build -f ci/docker/e2e/Dockerfile -t econ-graph-e2e:test . --progress=plain
}

# Function to test only mobile container
test_mobile() {
    echo -e "${YELLOW}Testing Mobile E2E Container Only${NC}"
    echo "Note: This requires the base container to exist"
    docker build -f ci/docker/e2e/Dockerfile.mobile -t econ-graph-e2e-mobile:test . --progress=plain
}

# Function to clean up
clean_docker() {
    echo -e "${YELLOW}Cleaning up Docker images and containers...${NC}"

    # Remove test containers
    docker rmi econ-graph-e2e:test 2>/dev/null || true
    docker rmi econ-graph-e2e-mobile:test 2>/dev/null || true

    # Remove any stopped containers
    docker container prune -f

    # Remove unused images
    docker image prune -f

    echo -e "${GREEN}✓ Docker cleanup completed${NC}"
}

# Main script logic
case "${1:-help}" in
    "quick-test")
        quick_test
        ;;
    "verbose-test")
        verbose_test
        ;;
    "test-base")
        test_base
        ;;
    "test-mobile")
        test_mobile
        ;;
    "clean")
        clean_docker
        ;;
    "help"|*)
        show_usage
        ;;
esac
