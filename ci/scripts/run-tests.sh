#!/bin/bash
set -e

# E2E Test Runner Script
# Usage: ./run-tests.sh [test-group] [options]

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DOCKER_DIR="$(dirname "$SCRIPT_DIR")/docker"

# Default values
TEST_GROUP="all"
CLEANUP=true
BUILD_IMAGES=true

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --group)
            TEST_GROUP="$2"
            shift 2
            ;;
        --no-cleanup)
            CLEANUP=false
            shift
            ;;
        --no-build)
            BUILD_IMAGES=false
            shift
            ;;
        --help)
            echo "Usage: $0 [options]"
            echo "Options:"
            echo "  --group <group>     Test group to run (core|analysis|debug|comprehensive|mobile-core|mobile-analysis|mobile-comprehensive|xbrl-financial|all)"
            echo "  --no-cleanup        Don't cleanup containers after tests"
            echo "  --no-build          Don't rebuild images"
            echo "  --help              Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

cd "$DOCKER_DIR"

echo "🧪 Starting E2E Tests - Group: $TEST_GROUP"

# Build images if requested
if [ "$BUILD_IMAGES" = true ]; then
    echo "🔨 Building test images..."
    docker-compose -f docker-compose.best-practice.yml build
fi

# Create test results directory
mkdir -p test-results/{core,analysis,debug,comprehensive,mobile-core,mobile-analysis,mobile-comprehensive,xbrl-financial}

# Function to run specific test group
run_test_group() {
    local group=$1
    echo "🚀 Running $group tests..."

    if [ "$group" = "all" ]; then
        # Run all test groups in parallel
        docker-compose -f docker-compose.best-practice.yml up \
            test-core test-analysis test-debug test-comprehensive \
            test-mobile-core test-mobile-analysis test-mobile-comprehensive \
            test-xbrl-financial
    else
        # Run specific test group
        docker-compose -f docker-compose.best-practice.yml up "test-$group"
    fi
}

# Function to cleanup
cleanup() {
    if [ "$CLEANUP" = true ]; then
        echo "🧹 Cleaning up containers..."
        docker-compose -f docker-compose.best-practice.yml down
    fi
}

# Set trap for cleanup on exit
trap cleanup EXIT

# Run tests
run_test_group "$TEST_GROUP"

echo "✅ Tests completed!"
echo "📊 Results available in: $DOCKER_DIR/test-results/"
