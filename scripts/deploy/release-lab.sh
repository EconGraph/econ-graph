#!/usr/bin/env bash
# Release lab, data-copy mode: a disposable kind cluster that runs the app stack on its
# own copy of the data, restored from dumps given by path. Needs Python 3 and PyYAML.
#
# Usage (details: scripts/deploy/release-lab.py --help, docs/deployment/RELEASE_LAB.md):
#   scripts/deploy/release-lab.sh create --lab NAME --data-dump FILE --app-dump FILE \
#       --keycloak-dump FILE --backend-image REF --frontend-image REF --crawler-image REF \
#       [--release vX.Y.Z] [--dry-run]
#   scripts/deploy/release-lab.sh status|port-forward --lab NAME
#   scripts/deploy/release-lab.sh measure [--lab NAME]
#   scripts/deploy/release-lab.sh list
#   scripts/deploy/release-lab.sh teardown --lab NAME [--yes] [--keep-storage] [--dry-run]
set -euo pipefail
exec python3 "$(dirname "${BASH_SOURCE[0]}")/release-lab.py" "$@"
