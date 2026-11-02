#!/bin/bash

# Take a pg_dump -Fc of both production databases (econ_graph on postgresql-0,
# keycloak on keycloak-postgres-0) to the host, outside the cluster, together
# with row counts and the app Secrets the databases were initialised with:
#
#   ~/econgraph-backups/<UTC timestamp>/
#     backup.tar.age  SHA256SUMS  VERIFIED   (owner-only)
# The encrypted payload holds all dumps, counts.env and secrets/<name>.yaml.
# Set BACKUP_AGE_RECIPIENT to the maintainer public age recipient.
# Open it interactively with scripts/backup/open-backup.sh before restoring.
#
# Read-only against the cluster. restore-kind-databases.sh restores from it.
# Never deletes or rotates older backups. --dry-run prints the steps; it fails
# (changing nothing) if the cluster's API server can't be reached.
#
# Usage: scripts/deploy/backup-kind-databases.sh [--dry-run] [--out DIR]
# A dry run may create a temporary directory under $TMPDIR (a copy of the
# cluster's kubectl and kubeconfig), removed on exit; it changes nothing else.

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
