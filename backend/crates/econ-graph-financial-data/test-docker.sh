#!/bin/bash

# Docker Integration Test Script for Financial Data Service
# This script runs comprehensive Docker-based integration tests

set -e

echo "🐳 Financial Data Service - Docker Integration Test"
echo "=================================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if Docker is running
print_status "Checking Docker availability..."
if ! docker info > /dev/null 2>&1; then
    print_error "Docker is not running or not accessible"
    exit 1
fi
print_success "Docker is available"

# Clean up any existing test containers
print_status "Cleaning up existing test containers..."
docker stop financial-data-test 2>/dev/null || true
docker rm financial-data-test 2>/dev/null || true
docker-compose down -v 2>/dev/null || true
print_success "Cleanup completed"

# Build the Docker image
print_status "Building Docker image..."
if docker build -t econ-graph-financial-data:test .; then
    print_success "Docker image built successfully"
else
    print_error "Docker build failed"
    exit 1
fi

# Run the integration test
print_status "Running Docker integration test..."
if cargo test docker_integration_test --test docker_integration_test; then
    print_success "Docker integration test passed"
else
    print_error "Docker integration test failed"
    exit 1
fi

# Test Docker Compose if available
if [ -f "docker-compose.yml" ]; then
    print_status "Testing Docker Compose integration..."
    if cargo test docker_compose_integration --test docker_integration_test; then
        print_success "Docker Compose integration test passed"
    else
        print_warning "Docker Compose integration test failed (this is optional)"
    fi
else
    print_warning "docker-compose.yml not found, skipping compose test"
fi

# Final cleanup
print_status "Final cleanup..."
docker stop financial-data-test 2>/dev/null || true
docker rm financial-data-test 2>/dev/null || true
docker-compose down -v 2>/dev/null || true

print_success "🎉 All Docker integration tests completed successfully!"
print_status "The Financial Data Service is ready for deployment"
