#!/bin/bash

# EconGraph Cost Analysis Auto-Update Script
# ==========================================
#
# PURPOSE:
#   This script automatically updates cost figures in documentation based on source data.
#   It extracts current codebase statistics, calculates development costs, and updates
#   all business documentation with the latest figures.
#
# USAGE:
#   ./scripts/update-cost-analysis.sh
#
# DEPENDENCIES:
#   - scripts/codebase-stats-corrected.sh (for codebase statistics - the only stats script)
#   - data/usage-events-2025-09-26.csv (for AI usage costs)
#   - bc (for mathematical calculations)
#
# OUTPUT:
#   - Updates all documentation files with latest cost figures
#   - Creates data/cost-analysis.json with structured cost data
#   - Provides summary of changes made
#
# AUTOMATION:
#   - Called by GitHub Actions workflow (.github/workflows/update-cost-analysis.yml)
#   - Can be triggered manually or via webhook
#   - Runs daily at 6 AM UTC via GitHub Actions
#
# AUTHOR: EconGraph Development Team
# VERSION: 1.0
# LAST UPDATED: September 2025

set -e

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${BLUE}📊 EconGraph Cost Analysis Auto-Update${NC}"
echo -e "${BLUE}=====================================${NC}"

# Change to project root
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Source data files
COST_DATA_FILE="data/cost-analysis.json"
USAGE_CSV_FILE="data/usage-events-2025-09-26.csv"
CODEBASE_STATS_FILE="scripts/codebase-stats-corrected.sh"

# Create data directory if it doesn't exist
mkdir -p data

# Function to get current codebase stats
# ======================================
# Extracts current codebase statistics by running the corrected stats script
# and parsing the output to get line counts for different code categories.
# This ensures we're always working with the latest codebase metrics.
get_codebase_stats() {
    echo -e "${YELLOW}📈 Getting current codebase statistics...${NC}"

    # Run the corrected stats script to get current codebase metrics
    # The script uses git ls-files to respect .gitignore and exclude auto-generated files
    if [ -f "$CODEBASE_STATS_FILE" ]; then
        bash "$CODEBASE_STATS_FILE" > /tmp/codebase_stats.txt

        # Extract key metrics from the stats script output
        # Use a more robust approach to handle ANSI color codes
        BACKEND_PROD=$(grep "Backend Production:" /tmp/codebase_stats.txt | grep -o '[0-9]\+' | head -1)
        BACKEND_TESTS=$(grep "Backend Tests:" /tmp/codebase_stats.txt | grep -o '[0-9]\+' | head -1)
        FRONTEND_PROD=$(grep "Frontend Production:" /tmp/codebase_stats.txt | grep -o '[0-9]\+' | head -1)
        FRONTEND_TESTS=$(grep "Frontend Tests:" /tmp/codebase_stats.txt | grep -o '[0-9]\+' | head -1)
        CONFIG=$(grep "Configuration:" /tmp/codebase_stats.txt | grep -o '[0-9]\+' | head -1)
        DOCS=$(grep "Documentation:" /tmp/codebase_stats.txt | grep -o '[0-9]\+' | head -1)
        SCRIPTS=$(grep "Scripts:" /tmp/codebase_stats.txt | grep -o '[0-9]\+' | head -1)
        TOTAL=$(grep "TOTAL:" /tmp/codebase_stats.txt | head -1 | grep -o '[0-9]\+' | tail -1)

        # Validate that we got numbers, not text
        if ! [[ "$TOTAL" =~ ^[0-9]+$ ]]; then
            echo -e "${RED}❌ Failed to parse TOTAL from stats output${NC}"
            echo "Debug: TOTAL='$TOTAL'"
            cat /tmp/codebase_stats.txt
            exit 1
        fi

        echo "Extracted stats: Total=$TOTAL, Backend=$BACKEND_PROD, Frontend=$FRONTEND_PROD"
    else
        echo -e "${RED}❌ Codebase stats script not found: $CODEBASE_STATS_FILE${NC}"
        exit 1
    fi
}

