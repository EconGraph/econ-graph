#!/bin/bash

# Grafana Dashboard Validation Test Suite
# This script validates Grafana dashboard JSON files for common issues
# that cause "Dashboard title cannot be empty" and other loading errors
#
# IMPORTANT: This linter validates LOCAL YAML/JSON files only.
# It does NOT contact the Kubernetes cluster or check running services.
# All validation is performed against the source files in the repository.

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Print status function
print_status() {
    local level="$1"
    local message="$2"
    local timestamp=$(date '+%H:%M:%S')

    case $level in
        "INFO")
            echo -e "${BLUE}ℹ️  [$timestamp] $message${NC}"
            ;;
        "SUCCESS")
            echo -e "${GREEN}✅ [$timestamp] $message${NC}"
            ;;
        "WARNING")
            echo -e "${YELLOW}⚠️  [$timestamp] $message${NC}"
            ;;
        "ERROR")
            echo -e "${RED}❌ [$timestamp] $message${NC}"
            ;;
    esac
}

# Ensure required dependencies are installed
ensure_dependencies() {
    local missing=0

    if ! command -v yq >/dev/null 2>&1; then
        echo -e "${RED}❌ [$(date '+%H:%M:%S')] Missing dependency: yq${NC}"
        echo -e "${YELLOW}⚠️  [$(date '+%H:%M:%S')] Install yq (v4+ recommended). Examples:${NC}"
        echo -e "${YELLOW}   - Ubuntu/Debian: sudo apt-get install -y yq${NC}"
        echo -e "${YELLOW}   - Snap: sudo snap install yq${NC}"
        echo -e "${YELLOW}   - Binary: see https://github.com/mikefarah/yq/releases${NC}"
        missing=1
    fi

    if [ $missing -ne 0 ]; then
        echo -e "${RED}❌ [$(date '+%H:%M:%S')] Aborting: required tools are not installed.${NC}"
        exit 1
    fi
}

# Function to validate JSON structure
validate_json_structure() {
    local file="$1"
    local errors=0

    print_status "INFO" "Validating JSON structure for $file"

    # Check if file exists
    if [ ! -f "$file" ]; then
        print_status "ERROR" "File $file does not exist"
        return 1
    fi

    # Determine if this is a YAML file or JSON file
    local json_content=""
    if [[ "$file" == *.yaml ]] || [[ "$file" == *.yml ]]; then
        print_status "INFO" "File $file is YAML, extracting JSON content"
        # Extract JSON from YAML ConfigMap - try different possible keys
        json_content=$(yq -r "$file" 'data.*' 2>/dev/null | head -1)
        if [ -z "$json_content" ] || [ "$json_content" = "null" ]; then
            # Try specific key names
            json_content=$(yq -r "$file" 'data."logging-dashboard.json"' 2>/dev/null)
        fi
        if [ -z "$json_content" ] || [ "$json_content" = "null" ]; then
            # Try any .json key
            json_content=$(yq -r "$file" 'data' 2>/dev/null | grep -o '"[^"]*\.json"' | head -1 | sed 's/"//g' | xargs -I {} yq "$file" "data.{}" 2>/dev/null)
        fi
        if [ -z "$json_content" ] || [ "$json_content" = "null" ]; then
            print_status "ERROR" "Could not extract JSON content from YAML file $file"
            errors=$((errors + 1))
            return $errors
        fi
    else
        # It's a JSON file, use it directly
        json_content=$(cat "$file")
    fi

    # Check if content is valid JSON
    if ! echo "$json_content" | jq empty 2>/dev/null; then
        print_status "ERROR" "File $file contains invalid JSON"
        errors=$((errors + 1))
    else
        print_status "SUCCESS" "File $file contains valid JSON"
    fi

    # Check for required dashboard structure
    if ! echo "$json_content" | jq -e '.dashboard' >/dev/null 2>&1; then
        print_status "ERROR" "File $file is missing 'dashboard' root key"
        errors=$((errors + 1))
    else
        print_status "SUCCESS" "File $file has correct 'dashboard' root key"
    fi

    # Check for required fields
    local required_fields=("title" "uid" "panels")
    for field in "${required_fields[@]}"; do
        if ! echo "$json_content" | jq -e ".dashboard.$field" >/dev/null 2>&1; then
            print_status "ERROR" "File $file is missing required field: dashboard.$field"
            errors=$((errors + 1))
        else
            print_status "SUCCESS" "File $file has required field: dashboard.$field"
        fi
    done

    # Check title is not empty
    local title=$(echo "$json_content" | jq -r '.dashboard.title // ""')
    if [ -z "$title" ] || [ "$title" = "null" ]; then
        print_status "ERROR" "File $file has empty or null title"
        errors=$((errors + 1))
    else
        print_status "SUCCESS" "File $file has title: '$title'"
    fi

    # Check UID is not empty
    local uid=$(echo "$json_content" | jq -r '.dashboard.uid // ""')
    if [ -z "$uid" ] || [ "$uid" = "null" ]; then
        print_status "ERROR" "File $file has empty or null UID"
        errors=$((errors + 1))
    else
        print_status "SUCCESS" "File $file has UID: '$uid'"
    fi

    # Check panels array exists and is not empty
    local panel_count=$(echo "$json_content" | jq '.dashboard.panels | length' 2>/dev/null || echo "0")
    if [ "$panel_count" -eq 0 ]; then
        print_status "WARNING" "File $file has no panels"
    else
        print_status "SUCCESS" "File $file has $panel_count panels"
    fi

    return $errors
}

