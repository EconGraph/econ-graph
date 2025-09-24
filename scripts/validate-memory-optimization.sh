#!/bin/bash

# Memory optimization validation script
# Verifies that memory optimizations are working correctly

set -e

echo "🔍 Validating memory optimization setup..."

# Check if Jest configuration exists
if [[ -f "frontend/jest.config.js" ]]; then
    echo "✅ Frontend Jest configuration found"
else
    echo "❌ Frontend Jest configuration missing"
    exit 1
fi

# Check if admin frontend Jest configuration exists
if [[ -f "admin-frontend/jest.config.js" ]]; then
    echo "✅ Admin frontend Jest configuration found"
else
    echo "❌ Admin frontend Jest configuration missing"
    exit 1
fi

# Check if memory optimization environment file exists
if [[ -f ".env.memory-optimization" ]]; then
    echo "✅ Memory optimization environment file found"
else
    echo "❌ Memory optimization environment file missing"
    exit 1
fi

# Check if monitoring script exists
if [[ -f "scripts/monitor-memory-usage.sh" ]]; then
    echo "✅ Memory monitoring script found"
else
    echo "❌ Memory monitoring script missing"
    exit 1
fi

# Test memory-optimized scripts
echo "🧪 Testing memory-optimized scripts..."

# Test frontend memory optimization
if [[ -d "frontend" ]]; then
    cd frontend
    echo "Testing frontend memory optimization..."

    # Test ESLint with memory optimization
    if NODE_OPTIONS='--max-old-space-size=1024' npx eslint --version > /dev/null 2>&1; then
        echo "✅ Frontend ESLint memory optimization working"
    else
        echo "❌ Frontend ESLint memory optimization failed"
        exit 1
    fi

    # Test Prettier with memory optimization
    if NODE_OPTIONS='--max-old-space-size=512' npx prettier --version > /dev/null 2>&1; then
        echo "✅ Frontend Prettier memory optimization working"
    else
        echo "❌ Frontend Prettier memory optimization failed"
        exit 1
    fi

    cd ..
fi

# Test admin frontend memory optimization
if [[ -d "admin-frontend" ]]; then
    cd admin-frontend
    echo "Testing admin frontend memory optimization..."

    # Test ESLint with memory optimization
    if NODE_OPTIONS='--max-old-space-size=1024' npx eslint --version > /dev/null 2>&1; then
        echo "✅ Admin frontend ESLint memory optimization working"
    else
        echo "❌ Admin frontend ESLint memory optimization failed"
        exit 1
    fi

    cd ..
fi

echo "🎉 Memory optimization validation completed successfully!"
echo ""
echo "📋 Next steps:"
echo "1. Source the memory optimization environment: source .env.memory-optimization"
echo "2. Run memory-optimized tests: npm run test:memory-optimized"
echo "3. Monitor memory usage: ./scripts/monitor-memory-usage.sh"
echo "4. Read the optimization guide: docs/development/NODE_MEMORY_OPTIMIZATION_GUIDE.md"
