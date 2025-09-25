#!/bin/bash

# This script wraps Playwright test execution with continuous frontend log tailing.
# It starts tailing frontend logs in the background, runs the provided command (Playwright tests),
# and then stops the log tailing.

echo "🚀 Starting frontend log tailing for E2E tests..."

# Start frontend log tailing in the background
docker logs -f frontend-server &
FRONTEND_LOG_TAIL_PID=$!

# Function to stop log tailing
stop_log_tailing() {
  if [ ! -z "$FRONTEND_LOG_TAIL_PID" ]; then
    echo "🛑 Stopping frontend log tailing..."
    kill $FRONTEND_LOG_TAIL_PID 2>/dev/null || true
    wait $FRONTEND_LOG_TAIL_PID 2>/dev/null || true
  fi
}

# Set trap to stop log tailing on exit
trap stop_log_tailing EXIT

# Execute the provided command (e.g., Playwright tests)
echo "▶️ Running E2E tests with frontend logs..."
"$@" # This executes all arguments passed to the script

# The trap will handle stopping the log tailing when this script exits
echo "✅ E2E tests command finished."
