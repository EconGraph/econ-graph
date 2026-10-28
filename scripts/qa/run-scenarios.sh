#!/usr/bin/env bash
#
# Run the QA scenario suite against a deployed site (docs/release/qa-scenarios.md).
#
#   scripts/qa/run-scenarios.sh --target BASE_URL --profile {lab,ci,production} \
#       [--only NAME,...] [--json OUT.json]
#   scripts/qa/run-scenarios.sh --list
#
# Credentials come from the environment or files it names, never from arguments.
set -euo pipefail
exec python3 "$(dirname "${BASH_SOURCE[0]}")/run_scenarios.py" "$@"
