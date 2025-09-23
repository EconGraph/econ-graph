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

# Test Grafana (should redirect to login and serve proper Grafana login page)
tests_total=$((tests_total + 1))
echo "🔍 Testing Grafana: /grafana"
response=$(curl -s -w "\n%{http_code}" "http://localhost:8080/grafana")

# Extract HTTP status code (last line)
http_status=$(echo "$response" | tail -n1)
# Extract response body (all but last line)
response_body=$(echo "$response" | sed '$d')

echo "  📊 HTTP Status: $http_status (expected: 302)"

if [ "$http_status" = "302" ]; then
    # Check if it redirects to login
    location=$(curl -s -I "http://localhost:8080/grafana" | grep -i "location:" | cut -d' ' -f2- | tr -d '\r\n')
    echo "  📍 Redirect Location: $location"

    if echo "$location" | grep -q "login"; then
        # Test the login page to make sure it's Grafana, not frontend
        # Use the actual redirect location from the response
        login_url="http://localhost:8080$location"
        login_response=$(curl -s "$login_url")
        if echo "$login_response" | grep -q "Grafana" && ! echo "$login_response" | grep -q "EconGraph"; then
            echo "  ✅ PASS - Grafana redirects to login and serves proper Grafana login page"
            tests_passed=$((tests_passed + 1))
        else
            echo "  ❌ FAIL - Grafana login page shows wrong content (frontend instead of Grafana)"
            echo "  Response: $(echo "$login_response" | head -1 | cut -c1-100)..."
        fi
    else
        echo "  ❌ FAIL - Grafana doesn't redirect to login page"
    fi
else
    echo "  ❌ FAIL - Grafana doesn't return 302 redirect"
    echo "  HTTP Status: $http_status"
    echo "  Response: $(echo "$response_body" | head -1 | cut -c1-100)..."
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
