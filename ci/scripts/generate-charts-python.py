#!/usr/bin/env python3
"""
Visual Chart Generator using Python
Generates actual PNG/SVG images from workflow descriptions
"""

import os
import sys
from pathlib import Path

# Try to import required libraries
try:
    import matplotlib.pyplot as plt
    import matplotlib.patches as patches
    from matplotlib.patches import FancyBboxPatch, ConnectionPatch
    import numpy as np
    MATPLOTLIB_AVAILABLE = True
except ImportError:
    MATPLOTLIB_AVAILABLE = False

try:
    import graphviz
    GRAPHVIZ_AVAILABLE = True
except ImportError:
    GRAPHVIZ_AVAILABLE = False

def create_workflow_chart_matplotlib():
    """Create workflow chart using matplotlib"""
    if not MATPLOTLIB_AVAILABLE:
        print("❌ matplotlib not available. Install with: pip install matplotlib")
        return False

    # Create figure
    fig, ax = plt.subplots(1, 1, figsize=(16, 10))
    ax.set_xlim(0, 10)
    ax.set_ylim(0, 8)
    ax.axis('off')

    # Define colors
    colors = {
        'trigger': '#e1f5fe',
        'build': '#f3e5f5',
        'test': '#e8f5e8',
        'deploy': '#fff3e0',
        'mobile': '#f3e5f5'
    }

    # Define boxes
    boxes = [
        # Row 1: Trigger and Setup
        {'xy': (1, 6.5), 'width': 1.5, 'height': 0.8, 'text': 'Code Push/PR', 'color': colors['trigger']},
        {'xy': (3, 6.5), 'width': 1.5, 'height': 0.8, 'text': 'Checkout Code', 'color': colors['trigger']},
        {'xy': (5, 6.5), 'width': 1.5, 'height': 0.8, 'text': 'Setup Docker', 'color': colors['trigger']},

        # Row 2: Build Phase
        {'xy': (1, 5), 'width': 1.2, 'height': 0.8, 'text': 'Build Backend', 'color': colors['build']},
        {'xy': (2.5, 5), 'width': 1.2, 'height': 0.8, 'text': 'Build Frontend', 'color': colors['build']},
        {'xy': (4, 5), 'width': 1.2, 'height': 0.8, 'text': 'Build Test Runner', 'color': colors['build']},
        {'xy': (5.5, 5), 'width': 1.2, 'height': 0.8, 'text': 'Push to Registry', 'color': colors['build']},

        # Row 3: Test Phase
        {'xy': (0.5, 3), 'width': 1, 'height': 0.8, 'text': 'Core Tests', 'color': colors['test']},
        {'xy': (1.8, 3), 'width': 1, 'height': 0.8, 'text': 'Analysis Tests', 'color': colors['test']},
        {'xy': (3.1, 3), 'width': 1, 'height': 0.8, 'text': 'Debug Tests', 'color': colors['test']},
        {'xy': (4.4, 3), 'width': 1, 'height': 0.8, 'text': 'Comprehensive', 'color': colors['test']},
        {'xy': (5.7, 3), 'width': 1, 'height': 0.8, 'text': 'Mobile Tests', 'color': colors['mobile']},

        # Row 4: Results
        {'xy': (2, 1.5), 'width': 2, 'height': 0.8, 'text': 'Upload Results', 'color': colors['deploy']},
        {'xy': (5, 1.5), 'width': 1.5, 'height': 0.8, 'text': 'Deploy/Notify', 'color': colors['deploy']},
    ]

    # Draw boxes
    for box in boxes:
        fancy_box = FancyBboxPatch(
            (box['xy'][0], box['xy'][1]), box['width'], box['height'],
            boxstyle="round,pad=0.1",
            facecolor=box['color'],
            edgecolor='black',
            linewidth=1
        )
        ax.add_patch(fancy_box)

        # Add text
        ax.text(
            box['xy'][0] + box['width']/2,
            box['xy'][1] + box['height']/2,
            box['text'],
            ha='center', va='center',
            fontsize=8, fontweight='bold'
        )

    # Add arrows
    arrows = [
        # Trigger flow
        ((1.75, 6.5), (3, 6.5)),
        ((4.5, 6.5), (5, 6.5)),

        # Build flow
        ((5.75, 6.5), (1, 5.4)),
        ((5.75, 6.5), (2.5, 5.4)),
        ((5.75, 6.5), (4, 5.4)),
        ((2.2, 5), (2.5, 5)),
        ((3.7, 5), (4, 5)),
        ((5.2, 5), (5.5, 5)),

        # Test flow
        ((5.5, 5), (1, 3.8)),
        ((5.5, 5), (2.8, 3.8)),
        ((5.5, 5), (4.1, 3.8)),
        ((5.5, 5), (5.4, 3.8)),
        ((5.5, 5), (5.7, 3.8)),

        # Results flow
        ((1, 3), (2, 2.3)),
        ((2.8, 3), (2, 2.3)),
        ((4.1, 3), (2, 2.3)),
        ((5.4, 3), (2, 2.3)),
        ((5.7, 3), (2, 2.3)),
        ((4, 1.5), (5, 1.5)),
    ]

    for start, end in arrows:
        arrow = ConnectionPatch(
            start, end, "data", "data",
            arrowstyle="->", shrinkA=5, shrinkB=5,
            mutation_scale=20, fc="black", lw=1
        )
        ax.add_patch(arrow)

    # Add title
    ax.text(5, 7.5, 'EconGraph CI/CD Workflow',
            ha='center', va='center', fontsize=16, fontweight='bold')

    # Add legend
    legend_elements = [
        patches.Patch(color=colors['trigger'], label='Trigger/Setup'),
        patches.Patch(color=colors['build'], label='Build Phase'),
        patches.Patch(color=colors['test'], label='Test Phase'),
        patches.Patch(color=colors['mobile'], label='Mobile Tests'),
        patches.Patch(color=colors['deploy'], label='Deploy/Results')
    ]
    ax.legend(handles=legend_elements, loc='upper right', bbox_to_anchor=(0.98, 0.98))

    plt.tight_layout()
    return fig

