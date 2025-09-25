#!/bin/bash

# Copyright (c) 2024 EconGraph. All rights reserved.
# Licensed under the Microsoft Reference Source License (MS-RSL).
# See LICENSE file for complete terms and conditions.

# Local E2E Test Runner
# This script replicates the CI environment locally for testing e2e fixes

set -e

echo "🚀 Starting local e2e test environment..."

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

# Check if required tools are installed
command -v docker-compose >/dev/null 2>&1 || { print_error "docker-compose is required but not installed. Aborting."; exit 1; }
command -v cargo >/dev/null 2>&1 || { print_error "Rust/cargo is required but not installed. Aborting."; exit 1; }
command -v npm >/dev/null 2>&1 || { print_error "Node.js/npm is required but not installed. Aborting."; exit 1; }

print_status "All required tools are available"

# Create a temporary directory for the test
TEST_DIR=$(mktemp -d)
print_status "Created test directory: $TEST_DIR"

# Copy the project to the test directory
print_status "Copying project files..."
cp -r . "$TEST_DIR/"
cd "$TEST_DIR"

# Create a docker-compose file for local testing
print_status "Creating docker-compose configuration..."
cat > docker-compose.test.yml << 'EOF'
version: '3.8'

services:
  postgres:
    image: postgres:17.6
    environment:
      POSTGRES_PASSWORD: password
      POSTGRES_USER: postgres
      POSTGRES_DB: econ_graph_test
    ports:
      - "5441:5432"
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 10s
      timeout: 5s
      retries: 5
    volumes:
      - postgres_data:/var/lib/postgresql/data

volumes:
  postgres_data:
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
    if [ $i -eq 30 ]; then
        print_error "PostgreSQL failed to start within 60 seconds"
        exit 1
    fi
    print_status "Waiting for PostgreSQL... ($i/30)"
    sleep 2
done

# Set up the backend
print_status "Setting up backend..."
cd backend

# Install diesel CLI
print_status "Installing diesel CLI..."
cargo install diesel_cli --no-default-features --features postgres

# Add cargo bin to PATH
export PATH="$HOME/.cargo/bin:$PATH"

# Set up database
print_status "Setting up database..."
export DATABASE_URL="postgresql://postgres:password@localhost:5441/econ_graph_test"
diesel migration run

# Build the backend
print_status "Building backend..."
cargo build --release --bin econ-graph-backend

# Start the backend
print_status "Starting backend server..."
./target/release/econ-graph-backend &
BACKEND_PID=$!
echo "Backend PID: $BACKEND_PID"

# Wait for backend to be ready
print_status "Waiting for backend to be ready..."
for i in {1..30}; do
    if curl -f http://localhost:9876/health > /dev/null 2>&1; then
        print_success "Backend is ready!"
        break
    fi
    if [ $i -eq 30 ]; then
        print_error "Backend failed to start within 60 seconds"
        kill $BACKEND_PID 2>/dev/null || true
        exit 1
    fi
    print_status "Waiting for backend... ($i/30)"
    sleep 2
done

# Set up the frontend
print_status "Setting up frontend..."
cd ../frontend

# Install dependencies
print_status "Installing frontend dependencies..."
npm ci

# Build frontend
print_status "Building frontend..."
npm run build

# Install dev-server dependencies
print_status "Installing dev-server dependencies..."
cd dev-server
npm ci
cd ..

# Install Playwright browsers
print_status "Installing Playwright browsers..."
npx playwright install --with-deps

# Run the e2e tests
print_status "Running e2e tests..."
npm run test:e2e

# Cleanup function
cleanup() {
    print_status "Cleaning up..."
    if [ ! -z "$BACKEND_PID" ]; then
        kill $BACKEND_PID 2>/dev/null || true
    fi
    cd "$TEST_DIR"
    docker-compose -f docker-compose.test.yml down -v
    cd ..
    rm -rf "$TEST_DIR"
    print_success "Cleanup complete"
}

# Set up trap to cleanup on exit
trap cleanup EXIT

print_success "E2E tests completed successfully!"
print_status "Backend is running on http://localhost:9876"
print_status "Press Ctrl+C to stop the test environment"
