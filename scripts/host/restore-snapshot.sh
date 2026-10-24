#!/bin/bash

# Put production data back to a btrfs snapshot (docs/deployment/BACKUPS.md,
# "Restore from a snapshot").
#
#   sudo scripts/host/restore-snapshot.sh [--dry-run] [--yes] <snapshot>
#
# <snapshot> is a name (or path) in /srv/econgraph/snapshots, e.g.
# data-plane.20261024T040000+0000 or app-prod.pre-20261024T044210Z-recrawl.
# Its prefix says which subvolume it replaces:
#   data-plane.*  -> @econgraph-data-plane at /srv/econgraph/data-plane (pod postgresql-0)
#   app-prod.*    -> @econgraph-app-prod   at /srv/econgraph/app-prod   (pod keycloak-postgres-0)
#
# Steps: refuse unless the Postgres pod using that subvolume is gone (scale
# its StatefulSet to 0 first); unmount the subvolume; rename it to
# @econgraph-<name>.replaced-<UTC stamp> (kept, so the restore can itself
# be undone; delete it later with scripts/host/delete-snapshots.sh); create
# a writable snapshot of <snapshot> under the original name; mount it.
#
# Postgres runs on the kind node econ-graph-worker (labelled
# econgraph.io/host-storage=true), which bind-mounts /srv/econgraph/data-plane
# and /srv/econgraph/app-prod. That bind mount keeps pointing at the OLD
# subvolume until the node container restarts, so the script ends by telling
# you to restart econ-graph-worker before scaling Postgres back up.
#
# Nothing is deleted. Safe to re-run with a different snapshot.

set -euo pipefail

FS_UUID="03ee0fcb-5f19-4f44-8596-e227f1fb647c"
SNAPSHOT_DIR="/srv/econgraph/snapshots"
SRV_ROOT="/srv/econgraph"
KIND_NODE="econ-graph-control-plane"      # where kubectl runs
STORAGE_NODE="econ-graph-worker"          # the node that bind-mounts /srv/econgraph/*
NAMESPACE="econ-graph"

DRY_RUN="false"
YES="false"
snap=""
for arg in "$@"; do
    case "$arg" in
        --dry-run|-n) DRY_RUN="true" ;;
        --yes|-y) YES="true" ;;
        -*) echo "usage: $0 [--dry-run] [--yes] <snapshot>" >&2; exit 2 ;;
        *) snap="$arg" ;;
    esac
done
[ -n "$snap" ] || { echo "usage: $0 [--dry-run] [--yes] <snapshot>" >&2; exit 2; }

run() {
    echo "+ $*"
    if [ "$DRY_RUN" = "false" ]; then
        "$@"
    fi
}

if [ "$(id -u)" -ne 0 ]; then
    echo "error: run with sudo" >&2
    exit 1
fi
OWNER="${SUDO_USER:-}"
if [ -z "$OWNER" ] || [ "$OWNER" = "root" ]; then
    echo "error: run with sudo from the account that runs the kind cluster (SUDO_USER is unset)" >&2
    exit 1
fi

snap="$(basename "$snap")"
case "$snap" in
    data-plane.*) name="data-plane"; pod="postgresql-0" ;;
    app-prod.*) name="app-prod"; pod="keycloak-postgres-0" ;;
    *) echo "error: $snap is not a data-plane.* or app-prod.* snapshot" >&2; exit 1 ;;
esac
target="$SRV_ROOT/$name"
subvol="@econgraph-$name"

if ! btrfs subvolume show "$SNAPSHOT_DIR/$snap" >/dev/null 2>&1; then
    echo "error: $SNAPSHOT_DIR/$snap is not a subvolume" >&2
    exit 1
fi
if [ "$(btrfs property get "$SNAPSHOT_DIR/$snap" ro)" != "ro=true" ]; then
    echo "error: $SNAPSHOT_DIR/$snap is not read-only; refusing to restore from a snapshot that may have been modified" >&2
    exit 1
fi

# Postgres must be stopped: ask the cluster (rootless Docker belongs to OWNER).
owner_uid="$(id -u "$OWNER")"
pod_state="$(sudo -u "$OWNER" env DOCKER_HOST="unix:///run/user/$owner_uid/docker.sock" \
    docker exec "$KIND_NODE" kubectl -n "$NAMESPACE" get pod "$pod" --ignore-not-found -o name 2>&1)" || {
    echo "error: could not ask the cluster whether $pod is running: $pod_state" >&2
    exit 1
}
if [ -n "$pod_state" ]; then
    echo "error: $pod still exists. Stop it first, e.g.:" >&2
    echo "  docker exec $KIND_NODE kubectl -n $NAMESPACE scale statefulset/${pod%-0} --replicas=0" >&2
    echo "  (scale the backend and crawler to 0 too, so nothing reconnects mid-restore)" >&2
    exit 1
fi
echo "= $pod is not running"

stamp="$(date -u +%Y%m%dT%H%M%SZ)"
echo "Restore plan:"
echo "  $subvol (mounted at $target) -> renamed $subvol.replaced-$stamp (kept)"
echo "  new writable $subvol <- snapshot of $SNAPSHOT_DIR/$snap"
if [ "$YES" = "false" ] && [ "$DRY_RUN" = "false" ]; then
    read -r -p "Proceed? [y/N] " answer
    case "$answer" in y|Y|yes) ;; *) echo "aborted"; exit 1 ;; esac
fi

top="$(mktemp -d /mnt/econgraph-top.XXXXXX)"
cleanup() {
    if mountpoint -q "$top"; then umount "$top"; fi
    rmdir "$top"
}
trap cleanup EXIT
if [ "$DRY_RUN" = "true" ]; then
    mount -o ro,subvolid=5 "UUID=$FS_UUID" "$top"
else
    mount -o subvolid=5 "UUID=$FS_UUID" "$top"
fi

for p in "$top/$subvol" "$top/@econgraph-snapshots/$snap"; do
    if ! btrfs subvolume show "$p" >/dev/null 2>&1; then
        echo "error: $p not found at the filesystem top level" >&2
        exit 1
    fi
done

if mountpoint -q "$target"; then
    run umount "$target"
fi
run mv "$top/$subvol" "$top/$subvol.replaced-$stamp"
run btrfs subvolume snapshot "$top/@econgraph-snapshots/$snap" "$top/$subvol"
run mount "$target"

if [ "$DRY_RUN" = "false" ]; then
    findmnt "$target"
fi
cat <<EOF

Restored $target from $snap. The previous data is kept as $subvol.replaced-$stamp.
Keep Postgres stopped. Its kind node, $STORAGE_NODE, still bind-mounts the
old subvolume and only picks up the new one when its container restarts.
As $OWNER, no sudo:
  docker restart $STORAGE_NODE
wait for the node to be Ready, then scale Postgres (and the backend/crawler)
back up and check the data.
EOF
