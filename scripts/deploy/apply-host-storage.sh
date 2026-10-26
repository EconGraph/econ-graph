#!/bin/bash

# Apply the StorageClass econgraph-host and the static PersistentVolumes for
# PostgreSQL and Keycloak's PostgreSQL (k8s/manifests/host-storage.yaml), after
# checking that they can only land on the host btrfs subvolumes. deploy.sh runs
# this before anything that touches Postgres; see docs/deployment/HOST_STORAGE.md.
#
#   1. /srv/econgraph/data-plane and /srv/econgraph/app-prod are mounted btrfs
#      subvolumes owned by you (scripts/host/setup-subvolumes.sh creates them).
#   2. Exactly one node, econ-graph-worker, is labelled
#      econgraph.io/host-storage=true and bind-mounts both, so that the data
#      directories have the same inode numbers there as on the host (the
#      cluster was created from terraform/k8s/kind-config.yaml).
#   3. The data directories pg/ and keycloak-pg/ exist. They are created only
#      with --create-dirs; otherwise a missing one stops the deploy.
#   4. If a data directory already holds a database but the Secret with the
#      passwords it was initialised with is missing, stop: create-secrets.sh
#      would generate new passwords that don't match it.
#   5. kubectl apply host-storage.yaml. On a cluster that already runs these
#      objects this changes nothing. A PV left Released by a deleted PVC (after
#      the namespace was deleted) is re-armed, so the StatefulSet's new PVC
#      binds to the same directory.
#
# Usage: scripts/deploy/apply-host-storage.sh [--dry-run] [--create-dirs]
#   --dry-run      run the checks, then show `kubectl diff` of host-storage.yaml
#                  and what would be created or re-armed; change nothing
#   --create-dirs  create a missing pg/ or keycloak-pg/ (as you, mode 0700)
#
# Never deletes a PV, a PVC, a directory or anything in one, and never changes
# the ownership of anything on the host.
#
# kubectl is ${KUBECTL:-kubectl} through kc (scripts/deploy/lib/cluster-role.sh):
# on the context deploy.sh pinned, or, run by hand, the kind cluster's own
# kubectl and kubeconfig from ~/.cache/econgraph when KUBECTL and KUBECONFIG
# are unset. docker reaches the daemon the kind cluster runs on through the
# caller's DOCKER_HOST, as `kind` in deploy.sh does.
#
# Environment: ECONGRAPH_HOST_ROOT (default /srv/econgraph) is where the host
# side is checked; the PVs always point at /srv/econgraph, so change it only
# for tests. KIND_STORAGE_NODE (default econ-graph-worker) is the node
# container that bind-mounts the subvolumes. NAMESPACE (default econ-graph).

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/deploy/lib/cluster-role.sh
source "${PROJECT_ROOT}/scripts/deploy/lib/cluster-role.sh"

DRY_RUN=false
CREATE_DIRS=false
for arg in "$@"; do
    case "$arg" in
        --dry-run|-n) DRY_RUN=true ;;
        --create-dirs) CREATE_DIRS=true ;;
        *) echo "usage: $0 [--dry-run] [--create-dirs]" >&2; exit 2 ;;
    esac
done

MANIFEST="${PROJECT_ROOT}/k8s/manifests/host-storage.yaml"
NAMESPACE="${NAMESPACE:-econ-graph}"
KIND_STORAGE_NODE="${KIND_STORAGE_NODE:-econ-graph-worker}"
STORAGE_LABEL="econgraph.io/host-storage=true"
# The paths in host-storage.yaml, which the node sees at the same place.
NODE_ROOT="/srv/econgraph"
HOST_ROOT="${ECONGRAPH_HOST_ROOT:-/srv/econgraph}"
SUBVOLUMES=(data-plane app-prod)
# subvolume/data-directory, the PGDATA marker inside it, its Secret, its PV
DATABASES=(
    "data-plane/pg pgdata/PG_VERSION econ-graph-postgres econgraph-data-plane-pg"
    "app-prod/keycloak-pg 18/docker/PG_VERSION econ-graph-keycloak econgraph-app-prod-keycloak-pg"
)

log() { echo "==> $*"; }
die() {
    echo "error: $*" >&2
    exit 1
}
# Run a command, or only print it under --dry-run.
run() {
    echo "+ $*"
    if [ "$DRY_RUN" != true ]; then
        "$@"
    fi
}

[ "$DRY_RUN" = true ] && log "DRY RUN: the checks run for real; nothing is changed"

# A context pinned by deploy.sh is kept; run by hand, pin the current one.
cluster_role_default_kubectl
cluster_role_pin_context || die "${KUBECTL:-kubectl} has no current context"
echo "  kubectl: ${KUBECTL:-kubectl}, context ${CLUSTER_ROLE_CONTEXT}"

log "Checking the host subvolumes under ${HOST_ROOT}"
ok=true
for sub in "${SUBVOLUMES[@]}"; do
    dir="${HOST_ROOT}/${sub}"
    if ! mountpoint -q "$dir" 2>/dev/null; then
        echo "  $dir is not a mount point" >&2
        ok=false
        continue
    fi
    fstype="$(findmnt -no FSTYPE --mountpoint "$dir")"
    owner="$(stat -c %U "$dir")"
    [ "$fstype" = btrfs ] || { echo "  $dir is ${fstype:-unknown}, not btrfs" >&2; ok=false; }
    [ "$owner" = "$(id -un)" ] || { echo "  $dir is owned by $owner, not $(id -un)" >&2; ok=false; }
done
[ "$ok" = true ] ||
    die "the host subvolumes are not ready. Create them with sudo scripts/host/setup-subvolumes.sh (docs/deployment/HOST_STORAGE.md)"
