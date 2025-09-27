#!/bin/bash
# Documentation check runner for changed crates only
# This script detects which crates have changed and runs doc check only on those crates

set -euo pipefail

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

# Function to run doc check on a specific crate
run_doc_check_on_crate() {
    local crate_path="$1"
    local crate_name=$(basename "$crate_path")

    echo "Running doc check on changed crate: $crate_name"
    cd "$crate_path"
    cargo doc --no-deps --document-private-items --all-features
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
    echo "No specific crates detected as changed, running doc check on all crates..."
    cargo doc --no-deps --document-private-items --all-features
    exit 0
fi

# Run doc check on changed crates
echo "Detected ${#CHANGED_CRATES[@]} changed crate(s):"
for crate in "${CHANGED_CRATES[@]}"; do
    echo "  - $(basename "$crate")"
done

# Run doc check on each changed crate
for crate in "${CHANGED_CRATES[@]}"; do
    run_doc_check_on_crate "$crate"
done

echo "Documentation check completed on all changed crates."
