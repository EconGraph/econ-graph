#!/usr/bin/env python3
"""
EconGraph Cost Analysis Webhook
Automatically updates cost figures when source data changes
"""

import json
import os
import subprocess
import sys
from datetime import datetime
from pathlib import Path

def load_cost_data():
    """Load current cost data from JSON file"""
    cost_file = Path("data/cost-analysis.json")
    if cost_file.exists():
        with open(cost_file, 'r') as f:
            return json.load(f)
    return None

def update_cost_analysis():
    """Run the cost analysis update script"""
    try:
        result = subprocess.run(
            ["./scripts/update-cost-analysis.sh"],
            capture_output=True,
            text=True,
            cwd=Path(__file__).parent.parent
        )

        if result.returncode == 0:
            print("✅ Cost analysis updated successfully")
            return True
        else:
            print(f"❌ Error updating cost analysis: {result.stderr}")
            return False
    except Exception as e:
        print(f"❌ Exception running update script: {e}")
        return False

def create_cost_summary():
    """Create a summary of current cost data"""
    cost_data = load_cost_data()
    if not cost_data:
        return "No cost data available"

    stats = cost_data.get('codebase_stats', {})
    costs = cost_data.get('cost_analysis', {})

    summary = f"""
# 📊 EconGraph Cost Analysis Summary

**Last Updated**: {cost_data.get('last_updated', 'Unknown')}

## Codebase Statistics
- **Total Lines**: {stats.get('total_lines', 'N/A'):,}
- **Production Code**: {stats.get('backend_production', 0) + stats.get('frontend_production', 0):,} lines
- **Test Code**: {stats.get('backend_tests', 0) + stats.get('frontend_tests', 0):,} lines
- **Infrastructure**: {stats.get('infrastructure', 0):,} lines
- **Documentation**: {stats.get('documentation', 0):,} lines

## Cost Analysis
- **Traditional Development**: ${costs.get('traditional_development', {}).get('total_cost', 0):,}
- **AI-Assisted Development**: ${costs.get('ai_assisted_development', {}).get('total_cost', 0):,}
- **Cost Savings**: ${costs.get('savings', {}).get('dollar_amount', 0):,} ({costs.get('savings', {}).get('percentage', 0):.1f}%)

## AI Usage
- **Total Cost**: ${cost_data.get('ai_usage', {}).get('total_cost', 0):.2f}
- **Total Tokens**: {cost_data.get('ai_usage', {}).get('total_tokens', 0):,}

---
*Generated automatically by EconGraph Cost Analysis System*
"""
    return summary

def main():
    """Main webhook handler"""
    print(f"🔄 Cost analysis webhook triggered at {datetime.now()}")

    # Update cost analysis
    if update_cost_analysis():
        # Create summary
        summary = create_cost_summary()

        # Write summary to file
        with open("data/cost-summary.md", "w") as f:
            f.write(summary)

        print("📊 Cost analysis webhook completed successfully")
        print(summary)
    else:
        print("❌ Cost analysis webhook failed")
        sys.exit(1)

if __name__ == "__main__":
    main()
