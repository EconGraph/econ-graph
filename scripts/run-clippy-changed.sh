#!/bin/bash
# Clippy runner for changed crates only
# This script detects which crates have changed and runs clippy only on those crates

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

# Change to backend directory if not already there
if [[ ! -f "Cargo.toml" ]]; then
    if [[ -d "backend" ]]; then
        cd backend
    else
        echo "Error: Not in a Rust project directory and no 'backend' directory found"
        exit 1
    fi
fi

# Function to check if a crate has changed
has_crate_changed() {
    local crate_path="$1"
    local crate_name=$(basename "$crate_path")
    
    # Check if any files in this crate have changed (staged or unstaged)
    if git diff --cached --name-only | grep -q "^backend/crates/$crate_name/" || \
       git diff --name-only | grep -q "^backend/crates/$crate_name/"; then
        return 0
    fi
    
    # Also check if we're in a specific file mode (when called with filenames)
    if [[ $# -gt 0 ]]; then
        for file in "$@"; do
            if [[ "$file" =~ ^backend/crates/$crate_name/ ]]; then
                return 0
            fi
        done
    fi
    
    return 1
}

# Function to run clippy on a specific crate
run_clippy_on_crate() {
    local crate_path="$1"
    local crate_name=$(basename "$crate_path")
    
    echo "Running clippy on changed crate: $crate_name"
    cd "$crate_path"
    cargo clippy "${CLIPPY_ARGS[@]}"
    cd - > /dev/null
}

# Get list of all crates
CRATES=()
if [[ -d "crates" ]]; then
    for crate_dir in crates/*/; do
        if [[ -f "${crate_dir}Cargo.toml" ]]; then
            CRATES+=("$crate_dir")
        fi
    done
fi

# Also check the root crate
if [[ -f "Cargo.toml" ]]; then
    CRATES+=(".")
fi

# Find changed crates
CHANGED_CRATES=()
for crate in "${CRATES[@]}"; do
    if has_crate_changed "$crate" "$@"; then
        CHANGED_CRATES+=("$crate")
    fi
done

# If no crates have changed, run on all crates (fallback)
if [[ ${#CHANGED_CRATES[@]} -eq 0 ]]; then
    echo "No specific crates detected as changed, running clippy on all crates..."
    cargo clippy "${CLIPPY_ARGS[@]}"
    exit 0
fi

# Run clippy on changed crates
echo "Detected ${#CHANGED_CRATES[@]} changed crate(s):"
for crate in "${CHANGED_CRATES[@]}"; do
    echo "  - $(basename "$crate")"
done

# Run clippy on each changed crate
for crate in "${CHANGED_CRATES[@]}"; do
    run_clippy_on_crate "$crate"
done

echo "Clippy completed on all changed crates."
