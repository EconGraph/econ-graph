#!/usr/bin/env bash
# Measure the v4.1 first-deploy effects of the catalog lifecycle (ECO-803), read-only.
# See scripts/deploy/measure-v41-first-deploy.py for usage. Python 3.11+ and psql are required.
set -euo pipefail
exec python3 "$(dirname "${BASH_SOURCE[0]}")/measure-v41-first-deploy.py" "$@"
