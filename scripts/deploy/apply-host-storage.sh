#!/bin/bash

# Apply the static host-subvolume PersistentVolumes for the data, app and
# Keycloak PostgreSQL databases (k8s/manifests/host-storage.yaml), after checking that
# they can only ever land on the real host directories. deploy.sh runs this
# before anything that touches Postgres. Safe to re-run.
#
#   1. The host subvolumes are mounted and owned by you (no sudo here; they are
#      created by scripts/host/setup-subvolumes.sh).
#   2. The kind storage node bind-mounts them (the cluster was created from
#      terraform/k8s/kind-config.yaml; see recreate-kind-cluster.sh).
#   3. pg/, app-pg/ and keycloak-pg/ exist (created here, as you, if missing).
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
if [ -n "${SCRATCH_STORAGE}" ]; then
    echo "  ok: ${HOST_DATA_PLANE} and ${HOST_APP_PROD} are btrfs subvolumes owned by $(id -un) (scratch storage for ${KIND_CLUSTER})"
else
    echo "  ok: ${HOST_DATA_PLANE} and ${HOST_APP_PROD} are mounted btrfs subvolumes owned by $(id -un)"
fi

DATA_DIRS=("${HOST_PG_DIR}" "${HOST_APP_PG_DIR}" "${HOST_KEYCLOAK_PG_DIR}")

log "Docker daemon"
check_docker_daemon
if [ "${DRY_RUN}" = "true" ] && ! ensure_kind_kubectl_quietly; then
    echo "  (dry run) kind cluster '${KIND_CLUSTER}' isn't reachable, so this is the plan, not checked against it:"
    for dir in "${DATA_DIRS[@]}"; do
        [ -d "$dir" ] || echo "+ mkdir $dir"
    done
    echo "+ kubectl apply -f k8s/manifests/host-storage.yaml; re-arm Released PVs"
    log "Dry run complete; nothing was changed"
    exit 0
fi
ensure_kind_kubectl

if ! check_host_storage_in_node; then
    die "node ${KIND_STORAGE_NODE} does not bind-mount ${NODE_DATA_PLANE} and ${NODE_APP_PROD}.
  The cluster predates the host-storage kind config. Migrate it with
  scripts/deploy/migrate-to-host-storage.sh (docs/deployment/HOST_STORAGE.md)."
fi

for dir in "${DATA_DIRS[@]}"; do
    if [ ! -d "$dir" ]; then
        run mkdir "$dir"
    fi
done

# Ownership of these directories is set by each StatefulSet's init-data-dir
# container; once Postgres has run they belong to host uid ${HOST_POSTGRES_UID}
# (pod uid 999), mode 0700, and are not readable from the host as you, so look
# from inside the node.
# refuse_without_secret VERSION_FILE HOST_DIR SECRET
refuse_without_secret() {
    local rc=0
    docker exec "${KIND_STORAGE_NODE}" test -e "$1" || rc=$?
    # test exits 1 when the file is absent; anything else is a failed check.
    case "$rc" in
        0) ;;
        1) return 0 ;;
        *) die "could not check ${KIND_STORAGE_NODE}:$1 (exit $rc); nothing was applied" ;;
    esac
    if [ -z "$(k get secret "$3" -o name --ignore-not-found)" ]; then
        die "$2 already holds a database, but Secret $3 is missing.
  Restore the Secrets saved with the last backup first (secrets/ in
  ${BACKUP_ROOT}/<timestamp>/; restore-kind-databases.sh does this)."
    fi
}
refuse_without_secret "${NODE_PG_VERSION_FILE}" "${HOST_PG_DIR}" econ-graph-postgres
refuse_without_secret "${NODE_APP_PG_VERSION_FILE}" "${HOST_APP_PG_DIR}" econ-graph-app-postgres
refuse_without_secret "${NODE_KEYCLOAK_PG_VERSION_FILE}" "${HOST_KEYCLOAK_PG_DIR}" econ-graph-keycloak

log "Applying k8s/manifests/host-storage.yaml"
if [ "${DRY_RUN}" = "true" ]; then
    run kubectl apply --dry-run=client -f "${PROJECT_ROOT}/k8s/manifests/host-storage.yaml"
else
    run kubectl apply -f "${PROJECT_ROOT}/k8s/manifests/host-storage.yaml"
fi

for pv in econgraph-data-plane-pg econgraph-app-prod-app-pg econgraph-app-prod-keycloak-pg; do
    phase="$(kubectl get pv "$pv" -o jsonpath='{.status.phase}' --ignore-not-found)"
    if [ "$phase" = "Released" ]; then
        # The PVC it was bound to is gone; forget that PVC's uid so a new PVC
        # with the same name (the claimRef) can bind. Data stays as it is.
        run kubectl patch pv "$pv" --type merge -p '{"spec":{"claimRef":{"uid":null,"resourceVersion":null}}}'
    fi
    echo "  $pv: ${phase:-not created yet (dry run)}"
done
