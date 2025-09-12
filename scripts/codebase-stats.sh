#!/bin/bash

# Quick EconGraph Codebase Statistics
# Simple script to get key metrics without full report generation

set -e

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${BLUE}📊 EconGraph Codebase Quick Stats${NC}"
echo -e "${BLUE}=================================${NC}"

# Change to project root
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Count lines by category
BACKEND_PROD=$(find ./backend/src -name "*.rs" -not -path "*/target/*" -not -name "*test*" -not -name "*_test.rs" -exec wc -l {} + 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")
BACKEND_TESTS=$(find ./backend/src -name "*.rs" -not -path "*/target/*" \( -name "*test*" -o -name "*_test.rs" \) -exec wc -l {} + 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")
FRONTEND_PROD=$(find ./frontend/src -name "*.ts" -o -name "*.tsx" -o -name "*.js" -o -name "*.jsx" 2>/dev/null | grep -v test | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")
FRONTEND_TESTS=$(find ./frontend -name "*.ts" -o -name "*.tsx" -o -name "*.js" -o -name "*.jsx" 2>/dev/null | grep test | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")
CONFIG=$(find . -name "*.yaml" -o -name "*.yml" -o -name "*.json" -o -name "*.toml" -o -name "Dockerfile*" -o -name "*.tf" 2>/dev/null | grep -v node_modules | grep -v target | grep -v .terraform | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")
DOCS=$(find . -name "*.md" -o -name "README*" -o -name "*.txt" 2>/dev/null | grep -v node_modules | grep -v target | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")
SCRIPTS=$(find . -name "*.sh" 2>/dev/null | grep -v test | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")

TOTAL=$((BACKEND_PROD + BACKEND_TESTS + FRONTEND_PROD + FRONTEND_TESTS + CONFIG + DOCS + SCRIPTS))

echo ""
echo -e "${YELLOW}Lines of Code:${NC}"
echo -e "  Backend Production: $(printf "%'d" $BACKEND_PROD)"
echo -e "  Backend Tests:      $(printf "%'d" $BACKEND_TESTS)"
echo -e "  Frontend Production: $(printf "%'d" $FRONTEND_PROD)"
echo -e "  Frontend Tests:     $(printf "%'d" $FRONTEND_TESTS)"
echo -e "  Configuration:      $(printf "%'d" $CONFIG)"
echo -e "  Documentation:      $(printf "%'d" $DOCS)"
echo -e "  Scripts:            $(printf "%'d" $SCRIPTS)"
echo -e "${GREEN}  TOTAL:              $(printf "%'d" $TOTAL)${NC}"

echo ""
echo -e "${YELLOW}Key Metrics:${NC}"
echo -e "  Production Code:    $(printf "%'d" $((BACKEND_PROD + FRONTEND_PROD))) ($(echo "scale=1; ($BACKEND_PROD + $FRONTEND_PROD) * 100 / $TOTAL" | bc)%)"
echo -e "  Test Code:          $(printf "%'d" $((BACKEND_TESTS + FRONTEND_TESTS))) ($(echo "scale=1; ($BACKEND_TESTS + $FRONTEND_TESTS) * 100 / $TOTAL" | bc)%)"
echo -e "  Infrastructure:     $(printf "%'d" $((CONFIG + DOCS + SCRIPTS))) ($(echo "scale=1; ($CONFIG + $DOCS + $SCRIPTS) * 100 / $TOTAL" | bc)%)"

# Quick cost estimate
PROD_COST=$(echo "scale=0; ($BACKEND_PROD + $FRONTEND_PROD) / 17.5 * 1.35 * 75 * 8" | bc | cut -d. -f1)
TEST_COST=$(echo "scale=0; ($BACKEND_TESTS + $FRONTEND_TESTS) / 40 * 1.35 * 75 * 8" | bc | cut -d. -f1)
INFRA_COST=$(echo "scale=0; ($CONFIG + $DOCS + $SCRIPTS) / 75 * 2.0 * 75 * 8" | bc | cut -d. -f1)
TOTAL_COST=$((PROD_COST + TEST_COST + INFRA_COST))

echo ""
echo -e "${YELLOW}Estimated Development Cost:${NC}"
echo -e "  Production Code:    \$$(printf "%'d" $PROD_COST)"
echo -e "  Test Code:          \$$(printf "%'d" $TEST_COST)"
echo -e "  Infrastructure:     \$$(printf "%'d" $INFRA_COST)"
echo -e "${GREEN}  TOTAL:              \$$(printf "%'d" $TOTAL_COST)${NC}"

echo ""
echo -e "${BLUE}💡 Run './scripts/analyze-codebase-simple.sh' for detailed analysis${NC}"