# Function to validate datasource references
validate_datasource_references() {
    local file="$1"
    local errors=0

    print_status "INFO" "Validating datasource references in $file"

    # Extract JSON content (same logic as validate_json_structure)
    local json_content=""
    if [[ "$file" == *.yaml ]] || [[ "$file" == *.yml ]]; then
        json_content=$(yq -r "$file" 'data.*' 2>/dev/null | head -1)
        if [ -z "$json_content" ] || [ "$json_content" = "null" ]; then
            json_content=$(yq -r "$file" 'data."logging-dashboard.json"' 2>/dev/null)
        fi
        if [ -z "$json_content" ] || [ "$json_content" = "null" ]; then
            json_content=$(yq -r "$file" 'data' 2>/dev/null | grep -o '"[^"]*\.json"' | head -1 | sed 's/"//g' | xargs -I {} yq "$file" "data.{}" 2>/dev/null)
        fi
    else
        json_content=$(cat "$file")
    fi

    # Check for datasource UIDs in targets
    local targets_with_datasource=$(echo "$json_content" | jq '[.dashboard.panels[]?.targets[]? | select(.datasource)] | length' 2>/dev/null || echo "0")
    local total_targets=$(echo "$json_content" | jq '[.dashboard.panels[]?.targets[]?] | length' 2>/dev/null || echo "0")

    if [ "$total_targets" -gt 0 ]; then
        if [ "$targets_with_datasource" -eq "$total_targets" ]; then
            print_status "SUCCESS" "All $total_targets targets have datasource references"
        else
            print_status "WARNING" "Only $targets_with_datasource of $total_targets targets have datasource references"
        fi

        # Check for consistent datasource UIDs
        local datasource_uids=$(echo "$json_content" | jq -r '.dashboard.panels[]?.targets[]?.datasource.uid // empty' 2>/dev/null | sort -u)
        local unique_uids=$(echo "$datasource_uids" | wc -l)

        if [ "$unique_uids" -le 2 ]; then
            print_status "SUCCESS" "Datasource UIDs are consistent: $(echo $datasource_uids | tr '\n' ' ')"
        else
            print_status "WARNING" "Multiple datasource UIDs found: $(echo $datasource_uids | tr '\n' ' ')"
        fi
    else
        print_status "WARNING" "No targets found in dashboard"
    fi

    return $errors
}

# Function to validate Loki queries
validate_loki_queries() {
    local file="$1"
    local errors=0

    print_status "INFO" "Validating Loki queries in $file"

    # Extract JSON content (same logic as other functions)
    local json_content=""
    if [[ "$file" == *.yaml ]] || [[ "$file" == *.yml ]]; then
        json_content=$(yq -r "$file" 'data.*' 2>/dev/null | head -1)
        if [ -z "$json_content" ] || [ "$json_content" = "null" ]; then
            json_content=$(yq -r "$file" 'data."logging-dashboard.json"' 2>/dev/null)
        fi
        if [ -z "$json_content" ] || [ "$json_content" = "null" ]; then
            json_content=$(yq -r "$file" 'data' 2>/dev/null | grep -o '"[^"]*\.json"' | head -1 | sed 's/"//g' | xargs -I {} yq "$file" "data.{}" 2>/dev/null)
        fi
    else
        json_content=$(cat "$file")
    fi

    # Check for Loki datasource usage
    local loki_targets=$(echo "$json_content" | jq '[.dashboard.panels[]?.targets[]? | select(.datasource.type == "loki")] | length' 2>/dev/null || echo "0")

    if [ "$loki_targets" -gt 0 ]; then
        print_status "SUCCESS" "Found $loki_targets Loki targets"

        # Check for proper job labels in Loki queries
        local queries_with_job=$(echo "$json_content" | jq -r '.dashboard.panels[]?.targets[]? | select(.datasource.type == "loki") | .expr' 2>/dev/null | grep -c 'job=' || echo "0")

        if [ "$queries_with_job" -eq "$loki_targets" ]; then
            print_status "SUCCESS" "All Loki queries use job labels"
        else
            print_status "WARNING" "Only $queries_with_job of $loki_targets Loki queries use job labels"
        fi

        # Validate LogQL syntax (basic validation)
        local logql_queries=$(echo "$json_content" | jq -r '.dashboard.panels[]?.targets[]? | select(.datasource.type == "loki") | .expr' 2>/dev/null)
        local invalid_queries=0

        while IFS= read -r query; do
            if [ -n "$query" ] && [ "$query" != "null" ]; then
                # Basic LogQL validation - check for common patterns
                if [[ "$query" =~ ^\{.*\}.*$ ]] || [[ "$query" =~ ^\{.*\}.*\|.*$ ]]; then
                    print_status "SUCCESS" "LogQL query appears valid: ${query:0:50}..."
                else
                    print_status "WARNING" "LogQL query may be invalid: ${query:0:50}..."
                    invalid_queries=$((invalid_queries + 1))
                fi
            fi
        done <<< "$logql_queries"

        if [ $invalid_queries -gt 0 ]; then
            print_status "WARNING" "Found $invalid_queries potentially invalid LogQL queries"
            errors=$((errors + 1))
        fi
    else
        print_status "INFO" "No Loki targets found in dashboard"
    fi

    return $errors
}

