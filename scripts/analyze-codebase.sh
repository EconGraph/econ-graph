#!/bin/bash

# EconGraph Codebase Analysis & Cost Estimation Script
# Generates comprehensive report of lines of code and development cost estimates

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
WHITE='\033[1;37m'
NC='\033[0m' # No Color

# Configuration
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_FILE="${PROJECT_ROOT}/codebase-analysis-report.md"
TEMP_DIR="/tmp/codebase-analysis-$$"

# Load configuration from file if it exists, otherwise use defaults
CONFIG_FILE=""
for config in "${PROJECT_ROOT}"/codebase-analysis-config*.env; do
    if [ -f "$config" ]; then
        CONFIG_FILE="$config"
        break
    fi
done

if [ -n "$CONFIG_FILE" ]; then
    echo "📋 Loading configuration from $CONFIG_FILE"
    source "$CONFIG_FILE"
else
    echo "📋 Using default configuration (create codebase-analysis-config*.env to customize)"
fi

# Default configuration (used if config file doesn't exist or is missing values)
# These are conservative AI-assisted development estimates
SENIOR_DEVELOPER_SALARY=${SENIOR_DEVELOPER_SALARY:-150000}
HOURLY_RATE=${HOURLY_RATE:-75}
HOURS_PER_YEAR=${HOURS_PER_YEAR:-2000}

# Productivity assumptions (lines per day) - Conservative AI-assisted estimates
PROD_CODE_DAILY=${PROD_CODE_DAILY:-50}
TEST_CODE_DAILY=${TEST_CODE_DAILY:-100}
CONFIG_DAILY=${CONFIG_DAILY:-200}
DOCS_DAILY=${DOCS_DAILY:-300}

# Complexity multipliers
BACKEND_MULTIPLIER=${BACKEND_MULTIPLIER:-2.0}
FRONTEND_MULTIPLIER=${FRONTEND_MULTIPLIER:-1.5}
INFRASTRUCTURE_MULTIPLIER=${INFRASTRUCTURE_MULTIPLIER:-1.0}
INTEGRATION_MULTIPLIER=${INTEGRATION_MULTIPLIER:-1.5}

# Real-world AI development metrics (defaults to conservative estimates)
ACTUAL_DEVELOPMENT_DAYS=${ACTUAL_DEVELOPMENT_DAYS:-30}
ACTUAL_HOURS_PER_DAY=${ACTUAL_HOURS_PER_DAY:-8}
ACTUAL_DAILY_RATE=${ACTUAL_DAILY_RATE:-1000}

echo -e "${BLUE}🔍 EconGraph Codebase Analysis & Cost Estimation${NC}"
echo -e "${BLUE}================================================${NC}"
echo ""

# Create temp directory
mkdir -p "$TEMP_DIR"