echo "  ok: ${SUBVOLUMES[*]} are mounted btrfs subvolumes owned by $(id -un)"

missing_dirs=()
for db in "${DATABASES[@]}"; do
    read -r datadir _ _ _ <<<"$db"
    [ -d "${HOST_ROOT}/${datadir}" ] || missing_dirs+=("${HOST_ROOT}/${datadir}")
done
if [ "${#missing_dirs[@]}" -gt 0 ]; then
    if [ "$CREATE_DIRS" != true ]; then
        die "missing data director$([ "${#missing_dirs[@]}" -gt 1 ] && echo ies || echo y): ${missing_dirs[*]}
  On a new host, create them with: scripts/deploy/apply-host-storage.sh --create-dirs
  (try --dry-run first). On a host that had them, find out why they are gone first."
    fi
    for dir in "${missing_dirs[@]}"; do
        run mkdir -m 0700 "$dir"
    done
fi

log "Checking node ${KIND_STORAGE_NODE}"
labelled="$(kc get nodes -l "$STORAGE_LABEL" -o name)" ||
    die "could not list the nodes labelled ${STORAGE_LABEL}"
[ "$labelled" = "node/${KIND_STORAGE_NODE}" ] ||
    die "expected exactly node/${KIND_STORAGE_NODE} to carry ${STORAGE_LABEL}, found: ${labelled:-none}.
  The cluster predates the host-storage kind config (terraform/k8s/kind-config.yaml);
  see docs/deployment/HOST_STORAGE.md."
for sub in "${SUBVOLUMES[@]}"; do
    docker exec "$KIND_STORAGE_NODE" mountpoint -q "${NODE_ROOT}/${sub}" ||
        die "node ${KIND_STORAGE_NODE} does not bind-mount ${NODE_ROOT}/${sub}.
  The cluster predates the host-storage kind config (terraform/k8s/kind-config.yaml);
  see docs/deployment/HOST_STORAGE.md."
done
# The same directories, not just something mounted at those paths: each data
# directory has the same inode number on the host and in the node.
for db in "${DATABASES[@]}"; do
    read -r datadir _ _ _ <<<"$db"
    [ -d "${HOST_ROOT}/${datadir}" ] || continue # only under --dry-run --create-dirs
    host_inode="$(stat -c %i "${HOST_ROOT}/${datadir}")"
    node_inode="$(docker exec "$KIND_STORAGE_NODE" stat -c %i "${NODE_ROOT}/${datadir}")" ||
        die "node ${KIND_STORAGE_NODE} has no ${NODE_ROOT}/${datadir}"
    [ "$host_inode" = "$node_inode" ] ||
        die "${NODE_ROOT}/${datadir} in node ${KIND_STORAGE_NODE} (inode ${node_inode}) is not the host's ${HOST_ROOT}/${datadir} (inode ${host_inode})"
done
echo "  ok: ${KIND_STORAGE_NODE} is labelled ${STORAGE_LABEL} and bind-mounts ${SUBVOLUMES[*]}"

# Once Postgres has run, its directory belongs to uid 999, mode 0700, so look
# from inside the node (root there) rather than from the host as you.
log "Checking each existing database has its Secret"
for db in "${DATABASES[@]}"; do
    read -r datadir marker secret _ <<<"$db"
    if docker exec "$KIND_STORAGE_NODE" test -e "${NODE_ROOT}/${datadir}/${marker}"; then
        found="$(kc -n "$NAMESPACE" get secret "$secret" -o name --ignore-not-found)" ||
            die "could not look up Secret ${secret}"
        [ -n "$found" ] ||
            die "${NODE_ROOT}/${datadir} already holds a database, but Secret ${secret} is missing.
  Restore the Secret it was initialised with before deploying; new generated
  passwords would not match the database."
        echo "  ${datadir}: database present, Secret ${secret} present"
    else
        echo "  ${datadir}: no database yet"
    fi
done

log "Applying k8s/manifests/host-storage.yaml"
if [ "$DRY_RUN" = true ]; then
    echo "+ kubectl diff -f k8s/manifests/host-storage.yaml (no output: no change)"
    rc=0
    kc diff -f "$MANIFEST" || rc=$?
    [ "$rc" -le 1 ] || die "kubectl diff failed"
else
    kc apply -f "$MANIFEST"
fi

for db in "${DATABASES[@]}"; do
    read -r _ _ _ pv <<<"$db"
    state="$(kc get pv "$pv" --ignore-not-found \
        -o jsonpath='{.status.phase} {.spec.claimRef.uid} {.spec.claimRef.resourceVersion}')" ||
        die "could not read PV ${pv}"
    read -r phase old_uid old_rv <<<"$state" || true
    if [ "$phase" = Released ] && [ -n "$old_uid" ]; then
        # The PVC it was bound to is gone; forget that PVC's uid so the new PVC
        # of the same name (the claimRef) can bind. The data is not touched.
        # The test op makes the patch fail instead of unbinding a claim that
        # changed since it was read (say, a new PVC bound in between).
        [[ "$old_uid" =~ ^[0-9a-f-]+$ ]] || die "PV ${pv}: unexpected claimRef uid '${old_uid}'"
        ops='{"op":"test","path":"/spec/claimRef/uid","value":"'"$old_uid"'"},{"op":"remove","path":"/spec/claimRef/uid"}'
        if [ -n "$old_rv" ]; then
            ops+=',{"op":"remove","path":"/spec/claimRef/resourceVersion"}'
        fi
        run kc patch pv "$pv" --type json -p "[${ops}]"
    fi
    echo "  ${pv}: ${phase:-not created yet}"
done