def create_performance_chart_matplotlib():
    """Create performance comparison chart"""
    if not MATPLOTLIB_AVAILABLE:
        return False

    fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(14, 6))

    # Before: Sequential
    ax1.set_xlim(0, 8)
    ax1.set_ylim(0, 6)
    ax1.axis('off')
    ax1.set_title('Before: Sequential Execution', fontsize=14, fontweight='bold')

    sequential_steps = [
        'Build Backend', 'Build Frontend', 'Build Test Runner',
        'Run Core Tests', 'Run Analysis Tests', 'Run Debug Tests',
        'Run Comprehensive Tests', 'Run Mobile Tests'
    ]

    for i, step in enumerate(sequential_steps):
        y_pos = 5 - i * 0.6
        box = FancyBboxPatch(
            (0.5, y_pos-0.2), 2.5, 0.4,
            boxstyle="round,pad=0.05",
            facecolor='#ffcdd2',
            edgecolor='black',
            linewidth=1
        )
        ax1.add_patch(box)
        ax1.text(1.75, y_pos, step, ha='center', va='center', fontsize=8)

        if i < len(sequential_steps) - 1:
            ax1.arrow(1.75, y_pos-0.2, 0, -0.2, head_width=0.1, head_length=0.05, fc='black', ec='black')

    ax1.text(1.75, 0.5, 'Total: ~45 minutes', ha='center', va='center',
             fontsize=12, fontweight='bold', color='red')

    # After: Parallel
    ax2.set_xlim(0, 8)
    ax2.set_ylim(0, 6)
    ax2.axis('off')
    ax2.set_title('After: Parallel Execution', fontsize=14, fontweight='bold')

    # Build phase
    build_steps = ['Build Backend', 'Build Frontend', 'Build Test Runner']
    for i, step in enumerate(build_steps):
        x_pos = 1 + i * 2
        box = FancyBboxPatch(
            (x_pos-0.6, 4.5), 1.2, 0.4,
            boxstyle="round,pad=0.05",
            facecolor='#e3f2fd',
            edgecolor='black',
            linewidth=1
        )
        ax2.add_patch(box)
        ax2.text(x_pos, 4.7, step, ha='center', va='center', fontsize=8)

    # Test phase
    test_steps = ['Core', 'Analysis', 'Debug', 'Comprehensive', 'Mobile']
    for i, step in enumerate(test_steps):
        x_pos = 0.5 + i * 1.4
        box = FancyBboxPatch(
            (x_pos-0.4, 2.5), 0.8, 0.4,
            boxstyle="round,pad=0.05",
            facecolor='#c8e6c9',
            edgecolor='black',
            linewidth=1
        )
        ax2.add_patch(box)
        ax2.text(x_pos, 2.7, step, ha='center', va='center', fontsize=8)

    # Arrows
    ax2.arrow(4, 4.5, 0, -1.5, head_width=0.2, head_length=0.1, fc='black', ec='black')
    ax2.text(4, 3.5, 'Parallel', ha='center', va='center', fontsize=10, fontweight='bold')

    ax2.text(4, 1, 'Total: ~15 minutes', ha='center', va='center',
             fontsize=12, fontweight='bold', color='green')

    plt.tight_layout()
    return fig

def main():
    """Main function to generate charts"""
    script_dir = Path(__file__).parent
    charts_dir = script_dir.parent / "docs" / "charts"
    charts_dir.mkdir(parents=True, exist_ok=True)

    print("🎨 Generating Visual Charts with Python...")

    if MATPLOTLIB_AVAILABLE:
        print("📊 Creating workflow chart...")
        fig1 = create_workflow_chart_matplotlib()
        if fig1:
            fig1.savefig(charts_dir / "workflow-chart.png", dpi=300, bbox_inches='tight')
            fig1.savefig(charts_dir / "workflow-chart.svg", bbox_inches='tight')
            plt.close(fig1)
            print("✅ Generated: workflow-chart.png and workflow-chart.svg")

        print("📊 Creating performance comparison chart...")
        fig2 = create_performance_chart_matplotlib()
        if fig2:
            fig2.savefig(charts_dir / "performance-comparison.png", dpi=300, bbox_inches='tight')
            fig2.savefig(charts_dir / "performance-comparison.svg", bbox_inches='tight')
            plt.close(fig2)
            print("✅ Generated: performance-comparison.png and performance-comparison.svg")
    else:
        print("❌ matplotlib not available. Install with: pip install matplotlib")
        print("💡 Alternative: Use the mermaid-cli version: ./ci/scripts/generate-visual-charts.sh")

    print(f"\n🎉 Charts saved to: {charts_dir}")
    print("📁 Generated files:")
    for file in charts_dir.glob("*"):
        print(f"  - {file.name}")

if __name__ == "__main__":
    main()
