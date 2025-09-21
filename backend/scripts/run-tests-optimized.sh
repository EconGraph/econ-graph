#!/bin/bash

# Optimized test runner for EconGraph backend
# This script provides multiple test execution strategies for maximum efficiency

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
THREADS=12
QUICK=false
VERBOSE=false
COVERAGE=false
BENCHMARK=false

# Function to print usage
usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  -t, --threads N     Number of test threads (default: 12)"
    echo "  -q, --quick         Run only fast unit tests (skip integration tests)"
    echo "  -v, --verbose       Verbose output"
    echo "  -c, --coverage      Run with coverage collection"
    echo "  -b, --benchmark     Run benchmarks"
    echo "  -h, --help          Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                  # Run all tests with 12 threads"
    echo "  $0 -q               # Run only quick unit tests"
    echo "  $0 -t 8             # Run with 8 threads"
    echo "  $0 -c               # Run with coverage"
    echo "  $0 -q -v            # Quick tests with verbose output"
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -t|--threads)
            THREADS="$2"
            shift 2
            ;;
        -q|--quick)
            QUICK=true
            shift
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -c|--coverage)
            COVERAGE=true
            shift
            ;;
        -b|--benchmark)
            BENCHMARK=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            usage
            exit 1
            ;;
    esac
done

echo -e "${BLUE}🚀 EconGraph Backend Test Runner${NC}"
echo -e "${BLUE}================================${NC}"
echo ""

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    echo -e "${RED}❌ Error: Not in backend directory. Please run from backend/ folder.${NC}"
    exit 1
fi

# Set environment variables for optimization
export RUST_TEST_THREADS=$THREADS
export RUST_BACKTRACE=0
export RUST_LOG=warn

# Build test arguments
TEST_ARGS="--test-threads $THREADS"

if [ "$VERBOSE" = true ]; then
    TEST_ARGS="$TEST_ARGS --nocapture"
fi

if [ "$QUICK" = true ]; then
    echo -e "${YELLOW}⚡ Running QUICK tests only (unit tests, skipping integration tests)${NC}"
    echo ""

    # Run only unit tests (lib tests)
    echo -e "${GREEN}📦 Running library unit tests...${NC}"
    time cargo test --lib -- $TEST_ARGS

    # Run only simple unit tests (skip integration tests)
    echo -e "${GREEN}🧪 Running simple unit tests...${NC}"
    time cargo test --lib -- --skip integration --skip e2e --skip epic $TEST_ARGS

elif [ "$COVERAGE" = true ]; then
    echo -e "${YELLOW}📊 Running tests with coverage collection${NC}"
    echo ""

    # Check if tarpaulin is installed
    if ! command -v cargo-tarpaulin &> /dev/null; then
        echo -e "${RED}❌ cargo-tarpaulin not found. Installing...${NC}"
        cargo install cargo-tarpaulin
    fi

    # Run tests with coverage
    cargo tarpaulin --test-threads $THREADS --out Html --output-dir coverage/
    echo -e "${GREEN}✅ Coverage report generated in coverage/ directory${NC}"

elif [ "$BENCHMARK" = true ]; then
    echo -e "${YELLOW}🏃 Running benchmarks${NC}"
    echo ""

    # Run benchmarks
    cargo bench

else
    echo -e "${GREEN}🧪 Running ALL tests with $THREADS threads${NC}"
    echo ""

    # Run all tests
    echo -e "${GREEN}📦 Running library tests...${NC}"
    time cargo test --lib -- $TEST_ARGS

    echo -e "${GREEN}🔧 Running binary tests...${NC}"
    time cargo test --bin econ-graph-backend -- $TEST_ARGS

    echo -e "${GREEN}📚 Running doctests...${NC}"
    time cargo test --doc -- $TEST_ARGS

    echo -e "${GREEN}🧪 Running XBRL financial integration tests...${NC}"
    time cargo test --package econ-graph-sec-crawler --test "xbrl_financial_integration_tests" -- $TEST_ARGS

    echo -e "${GREEN}🔗 Running other integration tests...${NC}"
    time cargo test --workspace --test "integration_*" -- $TEST_ARGS
fi

echo ""
echo -e "${GREEN}✅ All tests completed successfully!${NC}"

# Show test statistics
echo ""
echo -e "${BLUE}📊 Test Statistics:${NC}"
echo "  • CPU Cores: $THREADS"
echo "  • Test Mode: $([ "$QUICK" = true ] && echo "Quick (unit tests only)" || echo "Full (all tests)")"
echo "  • Coverage: $([ "$COVERAGE" = true ] && echo "Enabled" || echo "Disabled")"
echo "  • Benchmarks: $([ "$BENCHMARK" = true ] && echo "Enabled" || echo "Disabled")"
