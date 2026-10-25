#!/bin/bash

# Tear EconGraph down from the local kind cluster.
#
# Usage:
#   scripts/deploy/teardown.sh [--dry-run] [--yes] [--no-backup] [--delete-cluster]
#
# Steps:
#   1. pg_dump both databases to ~/econgraph-backups/<UTC timestamp>/
#      (backup-kind-databases.sh). If postgresql-0 is not running no backup
#      can be taken, and the script refuses unless --no-backup is given.
#   2. Set the PostgreSQL and backup PVs to Retain (protect-postgres-pv.sh).
#   3. Delete the application resources and the app Secrets
#      (econ-graph-secrets, econ-graph-postgres, crawler-api-keys,
#      econ-graph-keycloak, grafana-admin, monitoring-auth) and the
#      econ-graph namespace, which deletes its PVCs. You confirm this by
#      typing the namespace name; --yes answers it. Without a terminal and
#      without --yes the script refuses before changing anything.
#   4. Delete the kind cluster only with --delete-cluster, or if you type the
#      cluster name when asked (Enter, or no terminal, keeps it). --yes never
#      implies it: the cluster's nodes hold the in-cluster backup volume and
#      Grafana's data.
#
# The PostgreSQL and Keycloak PostgreSQL data live on host btrfs subvolumes
# (static PVs in k8s/manifests/host-storage.yaml, docs/deployment/HOST_STORAGE.md).
# This script never deletes those PVs or their directories; deleting the
# namespace only leaves them Released, and the next deploy.sh re-binds them.
#
# --dry-run runs the checks, prints every step and changes nothing. It may
# create a temporary directory under $TMPDIR (a copy of the cluster's kubectl
# and kubeconfig), removed on exit; it changes nothing else.

set -euo pipefail

NAMESPACE="econ-graph"
CLUSTER="econ-graph"
DRY_RUN="false"
ASSUME_YES="false"
NO_BACKUP="false"
DELETE_CLUSTER_FLAG="false"
usage="usage: $0 [--dry-run] [--yes] [--no-backup] [--delete-cluster]"
for arg in "$@"; do
    case "$arg" in
        --dry-run|-n) DRY_RUN="true" ;;
        --yes|-y) ASSUME_YES="true" ;;
        --no-backup) NO_BACKUP="true" ;;
        --delete-cluster) DELETE_CLUSTER_FLAG="true" ;;
        -h|--help) sed -n '3,31p' "$0"; exit 0 ;;
        *) echo "$usage" >&2; exit 2 ;;
    esac
done
export DRY_RUN ASSUME_YES

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$PROJECT_ROOT"

# The kind cluster runs on the rootful Docker daemon; use its own kubectl (on
# this host `kubectl` is microk8s'). Sourced first: it sets DOCKER_HOST.
# shellcheck source=scripts/deploy/lib/kind-env.sh
source "$PROJECT_ROOT/scripts/deploy/lib/kind-env.sh"

# Refuse up front, before anything changes, if the confirmation can't be typed.
if [ "${ASSUME_YES}" != "true" ] && [ "${DRY_RUN}" != "true" ] && [ ! -t 0 ]; then
    die "no terminal to confirm on; refusing (pass --yes only if you mean it)"
fi

[ "${DRY_RUN}" = "true" ] && log "DRY RUN: checks run for real; nothing is changed"
echo "Tearing down EconGraph from kind cluster '${CLUSTER}'"
check_docker_daemon
kind_cluster_exists || die "kind cluster '${CLUSTER}' not found on ${DOCKER_HOST}"
ensure_kind_kubectl

# 1. Off-cluster copy of both databases first, while they are still running.
if pod_running postgresql-0; then
    backup_databases "$(new_backup_dir)"
elif [ "${NO_BACKUP}" = "true" ]; then
    warn "postgresql-0 is not running; tearing down WITHOUT a backup (--no-backup)"
else
    die "postgresql-0 is not running, so no backup can be taken first; nothing was changed. Start it, or pass --no-backup to tear down without a backup"
fi

