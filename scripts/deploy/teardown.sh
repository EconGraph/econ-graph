#!/bin/bash

# Tear EconGraph down from the local kind cluster. By default only the app
# plane goes; the data plane stays up.
#
# Usage:
#   scripts/deploy/teardown.sh [--dry-run] [--yes] [--no-backup]
#                              [--include-data-plane [--delete-cluster]]
#
# Steps:
#   1. pg_dump the data, app and Keycloak databases to
#      ~/econgraph-backups/<UTC timestamp>/ (backup-kind-databases.sh). If
#      postgresql-0 is not running no backup can be taken, and the script
#      refuses unless --no-backup is given. The backup comes before the
#      confirmations below, so a run you abort still leaves it.
#   2. Set the PostgreSQL and backup PVs to Retain (protect-postgres-pv.sh).
#   3. Delete the app plane, after you type the namespace name (--yes answers
#      this one): the cloudflared tunnel connector, the routes (ingresses),
#      frontend, admin frontend, backend, chart API, Keycloak, and the app and
#      Keycloak Postgres StatefulSets and
#      Services. Their Secrets and PVCs stay, so the next deploy.sh brings the
#      same databases back.
#   4. Only with --include-data-plane, after you type the data database's name
#      (econ_graph) on a terminal (--yes never answers this): the data plane
#      too, i.e. the data Postgres StatefulSet (postgresql), its Services
#      (data-postgres-service, postgres-service), the crawler, the
#      crawler-migrate Job, the in-cluster
#      backup CronJob, the app Secrets (econ-graph-secrets,
#      econ-graph-postgres, econ-graph-app-postgres, crawler-api-keys,
#      econ-graph-keycloak, grafana-admin, monitoring-auth,
#      cloudflare-tunnel), the configmap and the econ-graph namespace with its
#      PVCs.
#   5. Only with --include-data-plane --delete-cluster, after you type the
#      cluster name on a terminal (--yes never answers this): the kind
#      cluster, with everything stored in its nodes (the in-cluster nightly
#      backups and Grafana's data).
#
# A confirmation that can't be typed (no terminal) stops the script before it
# changes anything.
#
# The databases live on host btrfs subvolumes (static PVs in
# k8s/manifests/host-storage.yaml, docs/deployment/HOST_STORAGE.md). This
# script never deletes those PVs or their directories; a deleted PVC only
# leaves its PV Released, and the next deploy.sh re-binds it.
#
# --dry-run runs the checks, prints every step and changes nothing. It may
# create a temporary directory under $TMPDIR (a copy of the cluster's kubectl
# and kubeconfig), removed on exit; it changes nothing else.

set -euo pipefail

DRY_RUN="false"
ASSUME_YES="false"
NO_BACKUP="false"
INCLUDE_DATA_PLANE="false"
DELETE_CLUSTER="false"
usage="usage: $0 [--dry-run] [--yes] [--no-backup] [--include-data-plane [--delete-cluster]]"
for arg in "$@"; do
    case "$arg" in
        --dry-run|-n) DRY_RUN="true" ;;
        --yes|-y) ASSUME_YES="true" ;;
        --no-backup) NO_BACKUP="true" ;;
        --include-data-plane) INCLUDE_DATA_PLANE="true" ;;
        --delete-cluster) DELETE_CLUSTER="true" ;;
        -h|--help) sed -n '3,48p' "$0"; exit 0 ;;
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
CLUSTER="${KIND_CLUSTER}"
DATA_DATABASE="econ_graph"

if [ "${DELETE_CLUSTER}" = "true" ] && [ "${INCLUDE_DATA_PLANE}" != "true" ]; then
    die "--delete-cluster deletes the data plane with the cluster; it needs --include-data-plane. Nothing was changed"
fi

# Refuse up front, before anything changes, if a confirmation can't be typed.
if [ "${DRY_RUN}" != "true" ] && [ ! -t 0 ]; then
    if [ "${ASSUME_YES}" != "true" ]; then
        die "no terminal to confirm on; refusing (pass --yes only if you mean it). Nothing was changed"
    fi
    if [ "${INCLUDE_DATA_PLANE}" = "true" ]; then
        die "--include-data-plane needs the data database's name typed on a terminal; --yes doesn't answer it. Nothing was changed"
    fi
fi
# --delete-cluster runs the pinned kind (scripts/deploy/install-kind.sh);
# checked before anything changes.
if [ "${DELETE_CLUSTER}" = "true" ]; then
    require_kind_version
fi

# typed_confirm PROMPT ANSWER: read ANSWER from the terminal or stop. --yes
# doesn't answer it.
typed_confirm() {
    local prompt="$1" expected="$2" answer
    if [ "${DRY_RUN}" = "true" ]; then
        echo "(dry run: would ask you to type '${expected}' here)"
        return 0
    fi
    read -r -p "${prompt} Type '${expected}' to continue: " answer || die "aborted; nothing was deleted"
    [ "${answer}" = "${expected}" ] || die "aborted; nothing was deleted"
}

[ "${DRY_RUN}" = "true" ] && log "DRY RUN: checks run for real; nothing is changed"
echo "Tearing down EconGraph from kind cluster '${CLUSTER}'"
check_docker_daemon
kind_cluster_exists || die "kind cluster '${CLUSTER}' not found on ${DOCKER_HOST}"
ensure_kind_kubectl

# 1. Off-cluster copy of the databases first, while they are still running.
if pod_running postgresql-0; then
    backup_databases "$(new_backup_dir)"
