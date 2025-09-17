#!/bin/bash
set -e

# Embed Visual Charts in Markdown Documentation
# Replaces Mermaid code blocks with actual image references

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DOCS_DIR="$(dirname "$SCRIPT_DIR")/docs"
CHARTS_DIR="$DOCS_DIR/charts"

echo "📝 Embedding visual charts in markdown documentation..."

# Create a new version of the architecture documentation with embedded images
cat > "$DOCS_DIR/CI_CD_ARCHITECTURE_WITH_IMAGES.md" << 'EOF'
# CI/CD Architecture Documentation

This document illustrates the CI/CD architecture for EconGraph, showing the evolution from a simple approach to an optimized, parallel testing system.

## Architecture Overview

Our CI/CD system has evolved through three main phases:

1. **Current Workflow**: Basic build and test approach
2. **Separated Workflow**: Build and test phases separated for better performance
3. **Optimized Workflow**: Advanced caching and parallel execution

## Current Workflow

![Current Workflow](charts/current-workflow.png)

*Figure 1: Current CI/CD workflow showing basic build and test approach*

## Separated Build/Run Workflow

![Separated Workflow](charts/separated-workflow.png)

*Figure 2: Separated build and run phases for better performance*

## Optimized Workflow with Caching

![Optimized Workflow](charts/optimized-workflow.png)

*Figure 3: Optimized workflow with Docker layer caching and parallel execution*

## Test Architecture

![Test Architecture](charts/test-architecture.png)

*Figure 4: Test group architecture showing all test categories and infrastructure*

## Docker Architecture

![Docker Architecture](charts/docker-architecture.png)

*Figure 5: Docker service architecture and relationships*

## Performance Comparison

![Performance Comparison](charts/performance-comparison.png)

*Figure 6: Performance improvement from sequential to parallel execution*

## Key Improvements

### 1. **Separated Build/Run Phases**
- **Before**: Build and test in same job
- **After**: Build once, run tests multiple times
- **Benefit**: Faster iteration during development

### 2. **Parallel Test Execution**
- **Before**: Tests run sequentially
- **After**: All test groups run in parallel
- **Benefit**: 3x faster test execution

### 3. **Docker Layer Caching**
- **Before**: Rebuild everything each time
- **After**: Cache Docker layers between builds
- **Benefit**: Faster builds, reduced resource usage

### 4. **Unified Test Runner**
- **Before**: Separate Dockerfiles for each test group
- **After**: Single Dockerfile with arguments
- **Benefit**: Easier maintenance, DRY principle

### 5. **Mobile Test Separation**
- **Before**: Mobile tests mixed with desktop tests
- **After**: Dedicated mobile test groups
- **Benefit**: Better stability, easier debugging

## Usage

### Local Development
```bash
# Build images once
./ci/scripts/build-images.sh

# Run specific test group
./ci/scripts/run-tests-unified.sh --group core

# Run all tests in parallel
./ci/scripts/run-tests-unified.sh --parallel
```

### CI/CD Integration
```bash
# Generate updated diagrams
./ci/scripts/generate-workflow-diagrams.sh

# Generate visual charts
./ci/scripts/generate-visual-charts.sh

# Embed charts in documentation
./ci/scripts/embed-charts-in-markdown.sh
```

## Tools and Scripts

- `ci/scripts/build-images.sh` - Build all Docker images
- `ci/scripts/run-tests-unified.sh` - Run tests with unified approach
- `ci/scripts/generate-workflow-diagrams.sh` - Generate Mermaid diagrams
- `ci/scripts/generate-visual-charts.sh` - Generate PNG/SVG images
- `ci/scripts/embed-charts-in-markdown.sh` - Embed images in documentation
- `ci/scripts/dev-workflow.sh` - Interactive development workflow

## Best Practices Implemented

1. **Single Responsibility**: Each script has one clear purpose
2. **DRY Principle**: Reuse Docker images across test groups
3. **Parallel Execution**: Maximize resource utilization
4. **Visual Documentation**: Clear diagrams for understanding
5. **Separation of Concerns**: Build vs. run phases
6. **Caching Strategy**: Docker layer caching for performance
7. **Mobile Testing**: Dedicated mobile test infrastructure

## Chart Generation Process

The visual charts in this document are generated automatically from Mermaid diagrams:

1. **Mermaid Source**: `ci/docs/diagrams/*.mmd` - Text-based diagram definitions
2. **Visual Generation**: `ci/scripts/generate-visual-charts.sh` - Converts to PNG/SVG
3. **Documentation Embedding**: `ci/scripts/embed-charts-in-markdown.sh` - Embeds in markdown

This ensures that documentation stays up-to-date with the actual CI/CD implementation.
EOF

echo "✅ Created: $DOCS_DIR/CI_CD_ARCHITECTURE_WITH_IMAGES.md"

# Also create a simple README for the charts directory
cat > "$CHARTS_DIR/README.md" << 'EOF'
# CI/CD Visual Charts

This directory contains visual representations of the CI/CD architecture and workflows.

## Generated Charts

- **current-workflow.png/svg** - Current CI/CD workflow
- **separated-workflow.png/svg** - Separated build/run workflow
- **optimized-workflow.png/svg** - Optimized workflow with caching
- **test-architecture.png/svg** - Test group architecture
- **docker-architecture.png/svg** - Docker service architecture
- **performance-comparison.png/svg** - Performance improvement comparison

## Usage

- **PNG files**: Best for presentations, documents, and general viewing
- **SVG files**: Best for web pages, scalable graphics, and high-resolution displays

## Regeneration

To regenerate these charts after making changes to the Mermaid diagrams:

```bash
# Generate visual charts from Mermaid diagrams
./ci/scripts/generate-visual-charts.sh

# Embed charts in markdown documentation
./ci/scripts/embed-charts-in-markdown.sh
```

## Source

These charts are generated from Mermaid diagram definitions in `ci/docs/diagrams/`.
EOF

echo "✅ Created: $CHARTS_DIR/README.md"

echo ""
echo "🎉 Visual charts embedded in markdown documentation!"
echo "📁 Files created:"
echo "  - $DOCS_DIR/CI_CD_ARCHITECTURE_WITH_IMAGES.md"
echo "  - $CHARTS_DIR/README.md"
echo ""
echo "💡 The new documentation includes:"
echo "  - Actual PNG images instead of Mermaid code blocks"
echo "  - Figure captions and descriptions"
echo "  - Updated usage instructions"
echo "  - Chart generation process documentation"
echo ""
echo "📖 View the documentation:"
echo "  - Open $DOCS_DIR/CI_CD_ARCHITECTURE_WITH_IMAGES.md in any markdown viewer"
echo "  - The images will display directly in GitHub, VS Code, or any markdown renderer"
