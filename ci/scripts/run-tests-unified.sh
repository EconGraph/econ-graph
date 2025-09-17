#!/bin/bash
set -e

# Unified E2E Test Runner Script
# Usage: ./run-tests-unified.sh [test-group] [options]

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DOCKER_DIR="$(dirname "$SCRIPT_DIR")/docker"

# Default values
TEST_GROUP="core"
CLEANUP=true
BUILD_IMAGES=false
PARALLEL=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --group)
            TEST_GROUP="$2"
            shift 2
            ;;
        --parallel)
            PARALLEL=true
            shift
            ;;
        --no-cleanup)
            CLEANUP=false
            shift
            ;;
        --build)
            BUILD_IMAGES=true
            shift
            ;;
        --help)
            echo "Usage: $0 [options]"
            echo "Options:"
            echo "  --group <group>     Test group to run (core|analysis|debug|comprehensive|mobile-core|mobile-analysis|mobile-comprehensive)"
            echo "  --parallel          Run all test groups in parallel"
            echo "  --no-cleanup        Don't cleanup containers after tests"
            echo "  --build             Build images before running tests"
            echo "  --help              Show this help message"
            echo ""
            echo "Examples:"
            echo "  $0 --group core                    # Run core tests only"
            echo "  $0 --group mobile-analysis         # Run mobile analysis tests only"
            echo "  $0 --parallel                      # Run all test groups in parallel"
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
    docker-compose -f docker-compose.unified.yml build
fi

# Create test results directory
mkdir -p test-results

# Function to run specific test group
run_test_group() {
    local group=$1
    echo "🚀 Running $group tests..."

    # Map test group to npm script
    case $group in
        core) npm_script="test:e2e:core" ;;
        analysis) npm_script="test:e2e:analysis" ;;
        debug) npm_script="test:e2e:debug" ;;
        comprehensive) npm_script="test:e2e:comprehensive" ;;
        mobile-core) npm_script="test:e2e:mobile:core" ;;
        mobile-analysis) npm_script="test:e2e:mobile:analysis" ;;
        mobile-comprehensive) npm_script="test:e2e:mobile:comprehensive" ;;
        *) echo "❌ Unknown test group: $group"; exit 1 ;;
    esac

    echo "📋 Running: npm run $npm_script"

    # Run the test with the specific command
    docker-compose -f docker-compose.unified.yml run --rm test-runner npm run "$npm_script"
}

# Function to run all test groups in parallel
run_all_parallel() {
    echo "🚀 Running all test groups in parallel..."

    # Start infrastructure
    docker-compose -f docker-compose.unified.yml up -d postgres backend frontend

    # Wait for services to be ready
    echo "⏳ Waiting for services to be ready..."
    sleep 10

    # Run all test groups in parallel
    docker-compose -f docker-compose.unified.yml run --rm -d test-runner npm run test:e2e:core &
    docker-compose -f docker-compose.unified.yml run --rm -d test-runner npm run test:e2e:analysis &
    docker-compose -f docker-compose.unified.yml run --rm -d test-runner npm run test:e2e:debug &
    docker-compose -f docker-compose.unified.yml run --rm -d test-runner npm run test:e2e:comprehensive &
    docker-compose -f docker-compose.unified.yml run --rm -d test-runner npm run test:e2e:mobile:core &
    docker-compose -f docker-compose.unified.yml run --rm -d test-runner npm run test:e2e:mobile:analysis &
    docker-compose -f docker-compose.unified.yml run --rm -d test-runner npm run test:e2e:mobile:comprehensive &

    # Wait for all background jobs to complete
    wait
}

# Function to cleanup
cleanup() {
    if [ "$CLEANUP" = true ]; then
        echo "🧹 Cleaning up containers..."
        docker-compose -f docker-compose.unified.yml down
    fi
}

# Set trap for cleanup on exit
trap cleanup EXIT

# Run tests
if [ "$PARALLEL" = true ]; then
    run_all_parallel
else
    run_test_group "$TEST_GROUP"
fi

echo "✅ Tests completed!"
echo "📊 Results available in: $DOCKER_DIR/test-results/"
