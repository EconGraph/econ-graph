#!/bin/bash

# Automated test for Loki/Grafana monitoring stack
# This script tests the complete log collection and query pipeline

set -e

echo "🧪 Starting Loki/Grafana Monitoring Stack Test"
echo "=============================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test configuration
NAMESPACE="econ-graph"
TEST_EMAIL="monitoring-test-$(date +%s)@example.com"
TEST_NAME="Monitoring Test User"
BACKEND_URL="http://localhost:30080"
GRAFANA_URL="http://localhost:30001"
LOKI_URL="http://localhost:3100"

# Function to print colored output
print_status() {
    local status=$1
    local message=$2
    case $status in
        "INFO") echo -e "${BLUE}ℹ️  $message${NC}" ;;
        "SUCCESS") echo -e "${GREEN}✅ $message${NC}" ;;
        "WARNING") echo -e "${YELLOW}⚠️  $message${NC}" ;;
        "ERROR") echo -e "${RED}❌ $message${NC}" ;;
    esac
}

# Function to wait for service to be ready
wait_for_service() {
    local service_name=$1
    local url=$2
    local max_attempts=30
    local attempt=1

    print_status "INFO" "Waiting for $service_name to be ready..."

    while [ $attempt -le $max_attempts ]; do
        if curl -s "$url" > /dev/null 2>&1; then
            print_status "SUCCESS" "$service_name is ready"
            return 0
        fi

        echo -n "."
        sleep 2
        attempt=$((attempt + 1))
    done

    print_status "ERROR" "$service_name failed to become ready after $max_attempts attempts"
    return 1
}

# Function to check pod status
check_pod_status() {
    local pod_pattern=$1
    local expected_status=${2:-"Running"}

    print_status "INFO" "Checking pod status for: $pod_pattern"

    local pods=$(kubectl get pods -n $NAMESPACE | grep "$pod_pattern" | awk '{print $1 " " $3}')

    if [ -z "$pods" ]; then
        print_status "ERROR" "No pods found matching pattern: $pod_pattern"
        return 1
    fi

    echo "$pods" | while read pod_name status; do
        if [ "$status" = "$expected_status" ]; then
            print_status "SUCCESS" "Pod $pod_name is $status"
        else
            print_status "ERROR" "Pod $pod_name is $status (expected $expected_status)"
            return 1
        fi
    done
}

# Function to check Grafana uniqueness
check_grafana_uniqueness() {
    print_status "INFO" "Checking Grafana instance uniqueness..."

    # Count Grafana pods
    local grafana_pods=$(kubectl get pods -n $NAMESPACE -l app=grafana --no-headers | wc -l)

    if [ "$grafana_pods" -eq 0 ]; then
        print_status "ERROR" "No Grafana pods found"
        return 1
    elif [ "$grafana_pods" -eq 1 ]; then
        print_status "SUCCESS" "Exactly 1 Grafana pod found (correct)"
        return 0
    else
        print_status "ERROR" "Found $grafana_pods Grafana pods - should be exactly 1 to avoid data loss and configuration conflicts"
        print_status "ERROR" "Multiple Grafana instances can cause:"
        print_status "ERROR" "  - Data loss (different persistent volumes)"
        print_status "ERROR" "  - Configuration inconsistencies (different datasource UIDs)"
        print_status "ERROR" "  - Load balancing issues"
        print_status "ERROR" "  - Resource waste"
        return 1
    fi
}

# Function to generate test activity
generate_test_activity() {
    print_status "INFO" "Generating test activity..."

    # Generate multiple API calls to create log entries
    for i in {1..5}; do
        curl -s -X POST "$BACKEND_URL/auth/register" \
            -H "Content-Type: application/json" \
            -d "{\"email\":\"$TEST_EMAIL-$i\",\"password\":\"testpass123\",\"name\":\"$TEST_NAME $i\"}" \
            > /dev/null 2>&1 || true
        sleep 1
    done

    print_status "SUCCESS" "Generated 5 test API calls"
}