# Function to validate Prometheus queries
validate_prometheus_queries() {
    local file="$1"
    local errors=0

    print_status "INFO" "Validating Prometheus queries in $file"

    # Check for Prometheus datasource usage
    local prometheus_targets=$(jq '[.dashboard.panels[]?.targets[]? | select(.datasource.type == "prometheus")] | length' "$file" 2>/dev/null || echo "0")

    if [ "$prometheus_targets" -gt 0 ]; then
        print_status "SUCCESS" "Found $prometheus_targets Prometheus targets"

        # Check for proper job labels in Prometheus queries
        local queries_with_job=$(jq -r '.dashboard.panels[]?.targets[]? | select(.datasource.type == "prometheus") | .expr' "$file" 2>/dev/null | grep -c 'job=' || echo "0")

        if [ "$queries_with_job" -gt 0 ]; then
            print_status "SUCCESS" "Prometheus queries use job labels"
        else
            print_status "WARNING" "Prometheus queries may not use job labels"
        fi
    else
        print_status "INFO" "No Prometheus targets found in dashboard"
    fi

    return $errors
}

# Function to validate ConfigMap structure
validate_configmap_structure() {
    local configmap_file="$1"
    local errors=0

    print_status "INFO" "Validating ConfigMap structure for $configmap_file"

    if [ ! -f "$configmap_file" ]; then
        print_status "ERROR" "ConfigMap file $configmap_file does not exist"
        return 1
    fi

    # Check if it's valid YAML
    if ! yq -r "$configmap_file" '.' >/dev/null 2>&1; then
        print_status "ERROR" "ConfigMap file $configmap_file is not valid YAML"
        errors=$((errors + 1))
    else
        print_status "SUCCESS" "ConfigMap file $configmap_file is valid YAML"
    fi

    # Check for required ConfigMap fields
    if ! yq -r "$configmap_file" '.metadata.name' >/dev/null 2>&1; then
        print_status "ERROR" "ConfigMap file $configmap_file is missing metadata.name"
        errors=$((errors + 1))
    else
        print_status "SUCCESS" "ConfigMap file $configmap_file has metadata.name"
    fi

    # Check for dashboard data
    local dashboard_files=$(yq -r "$configmap_file" '.data | keys | length' 2>/dev/null || echo "0")
    if [ "$dashboard_files" -gt 0 ]; then
        print_status "SUCCESS" "ConfigMap contains $dashboard_files dashboard files"
    else
        print_status "ERROR" "ConfigMap contains no dashboard files"
        errors=$((errors + 1))
    fi

    # Check for truncated JSON content (the ingenious check!)
    local json_keys=$(yq -r "$configmap_file" '.data | keys' 2>/dev/null | grep '\.json$' || echo "")
    for json_key in $json_keys; do
        local json_content=$(yq "$configmap_file" ".data.\"$json_key\"" 2>/dev/null)
        local configmap_json_size=$(echo "$json_content" | wc -c)

        # Try to find the corresponding source file
        local source_file=""
        if [[ "$json_key" == "econgraph-overview.json" ]]; then
            source_file="grafana-dashboards/econgraph-overview.json"
        elif [[ "$json_key" == "logging-dashboard.json" ]]; then
            # Extract from YAML ConfigMap
            source_file="k8s/monitoring/grafana-logging-dashboard.yaml"
        fi

        if [ -n "$source_file" ] && [ -f "$source_file" ]; then
            local source_size=0
            if [[ "$source_file" == *.yaml ]] || [[ "$source_file" == *.yml ]]; then
                # Extract JSON from YAML ConfigMap
                local extracted_json=$(yq -r "$source_file" 'data.*' 2>/dev/null | head -1)
                if [ -z "$extracted_json" ] || [ "$extracted_json" = "null" ]; then
                    extracted_json=$(yq -r "$source_file" 'data."logging-dashboard.json"' 2>/dev/null)
                fi
                if [ -z "$extracted_json" ] || [ "$extracted_json" = "null" ]; then
                    extracted_json=$(yq -r "$source_file" 'data' 2>/dev/null | grep -o '"[^"]*\.json"' | head -1 | sed 's/"//g' | xargs -I {} yq "$source_file" "data.{}" 2>/dev/null)
                fi
                source_size=$(echo "$extracted_json" | wc -c)
            else
                source_size=$(wc -c < "$source_file")
            fi

            # Allow for small differences (newlines, whitespace)
            local size_diff=$((source_size - configmap_json_size))
            if [ $size_diff -gt 100 ]; then
                print_status "ERROR" "❌ JSON content truncated in ConfigMap!"
                print_status "ERROR" "   Source file: $source_file ($source_size bytes)"
                print_status "ERROR" "   ConfigMap: $json_key ($configmap_json_size bytes)"
                print_status "ERROR" "   Missing: $size_diff bytes"
                print_status "INFO" "   This usually indicates YAML parsing issues or file truncation"
                errors=$((errors + 1))
            elif [ $size_diff -gt 10 ]; then
                print_status "WARNING" "⚠️  JSON content size mismatch in ConfigMap"
                print_status "WARNING" "   Source: $source_file ($source_size bytes)"
                print_status "WARNING" "   ConfigMap: $json_key ($configmap_json_size bytes)"
                print_status "WARNING" "   Difference: $size_diff bytes"
            else
                print_status "SUCCESS" "✅ JSON content size matches source: $json_key ($configmap_json_size bytes)"
            fi
        else
            print_status "WARNING" "⚠️  Could not find source file for $json_key to validate size"
        fi

        # Additional check: validate JSON syntax in ConfigMap
        if ! echo "$json_content" | jq empty 2>/dev/null; then
            print_status "ERROR" "❌ JSON content in ConfigMap $json_key is invalid"
            errors=$((errors + 1))
        else
            print_status "SUCCESS" "✅ JSON content in ConfigMap $json_key is valid"
        fi
    done

    return $errors
}

