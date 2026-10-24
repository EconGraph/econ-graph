#!/bin/bash

# Delete and recreate the kind cluster `econ-graph` from
# terraform/k8s/kind-config.yaml (ports 80/443/3000/8080, and the worker that
# bind-mounts the host Postgres subvolumes).
#
# DESTRUCTIVE: everything stored inside the cluster's node containers is lost
# (Grafana data, the in-cluster postgres-backup-data PVC, Secrets, and on a
# cluster that predates host storage, the databases themselves). So, before
# deleting an existing cluster, this script:
#   1. pg_dumps both databases, with row counts and the app Secrets, to
#      ~/econgraph-backups/<UTC timestamp>/ (see backup-kind-databases.sh);
#      if that fails, nothing is deleted;
#   2. asks for a typed confirmation (or --yes).
# If the kind config bind-mounts host directories, they must be mounted
# first (scripts/host/setup-subvolumes.sh), or nothing is deleted.
#
# Usage:
#   scripts/deploy/recreate-kind-cluster.sh [--yes] [--dry-run] [--config FILE]
#       [--no-backup]   only for callers that took a backup just before
#                       (migrate-to-host-storage.sh); still asks unless --yes
#
# Afterwards deploy with scripts/deploy/deploy.sh, and restore data with
# restore-kind-databases.sh if the databases weren't on host storage.

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/deploy/lib/kind-env.sh
source "${PROJECT_ROOT}/scripts/deploy/lib/kind-env.sh"

CONFIG="${PROJECT_ROOT}/terraform/k8s/kind-config.yaml"
BACKUP="true"
while [ $# -gt 0 ]; do
    case "$1" in
        --yes|-y) ASSUME_YES="true"; shift ;;
        --dry-run|-n) DRY_RUN="true"; shift ;;
        --config) CONFIG="$2"; shift 2 ;;
        --no-backup) BACKUP="false"; shift ;;
        *) echo "usage: $0 [--yes] [--dry-run] [--config FILE] [--no-backup]" >&2; exit 2 ;;
    esac
done
[ -f "$CONFIG" ] || die "kind config $CONFIG not found"

# Every bind-mounted host directory must already exist as a real mount, or
# kind would fail half-way (or, for a missing mount under an existing
# directory, bind an empty directory and Postgres would refuse to start).
# (grep, not yq: this host's yq is the jq-based one, other machines have the Go one.)
mounts="$(grep -E '"?hostPath"?:' "$CONFIG" | sed -E 's/.*hostPath"?:[[:blank:]]*"?([^"]*)"?.*/\1/' || true)"
if [ -n "$mounts" ]; then
    log "Checking the host directories the kind config bind-mounts"
    if [ "$CONFIG" = "${PROJECT_ROOT}/terraform/k8s/kind-config.yaml" ]; then
        check_host_storage_on_host || die "host subvolumes are not ready; nothing was deleted"
    fi
    for m in $mounts; do
        mountpoint -q "$m" || die "$m is not mounted; nothing was deleted"
        echo "  ok: $m"
    done
fi

if kind_cluster_exists; then
    ensure_kind_kubectl
    if [ "$BACKUP" = "true" ]; then
        backup_databases "$(new_backup_dir)"
    else
        log "Skipping the pre-delete backup (--no-backup)"
    fi
    confirm "About to DELETE kind cluster '${KIND_CLUSTER}' and everything stored in its nodes."
    log "Deleting kind cluster ${KIND_CLUSTER}"
    run kind delete cluster --name "${KIND_CLUSTER}"
else
    log "No kind cluster '${KIND_CLUSTER}'; nothing to back up or delete"
fi

log "Creating kind cluster ${KIND_CLUSTER} from ${CONFIG}"
run kind create cluster --name "${KIND_CLUSTER}" --config "$CONFIG"
if [ "${DRY_RUN}" = "true" ]; then
    exit 0
fi

use_kind_kubectl
kubectl wait --for=condition=Ready nodes --all --timeout=300s

log "Kind cluster recreated"
echo "  Frontend: http://localhost:3000   Backend: http://localhost:8080"
echo "  HTTP: http://localhost:80          HTTPS: https://localhost:443"
echo "  Next: scripts/deploy/deploy.sh"
