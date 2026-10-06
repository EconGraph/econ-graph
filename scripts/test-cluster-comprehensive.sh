#!/bin/bash

# Comprehensive cluster test script including Promtail-Loki connectivity
# This script tests all aspects of the cluster including log collection

set -e

echo "🧪 Comprehensive Cluster Testing"
echo "================================"

# Check if ingress port-forward is running
if ! curl -s http://localhost:8080/health > /dev/null 2>&1; then
    echo "❌ Ingress port-forward not running. Start it with:"
    echo "   kubectl port-forward service/ingress-nginx-controller 8080:80 -n ingress-nginx"
    exit 1
fi

echo "✅ Ingress port-forward is running"
echo ""

# Test function
test_endpoint() {
    local path="$1"
    local expected_content="$2"
    local description="$3"
    local expected_status="${4:-200}"

    echo "🔍 Testing $description: $path"

    # Get both response body and HTTP status code
    response=$(curl -s -w "\n%{http_code}" "http://localhost:8080$path")

    # Extract HTTP status code (last line)
    http_status=$(echo "$response" | tail -n1)
    # Extract response body (all but last line)
    response_body=$(echo "$response" | sed '$d')

    echo "  📊 HTTP Status: $http_status (expected: $expected_status)"

    # Check HTTP status code first
    if [ "$http_status" = "$expected_status" ]; then
        # If status is OK, also check content
        if [ "$http_status" = "200" ] && echo "$response_body" | grep -q "$expected_content"; then
            echo "  ✅ PASS - HTTP $http_status and content matches"
            return 0
        elif [ "$http_status" = "200" ]; then
            echo "  ⚠️  HTTP 200 but content doesn't match expected pattern"
            echo "  Expected: $expected_content"
            echo "  Got: $(echo "$response_body" | head -1 | cut -c1-100)..."
            return 1
        else
            # For non-200 status codes, we don't check content
            echo "  ✅ PASS - HTTP $http_status (as expected)"
            return 0
        fi
    else
        echo "  ❌ FAIL - HTTP $http_status (expected $expected_status)"
        echo "  Response: $(echo "$response_body" | head -1 | cut -c1-100)..."
        return 1
    fi
}

# Test results
tests_passed=0
tests_total=0

echo "🌐 Testing Ingress Routing"
echo "-------------------------"

# Test Frontend (root path)
tests_total=$((tests_total + 1))
if test_endpoint "/" "EconGraph - Economic Data Visualization" "Frontend (root path)"; then
    tests_passed=$((tests_passed + 1))
fi

# Test Admin UI
tests_total=$((tests_total + 1))
if test_endpoint "/admin" "EconGraph Admin - System Administration" "Admin UI"; then
    tests_passed=$((tests_passed + 1))
fi

# Test Backend Health
tests_total=$((tests_total + 1))
if test_endpoint "/health" "healthy" "Backend Health"; then
    tests_passed=$((tests_passed + 1))
fi

# Test GraphQL (should return GraphQL schema or error)
tests_total=$((tests_total + 1))
echo "🔍 Testing GraphQL: /graphql"
response=$(curl -s -w "\n%{http_code}" -X POST -H "Content-Type: application/json" -d '{"query":"{ __schema { queryType { name } } }"}' "http://localhost:8080/graphql")

# Extract HTTP status code (last line)
http_status=$(echo "$response" | tail -n1)
# Extract response body (all but last line)
response_body=$(echo "$response" | sed '$d')

echo "  📊 HTTP Status: $http_status (expected: 200)"

if [ "$http_status" = "200" ] && echo "$response_body" | grep -q -E "(queryType|__schema|Query)"; then
    echo "  ✅ PASS - HTTP $http_status and GraphQL schema returned"
    tests_passed=$((tests_passed + 1))
else
    echo "  ❌ FAIL - GraphQL endpoint not responding correctly"
    echo "  HTTP Status: $http_status"
    echo "  Response: $(echo "$response_body" | head -1 | cut -c1-100)..."
fi

# Test Backend API (expects internal server error for root /api path)
tests_total=$((tests_total + 1))
if test_endpoint "/api" "Internal server error" "Backend API" "500"; then
    tests_passed=$((tests_passed + 1))
fi

# Grafana is ClusterIP-only (not on the ingress); test it via port-forward instead.
tests_total=$((tests_total + 1))
echo "🔍 Testing Grafana (via port-forward, not the public ingress)"
kubectl port-forward service/grafana-service 3000:3000 -n econ-graph > /dev/null 2>&1 &
grafana_pf_pid=$!
sleep 2
http_status=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:3000/login" || echo "000")
kill $grafana_pf_pid 2>/dev/null || true
wait $grafana_pf_pid 2>/dev/null || true

