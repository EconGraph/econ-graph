#!/bin/bash

# Script to clean up orphaned GitHub Actions workflows
# Based on releng engineer persona requirements for workflow hygiene

set -e

echo "🔍 Analyzing GitHub Actions workflow status..."

# Get list of workflows that GitHub thinks are active
echo "📋 Workflows shown as active by GitHub:"
gh workflow list --all

echo ""
echo "📁 Workflows that actually exist in .github/workflows/:"
ls -la .github/workflows/*.yml | awk '{print $9}' | sed 's|.github/workflows/||' | sed 's|.yml||'

echo ""
echo "🧹 Orphaned workflows (shown as active but don't exist):"

# Get the list of workflows that GitHub shows as active
ACTIVE_WORKFLOWS=$(gh workflow list --all --json name,id --jq '.[].name' | tr -d '"')

# Get the list of workflows that actually exist
EXISTING_WORKFLOWS=$(ls .github/workflows/*.yml 2>/dev/null | xargs -n 1 basename | sed 's/.yml$//' || echo "")

# Find orphaned workflows
ORPHANED_WORKFLOWS=""
for workflow in $ACTIVE_WORKFLOWS; do
    if ! echo "$EXISTING_WORKFLOWS" | grep -q "^$workflow$"; then
        echo "  ❌ $workflow (orphaned)"
        ORPHANED_WORKFLOWS="$ORPHANED_WORKFLOWS $workflow"
    else
        echo "  ✅ $workflow (exists)"
    fi
done

if [ -z "$ORPHANED_WORKFLOWS" ]; then
    echo "🎉 No orphaned workflows found!"
else
    echo ""
    echo "⚠️  Found orphaned workflows. These need to be cleaned up."
    echo "   This is likely causing the CI parsing failures."
    echo ""
    echo "💡 To fix this issue:"
    echo "   1. The orphaned workflows are likely from deleted branches"
    echo "   2. GitHub's cache needs to be refreshed"
    echo "   3. Consider creating a dummy commit to force GitHub to refresh"
fi

echo ""
echo "🔍 Checking for other common workflow issues..."

# Check for workflows with no active triggers
echo "📋 Checking for workflows with potential trigger issues:"
for workflow in .github/workflows/*.yml; do
    if [ -f "$workflow" ]; then
        workflow_name=$(basename "$workflow" .yml)

        # Check if workflow has any active triggers
        if grep -q "^on:" "$workflow"; then
            # Check if all triggers are commented out
            if grep -A 10 "^on:" "$workflow" | grep -q "^#"; then
                echo "  ⚠️  $workflow_name: Has commented out triggers"
            else
                echo "  ✅ $workflow_name: Has active triggers"
            fi
        else
            echo "  ❌ $workflow_name: No 'on:' section found"
        fi
    fi
done

echo ""
echo "✅ Workflow analysis complete!"
