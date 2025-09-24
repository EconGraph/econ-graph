#!/bin/bash

# Memory-optimized test execution script
# Reduces memory usage for constrained environments

set -e

echo "🧠 Running memory-optimized tests..."

# Set memory limits
export NODE_OPTIONS="--max-old-space-size=1024 --max-semi-space-size=64"

# Run tests with memory optimization
echo "📊 Memory limit: $NODE_OPTIONS"
echo "🔧 Running Jest with memory optimization..."

# Use Jest directly with memory optimization
npx jest --maxWorkers=1 --runInBand --coverage --watchAll=false

echo "✅ Memory-optimized tests completed"