validate_k8s_deployment() {
    local errors=0

    print_status "INFO" "🔧 Step 6: Validating Kubernetes deployment status"

    # Check if kubectl is available
    if ! command -v kubectl >/dev/null 2>&1; then
        print_status "WARNING" "kubectl not available, skipping K8s deployment validation"
        return 0
    fi

    # Check if we're connected to a cluster
    if ! kubectl cluster-info >/dev/null 2>&1; then
        print_status "WARNING" "Not connected to Kubernetes cluster, skipping deployment validation"
        return 0
    fi

    # Check for required ConfigMaps in the econ-graph namespace
    local required_configmaps=(
        "grafana-datasources"
        "grafana-dashboard-provider"
        "grafana-dashboards"
        "grafana-logging-dashboard"
    )

    for configmap in "${required_configmaps[@]}"; do
        if kubectl get configmap "$configmap" -n econ-graph >/dev/null 2>&1; then
            print_status "SUCCESS" "✅ ConfigMap '$configmap' exists in cluster"
        else
            print_status "ERROR" "❌ ConfigMap '$configmap' missing from cluster"
            print_status "INFO" "Run: kubectl apply -f k8s/monitoring/$configmap.yaml"
            errors=$((errors + 1))
        fi
    done

    # Check for Grafana StatefulSet (not Deployment)
    if kubectl get statefulset "grafana" -n econ-graph >/dev/null 2>&1; then
        local ready_replicas=$(kubectl get statefulset "grafana" -n econ-graph -o jsonpath='{.status.readyReplicas}' 2>/dev/null || echo "0")
        local desired_replicas=$(kubectl get statefulset "grafana" -n econ-graph -o jsonpath='{.spec.replicas}' 2>/dev/null || echo "0")

        if [ "$ready_replicas" = "$desired_replicas" ] && [ "$ready_replicas" -gt 0 ]; then
            print_status "SUCCESS" "✅ StatefulSet 'grafana' is ready ($ready_replicas/$desired_replicas)"
        else
            print_status "WARNING" "⚠️  StatefulSet 'grafana' not ready ($ready_replicas/$desired_replicas)"
            errors=$((errors + 1))
        fi
    else
        print_status "ERROR" "❌ StatefulSet 'grafana' missing from cluster"
        errors=$((errors + 1))
    fi

    # Check for required deployments (loki and prometheus)
    local required_deployments=("loki" "prometheus")
    for deployment in "${required_deployments[@]}"; do
        if kubectl get deployment "$deployment" -n econ-graph >/dev/null 2>&1; then
            local ready_replicas=$(kubectl get deployment "$deployment" -n econ-graph -o jsonpath='{.status.readyReplicas}' 2>/dev/null || echo "0")
            local desired_replicas=$(kubectl get deployment "$deployment" -n econ-graph -o jsonpath='{.spec.replicas}' 2>/dev/null || echo "0")

            if [ "$ready_replicas" = "$desired_replicas" ] && [ "$ready_replicas" -gt 0 ]; then
                print_status "SUCCESS" "✅ Deployment '$deployment' is ready ($ready_replicas/$desired_replicas)"
            else
                print_status "WARNING" "⚠️  Deployment '$deployment' not ready ($ready_replicas/$desired_replicas)"
                errors=$((errors + 1))
            fi
        else
            print_status "ERROR" "❌ Deployment '$deployment' missing from cluster"
            errors=$((errors + 1))
        fi
    done

    # Check for required services
    local required_services=("grafana-service" "loki-service" "prometheus-service")
    for service in "${required_services[@]}"; do
        if kubectl get service "$service" -n econ-graph >/dev/null 2>&1; then
            print_status "SUCCESS" "✅ Service '$service' exists in cluster"
        else
            print_status "ERROR" "❌ Service '$service' missing from cluster"
            errors=$((errors + 1))
        fi
    done

    # Check for DaemonSet (Promtail)
    if kubectl get daemonset promtail -n econ-graph >/dev/null 2>&1; then
        local ready_nodes=$(kubectl get daemonset promtail -n econ-graph -o jsonpath='{.status.numberReady}' 2>/dev/null || echo "0")
        local desired_nodes=$(kubectl get daemonset promtail -n econ-graph -o jsonpath='{.status.desiredNumberScheduled}' 2>/dev/null || echo "0")

        if [ "$ready_nodes" = "$desired_nodes" ] && [ "$ready_nodes" -gt 0 ]; then
            print_status "SUCCESS" "✅ DaemonSet 'promtail' is ready ($ready_nodes/$desired_nodes nodes)"
        else
            print_status "WARNING" "⚠️  DaemonSet 'promtail' not ready ($ready_nodes/$desired_nodes nodes)"
            errors=$((errors + 1))
        fi
    else
        print_status "ERROR" "❌ DaemonSet 'promtail' missing from cluster"
        errors=$((errors + 1))
    fi

    return $errors
}

# Global counters for test statistics
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0
WARNING_TESTS=0

