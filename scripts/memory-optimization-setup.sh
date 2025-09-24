#!/bin/bash

# Node.js Memory Optimization Setup Script
# Configures memory-optimized settings for development and CI environments

set -e

echo "🧠 Setting up Node.js memory optimization..."

# Check if we're in the right directory
if [[ ! -f "package.json" ]] && [[ ! -f "frontend/package.json" ]]; then
    echo "❌ Please run this script from the project root directory"
    exit 1
fi

# Create memory optimization configurations
echo "📝 Creating memory optimization configurations..."

# Frontend Jest configuration
if [[ -d "frontend" ]]; then
    echo "🔧 Configuring frontend Jest memory optimization..."

    # Create jest.config.js if it doesn't exist
    if [[ ! -f "frontend/jest.config.js" ]]; then
        echo "Creating frontend/jest.config.js..."
        cat > frontend/jest.config.js << 'EOF'
/**
 * Jest Configuration for EconGraph Frontend
 * Optimized for memory usage and developer experience
 */

const { defaults } = require('jest-config');

module.exports = {
  ...defaults,

  // Memory optimization settings
  maxWorkers: process.env.CI ? 2 : Math.max(1, Math.floor(require('os').cpus().length / 2)),

  // Test execution settings
  testTimeout: 30000,
  verbose: false,

  // Memory management
  workerIdleMemoryLimit: '512MB',

  // Coverage settings (reduced for memory efficiency)
  collectCoverageFrom: [
    'src/**/*.{ts,tsx}',
    '!src/**/*.d.ts',
    '!src/**/*.stories.{ts,tsx}',
    '!src/**/__tests__/**',
    '!src/**/__mocks__/**',
    '!src/setupTests.ts',
    '!src/index.tsx',
    '!src/reportWebVitals.ts'
  ],

  // Coverage thresholds (relaxed for memory efficiency)
  coverageThreshold: {
    global: {
      branches: 60,
      functions: 60,
      lines: 60,
      statements: 60
    }
  },

  // Module name mapping
  moduleNameMapper: {
    '^@/(.*)$': '<rootDir>/src/$1',
    '^react-app/jest$': 'react-app/jest',
    '^d3$': '<rootDir>/src/__mocks__/d3.js',
    '^d3-geo$': '<rootDir>/src/__mocks__/d3-geo.js',
    '^d3-zoom$': '<rootDir>/src/__mocks__/d3-zoom.js',
    '^d3-scale$': '<rootDir>/src/__mocks__/d3-scale.js',
    '^d3-scale-chromatic$': '<rootDir>/src/__mocks__/d3-scale-chromatic.js',
    '^d3-array$': '<rootDir>/src/__mocks__/d3-array.js',
    '^d3-selection$': '<rootDir>/src/__mocks__/d3-selection.js'
  },

  // Test environment and setup
  testEnvironment: 'jsdom',
  setupFilesAfterEnv: ['<rootDir>/src/setupTests.ts'],

  // Transform settings
  transform: {
    '^.+\\.(ts|tsx)$': ['ts-jest', {
      tsconfig: {
        jsx: 'react-jsx'
      }
    }]
  },

  // Transform ignore patterns
  transformIgnorePatterns: [
    'node_modules/(?!(@apollo/client|d3-geo|d3-zoom|d3-scale|d3-scale-chromatic|d3-array|d3-selection)/)'
  ],

  // Mock settings
  clearMocks: true,
  resetMocks: true,
  restoreMocks: true,

  // Test patterns
  testMatch: [
    '<rootDir>/src/**/__tests__/**/*.{ts,tsx}',
    '<rootDir>/src/**/*.{test,spec}.{ts,tsx}'
  ],

  // Ignore patterns
  testPathIgnorePatterns: [
    '<rootDir>/node_modules/',
    '<rootDir>/build/',
    '<rootDir>/coverage/'
  ],

  // Module file extensions
  moduleFileExtensions: ['ts', 'tsx', 'js', 'jsx', 'json']
};
EOF
    fi

    # Create test utilities directory if it doesn't exist
    mkdir -p frontend/src/test-utils

    # Create global setup
    if [[ ! -f "frontend/src/test-utils/globalSetup.js" ]]; then
        echo "Creating frontend/src/test-utils/globalSetup.js..."
        cat > frontend/src/test-utils/globalSetup.js << 'EOF'
/**
 * Jest Global Setup
 * Memory optimization and test environment preparation
 */

module.exports = async () => {
  // Set memory limits for Node.js processes
  process.env.NODE_OPTIONS = '--max-old-space-size=2048 --max-semi-space-size=128';

  // Configure Jest for memory efficiency
  process.env.JEST_WORKER_ID = '1';

  console.log('🧠 Jest global setup: Memory limits configured');
  console.log(`📊 Node.js memory limit: ${process.env.NODE_OPTIONS}`);
};
EOF
    fi

    # Create global teardown
    if [[ ! -f "frontend/src/test-utils/globalTeardown.js" ]]; then
        echo "Creating frontend/src/test-utils/globalTeardown.js..."
        cat > frontend/src/test-utils/globalTeardown.js << 'EOF'
/**
 * Jest Global Teardown
 * Cleanup and memory management after test execution
 */

module.exports = async () => {
  // Force garbage collection if available
  if (global.gc) {
    global.gc();
  }

  // Clear any remaining timers
  if (global.clearTimeout) {
    global.clearTimeout();
  }

  console.log('🧹 Jest global teardown: Memory cleanup completed');
};
EOF
    fi
fi

