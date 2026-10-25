#!/bin/bash

# Apply the static host-subvolume PersistentVolumes for PostgreSQL and
# Keycloak's PostgreSQL (k8s/manifests/host-storage.yaml), after checking that
# they can only ever land on the real host directories. deploy.sh runs this
# before anything that touches Postgres. Safe to re-run.
#
#   1. The host subvolumes are mounted and owned by you (no sudo here; they are
#      created by scripts/host/setup-subvolumes.sh).
#   2. The kind storage node bind-mounts them (the cluster was created from
#      terraform/k8s/kind-config.yaml; see recreate-kind-cluster.sh).
#   3. pg/ and keycloak-pg/ exist (created here, as you, if missing).
#   4. If a host data directory already holds a database but the Secret with
#      the passwords it was initialised with is missing, stop: new generated
#      passwords would not match it. Restore the Secrets from the backup taken
#      before the cluster was recreated (migrate-to-host-storage.sh does).
#   5. Apply the StorageClass and PVs, and re-arm any PV left Released by a
#      deleted PVC (namespace teardown), so the StatefulSet's new PVC binds to
#      the same directory. The data is never touched.
#
# Usage: scripts/deploy/apply-host-storage.sh [--dry-run]
# A dry run may create a temporary directory under $TMPDIR (a copy of the
# cluster's kubectl and kubeconfig), removed on exit; it changes nothing else.
#
# Never deletes a PV, a PVC or a directory.

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/deploy/lib/kind-env.sh
source "${PROJECT_ROOT}/scripts/deploy/lib/kind-env.sh"

for arg in "$@"; do
    case "$arg" in
        --dry-run|-n) DRY_RUN="true" ;;
        *) echo "usage: $0 [--dry-run]" >&2; exit 2 ;;
    esac
done

[ "${DRY_RUN}" = "true" ] && log "DRY RUN: checks run for real; nothing is changed"
log "Checking host storage"
check_host_storage_on_host || die "host subvolumes are not ready"
echo "  ok: ${HOST_DATA_PLANE} and ${HOST_APP_PROD} are mounted btrfs subvolumes owned by $(id -un)"

log "Docker daemon"
check_docker_daemon
if [ "${DRY_RUN}" = "true" ] && ! ensure_kind_kubectl_quietly; then
    echo "  (dry run) kind cluster '${KIND_CLUSTER}' isn't reachable, so this is the plan, not checked against it:"
    for dir in "${HOST_PG_DIR}" "${HOST_KEYCLOAK_PG_DIR}"; do
        [ -d "$dir" ] || echo "+ mkdir $dir"
    done
    echo "+ kubectl apply -f k8s/manifests/host-storage.yaml; re-arm Released PVs"
    log "Dry run complete; nothing was changed"
    exit 0
fi
ensure_kind_kubectl

if ! check_host_storage_in_node; then
    die "node ${KIND_STORAGE_NODE} does not bind-mount ${HOST_DATA_PLANE} and ${HOST_APP_PROD}.
  The cluster predates the host-storage kind config. Migrate it with
  scripts/deploy/migrate-to-host-storage.sh (docs/deployment/HOST_STORAGE.md)."
fi

for dir in "${HOST_PG_DIR}" "${HOST_KEYCLOAK_PG_DIR}"; do
    if [ ! -d "$dir" ]; then
        run mkdir "$dir"
    fi
done

# Ownership of these directories is set by each StatefulSet's init-data-dir
# container; once Postgres has run they belong to host uid ${HOST_POSTGRES_UID}
# (pod uid 999), mode 0700, and are not readable from the host as you, so look
# from inside the node.
if docker exec "${KIND_STORAGE_NODE}" test -e "${NODE_PG_VERSION_FILE}" &&
    [ -z "$(k get secret econ-graph-postgres -o name --ignore-not-found)" ]; then
    die "${HOST_PG_DIR} already holds a database, but Secret econ-graph-postgres is missing.
  Restore the Secrets saved with the last backup first (secrets/ in
  ${BACKUP_ROOT}/<timestamp>/; migrate-to-host-storage.sh --resume does this)."
fi
if docker exec "${KIND_STORAGE_NODE}" test -e "${NODE_KEYCLOAK_PG_VERSION_FILE}" &&
    [ -z "$(k get secret econ-graph-keycloak -o name --ignore-not-found)" ]; then
    die "${HOST_KEYCLOAK_PG_DIR} already holds a database, but Secret econ-graph-keycloak is missing.
  Restore the Secrets saved with the last backup first."
fi

log "Applying k8s/manifests/host-storage.yaml"
if [ "${DRY_RUN}" = "true" ]; then
    run kubectl apply --dry-run=client -f "${PROJECT_ROOT}/k8s/manifests/host-storage.yaml"
else
    run kubectl apply -f "${PROJECT_ROOT}/k8s/manifests/host-storage.yaml"
fi

for pv in econgraph-data-plane-pg econgraph-app-prod-keycloak-pg; do
    phase="$(kubectl get pv "$pv" -o jsonpath='{.status.phase}' --ignore-not-found)"
    if [ "$phase" = "Released" ]; then
        # The PVC it was bound to is gone; forget that PVC's uid so a new PVC
        # with the same name (the claimRef) can bind. Data stays as it is.
        run kubectl patch pv "$pv" --type merge -p '{"spec":{"claimRef":{"uid":null,"resourceVersion":null}}}'
    fi
    echo "  $pv: ${phase:-not created yet (dry run)}"
done