# Function to increment test counters
increment_test_count() {
    local result="$1"
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    case $result in
        "PASS")
            PASSED_TESTS=$((PASSED_TESTS + 1))
            ;;
        "FAIL")
            FAILED_TESTS=$((FAILED_TESTS + 1))
            ;;
        "WARN")
            WARNING_TESTS=$((WARNING_TESTS + 1))
            ;;
    esac
}

# Function to run a validation test and track results
run_validation_test() {
    local test_name="$1"
    local test_function="$2"
    local file="$3"

    print_status "INFO" "Running $test_name for $file"

    if $test_function "$file"; then
        print_status "SUCCESS" "✅ $test_name passed"
        increment_test_count "PASS"
        return 0
    else
        local exit_code=$?
        if [ $exit_code -eq 1 ]; then
            print_status "ERROR" "❌ $test_name failed"
            increment_test_count "FAIL"
        else
            print_status "WARNING" "⚠️  $test_name has warnings"
            increment_test_count "WARN"
        fi
        return $exit_code
    fi
}

# Individual validation steps for granular CI reporting
validate_json_syntax() {
    local file="$1"
    local errors=0

    print_status "INFO" "🔍 Step 1: Validating JSON syntax for $file"

    # Extract JSON content
    local json_content=""
    if [[ "$file" == *.yaml ]] || [[ "$file" == *.yml ]]; then
        json_content=$(yq -r "$file" 'data.*' 2>/dev/null | head -1)
        if [ -z "$json_content" ] || [ "$json_content" = "null" ]; then
            json_content=$(yq -r "$file" 'data."logging-dashboard.json"' 2>/dev/null)
        fi
        if [ -z "$json_content" ] || [ "$json_content" = "null" ]; then
            json_content=$(yq -r "$file" 'data' 2>/dev/null | grep -o '"[^"]*\.json"' | head -1 | sed 's/"//g' | xargs -I {} yq "$file" "data.{}" 2>/dev/null)
        fi
    else
        json_content=$(cat "$file")
    fi

    # Check if content is valid JSON
    if ! echo "$json_content" | jq empty 2>/dev/null; then
        print_status "ERROR" "❌ JSON syntax is invalid"
        errors=$((errors + 1))
    else
        print_status "SUCCESS" "✅ JSON syntax is valid"
    fi

    return $errors
}

validate_dashboard_structure() {
    local file="$1"
    local errors=0

    print_status "INFO" "🏗️  Step 2: Validating dashboard structure for $file"

    # Extract JSON content
    local json_content=""
    if [[ "$file" == *.yaml ]] || [[ "$file" == *.yml ]]; then
        json_content=$(yq -r "$file" 'data.*' 2>/dev/null | head -1)
        if [ -z "$json_content" ] || [ "$json_content" = "null" ]; then
            json_content=$(yq -r "$file" 'data."logging-dashboard.json"' 2>/dev/null)
        fi
        if [ -z "$json_content" ] || [ "$json_content" = "null" ]; then
            json_content=$(yq -r "$file" 'data' 2>/dev/null | grep -o '"[^"]*\.json"' | head -1 | sed 's/"//g' | xargs -I {} yq "$file" "data.{}" 2>/dev/null)
        fi
    else
        json_content=$(cat "$file")
    fi

    # Check for required dashboard structure
    if ! echo "$json_content" | jq -e '.dashboard' >/dev/null 2>&1; then
        print_status "ERROR" "❌ Missing 'dashboard' root key"
        errors=$((errors + 1))
    else
        print_status "SUCCESS" "✅ Has 'dashboard' root key"
    fi

    # Check for required fields
    local required_fields=("title" "uid" "panels")
    for field in "${required_fields[@]}"; do
        if ! echo "$json_content" | jq -e ".dashboard.$field" >/dev/null 2>&1; then
            print_status "ERROR" "❌ Missing required field: dashboard.$field"
            errors=$((errors + 1))
        else
            print_status "SUCCESS" "✅ Has required field: dashboard.$field"
        fi
    done

    # Check title is not empty
    local title=$(echo "$json_content" | jq -r '.dashboard.title // ""')
    if [ -z "$title" ] || [ "$title" = "null" ]; then
        print_status "ERROR" "❌ Dashboard title is empty or null"
        errors=$((errors + 1))
    else
        print_status "SUCCESS" "✅ Dashboard title: '$title'"
    fi

    # Check UID is not empty
    local uid=$(echo "$json_content" | jq -r '.dashboard.uid // ""')
    if [ -z "$uid" ] || [ "$uid" = "null" ]; then
        print_status "ERROR" "❌ Dashboard UID is empty or null"
        errors=$((errors + 1))
    else
        print_status "SUCCESS" "✅ Dashboard UID: '$uid'"
    fi

    return $errors
}

