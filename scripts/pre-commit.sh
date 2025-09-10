#!/bin/bash
# Pre-commit utility script for EconGraph
# Makes it easy to run pre-commit commands with proper PATH setup

# Add pre-commit to PATH
export PATH="/Users/josephmalicki/Library/Python/3.9/bin:$PATH"

# Check if pre-commit is available
if ! command -v pre-commit &> /dev/null; then
    echo "Error: pre-commit not found. Please install it with: pip3 install pre-commit"
    exit 1
fi

# Run the provided command
pre-commit "$@"