# Function to get AI usage costs
get_ai_costs() {
    echo -e "${YELLOW}🤖 Getting AI usage costs...${NC}"

    if [ -f "$USAGE_CSV_FILE" ]; then
        # Extract total cost and tokens from CSV
        TOTAL_COST=$(tail -n +2 "$USAGE_CSV_FILE" | sed 's/"//g' | awk -F',' 'BEGIN{total=0} {if($10 < 10) total += $10} END {print total}')
        TOTAL_TOKENS=$(tail -n +2 "$USAGE_CSV_FILE" | sed 's/"//g' | awk -F',' 'BEGIN{total=0} {total += $9} END {print total}')

        echo "AI Usage: Cost=\$${TOTAL_COST}, Tokens=${TOTAL_TOKENS}"
    else
        echo -e "${YELLOW}⚠️  Usage CSV not found, using default values${NC}"
        TOTAL_COST="937.84"
        TOTAL_TOKENS="3250000000"
    fi
}

# Function to calculate costs
calculate_costs() {
    echo -e "${YELLOW}💰 Calculating updated costs...${NC}"

    # Production code cost
    PROD_LINES=$((BACKEND_PROD + FRONTEND_PROD))
    PROD_COST=$(echo "scale=0; $PROD_LINES * 2.50" | bc)

    # Test code cost
    TEST_LINES=$((BACKEND_TESTS + FRONTEND_TESTS))
    TEST_COST=$(echo "scale=0; $TEST_LINES * 1.25" | bc)

    # Infrastructure cost
    INFRA_LINES=$((CONFIG + SCRIPTS))
    INFRA_COST=$(echo "scale=0; $INFRA_LINES * 1.00" | bc)

    # Documentation cost
    DOCS_COST=$(echo "scale=0; $DOCS * 0.75" | bc)

    # Base cost
    BASE_COST=$(echo "scale=0; $PROD_COST + $TEST_COST + $INFRA_COST + $DOCS_COST" | bc)

    # Overhead (75% of base cost)
    OVERHEAD=$(echo "scale=0; $BASE_COST * 0.75" | bc)

    # Total traditional cost
    TRADITIONAL_COST=$(echo "scale=0; $BASE_COST + $OVERHEAD" | bc)

    # AI-assisted cost
    STAFF_COST="33600"  # 224 hours * $150/hour
    AI_COST=$(echo "scale=0; $TOTAL_COST + 20" | bc)  # Token cost + Cursor Pro
    AI_TOTAL=$(echo "scale=0; $STAFF_COST + $AI_COST" | bc)

    # Cost savings
    SAVINGS=$(echo "scale=0; $TRADITIONAL_COST - $AI_TOTAL" | bc)
    if [ $(echo "$TRADITIONAL_COST > 0" | bc) -eq 1 ]; then
        SAVINGS_PERCENT=$(echo "scale=1; $SAVINGS * 100 / $TRADITIONAL_COST" | bc)
    else
        SAVINGS_PERCENT="0"
    fi

    echo "Calculated: Traditional=\$${TRADITIONAL_COST}, AI=\$${AI_TOTAL}, Savings=${SAVINGS_PERCENT}%"
}

