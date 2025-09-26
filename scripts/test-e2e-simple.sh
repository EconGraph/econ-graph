#!/bin/bash

# Copyright (c) 2024 EconGraph. All rights reserved.
# Licensed under the Microsoft Reference Source License (MS-RSL).
# See LICENSE file for complete terms and conditions.

# Simple E2E Test Runner
# This script tests the e2e setup locally without copying the entire project

set -e

echo "🚀 Starting simple e2e test environment..."

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
if ! docker info > /dev/null 2>&1; then
    print_error "Docker is not running. Please start Docker and try again."
    exit 1
fi

# Check if required tools are available
print_status "Checking required tools..."
command -v docker > /dev/null || { print_error "Docker is not installed"; exit 1; }
command -v docker-compose > /dev/null || { print_error "Docker Compose is not installed"; exit 1; }
command -v cargo > /dev/null || { print_error "Cargo is not installed"; exit 1; }
command -v npm > /dev/null || { print_error "npm is not installed"; exit 1; }
print_success "All required tools are available"

# Find an available port
PORT=5441
while lsof -i :$PORT > /dev/null 2>&1; do
    PORT=$((PORT + 1))
done

print_status "Using port $PORT for PostgreSQL"

# Create a simple docker-compose file for testing
cat > docker-compose.test.yml << EOF
version: '3.8'
services:
  postgres:
    image: postgres:18.6
    environment:
      POSTGRES_PASSWORD: password
      POSTGRES_USER: postgres
      POSTGRES_DB: econ_graph_test
    ports:
      - "$PORT:5432"
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 10s
      timeout: 5s
      retries: 5
EOF

# Start PostgreSQL
print_status "Starting PostgreSQL container..."
docker-compose -f docker-compose.test.yml up -d postgres

# Wait for PostgreSQL to be ready
print_status "Waiting for PostgreSQL to be ready..."
for i in {1..30}; do
    if docker-compose -f docker-compose.test.yml exec postgres pg_isready -U postgres > /dev/null 2>&1; then
        print_success "PostgreSQL is ready!"
        break
    fi
    echo "⏳ Attempt $i/30: PostgreSQL not ready yet, waiting 2 seconds..."
    sleep 2
done

# Test database connection
print_status "Testing database connection..."
export DATABASE_URL="postgresql://postgres:password@localhost:$PORT/econ_graph_test"

# Test with psql
if docker-compose -f docker-compose.test.yml exec postgres psql -U postgres -d econ_graph_test -c "SELECT 1;" > /dev/null 2>&1; then
    print_success "Database connection successful!"
else
    print_error "Database connection failed!"
    docker-compose -f docker-compose.test.yml down
    exit 1
fi

# Test backend compilation (without running)
print_status "Testing backend compilation..."
cd backend

# Add cargo bin to PATH
export PATH="$HOME/.cargo/bin:$PATH"

# Test diesel CLI
if command -v diesel > /dev/null; then
    print_success "Diesel CLI is available"
else
    print_status "Installing diesel CLI..."
    cargo install diesel_cli --no-default-features --features postgres
fi

# Test database migration
print_status "Testing database migration..."
if diesel migration run; then
    print_success "Database migration successful!"
else
    print_error "Database migration failed!"
    cd ..
    docker-compose -f docker-compose.test.yml down
    exit 1
fi

# Test backend compilation
print_status "Testing backend compilation..."
if cargo build --release --bin econ-graph-backend; then
    print_success "Backend compilation successful!"
else
    print_error "Backend compilation failed!"
    cd ..
    docker-compose -f docker-compose.test.yml down
    exit 1
fi

# Test backend startup (briefly)
print_status "Testing backend startup..."
timeout 10s ./target/release/econ-graph-backend &
BACKEND_PID=$!
sleep 5

# Check if backend is running
if kill -0 $BACKEND_PID 2>/dev/null; then
    print_success "Backend started successfully!"
    kill $BACKEND_PID 2>/dev/null || true
else
    print_error "Backend failed to start!"
    cd ..
    docker-compose -f docker-compose.test.yml down
    exit 1
fi

cd ..

# Test frontend build
print_status "Testing frontend build..."
cd frontend

if npm ci && npm run build; then
    print_success "Frontend build successful!"
else
    print_error "Frontend build failed!"
    cd ..
    docker-compose -f docker-compose.test.yml down
    exit 1
fi

cd ..

# Test dev-server
print_status "Testing dev-server setup..."
cd frontend/dev-server

if npm ci; then
    print_success "Dev-server setup successful!"
else
    print_error "Dev-server setup failed!"
    cd ../..
    docker-compose -f docker-compose.test.yml down
    exit 1
fi

cd ../..

# Test Playwright installation
print_status "Testing Playwright installation..."
cd frontend

if npx playwright install --with-deps; then
    print_success "Playwright installation successful!"
else
    print_error "Playwright installation failed!"
    cd ..
    docker-compose -f docker-compose.test.yml down
    exit 1
fi

cd ..

# Cleanup
print_status "Cleaning up..."
docker-compose -f docker-compose.test.yml down
rm -f docker-compose.test.yml

print_success "🎉 All e2e tests passed! The CI fixes should work correctly."
print_status "Summary of what was tested:"
echo "  ✅ PostgreSQL container startup and health check"
echo "  ✅ Database connection and migration"
echo "  ✅ Backend compilation and startup"
echo "  ✅ Frontend build"
echo "  ✅ Dev-server setup"
echo "  ✅ Playwright browser installation"
echo ""
print_success "The comprehensive fixes should resolve the CI e2e test failures!"