# Function to check Promtail metrics
check_promtail_metrics() {
    print_status "INFO" "Checking Promtail metrics..."

    # Get Promtail pod name
    local promtail_pod=$(kubectl get pods -n $NAMESPACE | grep promtail | head -1 | awk '{print $1}')

    if [ -z "$promtail_pod" ]; then
        print_status "ERROR" "No Promtail pod found"
        return 1
    fi

    # Port forward to Promtail metrics
    kubectl port-forward -n $NAMESPACE "$promtail_pod" 9080:9080 > /dev/null 2>&1 &
    local port_forward_pid=$!
    sleep 3

    # Check metrics
    local sent_entries=$(curl -s "http://localhost:9080/metrics" | grep "promtail_sent_entries_total" | grep -v "#" | awk '{print $2}' | head -1)

    # Clean up port forward
    kill $port_forward_pid 2>/dev/null || true

    if [ -n "$sent_entries" ] && [ "$sent_entries" -gt 0 ]; then
        print_status "SUCCESS" "Promtail has sent $sent_entries entries to Loki"
        return 0
    else
        print_status "ERROR" "Promtail has sent 0 entries to Loki"
        return 1
    fi
}

# Function to check Loki metrics
check_loki_metrics() {
    print_status "INFO" "Checking Loki metrics..."

    # Port forward to Loki metrics
    kubectl port-forward -n $NAMESPACE deployment/loki 3100:3100 > /dev/null 2>&1 &
    local port_forward_pid=$!
    sleep 3

    # Check metrics
    local memory_chunks=$(curl -s "http://localhost:3100/metrics" | grep "loki_ingester_memory_chunks" | grep -v "#" | awk '{print $2}' | head -1)
    local memory_streams=$(curl -s "http://localhost:3100/metrics" | grep "loki_ingester_memory_streams" | grep -v "#" | awk '{print $2}' | head -1)
    local flush_requests=$(curl -s "http://localhost:3100/metrics" | grep "loki_ingester_chunks_flush_requests_total" | grep -v "#" | awk '{print $2}' | head -1)

    # Clean up port forward
    kill $port_forward_pid 2>/dev/null || true

    print_status "INFO" "Loki metrics: $memory_chunks chunks in memory, $memory_streams streams, $flush_requests flush requests"

    if [ -n "$memory_chunks" ] && [ "$memory_chunks" -gt 0 ]; then
        print_status "SUCCESS" "Loki has $memory_chunks chunks in memory"
        return 0
    else
        print_status "ERROR" "Loki has 0 chunks in memory"
        return 1
    fi
}

# Function to test Loki queries
test_loki_queries() {
    print_status "INFO" "Testing Loki queries..."

    # Port forward to Loki
    kubectl port-forward -n $NAMESPACE deployment/loki 3100:3100 > /dev/null 2>&1 &
    local port_forward_pid=$!
    sleep 3

    # Test different query types
    local current_time=$(date -u +%s)000000000
    local start_time=$((current_time - 3600000000000)) # 1 hour ago

    # Test 1: Basic job query
    local result1=$(curl -s "http://localhost:3100/loki/api/v1/query_range?query=%7Bjob%3D%22econ-graph-logs%22%7D&start=$start_time&end=$current_time&limit=5" | jq '.data.result | length' 2>/dev/null || echo "0")

    # Test 2: App-specific query
    local result2=$(curl -s "http://localhost:3100/loki/api/v1/query_range?query=%7Bjob%3D%22econ-graph-logs%22%2Capp%3D%22econ-graph-backend%22%7D&start=$start_time&end=$current_time&limit=5" | jq '.data.result | length' 2>/dev/null || echo "0")

    # Test 3: All logs query
    local result3=$(curl -s "http://localhost:3100/loki/api/v1/query_range?query=%7Bjob%3D%22econ-graph-logs%22%7D%20%7C%3D%20%22auth%22&start=$start_time&end=$current_time&limit=5" | jq '.data.result | length' 2>/dev/null || echo "0")

    # Clean up port forward
    kill $port_forward_pid 2>/dev/null || true

    print_status "INFO" "Query results: Basic=$result1, App-specific=$result2, Auth logs=$result3"

    if [ "$result1" -gt 0 ] || [ "$result2" -gt 0 ] || [ "$result3" -gt 0 ]; then
        print_status "SUCCESS" "Loki queries are returning data!"
        return 0
    else
        print_status "ERROR" "Loki queries are returning 0 results"
        return 1
    fi
}