validate_datasource_consistency() {
    local file="$1"
    local errors=0

    print_status "INFO" "🔗 Step 3: Validating datasource consistency for $file"

    # Extract JSON content
    local json_content=""
    if [[ "$file" == *.yaml ]] || [[ "$file" == *.yml ]]; then
        json_content=$(yq -r "$file" 'data.*' 2>/dev/null | head -1)
        if [ -z "$json_content" ] || [ "$json_content" = "null" ]; then
            json_content=$(yq -r "$file" 'data."logging-dashboard.json"' 2>/dev/null)
        fi
        if [ -z "$json_content" ] || [ "$json_content" = "null" ]; then
            json_content=$(yq -r "$file" 'data' 2>/dev/null | grep -o '"[^"]*\.json"' | head -1 | sed 's/"//g' | xargs -I {} yq "$file" "data.{}" 2>/dev/null)
        fi
    else
        json_content=$(cat "$file")
    fi

    # Check for datasource UIDs in targets
    local targets_with_datasource=$(echo "$json_content" | jq '[.dashboard.panels[]?.targets[]? | select(.datasource)] | length' 2>/dev/null || echo "0")
    local total_targets=$(echo "$json_content" | jq '[.dashboard.panels[]?.targets[]?] | length' 2>/dev/null || echo "0")

    if [ "$total_targets" -gt 0 ]; then
        if [ "$targets_with_datasource" -eq "$total_targets" ]; then
            print_status "SUCCESS" "✅ All $total_targets targets have datasource references"
        else
            print_status "WARNING" "⚠️  Only $targets_with_datasource of $total_targets targets have datasource references"
            errors=$((errors + 1))
        fi

        # Check for consistent datasource UIDs
        local datasource_uids=$(echo "$json_content" | jq -r '.dashboard.panels[]?.targets[]?.datasource.uid // empty' 2>/dev/null | sort -u)
        local unique_uids=$(echo "$datasource_uids" | wc -l)

        if [ "$unique_uids" -le 2 ]; then
            print_status "SUCCESS" "✅ Datasource UIDs are consistent: $(echo $datasource_uids | tr '\n' ' ')"
        else
            print_status "WARNING" "⚠️  Multiple datasource UIDs found: $(echo $datasource_uids | tr '\n' ' ')"
            errors=$((errors + 1))
        fi
    else
        print_status "WARNING" "⚠️  No targets found in dashboard"
    fi

    return $errors
}

validate_promql_queries() {
    local file="$1"
    local errors=0

    print_status "INFO" "📊 Step 4: Validating PromQL queries for $file"

    # Extract JSON content
    local json_content=""
    if [[ "$file" == *.yaml ]] || [[ "$file" == *.yml ]]; then
        json_content=$(yq -r "$file" 'data.*' 2>/dev/null | head -1)
        if [ -z "$json_content" ] || [ "$json_content" = "null" ]; then
            json_content=$(yq -r "$file" 'data."logging-dashboard.json"' 2>/dev/null)
        fi
        if [ -z "$json_content" ] || [ "$json_content" = "null" ]; then
            json_content=$(yq -r "$file" 'data' 2>/dev/null | grep -o '"[^"]*\.json"' | head -1 | sed 's/"//g' | xargs -I {} yq "$file" "data.{}" 2>/dev/null)
        fi
    else
        json_content=$(cat "$file")
    fi

    # Check for Prometheus datasource usage
    local prometheus_targets=$(echo "$json_content" | jq '[.dashboard.panels[]?.targets[]? | select(.datasource.type == "prometheus")] | length' 2>/dev/null || echo "0")

    if [ "$prometheus_targets" -gt 0 ]; then
        print_status "SUCCESS" "✅ Found $prometheus_targets Prometheus targets"

        # Check for proper job labels in Prometheus queries (optional for Kubernetes metrics)
        local queries_with_job=$(echo "$json_content" | jq -r '.dashboard.panels[]?.targets[]? | select(.datasource.type == "prometheus") | .expr' 2>/dev/null | grep -c 'job=' | head -1 || echo "0")
        local queries_with_kube=$(echo "$json_content" | jq -r '.dashboard.panels[]?.targets[]? | select(.datasource.type == "prometheus") | .expr' 2>/dev/null | grep -c 'kube_' | head -1 || echo "0")

        if [ "$queries_with_job" -gt 0 ]; then
            print_status "SUCCESS" "✅ Prometheus queries use job labels"
        elif [ "$queries_with_kube" -gt 0 ]; then
            print_status "SUCCESS" "✅ Prometheus queries use Kubernetes metrics (kube_*)"
        else
            print_status "WARNING" "⚠️  Prometheus queries may not use standard job or Kubernetes labels"
            errors=$((errors + 1))
        fi

        # Basic PromQL validation
        local promql_queries=$(echo "$json_content" | jq -r '.dashboard.panels[]?.targets[]? | select(.datasource.type == "prometheus") | .expr' 2>/dev/null)
        local invalid_queries=0

        while IFS= read -r query; do
            if [ -n "$query" ] && [ "$query" != "null" ]; then
                # Basic PromQL validation - check for common patterns
                if [[ "$query" =~ ^[a-zA-Z_][a-zA-Z0-9_]*.*$ ]] || [[ "$query" =~ ^sum\(.*\)$ ]] || [[ "$query" =~ ^rate\(.*\)$ ]]; then
                    print_status "SUCCESS" "✅ PromQL query appears valid: ${query:0:50}..."
                else
                    print_status "WARNING" "⚠️  PromQL query may be invalid: ${query:0:50}..."
                    invalid_queries=$((invalid_queries + 1))
                fi
            fi
        done <<< "$promql_queries"

        if [ $invalid_queries -gt 0 ]; then
            print_status "WARNING" "⚠️  Found $invalid_queries potentially invalid PromQL queries"
            errors=$((errors + 1))
        fi
    else
        print_status "INFO" "ℹ️  No Prometheus targets found in dashboard"
    fi

    return $errors
}