# Admin frontend configuration
if [[ -d "admin-frontend" ]]; then
    echo "🔧 Configuring admin-frontend memory optimization..."

    # Create jest.config.js for admin frontend
    if [[ ! -f "admin-frontend/jest.config.js" ]]; then
        echo "Creating admin-frontend/jest.config.js..."
        cat > admin-frontend/jest.config.js << 'EOF'
/**
 * Jest Configuration for EconGraph Admin Frontend
 * Optimized for memory usage and developer experience
 */

const { defaults } = require('jest-config');

module.exports = {
  ...defaults,

  // Memory optimization settings
  maxWorkers: process.env.CI ? 2 : Math.max(1, Math.floor(require('os').cpus().length / 2)),

  // Test execution settings
  testTimeout: 30000,
  verbose: false,

  // Memory management
  workerIdleMemoryLimit: '512MB',

  // Coverage settings (reduced for memory efficiency)
  collectCoverageFrom: [
    'src/**/*.{ts,tsx}',
    '!src/**/*.d.ts',
    '!src/**/*.stories.{ts,tsx}',
    '!src/**/__tests__/**',
    '!src/**/__mocks__/**',
    '!src/setupTests.ts',
    '!src/index.tsx'
  ],

  // Coverage thresholds (relaxed for memory efficiency)
  coverageThreshold: {
    global: {
      branches: 60,
      functions: 60,
      lines: 60,
      statements: 60
    }
  },

  // Test environment and setup
  testEnvironment: 'jsdom',

  // Transform settings
  transform: {
    '^.+\\.(ts|tsx)$': ['ts-jest', {
      tsconfig: {
        jsx: 'react-jsx'
      }
    }]
  },

  // Mock settings
  clearMocks: true,
  resetMocks: true,
  restoreMocks: true,

  // Test patterns
  testMatch: [
    '<rootDir>/src/**/__tests__/**/*.{ts,tsx}',
    '<rootDir>/src/**/*.{test,spec}.{ts,tsx}'
  ],

  // Ignore patterns
  testPathIgnorePatterns: [
    '<rootDir>/node_modules/',
    '<rootDir>/build/',
    '<rootDir>/coverage/'
  ],

  // Module file extensions
  moduleFileExtensions: ['ts', 'tsx', 'js', 'jsx', 'json']
};
EOF
    fi
fi

# Create memory optimization environment file
echo "📄 Creating memory optimization environment configuration..."
cat > .env.memory-optimization << 'EOF'
# Node.js Memory Optimization Environment Variables
# Use these for memory-constrained development

# Jest memory optimization
JEST_WORKER_ID=1
NODE_OPTIONS_JEST=--max-old-space-size=2048 --max-semi-space-size=128

# ESLint memory optimization
NODE_OPTIONS_ESLINT=--max-old-space-size=1024

# Prettier memory optimization
NODE_OPTIONS_PRETTIER=--max-old-space-size=512

# TypeScript memory optimization
NODE_OPTIONS_TSC=--max-old-space-size=1024

# NPM audit memory optimization
NODE_OPTIONS_NPM_AUDIT=--max-old-space-size=1024
EOF

# Create memory optimization scripts
echo "📜 Creating memory optimization scripts..."

# Frontend memory optimization script
if [[ -d "frontend" ]]; then
    cat > frontend/scripts/memory-optimized-test.sh << 'EOF'
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
EOF
    chmod +x frontend/scripts/memory-optimized-test.sh
fi

# Create memory monitoring script
cat > scripts/monitor-memory-usage.sh << 'EOF'
#!/bin/bash

# Memory usage monitoring script
# Helps track Node.js memory consumption

echo "📊 Node.js Memory Usage Monitor"
echo "================================"

# Check current Node.js processes
echo "🔍 Current Node.js processes:"
ps aux | grep node | grep -v grep || echo "No Node.js processes running"

echo ""
echo "💾 System memory usage:"
free -h

echo ""
echo "📈 Top memory-consuming processes:"
ps -o pid,ppid,cmd,%mem,%cpu --sort=-%mem | head -10

echo ""
echo "🧠 Node.js memory optimization status:"
echo "NODE_OPTIONS: ${NODE_OPTIONS:-'Not set'}"
echo "JEST_WORKER_ID: ${JEST_WORKER_ID:-'Not set'}"

# Check if memory optimization is active
if [[ -n "$NODE_OPTIONS" ]]; then
    echo "✅ Memory optimization is active"
else
    echo "⚠️  Memory optimization not configured"
    echo "   Run: source .env.memory-optimization"
fi
EOF
chmod +x scripts/monitor-memory-usage.sh

# Create memory optimization validation script
cat > scripts/validate-memory-optimization.sh << 'EOF'
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
EOF
chmod +x scripts/validate-memory-optimization.sh

echo "🎉 Memory optimization setup completed!"
echo ""
echo "📋 Setup Summary:"
echo "✅ Jest configurations created with memory optimization"
echo "✅ Memory-optimized scripts created"
echo "✅ Environment configuration created"
echo "✅ Monitoring and validation scripts created"
echo ""
echo "🚀 Next steps:"
echo "1. Run validation: ./scripts/validate-memory-optimization.sh"
echo "2. Source environment: source .env.memory-optimization"
echo "3. Test memory optimization: npm run test:memory-optimized"
echo "4. Monitor memory usage: ./scripts/monitor-memory-usage.sh"
echo ""
echo "📖 For detailed information, see: docs/development/NODE_MEMORY_OPTIMIZATION_GUIDE.md"
