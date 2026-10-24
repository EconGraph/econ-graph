#!/bin/bash

# Take a pg_dump -Fc of both production databases (econ_graph on postgresql-0,
# keycloak on keycloak-postgres-0) to the host, outside the cluster, together
# with row counts and the app Secrets the databases were initialised with:
#
#   ~/econgraph-backups/<UTC timestamp>/
#     econ_graph.dump  keycloak.dump  counts.env  secrets/<name>.yaml   (owner-only)
#
# Read-only against the cluster. restore-kind-databases.sh restores from it.
# Never deletes or rotates older backups. --dry-run prints the steps; it fails
# (changing nothing) if the cluster's API server can't be reached.
#
# Usage: scripts/deploy/backup-kind-databases.sh [--dry-run] [--out DIR]

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/deploy/lib/kind-env.sh
source "${PROJECT_ROOT}/scripts/deploy/lib/kind-env.sh"

OUT=""
while [ $# -gt 0 ]; do
    case "$1" in
        --dry-run|-n) DRY_RUN="true"; shift ;;
        --out) OUT="$2"; shift 2 ;;
        *) echo "usage: $0 [--dry-run] [--out DIR]" >&2; exit 2 ;;
    esac
done

log "Docker daemon"
check_docker_daemon
ensure_kind_kubectl
backup_databases "${OUT:-$(new_backup_dir)}"