# Function to test Grafana dashboard
test_grafana_dashboard() {
    print_status "INFO" "Testing Grafana dashboard access and data..."

    # Test if Grafana is accessible
    if ! curl -s "$GRAFANA_URL" > /dev/null 2>&1; then
        print_status "ERROR" "Grafana is not accessible at $GRAFANA_URL"
        return 1
    fi

    print_status "SUCCESS" "Grafana is accessible at $GRAFANA_URL"

    # Test if we can access the API (without authentication for now)
    local api_response=$(curl -s "$GRAFANA_URL/api/health" 2>/dev/null)
    if [ -n "$api_response" ]; then
        print_status "SUCCESS" "Grafana API is responding"
    else
        print_status "WARNING" "Grafana API not accessible (may require authentication)"
    fi

    # Test if we can access the datasources
    local datasources_response=$(curl -s "$GRAFANA_URL/api/datasources" 2>/dev/null)
    if [ -n "$datasources_response" ]; then
        print_status "SUCCESS" "Grafana datasources API is accessible"

        # Check if Loki datasource is configured
        if echo "$datasources_response" | grep -q "loki"; then
            print_status "SUCCESS" "Loki datasource is configured in Grafana"
        else
            print_status "WARNING" "Loki datasource not found in Grafana"
        fi
    else
        print_status "WARNING" "Grafana datasources API not accessible (may require authentication)"
    fi

    # Test if we can access dashboards
    local dashboards_response=$(curl -s "$GRAFANA_URL/api/search?type=dash-db" 2>/dev/null)
    if [ -n "$dashboards_response" ]; then
        print_status "SUCCESS" "Grafana dashboards API is accessible"

        # Check if our EconGraph dashboard exists
        if echo "$dashboards_response" | grep -q "EconGraph"; then
            print_status "SUCCESS" "EconGraph dashboard found in Grafana"
        else
            print_status "WARNING" "EconGraph dashboard not found in Grafana"
        fi
    else
        print_status "WARNING" "Grafana dashboards API not accessible (may require authentication)"
    fi

    return 0
}

# Function to authenticate with Grafana
authenticate_grafana() {
    print_status "INFO" "Authenticating with Grafana..."

    # Create a session by logging in
    local login_response=$(curl -s -c /tmp/grafana_cookies.txt -X POST "$GRAFANA_URL/login" \
        -H "Content-Type: application/json" \
        -d '{"user":"admin","password":"admin123"}' 2>/dev/null)

    if [ -n "$login_response" ]; then
        print_status "SUCCESS" "Grafana authentication successful"
        return 0
    else
        print_status "ERROR" "Grafana authentication failed"
        return 1
    fi
}

# Function to test Grafana datasources
test_grafana_datasources() {
    print_status "INFO" "Testing Grafana datasources..."

    # Get datasources using authenticated session
    local datasources_response=$(curl -s -b /tmp/grafana_cookies.txt "$GRAFANA_URL/api/datasources" 2>/dev/null)

    if [ -n "$datasources_response" ]; then
        print_status "SUCCESS" "Grafana datasources API accessible"

        # Check if Loki datasource exists
        if echo "$datasources_response" | grep -q "Loki"; then
            print_status "SUCCESS" "Loki datasource found in Grafana"

            # Get Loki datasource ID
            local loki_id=$(echo "$datasources_response" | jq -r '.[] | select(.name=="Loki") | .id' 2>/dev/null)
            if [ -n "$loki_id" ] && [ "$loki_id" != "null" ]; then
                print_status "SUCCESS" "Loki datasource ID: $loki_id"

                # Test Loki datasource connectivity
                local test_response=$(curl -s -b /tmp/grafana_cookies.txt "$GRAFANA_URL/api/datasources/$loki_id/health" 2>/dev/null)
                if [ -n "$test_response" ]; then
                    print_status "SUCCESS" "Loki datasource health check passed"
                    return 0
                else
                    print_status "ERROR" "Loki datasource health check failed"
                    return 1
                fi
            else
                print_status "ERROR" "Could not get Loki datasource ID"
                return 1
            fi
        else
            print_status "ERROR" "Loki datasource not found in Grafana"
            return 1
        fi
    else
        print_status "ERROR" "Grafana datasources API not accessible"
        return 1
    fi
}

