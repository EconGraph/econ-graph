#!/bin/bash
set -e

# Visual Chart Generator
# Generates actual PNG/SVG images from Mermaid diagrams

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DIAGRAMS_DIR="$(dirname "$SCRIPT_DIR")/docs/diagrams"
CHARTS_DIR="$(dirname "$SCRIPT_DIR")/docs/charts"

# Create output directory
mkdir -p "$CHARTS_DIR"

echo "🎨 Generating Visual Charts from Mermaid Diagrams..."

# Check if mermaid-cli is installed
if ! command -v mmdc &> /dev/null; then
    echo "❌ mermaid-cli not found. Installing..."
    npm install -g @mermaid-js/mermaid-cli
fi

# Function to generate chart from mermaid file
generate_chart() {
    local input_file="$1"
    local output_name="$2"
    local title="$3"

    echo "📊 Generating $title..."

    # Generate SVG only (better for version control)
    mmdc -i "$input_file" -o "$CHARTS_DIR/${output_name}.svg" \
        -t neutral -b white -w 1200 -H 800

    echo "✅ Generated: $CHARTS_DIR/${output_name}.svg"
}

# Generate all charts
if [ -f "$DIAGRAMS_DIR/current-workflow.mmd" ]; then
    generate_chart "$DIAGRAMS_DIR/current-workflow.mmd" "current-workflow" "Current Workflow"
fi

if [ -f "$DIAGRAMS_DIR/separated-workflow.mmd" ]; then
    generate_chart "$DIAGRAMS_DIR/separated-workflow.mmd" "separated-workflow" "Separated Build/Run Workflow"
fi

if [ -f "$DIAGRAMS_DIR/optimized-workflow.mmd" ]; then
    generate_chart "$DIAGRAMS_DIR/optimized-workflow.mmd" "optimized-workflow" "Optimized Workflow with Caching"
fi

if [ -f "$DIAGRAMS_DIR/test-architecture.mmd" ]; then
    generate_chart "$DIAGRAMS_DIR/test-architecture.mmd" "test-architecture" "Test Architecture"
fi

if [ -f "$DIAGRAMS_DIR/docker-architecture.mmd" ]; then
    generate_chart "$DIAGRAMS_DIR/docker-architecture.mmd" "docker-architecture" "Docker Architecture"
fi

if [ -f "$DIAGRAMS_DIR/performance-comparison.mmd" ]; then
    generate_chart "$DIAGRAMS_DIR/performance-comparison.mmd" "performance-comparison" "Performance Comparison"
fi

if [ -f "$DIAGRAMS_DIR/complete-ci-pipeline.mmd" ]; then
    generate_chart "$DIAGRAMS_DIR/complete-ci-pipeline.mmd" "complete-ci-pipeline" "Complete CI Pipeline"
fi

if [ -f "$DIAGRAMS_DIR/ci-job-dependencies.mmd" ]; then
    generate_chart "$DIAGRAMS_DIR/ci-job-dependencies.mmd" "ci-job-dependencies" "CI Job Dependencies"
fi

echo ""
echo "🎉 Visual charts generated successfully!"
echo "📁 Charts saved to: $CHARTS_DIR"
echo ""
echo "📊 Generated files:"
ls -la "$CHARTS_DIR"/*.svg 2>/dev/null || echo "No SVG files generated"
echo ""
echo "💡 Usage:"
echo "  - SVG files: Vector graphics, perfect for version control and web display"
echo "  - Open with any image viewer, web browser, or markdown renderer"
echo "  - Text-based format enables proper git diffs and smaller repository size"
