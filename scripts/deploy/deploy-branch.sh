#!/usr/bin/env bash
# Namespace-only branch environments. Python 3 and PyYAML are required.
set -euo pipefail
exec python3 "$(dirname "${BASH_SOURCE[0]}")/deploy-branch.py" "$@"
