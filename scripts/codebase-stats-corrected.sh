#!/usr/bin/env bash
# Human-readable report by default; --json is the validated updater's input.
set -euo pipefail
exec python3 "$(dirname "${BASH_SOURCE[0]}")/codebase_stats.py" "$@"
