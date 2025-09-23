#!/bin/bash

# Test script to verify ingress routing is working correctly
# This script tests that different paths return the correct services

set -e

echo "🧪 Testing Ingress Routing Configuration"
echo "========================================"

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

    echo "🔍 Testing $description: $path"

    response=$(curl -s -H "Host: admin.econ-graph.local" "http://localhost:8080$path")

    if echo "$response" | grep -q "$expected_content"; then
        echo "  ✅ PASS - Correct content returned"
        return 0
    else
        echo "  ❌ FAIL - Wrong content returned"
        echo "  Expected: $expected_content"
        echo "  Got: $(echo "$response" | head -1 | cut -c1-50)..."
        return 1
    fi
}

# Test results
tests_passed=0
tests_total=0

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
response=$(curl -s -X POST -H "Host: admin.econ-graph.local" -H "Content-Type: application/json" -d '{"query":"{ __schema { queryType { name } } }"}' "http://localhost:8080/graphql")
if echo "$response" | grep -q -E "(queryType|__schema|Query)"; then
    echo "  ✅ PASS - GraphQL endpoint responding correctly"
    tests_passed=$((tests_passed + 1))
else
    echo "  ❌ FAIL - GraphQL endpoint not responding correctly"
    echo "  Response: $response"
fi

# Test Backend API (expects internal server error for root /api path)
tests_total=$((tests_total + 1))
if test_endpoint "/api" "Internal server error" "Backend API"; then
    tests_passed=$((tests_passed + 1))
fi

echo ""
echo "📊 Test Results: $tests_passed/$tests_total tests passed"

if [ $tests_passed -eq $tests_total ]; then
    echo "🎉 All tests passed! Ingress routing is working correctly."
    exit 0
else
    echo "⚠️  Some tests failed. Ingress routing needs investigation."
    exit 1
fi
