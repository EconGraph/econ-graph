#!/bin/bash

# Populate Data Source Catalogs Script
# This script creates a dockerized database, runs the catalog crawler,
# and generates a diesel migration with the populated data.

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BACKEND_DIR="$PROJECT_ROOT"

# Default values
CONTAINER_NAME="econ-graph-catalog-db"
DB_PORT="5434"
DB_NAME="catalog_db"
DB_USER="postgres"
DB_PASSWORD="catalog_password"
SERIES_COUNT=5
DRY_RUN=false
SKIP_DATA_DOWNLOAD=false
OUTPUT_FILE="catalog_migration.sql"
API_KEYS=()

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Usage function
usage() {
    cat << EOF
Usage: $0 [OPTIONS]

This script creates a dockerized database, runs the catalog crawler against it,
and generates a diesel migration with the populated catalog data.

OPTIONS:
    -h, --help              Show this help message
    -p, --port PORT         Database port (default: 5434)
    -s, --series-count N    Number of series to download per source (default: 5)
    -d, --dry-run           Dry run mode - don't download actual data
    -k, --skip-data         Skip downloading actual series data
    -o, --output FILE       Output migration file (default: catalog_migration.sql)
    -a, --api-key SOURCE=KEY
                           API key for data source (can be used multiple times)
    --cleanup               Clean up existing container before starting

EXAMPLES:
    $0 --dry-run
    $0 --api-key FRED=your_fred_key --api-key BLS=your_bls_key
    $0 --series-count 10 --output my_catalog.sql

EOF
}

# Parse command line arguments
parse_args() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            -h|--help)
                usage
                exit 0
                ;;
            -p|--port)
                DB_PORT="$2"
                shift 2
                ;;
            -s|--series-count)
                SERIES_COUNT="$2"
                shift 2
                ;;
            -d|--dry-run)
                DRY_RUN=true
                shift
                ;;
            -k|--skip-data)
                SKIP_DATA_DOWNLOAD=true
                shift
                ;;
            -o|--output)
                OUTPUT_FILE="$2"
                shift 2
                ;;
            -a|--api-key)
                API_KEYS+=("$2")
                shift 2
                ;;
            --cleanup)
                CLEANUP=true
                shift
                ;;
            *)
                log_error "Unknown option: $1"
                usage
                exit 1
                ;;
        esac
    done
}

# Cleanup function
cleanup() {
    log_info "Cleaning up..."

    # Stop and remove container if it exists
    if docker ps -a --format 'table {{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
        log_info "Stopping and removing existing container..."
        docker stop "$CONTAINER_NAME" > /dev/null 2>&1 || true
        docker rm "$CONTAINER_NAME" > /dev/null 2>&1 || true
    fi

    # Remove migration file if it exists
    if [[ -f "$OUTPUT_FILE" ]]; then
        log_info "Removing existing output file..."
        rm -f "$OUTPUT_FILE"
    fi
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."

    # Check if Docker is installed and running
    if ! command -v docker &> /dev/null; then
        log_error "Docker is not installed. Please install Docker first."
        exit 1
    fi

    if ! docker info &> /dev/null; then
        log_error "Docker is not running. Please start Docker first."
        exit 1
    fi

    # Check if Rust is installed
    if ! command -v cargo &> /dev/null; then
        log_error "Cargo is not installed. Please install Rust first."
        exit 1
    fi

    # Check if we're in the right directory
    if [[ ! -f "$BACKEND_DIR/Cargo.toml" ]]; then
        log_error "Cargo.toml not found. Please run this script from the backend directory."
        exit 1
    fi

    log_success "Prerequisites check passed"
}

# Start database container
start_database() {
    log_info "Starting PostgreSQL container..."

    # Start the container
    docker run -d \
        --name "$CONTAINER_NAME" \
        -e POSTGRES_DB="$DB_NAME" \
        -e POSTGRES_USER="$DB_USER" \
        -e POSTGRES_PASSWORD="$DB_PASSWORD" \
        -p "$DB_PORT:5432" \
        postgres:17 > /dev/null

    # Wait for database to be ready
    log_info "Waiting for database to be ready..."
    local max_attempts=30
    local attempt=0

    while [[ $attempt -lt $max_attempts ]]; do
        if docker exec "$CONTAINER_NAME" pg_isready -U "$DB_USER" -d "$DB_NAME" > /dev/null 2>&1; then
            log_success "Database is ready"
            return 0
        fi

        ((attempt++))
        sleep 2
    done

    log_error "Database failed to start within expected time"
    return 1
}

# Run migrations
run_migrations() {
    log_info "Running database migrations..."

    local database_url="postgresql://${DB_USER}:${DB_PASSWORD}@localhost:${DB_PORT}/${DB_NAME}"

    cd "$BACKEND_DIR"

    # Set DATABASE_URL for diesel
    export DATABASE_URL="$database_url"

    # Run migrations
    if ! diesel migration run --migration-dir migrations; then
        log_error "Failed to run migrations"
        return 1
    fi

    log_success "Migrations completed successfully"
}