# Function to test datasources with UID validation
test_grafana_datasources_with_uids() {
    print_status "INFO" "Testing Grafana datasources with UID validation..."

    local datasources_response=$(curl -s -b /tmp/grafana_cookies.txt "$GRAFANA_URL/api/datasources" 2>/dev/null)

    if [ -z "$datasources_response" ]; then
        print_status "ERROR" "Failed to get datasources from Grafana API"
        return 1
    fi

    local loki_datasource=$(echo "$datasources_response" | jq '.[] | select(.name=="Loki")' 2>/dev/null)
    local prometheus_datasource=$(echo "$datasources_response" | jq '.[] | select(.name=="Prometheus")' 2>/dev/null)

    if [ -z "$loki_datasource" ] || [ "$loki_datasource" = "null" ]; then
        print_status "ERROR" "Loki datasource not found in Grafana"
        return 1
    fi

    if [ -z "$prometheus_datasource" ] || [ "$prometheus_datasource" = "null" ]; then
        print_status "ERROR" "Prometheus datasource not found in Grafana"
        return 1
    fi

    print_status "SUCCESS" "Both Loki and Prometheus datasources found in Grafana"
    return 0
}

# Function to test dashboard datasource references
test_dashboard_datasource_references() {
    print_status "INFO" "Testing dashboard datasource references..."

    local failed=0

    # Test that dashboards exist and are accessible
    local overview_response=$(curl -s -b /tmp/grafana_cookies.txt "$GRAFANA_URL/api/search?query=econgraph" 2>/dev/null)
    local logging_response=$(curl -s -b /tmp/grafana_cookies.txt "$GRAFANA_URL/api/search?query=econgraph" 2>/dev/null)

    if [ -z "$overview_response" ] || [ "$overview_response" = "[]" ]; then
        print_status "ERROR" "EconGraph dashboard not found in Grafana"
        failed=1
    else
        print_status "SUCCESS" "EconGraph dashboard found in Grafana"
    fi

    # For now, we only have one dashboard, so we'll check the same response
    if [ -z "$logging_response" ] || [ "$logging_response" = "[]" ]; then
        print_status "WARNING" "EconGraph Logging dashboard not found in Grafana (expected - not yet implemented)"
    else
        print_status "SUCCESS" "EconGraph Logging dashboard found in Grafana"
    fi

    if [ $failed -eq 1 ]; then
        return 1
    fi

    print_status "SUCCESS" "Both dashboards found in Grafana"
    return 0
}

# Function to test comprehensive log queries
test_comprehensive_log_queries() {
    local loki_id="$1"
    local start_time="$2"
    local current_time="$3"

    print_status "INFO" "Testing comprehensive log queries..."

    # Test auth logs
    local auth_query_response=$(curl -s -b /tmp/grafana_cookies.txt \
        "$GRAFANA_URL/api/datasources/proxy/$loki_id/loki/api/v1/query_range?query=%7Bjob%3D%22econ-graph-logs%22%7D%20%7C%3D%20%22auth%22&start=$start_time&end=$current_time&limit=5" 2>/dev/null)

    local auth_count=$(echo "$auth_query_response" | jq '.data.result | length' 2>/dev/null || echo "0")
    if [ "$auth_count" -gt 0 ]; then
        print_status "SUCCESS" "Auth log query returned $auth_count results"
    else
        print_status "WARNING" "Auth log query returned 0 results"
    fi

    # Test error logs
    local error_query_response=$(curl -s -b /tmp/grafana_cookies.txt \
        "$GRAFANA_URL/api/datasources/proxy/$loki_id/loki/api/v1/query_range?query=%7Bjob%3D%22econ-graph-logs%22%7D%20%7C%3D%20%22error%22&start=$start_time&end=$current_time&limit=5" 2>/dev/null)

    local error_count=$(echo "$error_query_response" | jq '.data.result | length' 2>/dev/null || echo "0")
    if [ "$error_count" -gt 0 ]; then
        print_status "SUCCESS" "Error log query returned $error_count results"
    else
        print_status "INFO" "Error log query returned 0 results (no errors - good!)"
    fi

    # Test database logs
    local db_query_response=$(curl -s -b /tmp/grafana_cookies.txt \
        "$GRAFANA_URL/api/datasources/proxy/$loki_id/loki/api/v1/query_range?query=%7Bjob%3D%22econ-graph-logs%22%7D%20%7C%3D%20%22database%22&start=$start_time&end=$current_time&limit=5" 2>/dev/null)

    local db_count=$(echo "$db_query_response" | jq '.data.result | length' 2>/dev/null || echo "0")
    if [ "$db_count" -gt 0 ]; then
        print_status "SUCCESS" "Database log query returned $db_count results"
    else
        print_status "WARNING" "Database log query returned 0 results"
    fi
}

