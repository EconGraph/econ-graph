#!/bin/bash

# Corrected EconGraph Codebase Statistics
# Only counts files that are tracked by git (respects .gitignore)

set -e

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${BLUE}📊 EconGraph Codebase Corrected Stats${NC}"
echo -e "${BLUE}=====================================${NC}"

# Change to project root
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo -e "${YELLOW}Methodology: Only counting files tracked by git (respects .gitignore)${NC}"
echo ""

# Count lines by category using git ls-files to respect .gitignore
BACKEND_PROD=$(git ls-files 'backend/src/*.rs' | grep -v test | grep -v _test.rs | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")
BACKEND_TESTS=$(git ls-files 'backend/src/*.rs' | grep -E "(test|_test\.rs)" | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")
FRONTEND_PROD=$(git ls-files 'frontend/src/*.ts' 'frontend/src/*.tsx' 'frontend/src/*.js' 'frontend/src/*.jsx' | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")
FRONTEND_TESTS=$(git ls-files 'frontend/tests/*.ts' 'frontend/tests/*.tsx' 'frontend/tests/*.js' 'frontend/tests/*.jsx' | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")
CONFIG=$(git ls-files '*.yaml' '*.yml' '*.json' '*.toml' 'Dockerfile*' '*.tf' | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")
DOCS=$(git ls-files '*.md' 'README*' '*.txt' | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")
SCRIPTS=$(git ls-files '*.sh' | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")

TOTAL=$((BACKEND_PROD + BACKEND_TESTS + FRONTEND_PROD + FRONTEND_TESTS + CONFIG + DOCS + SCRIPTS))

echo -e "${YELLOW}Lines of Code (Git-tracked files only):${NC}"
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

# Show what files are being counted
echo ""
echo -e "${YELLOW}File Counts:${NC}"
echo -e "  Backend Production Files: $(git ls-files 'backend/src/*.rs' | grep -v test | grep -v _test.rs | wc -l)"
echo -e "  Backend Test Files:       $(git ls-files 'backend/src/*.rs' | grep -E "(test|_test\.rs)" | wc -l)"
echo -e "  Frontend Production Files: $(git ls-files 'frontend/src/*.ts' 'frontend/src/*.tsx' 'frontend/src/*.js' 'frontend/src/*.jsx' | wc -l)"
echo -e "  Frontend Test Files:      $(git ls-files 'frontend/tests/*.ts' 'frontend/tests/*.tsx' 'frontend/tests/*.js' 'frontend/tests/*.jsx' | wc -l)"

# Quick cost estimate (more realistic)
PROD_COST=$(echo "scale=0; ($BACKEND_PROD + $FRONTEND_PROD) / 17.5 * 1.35 * 75 * 8" | bc | cut -d. -f1)
TEST_COST=$(echo "scale=0; ($BACKEND_TESTS + $FRONTEND_TESTS) / 40 * 1.35 * 75 * 8" | bc | cut -d. -f1)
INFRA_COST=$(echo "scale=0; ($CONFIG + $DOCS + $SCRIPTS) / 75 * 2.0 * 75 * 8" | bc | cut -d. -f1)
TOTAL_COST=$((PROD_COST + TEST_COST + INFRA_COST))

echo ""
echo -e "${YELLOW}Estimated Development Cost (Traditional):${NC}"
echo -e "  Production Code:    \$$(printf "%'d" $PROD_COST)"
echo -e "  Test Code:          \$$(printf "%'d" $TEST_COST)"
echo -e "  Infrastructure:     \$$(printf "%'d" $INFRA_COST)"
echo -e "${GREEN}  TOTAL:              \$$(printf "%'d" $TOTAL_COST)${NC}"

echo ""
echo -e "${BLUE}💡 This corrected count excludes node_modules, target/, and other ignored files${NC}"