echo "  📊 HTTP Status: $http_status (expected: 200)"
if [ "$http_status" = "200" ]; then
    echo "  ✅ PASS - Grafana login page reachable via port-forward"
    tests_passed=$((tests_passed + 1))
else
    echo "  ❌ FAIL - Grafana not reachable via port-forward"
fi

echo ""
echo "📊 Testing Promtail-Loki Log Collection"
echo "--------------------------------------"

# Test if Loki is accessible
tests_total=$((tests_total + 1))
echo "🔍 Testing Loki connectivity"
if kubectl exec grafana-0 -n econ-graph -- curl -s http://loki-service:3100/ready > /dev/null 2>&1; then
    echo "  ✅ PASS - Loki is accessible and ready"
    tests_passed=$((tests_passed + 1))
else
    echo "  ❌ FAIL - Loki is not accessible or not ready"
fi

# Test if Loki has any labels (indicating data ingestion)
tests_total=$((tests_total + 1))
echo "🔍 Testing Loki data ingestion"
loki_labels=$(kubectl exec grafana-0 -n econ-graph -- curl -s "http://loki-service:3100/loki/api/v1/label" 2>/dev/null | jq -r '.data | length' 2>/dev/null || echo "0")
if [ "$loki_labels" -gt 0 ]; then
    echo "  ✅ PASS - Loki has $loki_labels labels (data is being ingested)"
    tests_passed=$((tests_passed + 1))
else
    echo "  ❌ FAIL - Loki has no labels (no data being ingested)"
fi

# Test if Promtail is running and healthy
tests_total=$((tests_total + 1))
echo "🔍 Testing Promtail health"
promtail_pod=$(kubectl get pods -n econ-graph | grep promtail | awk '{print $1}' | head -1)
if [ -n "$promtail_pod" ] && kubectl get pod "$promtail_pod" -n econ-graph | grep -q "Running"; then
    echo "  ✅ PASS - Promtail pod $promtail_pod is running"
    tests_passed=$((tests_passed + 1))
else
    echo "  ❌ FAIL - Promtail pod is not running"
fi

# Test if Promtail is collecting logs from backend
tests_total=$((tests_total + 1))
echo "🔍 Testing Promtail log collection"
if kubectl exec grafana-0 -n econ-graph -- curl -s "http://loki-service:3100/loki/api/v1/label/job/values" 2>/dev/null | jq -r '.data[]' 2>/dev/null | grep -q "backend-logs"; then
    echo "  ✅ PASS - Promtail is collecting backend logs"
    tests_passed=$((tests_passed + 1))
else
    echo "  ❌ FAIL - Promtail is not collecting backend logs"
fi

# Test if logs dashboard will have data
tests_total=$((tests_total + 1))
echo "🔍 Testing logs dashboard data availability"
if kubectl exec grafana-0 -n econ-graph -- curl -s "http://loki-service:3100/loki/api/v1/label/job/values" 2>/dev/null | jq -r '.data[]' 2>/dev/null | grep -q "backend-logs"; then
    echo "  ✅ PASS - Backend logs are available for logs dashboard"
    tests_passed=$((tests_passed + 1))
else
    echo "  ❌ FAIL - Backend logs are not available for logs dashboard"
fi

echo ""
echo "📊 Test Results: $tests_passed/$tests_total tests passed"

if [ $tests_passed -eq $tests_total ]; then
    echo "🎉 All tests passed! Cluster is fully functional including log collection."
    echo ""
    echo "📋 Available Services:"
    echo "  • Frontend: http://localhost:8080/"
    echo "  • Admin UI: http://localhost:8080/admin"
    echo "  • Grafana: kubectl port-forward service/grafana-service 3000:3000 -n econ-graph"
    echo "  • Backend API: http://localhost:8080/api"
    echo "  • GraphQL: http://localhost:8080/graphql"
    echo ""
    echo "📊 Grafana Dashboards:"
    echo "  • EconGraph Platform Overview"
    echo "  • Database Statistics"
    echo "  • Crawler Status"
    echo "  • Backend Metrics"
    echo "  • Logs & Debugging (now with data!)"
    exit 0
else
    echo "⚠️  Some tests failed. Cluster needs investigation."
    exit 1
fi