# Build the catalog crawler
build_crawler() {
    log_info "Building catalog crawler..."

    cd "$BACKEND_DIR"

    if ! cargo build --release --bin catalog_crawler; then
        log_error "Failed to build catalog crawler"
        return 1
    fi

    log_success "Catalog crawler built successfully"
}

# Run catalog crawler
run_catalog_crawler() {
    log_info "Running catalog crawler..."

    local database_url="postgresql://${DB_USER}:${DB_PASSWORD}@localhost:${DB_PORT}/${DB_NAME}"
    local crawler_args=(
        "crawl-all"
        "--database-url" "$database_url"
        "--series-count" "$SERIES_COUNT"
    )

    # Add dry run flag if specified
    if [[ "$DRY_RUN" == "true" ]]; then
        crawler_args+=("--dry-run")
    fi

    # Add skip data download flag if specified
    if [[ "$SKIP_DATA_DOWNLOAD" == "true" ]]; then
        crawler_args+=("--skip-data-download")
    fi

    # Add API keys
    for api_key in "${API_KEYS[@]}"; do
        crawler_args+=("--api-key" "$api_key")
    done

    # Run the crawler
    if ! cargo run --release --bin catalog_crawler -- "${crawler_args[@]}"; then
        log_error "Catalog crawler failed"
        return 1
    fi

    log_success "Catalog crawler completed successfully"
}

# Export catalog data to migration
export_catalog() {
    log_info "Exporting catalog data to migration file..."

    local database_url="postgresql://${DB_USER}:${DB_PASSWORD}@localhost:${DB_PORT}/${DB_NAME}"

    if ! cargo run --release --bin catalog_crawler -- export-catalog \
        --database-url "$database_url" \
        --output-file "$OUTPUT_FILE"; then
        log_error "Failed to export catalog data"
        return 1
    fi

    log_success "Catalog data exported to $OUTPUT_FILE"
}

# Generate diesel migration
generate_migration() {
    log_info "Generating diesel migration..."

    local timestamp=$(date +"%Y-%m-%d-%H%M%S")
    local migration_name="populate_catalog_data_${timestamp}"

    cd "$BACKEND_DIR"

    # Create migration directory
    local migration_dir="migrations/${timestamp}_populate_catalog_data"
    mkdir -p "$migration_dir"

    # Create up.sql
    cat > "$migration_dir/up.sql" << EOF
-- Migration: Populate catalog data from real data sources
-- Generated: $(date)
-- Source: catalog_crawler binary

-- Clear existing catalog data (optional - comment out if you want to keep existing data)
-- DELETE FROM series_metadata;

EOF

    # Append the exported catalog data
    if [[ -f "$OUTPUT_FILE" ]]; then
        cat "$OUTPUT_FILE" >> "$migration_dir/up.sql"
    else
        log_warning "No catalog data file found, creating empty migration"
    fi

    # Create down.sql
    cat > "$migration_dir/down.sql" << EOF
-- Rollback: Remove populated catalog data
-- This will remove all series metadata entries that were added by this migration

-- Note: This is a destructive operation
-- Uncomment the following line if you want to rollback all catalog data
-- DELETE FROM series_metadata WHERE created_at >= '$(date -u +"%Y-%m-%d %H:%M:%S")';
EOF

    log_success "Migration created: $migration_dir"

    # Show summary
    if [[ -f "$OUTPUT_FILE" ]]; then
        local series_count=$(grep -c "INSERT INTO series_metadata" "$OUTPUT_FILE" 2>/dev/null || echo "0")
        log_info "Migration contains $series_count series metadata entries"
    fi
}

# Main execution
main() {
    log_info "Starting catalog population process..."

    # Parse arguments
    parse_args "$@"

    # Show configuration
    log_info "Configuration:"
    log_info "  Database port: $DB_PORT"
    log_info "  Series count: $SERIES_COUNT"
    log_info "  Dry run: $DRY_RUN"
    log_info "  Skip data download: $SKIP_DATA_DOWNLOAD"
    log_info "  Output file: $OUTPUT_FILE"
    log_info "  API keys: ${#API_KEYS[@]}"

    # Cleanup if requested
    if [[ "${CLEANUP:-false}" == "true" ]]; then
        cleanup
    fi

    # Check prerequisites
    check_prerequisites

    # Start database
    start_database

    # Set up cleanup trap
    trap cleanup EXIT

    # Run migrations
    run_migrations

    # Build crawler
    build_crawler

    # Run catalog crawler
    run_catalog_crawler

    # Export catalog data
    export_catalog

    # Generate migration
    generate_migration

    log_success "Catalog population process completed successfully!"
    log_info "Migration file: $OUTPUT_FILE"
    log_info "To apply the migration, run: diesel migration run"
}

# Run main function with all arguments
main "$@"
