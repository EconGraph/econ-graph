# Node.js Memory Optimization Guide

> **Release Engineer Standards**: This guide provides comprehensive memory optimization strategies for Node.js processes in pre-commit hooks, unit tests, and CI/CD pipelines to improve developer experience and reduce resource consumption.

## Overview

This guide addresses the extensive memory usage and slow performance of Node.js processes in pre-commit hooks and unit tests on developer machines. The optimizations focus on:

- **Memory Limits**: Setting appropriate `--max-old-space-size` limits
- **Parallelism Control**: Reducing Jest worker processes
- **ESLint Optimization**: Memory-efficient linting configurations
- **Pre-commit Hooks**: Optimized memory usage for faster execution
- **CI/CD Pipelines**: Memory-constrained test execution

## Memory Optimization Strategies

### 1. Node.js Memory Limits

#### Default Memory Allocation
- **Default**: ~1.4GB for 64-bit systems
- **Optimized**: 512MB - 2GB depending on process type

#### Memory Limit Configuration
```bash
# Light processes (Prettier, basic linting)
NODE_OPTIONS="--max-old-space-size=512"

# Medium processes (ESLint, TypeScript)
NODE_OPTIONS="--max-old-space-size=1024"

# Heavy processes (Jest tests, builds)
NODE_OPTIONS="--max-old-space-size=2048"

# Memory-constrained environments
NODE_OPTIONS="--max-old-space-size=1024 --max-semi-space-size=64"
```

### 2. Jest Configuration Optimizations

#### Memory-Efficient Jest Setup
```javascript
// jest.config.js
module.exports = {
  // Limit worker processes
  maxWorkers: process.env.CI ? 2 : Math.max(1, Math.floor(require('os').cpus().length / 2)),
  
  // Memory management
  workerIdleMemoryLimit: '512MB',
  
  // Test execution settings
  testTimeout: 30000,
  
  // Coverage optimization
  collectCoverageFrom: [
    'src/**/*.{ts,tsx}',
    '!src/**/*.d.ts',
    '!src/**/*.stories.{ts,tsx}',
    '!src/**/__tests__/**',
    '!src/**/__mocks__/**'
  ]
};
```

#### Jest Scripts Optimization
```json
{
  "scripts": {
    "test": "NODE_OPTIONS='--max-old-space-size=2048' react-scripts test",
    "test:serial": "NODE_OPTIONS='--max-old-space-size=2048' react-scripts test --runInBand --watchAll=false",
    "test:memory-optimized": "NODE_OPTIONS='--max-old-space-size=1024 --max-semi-space-size=64' jest --maxWorkers=1 --runInBand",
    "test:ci": "NODE_OPTIONS='--max-old-space-size=2048' jest --maxWorkers=2 --runInBand --coverage --watchAll=false"
  }
}
```

### 3. ESLint Memory Optimization

#### Memory-Efficient ESLint Configuration
```javascript
// .eslintrc.memory-optimized.js
module.exports = {
  // Disable memory-intensive rules
  rules: {
    'complexity': 'off',
    'max-lines-per-function': 'off',
    'max-lines': 'off',
    'max-statements': 'off',
    'max-depth': 'off',
    'max-nested-callbacks': 'off'
  },
  
  // Enable caching
  cache: true,
  cacheLocation: '.eslintcache'
};
```

#### ESLint Scripts with Memory Limits
```json
{
  "scripts": {
    "lint": "NODE_OPTIONS='--max-old-space-size=1024' eslint src --ext .ts,.tsx --max-warnings 20",
    "lint:memory-optimized": "NODE_OPTIONS='--max-old-space-size=512' eslint src --ext .ts,.tsx --max-warnings 20 --cache --cache-location .eslintcache"
  }
}
```

### 4. Pre-commit Hook Optimizations

#### Memory-Optimized Pre-commit Configuration
```yaml
# .pre-commit-config.yaml
repos:
  - repo: local
    hooks:
      - id: frontend-prettier
        name: Frontend Prettier Check (Memory Optimized)
        entry: bash -c 'cd frontend && NODE_OPTIONS="--max-old-space-size=512" npx prettier --check "src/**/*.{ts,tsx,js,jsx,json,css,md}"'
        
      - id: frontend-eslint
        name: Frontend ESLint Check (Memory Optimized)
        entry: bash -c 'cd frontend && NODE_OPTIONS="--max-old-space-size=1024" npx eslint "src/**/*.{ts,tsx,js,jsx}" --max-warnings 20 --cache --cache-location .eslintcache'
        
      - id: typescript-check
        name: TypeScript Compilation Check (Memory Optimized)
        entry: bash -c 'cd frontend && NODE_OPTIONS="--max-old-space-size=1024" npx tsc --noEmit'
```

### 5. CI/CD Pipeline Optimizations

#### Memory-Constrained CI Steps
```yaml
# .github/workflows/ci-core.yml
- name: Run linting (Memory Optimized)
  working-directory: frontend
  run: npm run lint:memory-optimized

- name: Run tests (Memory Optimized)
  working-directory: frontend
  run: npm run test:ci

- name: Run npm security audit (Memory Optimized)
  run: NODE_OPTIONS='--max-old-space-size=1024' npm audit --audit-level moderate
  working-directory: frontend
```

## Implementation Guide

### Step 1: Update Package.json Scripts