# Function to update documentation
update_documentation() {
    echo -e "${YELLOW}📝 Updating documentation...${NC}"

    # Update cost assumptions document
    if [ -f "docs/business/COST_ASSUMPTIONS_AND_PRODUCTIVITY_ANALYSIS.md" ]; then
        # Update total lines
        sed -i.bak "s/159,335 lines of manually written code/$TOTAL lines of manually written code/g" "docs/business/COST_ASSUMPTIONS_AND_PRODUCTIVITY_ANALYSIS.md"

        # Update production code lines
        sed -i.bak "s/72,610 lines - 45.6%/$PROD_LINES lines - $(echo "scale=1; $PROD_LINES * 100 / $TOTAL" | bc)%/g" "docs/business/COST_ASSUMPTIONS_AND_PRODUCTIVITY_ANALYSIS.md"

        # Update cost figures
        sed -i.bak "s/\$456,000/\$${TRADITIONAL_COST}/g" "docs/business/COST_ASSUMPTIONS_AND_PRODUCTIVITY_ANALYSIS.md"
        sed -i.bak "s/~\$35,000/~\$${AI_TOTAL}/g" "docs/business/COST_ASSUMPTIONS_AND_PRODUCTIVITY_ANALYSIS.md"
        sed -i.bak "s/92.3% cost reduction/${SAVINGS_PERCENT}% cost reduction/g" "docs/business/COST_ASSUMPTIONS_AND_PRODUCTIVITY_ANALYSIS.md"

        echo "✅ Updated COST_ASSUMPTIONS_AND_PRODUCTIVITY_ANALYSIS.md"
    fi

    # Update README
    if [ -f "README.md" ]; then
        sed -i.bak "s/159,335 lines of code/$TOTAL lines of code/g" "README.md"
        sed -i.bak "s/\$456,000/\$${TRADITIONAL_COST}/g" "README.md"
        sed -i.bak "s/~\$35,000/~\$${AI_TOTAL}/g" "README.md"

        echo "✅ Updated README.md"
    fi

    # Update other business documents
    for file in "docs/business/PRODUCT_SUMMARY_2025.md" "docs/business/INVESTOR_PITCH.md" "demo-tools/PROFESSIONAL_DEMO_SUMMARY.md"; do
        if [ -f "$file" ]; then
            sed -i.bak "s/159,335 lines of code/$TOTAL lines of code/g" "$file"
            sed -i.bak "s/\$456,000/\$${TRADITIONAL_COST}/g" "$file"
            sed -i.bak "s/~\$35,000/~\$${AI_TOTAL}/g" "$file"
            echo "✅ Updated $(basename "$file")"
        fi
    done
}

# Function to create cost data JSON
create_cost_data() {
    echo -e "${YELLOW}📊 Creating cost data JSON...${NC}"

    cat > "$COST_DATA_FILE" << EOF
{
  "last_updated": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
  "codebase_stats": {
    "total_lines": $TOTAL,
    "backend_production": $BACKEND_PROD,
    "backend_tests": $BACKEND_TESTS,
    "frontend_production": $FRONTEND_PROD,
    "frontend_tests": $FRONTEND_TESTS,
    "infrastructure": $INFRA_LINES,
    "documentation": $DOCS,
    "scripts": $SCRIPTS
  },
  "cost_analysis": {
    "traditional_development": {
      "base_cost": $BASE_COST,
      "overhead_cost": $OVERHEAD,
      "total_cost": $TRADITIONAL_COST
    },
    "ai_assisted_development": {
      "staff_engineer_cost": $STAFF_COST,
      "ai_tool_cost": $AI_COST,
      "total_cost": $AI_TOTAL
    },
    "savings": {
      "dollar_amount": $SAVINGS,
      "percentage": $SAVINGS_PERCENT
    }
  },
  "ai_usage": {
    "total_cost": $TOTAL_COST,
    "total_tokens": $TOTAL_TOKENS
  }
}
EOF

    echo "✅ Created cost data JSON: $COST_DATA_FILE"
}

# Main execution
echo -e "${GREEN}🚀 Starting cost analysis update...${NC}"

get_codebase_stats
get_ai_costs
calculate_costs
update_documentation
create_cost_data

echo -e "${GREEN}✅ Cost analysis update completed!${NC}"
echo -e "${BLUE}📊 Summary:${NC}"
echo -e "  Total Lines of Code: $TOTAL"
echo -e "  Traditional Cost: \$${TRADITIONAL_COST}"
echo -e "  AI-Assisted Cost: \$${AI_TOTAL}"
echo -e "  Cost Savings: \$${SAVINGS} (${SAVINGS_PERCENT}%)"

# Clean up backup files
find . -name "*.bak" -delete

echo -e "${GREEN}🎉 All documentation updated with latest cost figures!${NC}"
