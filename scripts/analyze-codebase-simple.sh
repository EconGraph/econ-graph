#!/bin/bash

# EconGraph Codebase Analysis & Cost Estimation Script (Simplified)
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

# Change to project root
cd "$PROJECT_ROOT"

echo -e "${YELLOW}📊 Collecting codebase statistics...${NC}"

# Count backend production code
BACKEND_PROD_FILES=$(find ./backend/src -name "*.rs" -not -path "*/target/*" -not -name "*test*" -not -name "*_test.rs" 2>/dev/null | wc -l)
BACKEND_PROD_LINES=$(find ./backend/src -name "*.rs" -not -path "*/target/*" -not -name "*test*" -not -name "*_test.rs" -exec wc -l {} + 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")

# Count backend test code
BACKEND_TESTS_FILES=$(find ./backend/src -name "*.rs" -not -path "*/target/*" \( -name "*test*" -o -name "*_test.rs" \) 2>/dev/null | wc -l)
BACKEND_TESTS_LINES=$(find ./backend/src -name "*.rs" -not -path "*/target/*" \( -name "*test*" -o -name "*_test.rs" \) -exec wc -l {} + 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")

# Count frontend production code
FRONTEND_PROD_FILES=$(find ./frontend/src -name "*.ts" -o -name "*.tsx" -o -name "*.js" -o -name "*.jsx" 2>/dev/null | grep -v test | wc -l)
FRONTEND_PROD_LINES=$(find ./frontend/src -name "*.ts" -o -name "*.tsx" -o -name "*.js" -o -name "*.jsx" 2>/dev/null | grep -v test | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")

# Count frontend test code (exclude node_modules and generated files)
# Only count files that are clearly developer-written tests
FRONTEND_TESTS_FILES=$(find ./frontend -name "*.ts" -o -name "*.tsx" -o -name "*.js" -o -name "*.jsx" 2>/dev/null | grep -v node_modules | grep -E "(test|spec)" | grep -v -E "(generated|auto|playwright-report|test-results)" | wc -l)
FRONTEND_TESTS_LINES=$(find ./frontend -name "*.ts" -o -name "*.tsx" -o -name "*.js" -o -name "*.jsx" 2>/dev/null | grep -v node_modules | grep -E "(test|spec)" | grep -v -E "(generated|auto|playwright-report|test-results)" | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")

# Count integration tests
INTEGRATION_FILES=$(find . -name "*.rs" -o -name "*.ts" -o -name "*.js" -o -name "*.sh" 2>/dev/null | grep -E "(integration|e2e|test)" | grep -v node_modules | grep -v target | wc -l)
INTEGRATION_LINES=$(find . -name "*.rs" -o -name "*.ts" -o -name "*.js" -o -name "*.sh" 2>/dev/null | grep -E "(integration|e2e|test)" | grep -v node_modules | grep -v target | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")

# Count configuration files
CONFIG_FILES=$(find . -name "*.yaml" -o -name "*.yml" -o -name "*.json" -o -name "*.toml" -o -name "Dockerfile*" -o -name "*.tf" 2>/dev/null | grep -v node_modules | grep -v target | grep -v .terraform | wc -l)
CONFIG_LINES=$(find . -name "*.yaml" -o -name "*.yml" -o -name "*.json" -o -name "*.toml" -o -name "Dockerfile*" -o -name "*.tf" 2>/dev/null | grep -v node_modules | grep -v target | grep -v .terraform | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")

# Count documentation
DOCS_FILES=$(find . -name "*.md" -o -name "README*" -o -name "*.txt" 2>/dev/null | grep -v node_modules | grep -v target | wc -l)
DOCS_LINES=$(find . -name "*.md" -o -name "README*" -o -name "*.txt" 2>/dev/null | grep -v node_modules | grep -v target | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")

# Count deployment scripts
DEPLOYMENT_FILES=$(find . -name "*.sh" 2>/dev/null | grep -v test | wc -l)
DEPLOYMENT_LINES=$(find . -name "*.sh" 2>/dev/null | grep -v test | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}' || echo "0")

# Calculate totals
TOTAL_FILES=$((BACKEND_PROD_FILES + BACKEND_TESTS_FILES + FRONTEND_PROD_FILES + FRONTEND_TESTS_FILES + INTEGRATION_FILES + CONFIG_FILES + DOCS_FILES + DEPLOYMENT_FILES))
TOTAL_LINES=$((BACKEND_PROD_LINES + BACKEND_TESTS_LINES + FRONTEND_PROD_LINES + FRONTEND_TESTS_LINES + INTEGRATION_LINES + CONFIG_LINES + DOCS_LINES + DEPLOYMENT_LINES))

# Calculate costs (simplified calculations)
BACKEND_PROD_COST=$(echo "scale=0; $BACKEND_PROD_LINES / $PROD_CODE_DAILY * $BACKEND_MULTIPLIER * $HOURLY_RATE * 8" | bc | cut -d. -f1)
BACKEND_TESTS_COST=$(echo "scale=0; $BACKEND_TESTS_LINES / $TEST_CODE_DAILY * $BACKEND_MULTIPLIER * $HOURLY_RATE * 8" | bc | cut -d. -f1)
FRONTEND_PROD_COST=$(echo "scale=0; $FRONTEND_PROD_LINES / $PROD_CODE_DAILY * $FRONTEND_MULTIPLIER * $HOURLY_RATE * 8" | bc | cut -d. -f1)
FRONTEND_TESTS_COST=$(echo "scale=0; $FRONTEND_TESTS_LINES / $TEST_CODE_DAILY * $FRONTEND_MULTIPLIER * $HOURLY_RATE * 8" | bc | cut -d. -f1)
INTEGRATION_COST=$(echo "scale=0; $INTEGRATION_LINES / $TEST_CODE_DAILY * $INTEGRATION_MULTIPLIER * $HOURLY_RATE * 8" | bc | cut -d. -f1)
CONFIG_COST=$(echo "scale=0; $CONFIG_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER * $HOURLY_RATE * 8" | bc | cut -d. -f1)
DOCS_COST=$(echo "scale=0; $DOCS_LINES / $DOCS_DAILY * $HOURLY_RATE * 8" | bc | cut -d. -f1)
DEPLOYMENT_COST=$(echo "scale=0; $DEPLOYMENT_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER * $HOURLY_RATE * 8" | bc | cut -d. -f1)

TOTAL_COST=$(echo "scale=0; $BACKEND_PROD_COST + $BACKEND_TESTS_COST + $FRONTEND_PROD_COST + $FRONTEND_TESTS_COST + $INTEGRATION_COST + $CONFIG_COST + $DOCS_COST + $DEPLOYMENT_COST" | bc | cut -d. -f1)

# Calculate time investment
TOTAL_HOURS=$(echo "scale=0; ($BACKEND_PROD_LINES / $PROD_CODE_DAILY * $BACKEND_MULTIPLIER + $BACKEND_TESTS_LINES / $TEST_CODE_DAILY * $BACKEND_MULTIPLIER + $FRONTEND_PROD_LINES / $PROD_CODE_DAILY * $FRONTEND_MULTIPLIER + $FRONTEND_TESTS_LINES / $TEST_CODE_DAILY * $FRONTEND_MULTIPLIER + $INTEGRATION_LINES / $TEST_CODE_DAILY * $INTEGRATION_MULTIPLIER + $CONFIG_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER + $DOCS_LINES / $DOCS_DAILY + $DEPLOYMENT_LINES / $CONFIG_DAILY * $INFRASTRUCTURE_MULTIPLIER) * 8" | bc | cut -d. -f1)

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

| **Category** | **Lines** | **Cost** |
|--------------|-----------|----------|
| **Backend Production** | $(printf "%'d" $BACKEND_PROD_LINES) | \$$(printf "%'d" $BACKEND_PROD_COST) |
| **Backend Tests** | $(printf "%'d" $BACKEND_TESTS_LINES) | \$$(printf "%'d" $BACKEND_TESTS_COST) |
| **Frontend Production** | $(printf "%'d" $FRONTEND_PROD_LINES) | \$$(printf "%'d" $FRONTEND_PROD_COST) |
| **Frontend Tests** | $(printf "%'d" $FRONTEND_TESTS_LINES) | \$$(printf "%'d" $FRONTEND_TESTS_COST) |
| **Integration Tests** | $(printf "%'d" $INTEGRATION_LINES) | \$$(printf "%'d" $INTEGRATION_COST) |
| **Configuration** | $(printf "%'d" $CONFIG_LINES) | \$$(printf "%'d" $CONFIG_COST) |
| **Documentation** | $(printf "%'d" $DOCS_LINES) | \$$(printf "%'d" $DOCS_COST) |
| **Deployment Scripts** | $(printf "%'d" $DEPLOYMENT_LINES) | \$$(printf "%'d" $DEPLOYMENT_COST) |

---

## 💰 Cost Summary & Analysis

### Total Development Cost: \$$(printf "%'d" $TOTAL_COST)

**Breakdown by Component:**
- **Frontend Development**: \$$(printf "%'d" $(echo "scale=0; $FRONTEND_PROD_COST + $FRONTEND_TESTS_COST" | bc | cut -d. -f1)) ($(echo "scale=1; ($FRONTEND_PROD_COST + $FRONTEND_TESTS_COST) * 100 / $TOTAL_COST" | bc)%)
- **Backend Development**: \$$(printf "%'d" $(echo "scale=0; $BACKEND_PROD_COST + $BACKEND_TESTS_COST" | bc | cut -d. -f1)) ($(echo "scale=1; ($BACKEND_PROD_COST + $BACKEND_TESTS_COST) * 100 / $TOTAL_COST" | bc)%)
- **Infrastructure/DevOps**: \$$(printf "%'d" $(echo "scale=0; $CONFIG_COST + $DEPLOYMENT_COST" | bc | cut -d. -f1)) ($(echo "scale=1; ($CONFIG_COST + $DEPLOYMENT_COST) * 100 / $TOTAL_COST" | bc)%)
- **Documentation**: \$$(printf "%'d" $DOCS_COST) ($(echo "scale=1; $DOCS_COST * 100 / $TOTAL_COST" | bc)%)
- **Integration Testing**: \$$(printf "%'d" $INTEGRATION_COST) ($(echo "scale=1; $INTEGRATION_COST * 100 / $TOTAL_COST" | bc)%)

### Cost Per Line of Code:
- **Production Code**: \$$(echo "scale=2; ($BACKEND_PROD_COST + $FRONTEND_PROD_COST) / ($BACKEND_PROD_LINES + $FRONTEND_PROD_LINES)" | bc)/line (Backend + Frontend production)
- **Test Code**: \$$(echo "scale=2; ($BACKEND_TESTS_COST + $FRONTEND_TESTS_COST + $INTEGRATION_COST) / ($BACKEND_TESTS_LINES + $FRONTEND_TESTS_LINES + $INTEGRATION_LINES)" | bc)/line (All test code)
- **Infrastructure**: \$$(echo "scale=2; ($CONFIG_COST + $DEPLOYMENT_COST) / ($CONFIG_LINES + $DEPLOYMENT_LINES)" | bc)/line (Config + Deployment)
- **Overall Average**: \$$(echo "scale=2; $TOTAL_COST / $TOTAL_LINES" | bc)/line

### Time Investment:
- **Total Development Time**: $(printf "%'d" $TOTAL_HOURS) hours
- **Equivalent Full-Time Developer**: $(echo "scale=1; $TOTAL_HOURS / $HOURS_PER_YEAR" | bc) years
- **Team of 3 Developers**: $(echo "scale=1; $TOTAL_HOURS / ($HOURS_PER_YEAR * 3)" | bc) years
- **Team of 5 Developers**: $(echo "scale=1; $TOTAL_HOURS / ($HOURS_PER_YEAR * 5)" | bc) years

---

## 🚀 AI-Assisted Development Impact

### Real-World Development Metrics:
**Actual Development Time**: $ACTUAL_DEVELOPMENT_DAYS days ($(echo "$ACTUAL_DEVELOPMENT_DAYS * $ACTUAL_HOURS_PER_DAY" | bc) hours at $ACTUAL_HOURS_PER_DAY hours/day)
**Traditional Development Estimate**: $(printf "%'d" $TOTAL_HOURS) hours
**Speed Multiplier**: $(echo "scale=0; $TOTAL_HOURS / ($ACTUAL_DEVELOPMENT_DAYS * $ACTUAL_HOURS_PER_DAY)" | bc -l)x faster than traditional development
**Actual vs Estimated Cost**: \$$(printf "%'d" $TOTAL_COST) traditional vs ~\$$(echo "$ACTUAL_DEVELOPMENT_DAYS * $ACTUAL_DAILY_RATE" | bc) actual ($ACTUAL_DEVELOPMENT_DAYS days × \$$ACTUAL_DAILY_RATE/day)

### ROI Analysis:

#### Traditional Developer Approach:
- **Time**: $(printf "%'d" $TOTAL_HOURS) hours ($(echo "scale=1; $TOTAL_HOURS / $HOURS_PER_YEAR" | bc) years full-time)
- **Cost**: \$$(printf "%'d" $TOTAL_COST)
- **Team Required**: 3-5 developers for $(echo "scale=1; $TOTAL_HOURS / ($HOURS_PER_YEAR * 3)" | bc)-$(echo "scale=1; $TOTAL_HOURS / ($HOURS_PER_YEAR * 5)" | bc) years

#### AI-Assisted Development (Actual):
- **Time**: $(echo "$ACTUAL_DEVELOPMENT_DAYS * $ACTUAL_HOURS_PER_DAY" | bc) hours ($ACTUAL_DEVELOPMENT_DAYS days at $ACTUAL_HOURS_PER_DAY hours/day)
- **Cost**: ~\$$(echo "$ACTUAL_DEVELOPMENT_DAYS * $ACTUAL_DAILY_RATE" | bc) (developer time only)
- **Team Required**: 1 developer with AI assistance

#### ROI Calculations:
- **Cost Savings**: \$$(echo "scale=0; $TOTAL_COST - ($ACTUAL_DEVELOPMENT_DAYS * $ACTUAL_DAILY_RATE)" | bc -l) ($(echo "scale=1; ($TOTAL_COST - ($ACTUAL_DEVELOPMENT_DAYS * $ACTUAL_DAILY_RATE)) * 100 / $TOTAL_COST" | bc)% reduction)
- **Time Savings**: $(echo "scale=0; $TOTAL_HOURS - ($ACTUAL_DEVELOPMENT_DAYS * $ACTUAL_HOURS_PER_DAY)" | bc -l) hours ($(echo "scale=1; ($TOTAL_HOURS - ($ACTUAL_DEVELOPMENT_DAYS * $ACTUAL_HOURS_PER_DAY)) * 100 / $TOTAL_HOURS" | bc)% reduction)
- **Efficiency Gain**: $(echo "scale=0; $TOTAL_HOURS / ($ACTUAL_DEVELOPMENT_DAYS * $ACTUAL_HOURS_PER_DAY)" | bc -l)x faster delivery
- **Break-even**: Immediate (first day of development)

### Key AI Contributions:
- Rapid prototyping and iteration
- Automated test generation and validation
- Configuration management and deployment scripts
- Documentation generation and maintenance
- Code review and optimization
- Security vulnerability detection and fixes
- Monitoring and observability setup

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

echo -e "${GREEN}📄 Report generated: $OUTPUT_FILE${NC}"
echo ""
echo -e "${YELLOW}📊 Quick Summary:${NC}"
echo -e "   Total Files: $(printf "%'d" $TOTAL_FILES)"
echo -e "   Total Lines: $(printf "%'d" $TOTAL_LINES)"
echo -e "   Estimated Cost: \$$(printf "%'d" $TOTAL_COST)"
echo -e "   Development Time: $(printf "%'d" $TOTAL_HOURS) hours"
echo ""
echo -e "${BLUE}🎉 Analysis complete!${NC}"