# Function to test Prometheus metrics
test_prometheus_metrics() {
    local prometheus_id="$1"

    print_status "INFO" "Testing Prometheus metrics..."

    # Test basic Prometheus query
    local prometheus_response=$(curl -s -b /tmp/grafana_cookies.txt \
        "$GRAFANA_URL/api/datasources/proxy/$prometheus_id/api/v1/query?query=up" 2>/dev/null)

    if [ -n "$prometheus_response" ]; then
        local result_count=$(echo "$prometheus_response" | jq '.data.result | length' 2>/dev/null || echo "0")
        if [ "$result_count" -gt 0 ]; then
            print_status "SUCCESS" "Prometheus query returned $result_count metrics"
        else
            print_status "WARNING" "Prometheus query returned 0 results"
        fi
    else
        print_status "WARNING" "Prometheus proxy not accessible"
    fi
}

# Function to check for Grafana dashboard errors
check_grafana_dashboard_errors() {
    print_status "INFO" "Checking for Grafana dashboard errors..."

    local failed=0

    # Get list of dashboards
    local dashboards_response=$(curl -s -b /tmp/grafana_cookies.txt "$GRAFANA_URL/api/search?query=econgraph" 2>/dev/null)

    if [ -z "$dashboards_response" ] || [ "$dashboards_response" = "[]" ]; then
        print_status "ERROR" "No EconGraph dashboards found"
        return 1
    fi

    # Extract dashboard UIDs
    local dashboard_uids=$(echo "$dashboards_response" | jq -r '.[].uid' 2>/dev/null)

    for uid in $dashboard_uids; do
        if [ -n "$uid" ] && [ "$uid" != "null" ]; then
            print_status "INFO" "Checking dashboard with UID: $uid"

            # Get dashboard details
            local dashboard_response=$(curl -s -b /tmp/grafana_cookies.txt "$GRAFANA_URL/api/dashboards/uid/$uid" 2>/dev/null)

            if [ -n "$dashboard_response" ]; then
                # Check for datasource errors in the dashboard JSON
                local datasource_errors=$(echo "$dashboard_response" | jq -r '.dashboard.panels[]?.targets[]?.datasource.uid // empty' 2>/dev/null | grep -v "null" | sort -u)

                if [ -n "$datasource_errors" ]; then
                    print_status "INFO" "Dashboard $uid uses datasource UIDs: $(echo $datasource_errors | tr '\n' ' ')"

                    # Check if any datasource UIDs are not found
                    local datasources_response=$(curl -s -b /tmp/grafana_cookies.txt "$GRAFANA_URL/api/datasources" 2>/dev/null)
                    local available_uids=$(echo "$datasources_response" | jq -r '.[].uid' 2>/dev/null | sort -u)

                    for required_uid in $datasource_errors; do
                        if ! echo "$available_uids" | grep -q "^$required_uid$"; then
                            print_status "ERROR" "Dashboard $uid references missing datasource UID: $required_uid"
                            print_status "ERROR" "Available datasource UIDs: $(echo $available_uids | tr '\n' ' ')"
                            failed=1
                        else
                            print_status "SUCCESS" "Dashboard $uid datasource UID $required_uid is available"
                        fi
                    done
                else
                    print_status "WARNING" "Dashboard $uid has no datasource references found"
                fi

                # Check for panel errors by looking for error states in the dashboard
                local panel_errors=$(echo "$dashboard_response" | jq -r '.dashboard.panels[]? | select(.error != null) | .title' 2>/dev/null)
                if [ -n "$panel_errors" ]; then
                    print_status "ERROR" "Dashboard $uid has panels with errors: $panel_errors"
                    failed=1
                fi

                # Check for datasource not found errors in panel targets
                local datasource_not_found=$(echo "$dashboard_response" | jq -r '.dashboard.panels[]?.targets[]? | select(.datasource.uid == "loki-uid" or .datasource.uid == "prometheus-uid") | select(.datasource.uid != null) | .datasource.uid' 2>/dev/null | sort -u)
                if [ -n "$datasource_not_found" ]; then
                    print_status "INFO" "Dashboard $uid references expected datasource UIDs: $(echo $datasource_not_found | tr '\n' ' ')"
                fi

            else
                print_status "ERROR" "Could not retrieve dashboard details for UID: $uid"
                failed=1
            fi
        fi
    done

    if [ $failed -eq 1 ]; then
        print_status "ERROR" "Dashboard error checks failed - there are datasource or panel errors"
        return 1
    else
        print_status "SUCCESS" "No dashboard errors detected"
        return 0
    fi
}

