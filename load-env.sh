#!/bin/bash

# Load environment configuration for EconGraph development
# This script loads both secret and non-secret environment variables

# Load port configuration (non-secret)
if [ -f "ports.env" ]; then
    echo "Loading port configuration from ports.env..."
    export $(cat ports.env | grep -v '^#' | xargs)
elif [ -f "../ports.env" ]; then
    echo "Loading port configuration from ../ports.env..."
    export $(cat ../ports.env | grep -v '^#' | xargs)
else
    echo "Warning: ports.env not found, using defaults"
fi

# Load secrets (if available)
if [ -f "secrets.env" ]; then
    echo "Loading secrets from secrets.env..."
    export $(cat secrets.env | grep -v '^#' | xargs)
elif [ -f "../secrets.env" ]; then
    echo "Loading secrets from ../secrets.env..."
    export $(cat ../secrets.env | grep -v '^#' | xargs)
else
    echo "Warning: secrets.env not found"
fi

# Set React environment variables for frontend
export REACT_APP_BACKEND_PORT=${BACKEND_PORT:-8081}
export REACT_APP_FRONTEND_PORT=${FRONTEND_PORT:-3000}

echo "Environment loaded:"
echo "  BACKEND_PORT: ${BACKEND_PORT:-8081}"
echo "  FRONTEND_PORT: ${FRONTEND_PORT:-3000}"
echo "  ADMIN_FRONTEND_PORT: ${ADMIN_FRONTEND_PORT:-3001}"
echo "  REACT_APP_BACKEND_PORT: ${REACT_APP_BACKEND_PORT}"
echo "  REACT_APP_FRONTEND_PORT: ${REACT_APP_FRONTEND_PORT}"
