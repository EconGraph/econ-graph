#!/usr/bin/env bash
# Recalculate from structured Git statistics and the usage CSV. Requires Python 3.
# All input, arithmetic, JSON, and documentation validation precedes any writes.
set -euo pipefail
exec python3 "$(dirname "${BASH_SOURCE[0]}")/cost_analysis.py" "$@"