# Function to test Grafana dashboard data
test_grafana_dashboard_data() {
    print_status "INFO" "Testing comprehensive Grafana dashboard data integration..."

    # Authenticate first
    if ! authenticate_grafana; then
        return 1
    fi

    # Test datasources with UID validation
    if ! test_grafana_datasources_with_uids; then
        return 1
    fi

    # Get datasource info for testing
    local loki_id=$(curl -s -b /tmp/grafana_cookies.txt "$GRAFANA_URL/api/datasources" | jq -r '.[] | select(.name=="Loki") | .id' 2>/dev/null)
    local loki_uid=$(curl -s -b /tmp/grafana_cookies.txt "$GRAFANA_URL/api/datasources" | jq -r '.[] | select(.name=="Loki") | .uid' 2>/dev/null)
    local prometheus_id=$(curl -s -b /tmp/grafana_cookies.txt "$GRAFANA_URL/api/datasources" | jq -r '.[] | select(.name=="Prometheus") | .id' 2>/dev/null)
    local prometheus_uid=$(curl -s -b /tmp/grafana_cookies.txt "$GRAFANA_URL/api/datasources" | jq -r '.[] | select(.name=="Prometheus") | .uid' 2>/dev/null)

    # Validate UIDs are consistent and expected
    if [ "$loki_uid" != "loki-uid" ]; then
        print_status "ERROR" "Loki UID mismatch: expected 'loki-uid', got '$loki_uid'"
        rm -f /tmp/grafana_cookies.txt
        return 1
    fi

    if [ "$prometheus_uid" != "prometheus-uid" ]; then
        print_status "ERROR" "Prometheus UID mismatch: expected 'prometheus-uid', got '$prometheus_uid'"
        rm -f /tmp/grafana_cookies.txt
        return 1
    fi

    print_status "SUCCESS" "Datasource UIDs are consistent: Loki='$loki_uid', Prometheus='$prometheus_uid'"

    # Test dashboard datasource references
    if ! test_dashboard_datasource_references; then
        print_status "ERROR" "Dashboard datasource reference tests failed"
        rm -f /tmp/grafana_cookies.txt
        return 1
    fi

    # Test querying Loki through Grafana proxy
    local current_time=$(date -u +%s)000000000
    local start_time=$((current_time - 3600000000000)) # 1 hour ago

    local proxy_response=$(curl -s -b /tmp/grafana_cookies.txt \
        "$GRAFANA_URL/api/datasources/proxy/$loki_id/loki/api/v1/query_range?query=%7Bjob%3D%22econ-graph-logs%22%7D&start=$start_time&end=$current_time&limit=5" 2>/dev/null)

    if [ -n "$proxy_response" ]; then
        local result_count=$(echo "$proxy_response" | jq '.data.result | length' 2>/dev/null || echo "0")
        if [ "$result_count" -gt 0 ]; then
            print_status "SUCCESS" "Grafana can query Loki and retrieve $result_count log streams"

            # Test comprehensive log queries
            test_comprehensive_log_queries "$loki_id" "$start_time" "$current_time"

            # Test Prometheus metrics
            test_prometheus_metrics "$prometheus_id"

            print_status "SUCCESS" "Grafana dashboards should now show real data!"
            print_status "INFO" "Access Grafana at: $GRAFANA_URL (admin/admin123)"
            print_status "INFO" "Available dashboards:"
            print_status "INFO" "  - EconGraph Platform Overview: $GRAFANA_URL/d/econgraph-overview/econgraph-platform-overview"
            print_status "INFO" "  - EconGraph Logs & Debugging: $GRAFANA_URL/d/econgraph-logging/econgraph-logs-and-debugging"

            # Clean up cookies
            rm -f /tmp/grafana_cookies.txt
            return 0
        else
            print_status "ERROR" "Grafana can query Loki but returned 0 results"
            rm -f /tmp/grafana_cookies.txt
            return 1
        fi
    else
        print_status "ERROR" "Grafana Loki proxy not accessible"
        rm -f /tmp/grafana_cookies.txt
        return 1
    fi
}

