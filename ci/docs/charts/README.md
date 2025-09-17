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
