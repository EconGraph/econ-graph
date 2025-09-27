#!/bin/bash
# Centralized Clippy Configuration for EconGraph
# This script ensures consistent clippy settings across pre-commit, CI, and local development
#
# Usage:
#   ./scripts/run-clippy.sh          # Run clippy on all crates
#   ./scripts/run-clippy.sh --crate  # Run clippy on current crate only
#   ./scripts/run-clippy.sh --help   # Show help

set -euo pipefail

# Enhanced clippy configuration with documentation requirements
CLIPPY_ARGS=(
    --all-targets
    --all-features
    --
    -D warnings
    # Allow common warnings that are project-specific
    -A unused_imports
    -A dead_code
    -A clippy::empty_line_after_doc_comments
    -A clippy::explicit_auto_deref
    -A clippy::unused_enumerate_index
    -A clippy::manual_range_contains
    -A clippy::unnecessary_map_or
    -A clippy::inherent_to_string
    -A clippy::too_many_arguments
    -A clippy::uninlined_format_args
    -A clippy::needless_borrow
    -A unused_variables
    -A clippy::if_same_then_else
    -A clippy::assertions_on_constants
    -A clippy::useless_vec
    -A clippy::overly_complex_bool_expr
    -A clippy::manual_clamp
    -A clippy::upper_case_acronyms
    -A unused_must_use
    -A unused_mut
    -A clippy::derivable_impls
    # Enhanced documentation requirements
    -D clippy::missing_docs_in_private_items
    -D clippy::missing_errors_doc
    -D clippy::missing_panics_doc
    -D clippy::missing_safety_doc
    -D clippy::doc_markdown
    -D clippy::doc_link_with_quotes
)

# Parse command line arguments
CRATE_ONLY=false
SHOW_HELP=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --crate)
            CRATE_ONLY=true
            shift
            ;;
        --help|-h)
            SHOW_HELP=true
            shift
            ;;
        *)
            echo "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Show help if requested
if [[ "$SHOW_HELP" == "true" ]]; then
    echo "EconGraph Clippy Runner"
    echo ""
    echo "Usage:"
    echo "  $0                 # Run clippy on all crates in the workspace"
    echo "  $0 --crate         # Run clippy on current crate only"
    echo "  $0 --help          # Show this help message"
    echo ""
    echo "Options:"
    echo "  --crate    Run clippy only on the current crate (faster for development)"
    echo "  --help     Show this help message"
    echo ""
    echo "The script uses enhanced clippy checks with documentation requirements:"
    echo "  - missing_docs_in_private_items"
    echo "  - missing_errors_doc"
    echo "  - missing_panics_doc"
    echo "  - missing_safety_doc"
    echo "  - doc_markdown"
    echo "  - doc_link_with_quotes"
    exit 0
fi

# Change to backend directory if not already there
if [[ ! -f "Cargo.toml" ]]; then
    if [[ -d "backend" ]]; then
        cd backend
    else
        echo "Error: Not in a Rust project directory and no 'backend' directory found"
        exit 1
    fi
fi

# Run clippy with the centralized configuration
if [[ "$CRATE_ONLY" == "true" ]]; then
    echo "Running clippy on current crate with enhanced documentation requirements..."
    cargo clippy "${CLIPPY_ARGS[@]}"
else
    echo "Running clippy on all crates with enhanced documentation requirements..."
    cargo clippy "${CLIPPY_ARGS[@]}"
fi