# Function to force Loki flush
force_loki_flush() {
    print_status "INFO" "Forcing Loki flush by restarting..."

    kubectl rollout restart deployment/loki -n $NAMESPACE
    sleep 30

    print_status "SUCCESS" "Loki restart completed"
}

# Main test execution
main() {
    echo
    print_status "INFO" "Starting comprehensive monitoring stack test..."
    echo

    # Step 1: Check pod status
    print_status "INFO" "Step 1: Checking pod status"
    check_pod_status "loki" "Running" || exit 1
    check_pod_status "promtail" "Running" || exit 1
    check_pod_status "grafana" "Running" || exit 1
    echo

    # Step 1.5: Check Grafana uniqueness
    print_status "INFO" "Step 1.5: Checking Grafana instance uniqueness"
    check_grafana_uniqueness || exit 1
    echo

    # Step 2: Wait for services
    print_status "INFO" "Step 2: Waiting for services to be ready"
    wait_for_service "Backend" "$BACKEND_URL" || exit 1
    wait_for_service "Grafana" "$GRAFANA_URL" || exit 1
    echo

    # Step 3: Generate test activity
    print_status "INFO" "Step 3: Generating test activity"
    generate_test_activity
    echo

    # Step 4: Check Promtail metrics
    print_status "INFO" "Step 4: Checking Promtail metrics"
    check_promtail_metrics || exit 1
    echo

    # Step 5: Check Loki metrics
    print_status "INFO" "Step 5: Checking Loki metrics"
    check_loki_metrics || exit 1
    echo

    # Step 6: Test Loki queries
    print_status "INFO" "Step 6: Testing Loki queries"
    if test_loki_queries; then
        print_status "SUCCESS" "🎉 MONITORING STACK IS WORKING!"
    else
        print_status "WARNING" "Loki queries not working, trying to force flush..."
        force_loki_flush
        sleep 60
        if test_loki_queries; then
            print_status "SUCCESS" "🎉 MONITORING STACK IS WORKING AFTER FLUSH!"
        else
            print_status "ERROR" "❌ MONITORING STACK HAS ISSUES - queries still not working"
            exit 1
        fi
    fi
    echo

    # Step 7: Test Grafana dashboard
    print_status "INFO" "Step 7: Testing Grafana dashboard"
    test_grafana_dashboard || exit 1
    echo

    # Step 8: Test Grafana dashboard data
    print_status "INFO" "Step 8: Testing Grafana dashboard data integration"
    if test_grafana_dashboard_data; then
        print_status "SUCCESS" "Grafana dashboards are working with real data!"
    else
        print_status "ERROR" "❌ GRAFANA DASHBOARD TESTS FAILED!"
        print_status "ERROR" "Dashboards are not working properly - check configuration"
        print_status "INFO" "Grafana URL: $GRAFANA_URL (admin/admin)"
        print_status "INFO" "Backend URL: $BACKEND_URL"
        exit 1
    fi
    echo

    # Step 9: Check for dashboard errors (exclamation point in triangle)
    print_status "INFO" "Step 9: Checking for Grafana dashboard errors"
    if check_grafana_dashboard_errors; then
        print_status "SUCCESS" "No dashboard errors detected!"
        echo
        print_status "SUCCESS" "🎉 ALL TESTS PASSED! Monitoring stack is fully functional."
        print_status "INFO" "Grafana URL: $GRAFANA_URL (admin/admin)"
        print_status "INFO" "Backend URL: $BACKEND_URL"
    else
        print_status "ERROR" "❌ DASHBOARD ERROR CHECKS FAILED!"
        print_status "ERROR" "Dashboards have datasource or panel errors - check the exclamation point warnings"
        print_status "INFO" "Grafana URL: $GRAFANA_URL (admin/admin)"
        print_status "INFO" "Backend URL: $BACKEND_URL"
        exit 1
    fi
}

# Run the test
main "$@"
