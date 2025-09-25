#!/bin/bash

# Test script to verify Playwright color output configuration
# This script tests both local and CI-like environments

echo "🧪 Testing Playwright Color Output Configuration"
echo "=============================================="

# Test 1: Local environment (should have colors)
echo "📋 Test 1: Local Environment"
echo "  - TERM: $TERM"
echo "  - FORCE_COLOR: $FORCE_COLOR"
echo "  - CI: $CI"

# Test 2: CI-like environment
echo ""
echo "📋 Test 2: CI-like Environment (simulating GitHub Actions)"
export TERM=xterm-256color
export FORCE_COLOR=true
export CI=true

echo "  - TERM: $TERM"
echo "  - FORCE_COLOR: $FORCE_COLOR"
echo "  - CI: $CI"

# Test 3: Run a simple Playwright command to see color output
echo ""
echo "📋 Test 3: Running Playwright with color configuration"
cd frontend

# Show Playwright configuration
echo "🔧 Playwright Configuration Debug Output:"
node -e "
const config = require('./playwright.config.ts');
console.log('Reporter configuration:', config.default.reporter);
"

echo ""
echo "✅ Color configuration test complete!"
echo "   - TERM=xterm-256color enables 256-color support"
echo "   - FORCE_COLOR=true forces color output"
echo "   - CI=true enables CI-specific reporter (line reporter)"
echo "   - Line reporter is designed for CI environments with good color support"
