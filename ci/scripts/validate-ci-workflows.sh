#!/bin/bash

# CI/CD Workflow Validation Script
# Validates GitHub Actions CI/CD workflow files for common issues
# Based on RelEng persona requirements for workflow hygiene

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
WORKFLOW_DIR=".github/workflows"
VALIDATION_ERRORS=0

echo "🔍 Validating GitHub Actions CI/CD workflows..."

# Function to report errors
report_error() {
    echo -e "${RED}❌ $1${NC}"
    VALIDATION_ERRORS=$((VALIDATION_ERRORS + 1))
}

# Function to report success
report_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

# Function to report warning
report_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

# Check if workflow directory exists
if [[ ! -d "$WORKFLOW_DIR" ]]; then
    report_error "Workflow directory $WORKFLOW_DIR does not exist"
    exit 1
fi

# Validate YAML syntax
echo "📋 Checking YAML syntax..."
for workflow_file in "$WORKFLOW_DIR"/*.yml "$WORKFLOW_DIR"/*.yaml; do
    if [[ -f "$workflow_file" ]]; then
        filename=$(basename "$workflow_file")

        # Check YAML syntax using Python
        if python3 -c "
import yaml
import sys
try:
    with open('$workflow_file', 'r') as f:
        yaml.safe_load(f)
    print('Valid YAML')
except yaml.YAMLError as e:
    print(f'YAML Error: {e}')
    sys.exit(1)
except Exception as e:
    print(f'Error: {e}')
    sys.exit(1)
" > /dev/null 2>&1; then
            report_success "$filename - Valid YAML syntax"
        else
            report_error "$filename - Invalid YAML syntax"
        fi
    fi
done

# Validate job structure
echo "🔧 Checking job structure..."
for workflow_file in "$WORKFLOW_DIR"/*.yml "$WORKFLOW_DIR"/*.yaml; do
    if [[ -f "$workflow_file" ]]; then
        filename=$(basename "$workflow_file")

        # Check job structure using Python
        python3 -c "
import yaml
import sys

try:
    with open('$workflow_file', 'r') as f:
        content = yaml.safe_load(f)

    if 'jobs' in content:
        for job_name, job_config in content['jobs'].items():
            if 'steps' not in job_config:
                print(f'Job \"{job_name}\" has no steps section')
                sys.exit(1)
            elif not job_config['steps']:
                print(f'Job \"{job_name}\" has empty steps section')
                sys.exit(1)
    print('All jobs have valid structure')
except Exception as e:
    print(f'Error checking job structure: {e}')
    sys.exit(1)
" > /dev/null 2>&1

        if [[ $? -eq 0 ]]; then
            report_success "$filename - All jobs have valid structure"
        else
            report_error "$filename - Job structure issues found"
        fi
    fi
done

# Check for orphaned workflows (workflows with no active triggers)
echo "🔍 Checking for orphaned workflows..."
for workflow_file in "$WORKFLOW_DIR"/*.yml "$WORKFLOW_DIR"/*.yaml; do
    if [[ -f "$workflow_file" ]]; then
        filename=$(basename "$workflow_file")

        # Check if workflow has any active triggers
        has_triggers=$(python3 -c "
import yaml
try:
    with open('$workflow_file', 'r') as f:
        content = yaml.safe_load(f)

    if 'on' in content:
        triggers = content['on']
        if isinstance(triggers, dict):
            # Check for active trigger types
            active_triggers = [k for k in triggers.keys() if k in ['push', 'pull_request', 'schedule', 'workflow_dispatch', 'repository_dispatch', 'release', 'deployment', 'deployment_status', 'check_run', 'check_suite', 'issue_comment', 'issues', 'label', 'milestone', 'page_build', 'project', 'project_card', 'project_column', 'public', 'pull_request_review', 'pull_request_review_comment', 'pull_request_target', 'registry_package', 'status', 'watch', 'workflow_call', 'workflow_run']]
            if active_triggers:
                print('Has active triggers')
            else:
                print('No active triggers')
        elif isinstance(triggers, list):
            print('Has active triggers')
        elif isinstance(triggers, str):
            # Handle string triggers like 'push' or 'pull_request'
            if triggers in ['push', 'pull_request', 'schedule', 'workflow_dispatch', 'repository_dispatch', 'release', 'deployment', 'deployment_status', 'check_run', 'check_suite', 'issue_comment', 'issues', 'label', 'milestone', 'page_build', 'project', 'project_card', 'project_column', 'public', 'pull_request_review', 'pull_request_review_comment', 'pull_request_target', 'registry_package', 'status', 'watch', 'workflow_call', 'workflow_run']:
                print('Has active triggers')
            else:
                print('No active triggers')
        else:
            print('No active triggers')
    else:
        print('No triggers defined')
except Exception as e:
    print(f'Error: {e}')
" 2>/dev/null)

        if [[ "$has_triggers" == "Has active triggers" ]]; then
            report_success "$filename - Has active triggers"
        else
            report_warning "$filename - No active triggers (may be orphaned)"
        fi
    fi
done

# Check for workflow naming consistency
echo "📝 Checking workflow naming consistency..."
for workflow_file in "$WORKFLOW_DIR"/*.yml "$WORKFLOW_DIR"/*.yaml; do
    if [[ -f "$workflow_file" ]]; then
        filename=$(basename "$workflow_file")

        # Check if workflow has a name field
        has_name=$(python3 -c "
import yaml
try:
    with open('$workflow_file', 'r') as f:
        content = yaml.safe_load(f)

    if 'name' in content and content['name']:
        print('Has name')
    else:
        print('No name')
except Exception as e:
    print(f'Error: {e}')
" 2>/dev/null)

        if [[ "$has_name" == "Has name" ]]; then
            report_success "$filename - Has descriptive name"
        else
            report_warning "$filename - Missing descriptive name"
        fi
    fi
done

# Summary
echo ""
echo "📊 Validation Summary:"
if [[ $VALIDATION_ERRORS -eq 0 ]]; then
    echo -e "${GREEN}🎉 All CI/CD workflow validations passed!${NC}"
    exit 0
else
    echo -e "${RED}❌ Found $VALIDATION_ERRORS validation errors${NC}"
    echo -e "${YELLOW}💡 Fix the errors above before committing workflow changes${NC}"
    exit 1
fi