#### Frontend Package.json
```json
{
  "scripts": {
    "test": "NODE_OPTIONS='--max-old-space-size=2048' react-scripts test",
    "test:serial": "NODE_OPTIONS='--max-old-space-size=2048' react-scripts test --runInBand --watchAll=false",
    "test:memory-optimized": "NODE_OPTIONS='--max-old-space-size=1024 --max-semi-space-size=64' jest --maxWorkers=1 --runInBand",
    "test:ci": "NODE_OPTIONS='--max-old-space-size=2048' jest --maxWorkers=2 --runInBand --coverage --watchAll=false",
    "lint": "NODE_OPTIONS='--max-old-space-size=1024' eslint src --ext .ts,.tsx --max-warnings 20",
    "lint:memory-optimized": "NODE_OPTIONS='--max-old-space-size=512' eslint src --ext .ts,.tsx --max-warnings 20 --cache --cache-location .eslintcache",
    "prettier-check": "NODE_OPTIONS='--max-old-space-size=512' prettier --check 'src/**/*.{js,ts,tsx,css}'",
    "typecheck": "NODE_OPTIONS='--max-old-space-size=1024' tsc --noEmit"
  }
}
```

### Step 2: Configure Jest Memory Optimization

#### Create jest.config.js
```javascript
// frontend/jest.config.js
module.exports = {
  maxWorkers: process.env.CI ? 2 : Math.max(1, Math.floor(require('os').cpus().length / 2)),
  workerIdleMemoryLimit: '512MB',
  testTimeout: 30000,
  // ... other configurations
};
```

### Step 3: Update Pre-commit Hooks

#### Memory-Optimized Pre-commit Configuration
Update `.pre-commit-config.yaml` with memory limits for all Node.js processes.

### Step 4: Update CI Workflows

#### Memory-Constrained CI Steps
Update GitHub Actions workflows to use memory-optimized scripts.

## Memory Usage Guidelines

### Process-Specific Memory Limits

| Process Type | Memory Limit | Use Case |
|--------------|--------------|----------|
| Prettier | 512MB | Code formatting |
| ESLint (Basic) | 1024MB | Code linting |
| ESLint (Memory Optimized) | 512MB | Cached linting |
| TypeScript | 1024MB | Type checking |
| Jest (Unit Tests) | 2048MB | Test execution |
| Jest (Memory Optimized) | 1024MB | Constrained environments |
| NPM Audit | 1024MB | Security scanning |

### Environment-Specific Configurations

#### Development Environment
- **Jest**: 2GB memory, 2 workers max
- **ESLint**: 1GB memory with caching
- **Prettier**: 512MB memory

#### CI Environment
- **Jest**: 2GB memory, 2 workers max
- **ESLint**: 1GB memory with caching
- **Prettier**: 512MB memory

#### Memory-Constrained Environment
- **Jest**: 1GB memory, 1 worker (--runInBand)
- **ESLint**: 512MB memory with caching
- **Prettier**: 512MB memory

## Performance Monitoring

### Memory Usage Monitoring
```bash
# Monitor Node.js memory usage
node --inspect --max-old-space-size=2048 your-script.js

# Check memory usage during tests
NODE_OPTIONS='--max-old-space-size=2048' npm test

# Monitor with process explorer
ps aux | grep node
```

### Performance Metrics
- **Pre-commit Hook Time**: Target < 30 seconds
- **Unit Test Time**: Target < 2 minutes
- **Memory Usage**: Target < 2GB per process
- **Worker Processes**: Target ≤ 2 workers

## Troubleshooting

### Common Issues

#### Out of Memory Errors
```bash
# Increase memory limit
NODE_OPTIONS='--max-old-space-size=4096' npm test

# Use serial execution
npm run test:serial
```

#### Slow Test Execution
```bash
# Reduce parallelism
NODE_OPTIONS='--max-old-space-size=1024' jest --maxWorkers=1 --runInBand

# Use memory-optimized configuration
npm run test:memory-optimized
```

#### ESLint Performance Issues
```bash
# Enable caching
NODE_OPTIONS='--max-old-space-size=512' eslint --cache --cache-location .eslintcache

# Use memory-optimized configuration
npm run lint:memory-optimized
```

### Debugging Memory Usage

#### Check Current Memory Usage
```bash
# Node.js process memory
node -e "console.log(process.memoryUsage())"

# System memory usage
free -h

# Process-specific memory
ps -o pid,ppid,cmd,%mem,%cpu --sort=-%mem | head -10
```

#### Memory Profiling
```bash
# Generate heap snapshot
NODE_OPTIONS='--max-old-space-size=2048 --inspect' npm test

# Use Chrome DevTools to analyze memory usage
# Navigate to chrome://inspect
```

## Best Practices

### 1. Gradual Implementation
- Start with memory limits for new scripts
- Gradually update existing configurations
- Monitor performance impact

### 2. Environment-Specific Optimization
- Development: Balanced memory and speed
- CI: Memory-constrained for resource efficiency
- Production: Optimized for performance

### 3. Regular Monitoring
- Track memory usage trends
- Monitor test execution times
- Update limits based on actual usage

### 4. Documentation
- Document memory limits for each process
- Maintain performance benchmarks
- Update optimization strategies

## Migration Checklist

- [ ] Update package.json scripts with memory limits
- [ ] Create jest.config.js with memory optimization
- [ ] Update .eslintrc.memory-optimized.js
- [ ] Modify .pre-commit-config.yaml
- [ ] Update CI workflows
- [ ] Test memory-optimized configurations
- [ ] Monitor performance improvements
- [ ] Document new memory limits

## Conclusion

These memory optimizations significantly reduce Node.js memory usage in pre-commit hooks and unit tests, improving developer experience and reducing resource consumption. The configurations are designed to be:

- **Scalable**: Adapt to different environment constraints
- **Maintainable**: Easy to update and modify
- **Efficient**: Balance memory usage with performance
- **Compatible**: Work with existing tooling and workflows

For questions or issues with memory optimization, refer to the troubleshooting section or contact the Release Engineering team.