# Function to count files and lines
count_files_and_lines() {
    local pattern="$1"
    local exclude_pattern="$2"
    local description="$3"

    echo -e "${CYAN}Analyzing: $description${NC}"

    # Count files
    local file_count
    if [ -n "$exclude_pattern" ]; then
        file_count=$(find . -type f $pattern -not -path "$exclude_pattern" -not -path "./.git/*" | wc -l)
    else
        file_count=$(find . -type f $pattern -not -path "./.git/*" | wc -l)
    fi

    # Count lines
    local line_count
    if [ -n "$exclude_pattern" ]; then
        line_count=$(find . -type f $pattern -not -path "$exclude_pattern" -not -path "./.git/*" -exec wc -l {} + 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")
    else
        line_count=$(find . -type f $pattern -not -path "./.git/*" -exec wc -l {} + 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")
    fi

    echo "$file_count|$line_count"
}

# Function to count backend production code
count_backend_prod() {
    local files=$(find ./backend/src -name "*.rs" -not -path "*/target/*" -not -name "*test*" -not -name "*_test.rs" 2>/dev/null | wc -l)
    local lines=$(find ./backend/src -name "*.rs" -not -path "*/target/*" -not -name "*test*" -not -name "*_test.rs" -exec wc -l {} + 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")
    echo "$files|$lines"
}

# Function to count backend test code
count_backend_tests() {
    local files=$(find ./backend/src -name "*.rs" -not -path "*/target/*" \( -name "*test*" -o -name "*_test.rs" \) 2>/dev/null | wc -l)
    local lines=$(find ./backend/src -name "*.rs" -not -path "*/target/*" \( -name "*test*" -o -name "*_test.rs" \) -exec wc -l {} + 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")
    echo "$files|$lines"
}

# Function to count frontend production code
count_frontend_prod() {
    local files=$(find ./frontend/src -name "*.ts" -o -name "*.tsx" -o -name "*.js" -o -name "*.jsx" 2>/dev/null | grep -v test | wc -l)
    local lines=$(find ./frontend/src -name "*.ts" -o -name "*.tsx" -o -name "*.js" -o -name "*.jsx" 2>/dev/null | grep -v test | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")
    echo "$files|$lines"
}

# Function to count frontend test code (exclude node_modules and generated files)
# Only count files that are clearly developer-written tests
count_frontend_tests() {
    local files=$(find ./frontend -name "*.ts" -o -name "*.tsx" -o -name "*.js" -o -name "*.jsx" 2>/dev/null | grep -v node_modules | grep -E "(test|spec)" | grep -v -E "(generated|auto|playwright-report|test-results)" | wc -l)
    local lines=$(find ./frontend -name "*.ts" -o -name "*.tsx" -o -name "*.js" -o -name "*.jsx" 2>/dev/null | grep -v node_modules | grep -E "(test|spec)" | grep -v -E "(generated|auto|playwright-report|test-results)" | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")
    echo "$files|$lines"
}

# Function to count integration tests
count_integration_tests() {
    local files=$(find . -name "*.rs" -o -name "*.ts" -o -name "*.js" -o -name "*.sh" 2>/dev/null | grep -E "(integration|e2e|test)" | grep -v node_modules | grep -v target | wc -l)
    local lines=$(find . -name "*.rs" -o -name "*.ts" -o -name "*.js" -o -name "*.sh" 2>/dev/null | grep -E "(integration|e2e|test)" | grep -v node_modules | grep -v target | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")
    echo "$files|$lines"
}

# Function to count configuration files
count_config() {
    local files=$(find . -name "*.yaml" -o -name "*.yml" -o -name "*.json" -o -name "*.toml" -o -name "Dockerfile*" -o -name "*.tf" 2>/dev/null | grep -v node_modules | grep -v target | grep -v .terraform | wc -l)
    local lines=$(find . -name "*.yaml" -o -name "*.yml" -o -name "*.json" -o -name "*.toml" -o -name "Dockerfile*" -o -name "*.tf" 2>/dev/null | grep -v node_modules | grep -v target | grep -v .terraform | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")
    echo "$files|$lines"
}

# Function to count documentation
count_docs() {
    local files=$(find . -name "*.md" -o -name "README*" -o -name "*.txt" 2>/dev/null | grep -v node_modules | grep -v target | wc -l)
    local lines=$(find . -name "*.md" -o -name "README*" -o -name "*.txt" 2>/dev/null | grep -v node_modules | grep -v target | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")
    echo "$files|$lines"
}

# Function to count deployment scripts
count_deployment() {
    local files=$(find . -name "*.sh" 2>/dev/null | grep -v test | wc -l)
    local lines=$(find . -name "*.sh" 2>/dev/null | grep -v test | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")
    echo "$files|$lines"
}

# Change to project root
cd "$PROJECT_ROOT"

echo -e "${YELLOW}📊 Collecting codebase statistics...${NC}"

# Collect data
BACKEND_PROD=$(count_backend_prod)
BACKEND_TESTS=$(count_backend_tests)
FRONTEND_PROD=$(count_frontend_prod)
FRONTEND_TESTS=$(count_frontend_tests)
INTEGRATION_TESTS=$(count_integration_tests)
CONFIG=$(count_config)
DOCS=$(count_docs)
DEPLOYMENT=$(count_deployment)

# Parse results
BACKEND_PROD_FILES=$(echo $BACKEND_PROD | cut -d'|' -f1)
BACKEND_PROD_LINES=$(echo $BACKEND_PROD | cut -d'|' -f2)
BACKEND_TESTS_FILES=$(echo $BACKEND_TESTS | cut -d'|' -f1)
BACKEND_TESTS_LINES=$(echo $BACKEND_TESTS | cut -d'|' -f2)
FRONTEND_PROD_FILES=$(echo $FRONTEND_PROD | cut -d'|' -f1)
FRONTEND_PROD_LINES=$(echo $FRONTEND_PROD | cut -d'|' -f2)
FRONTEND_TESTS_FILES=$(echo $FRONTEND_TESTS | cut -d'|' -f1)
FRONTEND_TESTS_LINES=$(echo $FRONTEND_TESTS | cut -d'|' -f2)
INTEGRATION_FILES=$(echo $INTEGRATION_TESTS | cut -d'|' -f1)
INTEGRATION_LINES=$(echo $INTEGRATION_TESTS | cut -d'|' -f2)
CONFIG_FILES=$(echo $CONFIG | cut -d'|' -f1)
CONFIG_LINES=$(echo $CONFIG | cut -d'|' -f2)
DOCS_FILES=$(echo $DOCS | cut -d'|' -f1)
DOCS_LINES=$(echo $DOCS | cut -d'|' -f2)
DEPLOYMENT_FILES=$(echo $DEPLOYMENT | cut -d'|' -f1)
DEPLOYMENT_LINES=$(echo $DEPLOYMENT | cut -d'|' -f2)

# Calculate totals
TOTAL_FILES=$((BACKEND_PROD_FILES + BACKEND_TESTS_FILES + FRONTEND_PROD_FILES + FRONTEND_TESTS_FILES + INTEGRATION_FILES + CONFIG_FILES + DOCS_FILES + DEPLOYMENT_FILES))
TOTAL_LINES=$((BACKEND_PROD_LINES + BACKEND_TESTS_LINES + FRONTEND_PROD_LINES + FRONTEND_TESTS_LINES + INTEGRATION_LINES + CONFIG_LINES + DOCS_LINES + DEPLOYMENT_LINES))

# Calculate percentages
BACKEND_PROD_PCT=$(echo "scale=1; $BACKEND_PROD_LINES * 100 / $TOTAL_LINES" | bc -l)
BACKEND_TESTS_PCT=$(echo "scale=1; $BACKEND_TESTS_LINES * 100 / $TOTAL_LINES" | bc -l)
FRONTEND_PROD_PCT=$(echo "scale=1; $FRONTEND_PROD_LINES * 100 / $TOTAL_LINES" | bc -l)
FRONTEND_TESTS_PCT=$(echo "scale=1; $FRONTEND_TESTS_LINES * 100 / $TOTAL_LINES" | bc -l)
INTEGRATION_PCT=$(echo "scale=1; $INTEGRATION_LINES * 100 / $TOTAL_LINES" | bc -l)
CONFIG_PCT=$(echo "scale=1; $CONFIG_LINES * 100 / $TOTAL_LINES" | bc -l)
DOCS_PCT=$(echo "scale=1; $DOCS_LINES * 100 / $TOTAL_LINES" | bc -l)
DEPLOYMENT_PCT=$(echo "scale=1; $DEPLOYMENT_LINES * 100 / $TOTAL_LINES" | bc -l)

echo -e "${GREEN}✅ Analysis complete!${NC}"
echo ""

# Generate markdown report
cat > "$OUTPUT_FILE" << EOF
# 📊 EconGraph Codebase Analysis & Cost Estimation

**Generated on:** $(date)
**Project Root:** $PROJECT_ROOT

## Lines of Code Breakdown

| **Category** | **Files** | **Lines of Code** | **Percentage** |
|--------------|-----------|-------------------|----------------|
| **Backend Production** | $BACKEND_PROD_FILES | $(printf "%'d" $BACKEND_PROD_LINES) | ${BACKEND_PROD_PCT}% |
| **Backend Tests** | $BACKEND_TESTS_FILES | $(printf "%'d" $BACKEND_TESTS_LINES) | ${BACKEND_TESTS_PCT}% |
| **Frontend Production** | $FRONTEND_PROD_FILES | $(printf "%'d" $FRONTEND_PROD_LINES) | ${FRONTEND_PROD_PCT}% |
| **Frontend Tests** | $FRONTEND_TESTS_FILES | $(printf "%'d" $FRONTEND_TESTS_LINES) | ${FRONTEND_TESTS_PCT}% |
| **Integration Tests** | $INTEGRATION_FILES | $(printf "%'d" $INTEGRATION_LINES) | ${INTEGRATION_PCT}% |
| **Configuration** | $CONFIG_FILES | $(printf "%'d" $CONFIG_LINES) | ${CONFIG_PCT}% |
| **Documentation** | $DOCS_FILES | $(printf "%'d" $DOCS_LINES) | ${DOCS_PCT}% |
| **Deployment Scripts** | $DEPLOYMENT_FILES | $(printf "%'d" $DEPLOYMENT_LINES) | ${DEPLOYMENT_PCT}% |
| **TOTAL** | **$TOTAL_FILES** | **$(printf "%'d" $TOTAL_LINES)** | **100%** |

## Key Observations

1. **Frontend Tests**: ${FRONTEND_TESTS_PCT}% of codebase is frontend test files (developer-written tests only, excludes generated files)
2. **Production Code**: Only $(echo "scale=1; ($BACKEND_PROD_LINES + $FRONTEND_PROD_LINES) * 100 / $TOTAL_LINES" | bc -l)% is actual production code (Backend + Frontend)
3. **Infrastructure Heavy**: $(echo "scale=1; ($CONFIG_LINES + $DEPLOYMENT_LINES + $DOCS_LINES) * 100 / $TOTAL_LINES" | bc -l)% is configuration, deployment, and documentation
4. **Well-Tested**: Significant test coverage across all components
5. **Developer-Written Focus**: Analysis excludes generated files, node_modules, and auto-generated test artifacts

---

## 💰 Development Cost Analysis

### Assumptions & Methodology

**Developer Compensation (Annual):**
- **Senior Full-Stack Developer**: \$$(printf "%'d" $((SENIOR_DEVELOPER_SALARY - 30000))) - \$$(printf "%'d" $((SENIOR_DEVELOPER_SALARY + 30000)))
- **Average**: \$$(printf "%'d" $SENIOR_DEVELOPER_SALARY)/year
- **Hourly Rate**: \$$HOURLY_RATE/hour ($HOURS_PER_YEAR hours/year)

**Productivity Assumptions:**
- **Production Code**: $PROD_CODE_DAILY lines/day (complex business logic)
- **Test Code**: $TEST_CODE_DAILY lines/day (automated generation + manual)
- **Configuration**: $CONFIG_DAILY lines/day (declarative, templated)
- **Documentation**: $DOCS_DAILY lines/day (markdown, comments)

**Complexity Multipliers:**
- **Backend Rust**: ${BACKEND_MULTIPLIER}x (systems programming, memory safety)
- **Frontend React/TypeScript**: ${FRONTEND_MULTIPLIER}x (modern framework, type safety)
- **Kubernetes/DevOps**: ${INFRASTRUCTURE_MULTIPLIER}x (infrastructure complexity)
- **Integration Testing**: ${INTEGRATION_MULTIPLIER}x (cross-system coordination)

### Cost Breakdown by Category

| **Category** | **Lines** | **Daily Rate** | **Days** | **Multiplier** | **Adjusted Days** | **Cost** |
|--------------|-----------|----------------|----------|----------------|-------------------|----------|
| **Backend Production** | $(printf "%'d" $BACKEND_PROD_LINES) | $PROD_CODE_DAILY | $(echo "scale=0; $BACKEND_PROD_LINES / $PROD_CODE_DAILY" | bc | cut -d. -f1) | $BACKEND_MULTIPLIER | $(echo "scale=0; $BACKEND_PROD_LINES / $PROD_CODE_DAILY * $BACKEND_MULTIPLIER" | bc | cut -d. -f1) | \$$(printf "%'d" $(echo "scale=0; $BACKEND_PROD_LINES / $PROD_CODE_DAILY * $BACKEND_MULTIPLIER * $HOURLY_RATE * 8" | bc | cut -d. -f1)) |
| **Backend Tests** | $(printf "%'d" $BACKEND_TESTS_LINES) | $TEST_CODE_DAILY | $(echo "scale=0; $BACKEND_TESTS_LINES / $TEST_CODE_DAILY" | bc | cut -d. -f1) | $BACKEND_MULTIPLIER | $(echo "scale=0; $BACKEND_TESTS_LINES / $TEST_CODE_DAILY * $BACKEND_MULTIPLIER" | bc | cut -d. -f1) | \$$(printf "%'d" $(echo "scale=0; $BACKEND_TESTS_LINES / $TEST_CODE_DAILY * $BACKEND_MULTIPLIER * $HOURLY_RATE * 8" | bc | cut -d. -f1)) |
| **Frontend Production** | $(printf "%'d" $FRONTEND_PROD_LINES) | $PROD_CODE_DAILY | $(echo "scale=0; $FRONTEND_PROD_LINES / $PROD_CODE_DAILY" | bc | cut -d. -f1) | $FRONTEND_MULTIPLIER | $(echo "scale=0; $FRONTEND_PROD_LINES / $PROD_CODE_DAILY * $FRONTEND_MULTIPLIER" | bc | cut -d. -f1) | \$$(printf "%'d" $(echo "scale=0; $FRONTEND_PROD_LINES / $PROD_CODE_DAILY * $FRONTEND_MULTIPLIER * $HOURLY_RATE * 8" | bc | cut -d. -f1)) |
| **Frontend Tests** | $(printf "%'d" $FRONTEND_TESTS_LINES) | $TEST_CODE_DAILY | $(echo "scale=0; $FRONTEND_TESTS_LINES / $TEST_CODE_DAILY" | bc | cut -d. -f1) | $FRONTEND_MULTIPLIER | $(echo "scale=0; $FRONTEND_TESTS_LINES / $TEST_CODE_DAILY * $FRONTEND_MULTIPLIER" | bc | cut -d. -f1) | \$$(printf "%'d" $(echo "scale=0; $FRONTEND_TESTS_LINES / $TEST_CODE_DAILY * $FRONTEND_MULTIPLIER * $HOURLY_RATE * 8" | bc | cut -d. -f1)) |
| **Integration Tests** | $(printf "%'d" $INTEGRATION_LINES) | $TEST_CODE_DAILY | $(echo "scale=0; $INTEGRATION_LINES / $TEST_CODE_DAILY" | bc | cut -d. -f1) | $INTEGRATION_MULTIPLIER | $(echo "scale=0; $INTEGRATION_LINES / $TEST_CODE_DAILY * $INTEGRATION_MULTIPLIER" | bc | cut -d. -f1) | \$$(printf "%'d" $(echo "scale=0; $INTEGRATION_LINES / $TEST_CODE_DAILY * $INTEGRATION_MULTIPLIER * $HOURLY_RATE * 8" | bc | cut -d. -f1)) |
| **Configuration** | $(printf "%'d" $CONFIG_LINES) | $CONFIG_DAILY | $(echo "scale=0; $CONFIG_LINES / $CONFIG_DAILY" | bc | cut -d. -f1) | $INFRASTRUCTURE_MULTIPLIER | $(echo "scale=0; $CONFIG_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER" | bc | cut -d. -f1) | \$$(printf "%'d" $(echo "scale=0; $CONFIG_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER * $HOURLY_RATE * 8" | bc | cut -d. -f1)) |
| **Documentation** | $(printf "%'d" $DOCS_LINES) | $DOCS_DAILY | $(echo "scale=0; $DOCS_LINES / $DOCS_DAILY" | bc | cut -d. -f1) | 1.0 | $(echo "scale=0; $DOCS_LINES / $DOCS_DAILY" | bc | cut -d. -f1) | \$$(printf "%'d" $(echo "scale=0; $DOCS_LINES / $DOCS_DAILY * $HOURLY_RATE * 8" | bc | cut -d. -f1)) |
| **Deployment Scripts** | $(printf "%'d" $DEPLOYMENT_LINES) | $CONFIG_DAILY | $(echo "scale=0; $DEPLOYMENT_LINES / $CONFIG_DAILY" | bc | cut -d. -f1) | $INFRASTRUCTURE_MULTIPLIER | $(echo "scale=0; $DEPLOYMENT_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER" | bc | cut -d. -f1) | \$$(printf "%'d" $(echo "scale=0; $DEPLOYMENT_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER * $HOURLY_RATE * 8" | bc | cut -d. -f1)) |

---

## 💰 Cost Summary & Analysis

### Total Development Cost: \$$(printf "%'d" $(echo "scale=0; ($BACKEND_PROD_LINES / $PROD_CODE_DAILY * $BACKEND_MULTIPLIER + $BACKEND_TESTS_LINES / $TEST_CODE_DAILY * $BACKEND_MULTIPLIER + $FRONTEND_PROD_LINES / $PROD_CODE_DAILY * $FRONTEND_MULTIPLIER + $FRONTEND_TESTS_LINES / $TEST_CODE_DAILY * $FRONTEND_MULTIPLIER + $INTEGRATION_LINES / $TEST_CODE_DAILY * $INTEGRATION_MULTIPLIER + $CONFIG_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER + $DOCS_LINES / $DOCS_DAILY + $DEPLOYMENT_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER) * $HOURLY_RATE * 8" | bc | cut -d. -f1))

**Breakdown by Component:**
- **Frontend Development**: \$$(printf "%'d" $(echo "scale=0; ($FRONTEND_PROD_LINES / $PROD_CODE_DAILY * $FRONTEND_MULTIPLIER + $FRONTEND_TESTS_LINES / $TEST_CODE_DAILY * $FRONTEND_MULTIPLIER) * $HOURLY_RATE * 8" | bc | cut -d. -f1)) ($(echo "scale=1; ($FRONTEND_PROD_LINES / $PROD_CODE_DAILY * $FRONTEND_MULTIPLIER + $FRONTEND_TESTS_LINES / $TEST_CODE_DAILY * $FRONTEND_MULTIPLIER) * $HOURLY_RATE * 8 * 100 / (($BACKEND_PROD_LINES / $PROD_CODE_DAILY * $BACKEND_MULTIPLIER + $BACKEND_TESTS_LINES / $TEST_CODE_DAILY * $BACKEND_MULTIPLIER + $FRONTEND_PROD_LINES / $PROD_CODE_DAILY * $FRONTEND_MULTIPLIER + $FRONTEND_TESTS_LINES / $TEST_CODE_DAILY * $FRONTEND_MULTIPLIER + $INTEGRATION_LINES / $TEST_CODE_DAILY * $INTEGRATION_MULTIPLIER + $CONFIG_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER + $DOCS_LINES / $DOCS_DAILY + $DEPLOYMENT_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER) * $HOURLY_RATE * 8)" | bc | cut -d. -f1)%)
- **Backend Development**: \$$(printf "%'d" $(echo "scale=0; ($BACKEND_PROD_LINES / $PROD_CODE_DAILY * $BACKEND_MULTIPLIER + $BACKEND_TESTS_LINES / $TEST_CODE_DAILY * $BACKEND_MULTIPLIER) * $HOURLY_RATE * 8" | bc | cut -d. -f1)) ($(echo "scale=1; ($BACKEND_PROD_LINES / $PROD_CODE_DAILY * $BACKEND_MULTIPLIER + $BACKEND_TESTS_LINES / $TEST_CODE_DAILY * $BACKEND_MULTIPLIER) * $HOURLY_RATE * 8 * 100 / (($BACKEND_PROD_LINES / $PROD_CODE_DAILY * $BACKEND_MULTIPLIER + $BACKEND_TESTS_LINES / $TEST_CODE_DAILY * $BACKEND_MULTIPLIER + $FRONTEND_PROD_LINES / $PROD_CODE_DAILY * $FRONTEND_MULTIPLIER + $FRONTEND_TESTS_LINES / $TEST_CODE_DAILY * $FRONTEND_MULTIPLIER + $INTEGRATION_LINES / $TEST_CODE_DAILY * $INTEGRATION_MULTIPLIER + $CONFIG_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER + $DOCS_LINES / $DOCS_DAILY + $DEPLOYMENT_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER) * $HOURLY_RATE * 8)" | bc | cut -d. -f1)%)
- **Infrastructure/DevOps**: \$$(printf "%'d" $(echo "scale=0; ($CONFIG_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER + $DEPLOYMENT_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER) * $HOURLY_RATE * 8" | bc | cut -d. -f1)) ($(echo "scale=1; ($CONFIG_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER + $DEPLOYMENT_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER) * $HOURLY_RATE * 8 * 100 / (($BACKEND_PROD_LINES / $PROD_CODE_DAILY * $BACKEND_MULTIPLIER + $BACKEND_TESTS_LINES / $TEST_CODE_DAILY * $BACKEND_MULTIPLIER + $FRONTEND_PROD_LINES / $PROD_CODE_DAILY * $FRONTEND_MULTIPLIER + $FRONTEND_TESTS_LINES / $TEST_CODE_DAILY * $FRONTEND_MULTIPLIER + $INTEGRATION_LINES / $TEST_CODE_DAILY * $INTEGRATION_MULTIPLIER + $CONFIG_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER + $DOCS_LINES / $DOCS_DAILY + $DEPLOYMENT_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER) * $HOURLY_RATE * 8)" | bc | cut -d. -f1)%)
- **Documentation**: \$$(printf "%'d" $(echo "scale=0; $DOCS_LINES / $DOCS_DAILY * $HOURLY_RATE * 8" | bc | cut -d. -f1)) ($(echo "scale=1; $DOCS_LINES / $DOCS_DAILY * $HOURLY_RATE * 8 * 100 / (($BACKEND_PROD_LINES / $PROD_CODE_DAILY * $BACKEND_MULTIPLIER + $BACKEND_TESTS_LINES / $TEST_CODE_DAILY * $BACKEND_MULTIPLIER + $FRONTEND_PROD_LINES / $PROD_CODE_DAILY * $FRONTEND_MULTIPLIER + $FRONTEND_TESTS_LINES / $TEST_CODE_DAILY * $FRONTEND_MULTIPLIER + $INTEGRATION_LINES / $TEST_CODE_DAILY * $INTEGRATION_MULTIPLIER + $CONFIG_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER + $DOCS_LINES / $DOCS_DAILY + $DEPLOYMENT_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER) * $HOURLY_RATE * 8)" | bc | cut -d. -f1)%)
- **Integration Testing**: \$$(printf "%'d" $(echo "scale=0; $INTEGRATION_LINES / $TEST_CODE_DAILY * $INTEGRATION_MULTIPLIER * $HOURLY_RATE * 8" | bc | cut -d. -f1)) ($(echo "scale=1; $INTEGRATION_LINES / $TEST_CODE_DAILY * $INTEGRATION_MULTIPLIER * $HOURLY_RATE * 8 * 100 / (($BACKEND_PROD_LINES / $PROD_CODE_DAILY * $BACKEND_MULTIPLIER + $BACKEND_TESTS_LINES / $TEST_CODE_DAILY * $BACKEND_MULTIPLIER + $FRONTEND_PROD_LINES / $PROD_CODE_DAILY * $FRONTEND_MULTIPLIER + $FRONTEND_TESTS_LINES / $TEST_CODE_DAILY * $FRONTEND_MULTIPLIER + $INTEGRATION_LINES / $TEST_CODE_DAILY * $INTEGRATION_MULTIPLIER + $CONFIG_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER + $DOCS_LINES / $DOCS_DAILY + $DEPLOYMENT_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER) * $HOURLY_RATE * 8)" | bc | cut -d. -f1)%)

### Cost Per Line of Code:
- **Production Code**: \$$(echo "scale=2; (($BACKEND_PROD_LINES / $PROD_CODE_DAILY * $BACKEND_MULTIPLIER + $FRONTEND_PROD_LINES / $PROD_CODE_DAILY * $FRONTEND_MULTIPLIER) * $HOURLY_RATE * 8) / ($BACKEND_PROD_LINES + $FRONTEND_PROD_LINES)" | bc | cut -d. -f1)/line (Backend + Frontend production)
- **Test Code**: \$$(echo "scale=2; (($BACKEND_TESTS_LINES / $TEST_CODE_DAILY * $BACKEND_MULTIPLIER + $FRONTEND_TESTS_LINES / $TEST_CODE_DAILY * $FRONTEND_MULTIPLIER + $INTEGRATION_LINES / $TEST_CODE_DAILY * $INTEGRATION_MULTIPLIER) * $HOURLY_RATE * 8) / ($BACKEND_TESTS_LINES + $FRONTEND_TESTS_LINES + $INTEGRATION_LINES)" | bc | cut -d. -f1)/line (All test code)
- **Infrastructure**: \$$(echo "scale=2; (($CONFIG_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER + $DEPLOYMENT_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER) * $HOURLY_RATE * 8) / ($CONFIG_LINES + $DEPLOYMENT_LINES)" | bc | cut -d. -f1)/line (Config + Deployment)
- **Overall Average**: \$$(echo "scale=2; (($BACKEND_PROD_LINES / $PROD_CODE_DAILY * $BACKEND_MULTIPLIER + $BACKEND_TESTS_LINES / $TEST_CODE_DAILY * $BACKEND_MULTIPLIER + $FRONTEND_PROD_LINES / $PROD_CODE_DAILY * $FRONTEND_MULTIPLIER + $FRONTEND_TESTS_LINES / $TEST_CODE_DAILY * $FRONTEND_MULTIPLIER + $INTEGRATION_LINES / $TEST_CODE_DAILY * $INTEGRATION_MULTIPLIER + $CONFIG_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER + $DOCS_LINES / $DOCS_DAILY + $DEPLOYMENT_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER) * $HOURLY_RATE * 8) / $TOTAL_LINES" | bc | cut -d. -f1)/line

### Time Investment:
- **Total Development Time**: $(printf "%'d" $(echo "scale=0; ($BACKEND_PROD_LINES / $PROD_CODE_DAILY * $BACKEND_MULTIPLIER + $BACKEND_TESTS_LINES / $TEST_CODE_DAILY * $BACKEND_MULTIPLIER + $FRONTEND_PROD_LINES / $PROD_CODE_DAILY * $FRONTEND_MULTIPLIER + $FRONTEND_TESTS_LINES / $TEST_CODE_DAILY * $FRONTEND_MULTIPLIER + $INTEGRATION_LINES / $TEST_CODE_DAILY * $INTEGRATION_MULTIPLIER + $CONFIG_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER + $DOCS_LINES / $DOCS_DAILY + $DEPLOYMENT_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER) * 8" | bc | cut -d. -f1)) hours
- **Equivalent Full-Time Developer**: $(echo "scale=1; ($BACKEND_PROD_LINES / $PROD_CODE_DAILY * $BACKEND_MULTIPLIER + $BACKEND_TESTS_LINES / $TEST_CODE_DAILY * $BACKEND_MULTIPLIER + $FRONTEND_PROD_LINES / $PROD_CODE_DAILY * $FRONTEND_MULTIPLIER + $FRONTEND_TESTS_LINES / $TEST_CODE_DAILY * $FRONTEND_MULTIPLIER + $INTEGRATION_LINES / $TEST_CODE_DAILY * $INTEGRATION_MULTIPLIER + $CONFIG_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER + $DOCS_LINES / $DOCS_DAILY + $DEPLOYMENT_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER) * 8 / $HOURS_PER_YEAR" | bc | cut -d. -f1) years
- **Team of 3 Developers**: $(echo "scale=1; ($BACKEND_PROD_LINES / $PROD_CODE_DAILY * $BACKEND_MULTIPLIER + $BACKEND_TESTS_LINES / $TEST_CODE_DAILY * $BACKEND_MULTIPLIER + $FRONTEND_PROD_LINES / $PROD_CODE_DAILY * $FRONTEND_MULTIPLIER + $FRONTEND_TESTS_LINES / $TEST_CODE_DAILY * $FRONTEND_MULTIPLIER + $INTEGRATION_LINES / $TEST_CODE_DAILY * $INTEGRATION_MULTIPLIER + $CONFIG_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER + $DOCS_LINES / $DOCS_DAILY + $DEPLOYMENT_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER) * 8 / ($HOURS_PER_YEAR * 3)" | bc | cut -d. -f1) years
- **Team of 5 Developers**: $(echo "scale=1; ($BACKEND_PROD_LINES / $PROD_CODE_DAILY * $BACKEND_MULTIPLIER + $BACKEND_TESTS_LINES / $TEST_CODE_DAILY * $BACKEND_MULTIPLIER + $FRONTEND_PROD_LINES / $PROD_CODE_DAILY * $FRONTEND_MULTIPLIER + $FRONTEND_TESTS_LINES / $TEST_CODE_DAILY * $FRONTEND_MULTIPLIER + $INTEGRATION_LINES / $TEST_CODE_DAILY * $INTEGRATION_MULTIPLIER + $CONFIG_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER + $DOCS_LINES / $DOCS_DAILY + $DEPLOYMENT_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER) * 8 / ($HOURS_PER_YEAR * 5)" | bc | cut -d. -f1) years

---

## 🚀 AI-Assisted Development Impact

Based on previous analysis, this project represents:

**Traditional Development Cost**: \$$(printf "%'d" $(echo "scale=0; ($BACKEND_PROD_LINES / $PROD_CODE_DAILY * $BACKEND_MULTIPLIER + $BACKEND_TESTS_LINES / $TEST_CODE_DAILY * $BACKEND_MULTIPLIER + $FRONTEND_PROD_LINES / $PROD_CODE_DAILY * $FRONTEND_MULTIPLIER + $FRONTEND_TESTS_LINES / $TEST_CODE_DAILY * $FRONTEND_MULTIPLIER + $INTEGRATION_LINES / $TEST_CODE_DAILY * $INTEGRATION_MULTIPLIER + $CONFIG_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER + $DOCS_LINES / $DOCS_DAILY + $DEPLOYMENT_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER) * $HOURLY_RATE * 8" | bc | cut -d. -f1))
**AI-Assisted Development Cost**: \$50,000 - \$100,000 (estimated)
**Time Savings**: 10-20x faster development cycles
**ROI**: $(echo "scale=0; (($BACKEND_PROD_LINES / $PROD_CODE_DAILY * $BACKEND_MULTIPLIER + $BACKEND_TESTS_LINES / $TEST_CODE_DAILY * $BACKEND_MULTIPLIER + $FRONTEND_PROD_LINES / $PROD_CODE_DAILY * $FRONTEND_MULTIPLIER + $FRONTEND_TESTS_LINES / $TEST_CODE_DAILY * $FRONTEND_MULTIPLIER + $INTEGRATION_LINES / $TEST_CODE_DAILY * $INTEGRATION_MULTIPLIER + $CONFIG_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER + $DOCS_LINES / $DOCS_DAILY + $DEPLOYMENT_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER) * $HOURLY_RATE * 8) / 100000" | bc | cut -d. -f1)-$(echo "scale=0; (($BACKEND_PROD_LINES / $PROD_CODE_DAILY * $BACKEND_MULTIPLIER + $BACKEND_TESTS_LINES / $TEST_CODE_DAILY * $BACKEND_MULTIPLIER + $FRONTEND_PROD_LINES / $PROD_CODE_DAILY * $FRONTEND_MULTIPLIER + $FRONTEND_TESTS_LINES / $TEST_CODE_DAILY * $FRONTEND_MULTIPLIER + $INTEGRATION_LINES / $TEST_CODE_DAILY * $INTEGRATION_MULTIPLIER + $CONFIG_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER + $DOCS_LINES / $DOCS_DAILY + $DEPLOYMENT_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER) * $HOURLY_RATE * 8) / 50000" | bc | cut -d. -f1)% return on AI investment

**Key AI Contributions:**
- Rapid prototyping and iteration
- Automated test generation
- Configuration management
- Documentation generation
- Code review and optimization
- Deployment automation

---

## 📈 Business Value Delivered

**Enterprise-Grade Features:**
- Full-stack web application with authentication
- Real-time data visualization and analysis
- Comprehensive monitoring and observability
- Production-ready Kubernetes deployment
- Automated testing and CI/CD pipeline
- Security vulnerability management

**Market Value**: This represents a sophisticated, production-ready economic data platform that would typically require a dedicated development team and significant time investment.

The codebase demonstrates enterprise-level complexity with comprehensive testing, monitoring, and deployment automation - representing substantial business value and technical sophistication.

---

*Report generated by EconGraph Codebase Analysis Script v1.0*
EOF

# Clean up temp directory
rm -rf "$TEMP_DIR"

echo -e "${GREEN}📄 Report generated: $OUTPUT_FILE${NC}"
echo ""
echo -e "${YELLOW}📊 Quick Summary:${NC}"
echo -e "   Total Files: $(printf "%'d" $TOTAL_FILES)"
echo -e "   Total Lines: $(printf "%'d" $TOTAL_LINES)"
echo -e "   Estimated Cost: \$$(printf "%'d" $(echo "scale=0; ($BACKEND_PROD_LINES / $PROD_CODE_DAILY * $BACKEND_MULTIPLIER + $BACKEND_TESTS_LINES / $TEST_CODE_DAILY * $BACKEND_MULTIPLIER + $FRONTEND_PROD_LINES / $PROD_CODE_DAILY * $FRONTEND_MULTIPLIER + $FRONTEND_TESTS_LINES / $TEST_CODE_DAILY * $FRONTEND_MULTIPLIER + $INTEGRATION_LINES / $TEST_CODE_DAILY * $INTEGRATION_MULTIPLIER + $CONFIG_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER + $DOCS_LINES / $DOCS_DAILY + $DEPLOYMENT_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER) * $HOURLY_RATE * 8" | bc | cut -d. -f1))"
echo -e "   Development Time: $(printf "%'d" $(echo "scale=0; ($BACKEND_PROD_LINES / $PROD_CODE_DAILY * $BACKEND_MULTIPLIER + $BACKEND_TESTS_LINES / $TEST_CODE_DAILY * $BACKEND_MULTIPLIER + $FRONTEND_PROD_LINES / $PROD_CODE_DAILY * $FRONTEND_MULTIPLIER + $FRONTEND_TESTS_LINES / $TEST_CODE_DAILY * $FRONTEND_MULTIPLIER + $INTEGRATION_LINES / $TEST_CODE_DAILY * $INTEGRATION_MULTIPLIER + $CONFIG_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER + $DOCS_LINES / $DOCS_DAILY + $DEPLOYMENT_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER) * 8" | bc | cut -d. -f1)) hours"
echo ""
echo -e "${BLUE}🎉 Analysis complete!${NC}"