# 2. Preserve both data and backup PVs before any resource or namespace
# deletion. A failed patch stops teardown before anything is deleted.
run "$PROJECT_ROOT/scripts/deploy/protect-postgres-pv.sh"

# 3. One typed confirmation for the application, its Secrets and the namespace.
echo ""
echo "This deletes, in namespace ${NAMESPACE}:"
echo "  - the application resources (ingresses, frontend, admin frontend, crawler,"
echo "    backend, chart API, PostgreSQL StatefulSet, postgres-backup CronJob, configmap)"
echo "  - the app Secrets: econ-graph-secrets econ-graph-postgres crawler-api-keys"
echo "    econ-graph-keycloak grafana-admin monitoring-auth"
echo "  - the namespace itself, with its PVCs. Their PVs are kept (Retain); the"
echo "    next deploy.sh re-binds the host-storage ones."
if [ "${ASSUME_YES}" = "true" ]; then
    echo "(--yes given)"
elif [ "${DRY_RUN}" = "true" ]; then
    echo "(dry run: would ask you to type '${NAMESPACE}' here)"
else
    read -r -p "Type the namespace name (${NAMESPACE}) to delete all of this: " answer
    [ "${answer}" = "${NAMESPACE}" ] || die "aborted; nothing was deleted"
fi

echo "Removing application resources..."
for manifest in admin-ingress graphql-ingress ingress frontend-service frontend-deployment \
    admin-frontend-service admin-frontend-deployment crawler-worker crawler-migrate-job \
    backend-service backend-deployment chart-api-service chart-api-deployment postgres \
    postgres-deployment postgres-init; do
    run kubectl delete -f "k8s/manifests/${manifest}.yaml" --ignore-not-found=true
done
run kubectl delete cronjob postgres-backup -n "${NAMESPACE}" --ignore-not-found=true
# Secrets created by scripts/deploy/create-secrets.sh (the namespace deletion
# below would remove them too; listed so the intent is explicit).
run kubectl -n "${NAMESPACE}" delete secret econ-graph-secrets econ-graph-postgres crawler-api-keys \
    econ-graph-keycloak grafana-admin monitoring-auth --ignore-not-found=true
run kubectl delete -f k8s/manifests/configmap.yaml --ignore-not-found=true
run kubectl delete -f k8s/manifests/namespace.yaml --ignore-not-found=true
[ "${DRY_RUN}" = "true" ] || echo "Application, Secrets and namespace removed."

# 4. The cluster, only on --delete-cluster or a typed cluster name. The
# databases survive it (host storage, plus the backup taken above), but the
# in-cluster backup volume and Grafana's data do not.
DELETE_CLUSTER="${DELETE_CLUSTER_FLAG}"
if [ "${DELETE_CLUSTER_FLAG}" != "true" ]; then
    echo ""
    echo "Deleting the kind cluster destroys everything stored in its nodes:"
    echo "  the in-cluster nightly backups and Grafana's data. The databases on"
    echo "  /srv/econgraph are kept, and so is the backup taken above."
    if [ "${DRY_RUN}" = "true" ]; then
        echo "(dry run: would ask you to type '${CLUSTER}' to delete the cluster; Enter keeps it)"
    elif [ -t 0 ]; then
        read -r -p "Type the cluster name (${CLUSTER}) to delete it, or press Enter to keep it: " answer
        [ "${answer}" = "${CLUSTER}" ] && DELETE_CLUSTER="true"
    else
        echo "(no terminal: the cluster is kept; pass --delete-cluster to delete it)"
    fi
fi

if [ "${DELETE_CLUSTER}" = "true" ]; then
    echo "Deleting kind cluster ${CLUSTER}..."
    run kind delete cluster --name "${CLUSTER}" --kubeconfig "${KUBECONFIG}"
else
    echo "Kind cluster kept running. Delete it later with:"
    echo "  scripts/deploy/teardown.sh --delete-cluster   (or kind delete cluster --name ${CLUSTER})"
fi
if [ "${DRY_RUN}" = "true" ]; then
    log "Dry run complete; nothing was changed"
fi