elif [ "${NO_BACKUP}" = "true" ]; then
    warn "postgresql-0 is not running; tearing down WITHOUT a backup (--no-backup)"
else
    die "postgresql-0 is not running, so no backup can be taken first; nothing was changed. Start it, or pass --no-backup to tear down without a backup"
fi

# 2. Preserve the data and backup PVs before anything is deleted. A failed
# patch stops teardown before anything is deleted.
if [ "${DRY_RUN}" = "true" ]; then
    "$PROJECT_ROOT/scripts/deploy/protect-postgres-pv.sh" --dry-run
else
    "$PROJECT_ROOT/scripts/deploy/protect-postgres-pv.sh"
fi

# 3-5. Every confirmation is asked before anything is deleted.
echo ""
echo "This deletes the app plane, in namespace ${NAMESPACE}:"
echo "  - the cloudflared tunnel connector, ingresses, frontend, admin frontend,"
echo "    backend, chart API, Keycloak"
echo "  - the app Postgres (app-postgres) and Keycloak Postgres (keycloak-postgres)"
echo "    StatefulSets and Services. Their Secrets and PVCs are kept: the next"
echo "    deploy.sh brings the same databases back."
if [ "${ASSUME_YES}" = "true" ]; then
    echo "(--yes given)"
elif [ "${DRY_RUN}" = "true" ]; then
    echo "(dry run: would ask you to type '${NAMESPACE}' here)"
else
    read -r -p "Type the namespace name (${NAMESPACE}) to delete the app plane: " answer || die "aborted; nothing was deleted"
    [ "${answer}" = "${NAMESPACE}" ] || die "aborted; nothing was deleted"
fi

if [ "${INCLUDE_DATA_PLANE}" = "true" ]; then
    echo ""
    echo "--include-data-plane: this also deletes the data plane:"
    echo "  - the data Postgres StatefulSet (postgresql) and its Services"
    echo "    (data-postgres-service, postgres-service), the crawler, the"
    echo "    crawler-migrate Job and the postgres-backup CronJob"
    echo "  - the app Secrets: ${APP_SECRETS[*]}"
    echo "  - the configmap and the namespace itself, with its PVCs. Their PVs are"
    echo "    kept (Retain); the next deploy.sh re-binds the host-storage ones once"
    echo "    the Secrets are restored from the backup above."
    typed_confirm "Delete the data plane?" "${DATA_DATABASE}"
else
    echo "The data plane (postgresql, its Services and Secret, the crawler) is kept;"
    echo "  --include-data-plane deletes it too."
fi

if [ "${DELETE_CLUSTER}" = "true" ]; then
    echo ""
    echo "--delete-cluster: deleting the kind cluster destroys everything stored in"
    echo "  its nodes: the in-cluster nightly backups and Grafana's data. The"
    echo "  databases' host directories are kept, and so is the backup taken above."
    typed_confirm "Delete kind cluster ${CLUSTER}?" "${CLUSTER}"
fi

echo "Removing the app plane..."
# The public edge first (deploy.sh's apply_public_edge and apply_cloudflared),
# so no tunnel keeps routing to Services that are going away.
run kubectl delete -f k8s/manifests/cloudflared.yaml --ignore-not-found=true
run kubectl -n "${NAMESPACE}" delete ingress -l econgraph.io/public-host=true --ignore-not-found=true
for manifest in graphql-ingress ingress frontend-service frontend-deployment \
    admin-frontend-service admin-frontend-deployment backend-service \
    backend-deployment chart-api-service chart-api-deployment; do
    run kubectl delete -f "k8s/manifests/${manifest}.yaml" --ignore-not-found=true
done
run kubectl -n "${NAMESPACE}" delete job keycloak-realm-import --ignore-not-found=true
for manifest in keycloak/ingress keycloak/deployment keycloak/postgres app-postgres; do
    run kubectl delete -f "k8s/manifests/${manifest}.yaml" --ignore-not-found=true
done
run kubectl delete -f k8s/manifests/app-postgres-init.yaml --ignore-not-found=true
[ "${DRY_RUN}" = "true" ] || echo "App plane removed."

if [ "${INCLUDE_DATA_PLANE}" = "true" ]; then
    echo "Removing the data plane..."
    for manifest in crawler-worker crawler-migrate-job postgres postgres-deployment postgres-init; do
        run kubectl delete -f "k8s/manifests/${manifest}.yaml" --ignore-not-found=true
    done
    run kubectl delete cronjob postgres-backup -n "${NAMESPACE}" --ignore-not-found=true
    # Secrets created by scripts/deploy/create-secrets.sh (the namespace
    # deletion below would remove them too; listed so the intent is explicit).
    run kubectl -n "${NAMESPACE}" delete secret "${APP_SECRETS[@]}" --ignore-not-found=true
    run kubectl delete -f k8s/manifests/configmap.yaml --ignore-not-found=true
    run kubectl delete -f k8s/manifests/namespace.yaml --ignore-not-found=true
    [ "${DRY_RUN}" = "true" ] || echo "Data plane, Secrets and namespace removed."
fi

if [ "${DELETE_CLUSTER}" = "true" ]; then
    echo "Deleting kind cluster ${CLUSTER}..."
    run kind delete cluster --name "${CLUSTER}" --kubeconfig "${KUBECONFIG}"
fi
if [ "${DRY_RUN}" = "true" ]; then
    log "Dry run complete; nothing was changed"
fi
