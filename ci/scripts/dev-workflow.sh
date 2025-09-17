#!/bin/bash
set -e

# Development Workflow Script
# Shows how to separate building from running in local development

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DOCKER_DIR="$(dirname "$SCRIPT_DIR")/docker"

echo "🚀 EconGraph E2E Development Workflow"
echo "======================================"

cd "$DOCKER_DIR"

# Step 1: Build images (only when needed)
echo ""
echo "📦 Step 1: Building Docker images..."
echo "This step only needs to be run when:"
echo "  - Dependencies change (package.json, Cargo.toml)"
echo "  - Dockerfiles change"
echo "  - First time setup"
echo ""

read -p "Do you want to build images now? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    ./scripts/build-images.sh
else
    echo "⏭️  Skipping image build (assuming images are up to date)"
fi

# Step 2: Run tests
echo ""
echo "🧪 Step 2: Running E2E tests..."
echo "Available test groups:"
echo "  - core: Basic functionality tests"
echo "  - analysis: Professional and global analysis tests"
echo "  - debug: Visual and debugging tests"
echo "  - comprehensive: Complete workflow tests"
echo "  - mobile-core: Mobile basic functionality"
echo "  - mobile-analysis: Mobile analysis features"
echo "  - mobile-comprehensive: Mobile complete workflows"
echo "  - all: Run all test groups in parallel"
echo ""

read -p "Enter test group (or 'all' for parallel): " test_group
test_group=${test_group:-core}

if [ "$test_group" = "all" ]; then
    echo "🚀 Running all test groups in parallel..."
    ./scripts/run-tests-unified.sh --parallel
else
    echo "🚀 Running $test_group tests..."
    ./scripts/run-tests-unified.sh --group "$test_group"
fi

echo ""
echo "✅ Development workflow completed!"
echo "📊 Check results in: $DOCKER_DIR/test-results/"