validate_logql_queries() {
    local file="$1"
    local errors=0

    print_status "INFO" "📝 Step 5: Validating LogQL queries for $file"

    # Extract JSON content
    local json_content=""
    if [[ "$file" == *.yaml ]] || [[ "$file" == *.yml ]]; then
        json_content=$(yq -r "$file" 'data.*' 2>/dev/null | head -1)
        if [ -z "$json_content" ] || [ "$json_content" = "null" ]; then
            json_content=$(yq -r "$file" 'data."logging-dashboard.json"' 2>/dev/null)
        fi
        if [ -z "$json_content" ] || [ "$json_content" = "null" ]; then
            json_content=$(yq -r "$file" 'data' 2>/dev/null | grep -o '"[^"]*\.json"' | head -1 | sed 's/"//g' | xargs -I {} yq "$file" "data.{}" 2>/dev/null)
        fi
    else
        json_content=$(cat "$file")
    fi

    # Check for Loki datasource usage
    local loki_targets=$(echo "$json_content" | jq '[.dashboard.panels[]?.targets[]? | select(.datasource.type == "loki")] | length' 2>/dev/null || echo "0")

    if [ "$loki_targets" -gt 0 ]; then
        print_status "SUCCESS" "✅ Found $loki_targets Loki targets"

        # Check for proper job labels in Loki queries (optional for app-based queries)
        local queries_with_job=$(echo "$json_content" | jq -r '.dashboard.panels[]?.targets[]? | select(.datasource.type == "loki") | .expr' 2>/dev/null | grep -c 'job=' | head -1 || echo "0")
        local queries_with_app=$(echo "$json_content" | jq -r '.dashboard.panels[]?.targets[]? | select(.datasource.type == "loki") | .expr' 2>/dev/null | grep -c 'app=' | head -1 || echo "0")

        if [ "$queries_with_job" -eq "$loki_targets" ]; then
            print_status "SUCCESS" "✅ All Loki queries use job labels"
        elif [ "$queries_with_app" -gt 0 ]; then
            print_status "SUCCESS" "✅ Loki queries use app labels (Kubernetes-native)"
        else
            print_status "WARNING" "⚠️  Loki queries may not use standard job or app labels"
            errors=$((errors + 1))
        fi

        # Validate LogQL syntax (basic validation)
        local logql_queries=$(echo "$json_content" | jq -r '.dashboard.panels[]?.targets[]? | select(.datasource.type == "loki") | .expr' 2>/dev/null)
        local invalid_queries=0

        while IFS= read -r query; do
            if [ -n "$query" ] && [ "$query" != "null" ]; then
                # Basic LogQL validation - check for common patterns
                if [[ "$query" =~ ^\{.*\}.*$ ]] || [[ "$query" =~ ^\{.*\}.*\|.*$ ]]; then
                    print_status "SUCCESS" "✅ LogQL query appears valid: ${query:0:50}..."
                else
                    print_status "WARNING" "⚠️  LogQL query may be invalid: ${query:0:50}..."
                    invalid_queries=$((invalid_queries + 1))
                fi
            fi
        done <<< "$logql_queries"

        if [ $invalid_queries -gt 0 ]; then
            print_status "WARNING" "⚠️  Found $invalid_queries potentially invalid LogQL queries"
            errors=$((errors + 1))
        fi
    else
        print_status "INFO" "ℹ️  No Loki targets found in dashboard"
    fi

    return $errors
}

# Main validation function
validate_dashboard_file() {
    local file="$1"
    local total_errors=0

    print_status "INFO" "Starting validation for $file"
    echo "========================================"

    # Run each validation step and track results
    run_validation_test "JSON Syntax" "validate_json_syntax" "$file"
    total_errors=$((total_errors + $?))

    run_validation_test "Dashboard Structure" "validate_dashboard_structure" "$file"
    total_errors=$((total_errors + $?))

    run_validation_test "Datasource Consistency" "validate_datasource_consistency" "$file"
    total_errors=$((total_errors + $?))

    run_validation_test "PromQL Queries" "validate_promql_queries" "$file"
    total_errors=$((total_errors + $?))

    run_validation_test "LogQL Queries" "validate_logql_queries" "$file"
    total_errors=$((total_errors + $?))

    echo "========================================"
    if [ $total_errors -eq 0 ]; then
        print_status "SUCCESS" "All validations passed for $file"
    else
        print_status "ERROR" "Found $total_errors validation errors in $file"
    fi

    return $total_errors
}

