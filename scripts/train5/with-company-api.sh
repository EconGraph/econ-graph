#!/usr/bin/env bash
# Exercise the actual build flag without changing the committed release configuration.
set -euo pipefail
if [[ $# -eq 0 ]]; then
  echo 'usage: with-company-api.sh COMMAND [ARG ...]' >&2
  exit 2
fi
company_repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
company_flags_path="$company_repo/backend/flags/dev.json"
company_flags_snapshot="$(mktemp)"
cp -- "$company_flags_path" "$company_flags_snapshot"
restore_company_flags() {
  local command_status=$?
  cp -- "$company_flags_snapshot" "$company_flags_path"
  rm -f -- "$company_flags_snapshot"
  return "$command_status"
}
trap restore_company_flags EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
python3 - "$company_flags_path" <<'PY'
import json
from pathlib import Path
import sys

path = Path(sys.argv[1])
profile = json.loads(path.read_text())
profile["flags"]["financial_components"]["value"] = True
path.write_text(json.dumps(profile, indent=2) + "\n")
PY
FLAGS_PROFILE=dev "$@"
