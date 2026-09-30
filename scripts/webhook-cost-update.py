#!/usr/bin/env python3
"""
EconGraph Cost Analysis Webhook
Automatically updates cost figures when source data changes
"""

import subprocess
import sys
from datetime import datetime
from pathlib import Path

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

def main():
    """Main webhook handler"""
    print(f"🔄 Cost analysis webhook triggered at {datetime.now()}")

    # Update cost analysis
    if update_cost_analysis():
        # The updater publishes only the standalone archive report and JSON.
        print("Cost analysis webhook completed successfully")
    else:
        print("❌ Cost analysis webhook failed")
        sys.exit(1)

if __name__ == "__main__":
    main()