# Function to run specific validation step
run_specific_step() {
    local step="$1"
    local total_errors=0
    local files_checked=0

    # Dashboard files to validate
    local dashboard_files=(
        "grafana-dashboards/econgraph-overview.json"
        "k8s/monitoring/grafana-logging-dashboard.yaml"
    )

    case "$step" in
        "json-syntax")
            print_status "INFO" "🔍 Running JSON Syntax Validation"
            for file in "${dashboard_files[@]}"; do
                if [ -f "$file" ]; then
                    validate_json_syntax "$file"
                    total_errors=$((total_errors + $?))
                    files_checked=$((files_checked + 1))
                fi
            done
            ;;
        "dashboard-structure")
            print_status "INFO" "🏗️ Running Dashboard Structure Validation"
            for file in "${dashboard_files[@]}"; do
                if [ -f "$file" ]; then
                    validate_dashboard_structure "$file"
                    total_errors=$((total_errors + $?))
                    files_checked=$((files_checked + 1))
                fi
            done
            ;;
        "datasource-consistency")
            print_status "INFO" "🔗 Running Datasource Consistency Validation"
            for file in "${dashboard_files[@]}"; do
                if [ -f "$file" ]; then
                    validate_datasource_consistency "$file"
                    total_errors=$((total_errors + $?))
                    files_checked=$((files_checked + 1))
                fi
            done
            ;;
        "promql-queries")
            print_status "INFO" "📊 Running PromQL Query Validation"
            for file in "${dashboard_files[@]}"; do
                if [ -f "$file" ]; then
                    validate_promql_queries "$file"
                    total_errors=$((total_errors + $?))
                    files_checked=$((files_checked + 1))
                fi
            done
            ;;
        "logql-queries")
            print_status "INFO" "📝 Running LogQL Query Validation"
            for file in "${dashboard_files[@]}"; do
                if [ -f "$file" ]; then
                    validate_logql_queries "$file"
                    total_errors=$((total_errors + $?))
                    files_checked=$((files_checked + 1))
                fi
            done
            ;;
        "configmap-structure")
            print_status "INFO" "📋 Running ConfigMap Structure Validation"
            local configmap_files=("k8s/monitoring/grafana-dashboards.yaml")
            for file in "${configmap_files[@]}"; do
                if [ -f "$file" ]; then
                    validate_configmap_structure "$file"
                    total_errors=$((total_errors + $?))
                    files_checked=$((files_checked + 1))
                fi
            done
            ;;
        "k8s-deployment")
            print_status "INFO" "🔧 Running Kubernetes Deployment Validation"
            validate_k8s_deployment
            total_errors=$((total_errors + $?))
            files_checked=$((files_checked + 1))
            ;;
        *)
            print_status "ERROR" "Unknown step: $step"
            print_status "INFO" "Available steps: json-syntax, dashboard-structure, datasource-consistency, promql-queries, logql-queries, configmap-structure, k8s-deployment"
            exit 1
            ;;
    esac

    echo "========================================"
    if [ $total_errors -eq 0 ]; then
        print_status "SUCCESS" "✅ $step validation passed for $files_checked files"
        exit 0
    else
        print_status "ERROR" "❌ $step validation failed with $total_errors errors across $files_checked files"
        exit 1
    fi
}

# Main execution
main() {
    ensure_dependencies
    # Check for step parameter
    if [ "$1" = "--step" ] && [ -n "$2" ]; then
        run_specific_step "$2"
        return
    elif [[ "$1" == --step=* ]]; then
        local step_name="${1#--step=}"
        run_specific_step "$step_name"
        return
    fi

    echo
    print_status "INFO" "Starting Grafana Dashboard Validation Test Suite"
    echo "================================================================"

    local total_errors=0
    local files_checked=0

    # Validate individual dashboard files
    local dashboard_files=(
        "grafana-dashboards/econgraph-overview.json"
        "k8s/monitoring/grafana-logging-dashboard.yaml"
    )

    for file in "${dashboard_files[@]}"; do
        if [ -f "$file" ]; then
            validate_dashboard_file "$file"
            total_errors=$((total_errors + $?))
            files_checked=$((files_checked + 1))
        else
            print_status "WARNING" "Dashboard file $file not found, skipping"
        fi
    done

    # Validate ConfigMap files
    local configmap_files=(
        "k8s/monitoring/grafana-dashboards.yaml"
    )

    for file in "${configmap_files[@]}"; do
        if [ -f "$file" ]; then
            validate_configmap_structure "$file"
            total_errors=$((total_errors + $?))
            files_checked=$((files_checked + 1))
        else
            print_status "WARNING" "ConfigMap file $file not found, skipping"
        fi
    done

    # Validate Kubernetes deployment
    print_status "INFO" "🔧 Validating Kubernetes deployment status"
    validate_k8s_deployment
    total_errors=$((total_errors + $?))
    files_checked=$((files_checked + 1))

    echo
    echo "================================================================"
    print_status "INFO" "📊 VALIDATION TEST SUMMARY"
    echo "================================================================"
    print_status "INFO" "Total Tests Run: $TOTAL_TESTS"
    print_status "SUCCESS" "✅ Passed: $PASSED_TESTS"
    if [ $WARNING_TESTS -gt 0 ]; then
        print_status "WARNING" "⚠️  Warnings: $WARNING_TESTS"
    fi
    if [ $FAILED_TESTS -gt 0 ]; then
        print_status "ERROR" "❌ Failed: $FAILED_TESTS"
    fi
    print_status "INFO" "Files Checked: $files_checked"

    # Calculate success rate
    if [ $TOTAL_TESTS -gt 0 ]; then
        local success_rate=$((PASSED_TESTS * 100 / TOTAL_TESTS))
        print_status "INFO" "Success Rate: $success_rate%"
    fi

    echo "================================================================"

    if [ $total_errors -eq 0 ] && [ $FAILED_TESTS -eq 0 ]; then
        print_status "SUCCESS" "🎉 ALL DASHBOARD VALIDATIONS PASSED!"
        if [ $WARNING_TESTS -gt 0 ]; then
            print_status "INFO" "Note: $WARNING_TESTS tests had warnings but no critical failures"
        fi
        exit 0
    else
        print_status "ERROR" "❌ DASHBOARD VALIDATION FAILED!"
        print_status "ERROR" "Found $total_errors total errors across $files_checked files"
        print_status "ERROR" "$FAILED_TESTS tests failed, $WARNING_TESTS tests had warnings"
        print_status "INFO" "Fix the errors above before deploying dashboards"
        exit 1
    fi
}

# Run the validation
main "$@"
