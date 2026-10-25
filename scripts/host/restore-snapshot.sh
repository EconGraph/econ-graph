#!/bin/bash

# Put production data back to a btrfs snapshot (docs/deployment/BACKUPS.md,
# "Restore from a snapshot").
#
#   sudo scripts/host/restore-snapshot.sh [--dry-run] [--yes] [--from-pool] <snapshot>
#
# <snapshot> is a name (or path) in /srv/econgraph/snapshots, e.g.
# data-plane.20261024T040000Z or app-prod.pre-20261024T044210Z-recrawl.
# With --from-pool it is a name in /srv/econgraph-backups/snapshots (the
# copy on the SATA backup pool): it is first copied back into
# /srv/econgraph/snapshots with btrfs send/receive, then restored as usual.
# Its prefix says which subvolume it replaces:
#   data-plane.*  -> @econgraph-data-plane at /srv/econgraph/data-plane (pod postgresql-0)
#   app-prod.*    -> @econgraph-app-prod   at /srv/econgraph/app-prod   (pod keycloak-postgres-0)
#
# Steps: refuse unless the Postgres pod using that subvolume is gone (scale
# its StatefulSet to 0 first); ask you to type the subvolume name (e.g.
# @econgraph-data-plane; refused without a terminal unless --yes); unmount the subvolume; rename it to
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
#
# --dry-run (-n) writes nothing and mounts nothing: it runs the read-only
# checks (the snapshot is a read-only subvolume, the Postgres pod is gone,
# and, through findmnt on the existing mounts, that the target and the
# snapshot directory are the expected subvolumes of this filesystem), then
# prints each step. With --from-pool it stops before the copy back.

set -euo pipefail

FS_UUID="03ee0fcb-5f19-4f44-8596-e227f1fb647c"
SNAPSHOT_DIR="/srv/econgraph/snapshots"
SRV_ROOT="/srv/econgraph"
KIND_NODE="econ-graph-control-plane"      # where kubectl runs
STORAGE_NODE="econ-graph-worker"          # the node that bind-mounts /srv/econgraph/*
NAMESPACE="econ-graph"
# The rootful daemon the kind cluster runs on (as scripts/deploy/lib/kind-env.sh;
# pass ECONGRAPH_DOCKER_HOST through sudo to override).
DOCKER_SOCK="${ECONGRAPH_DOCKER_HOST:-unix:///var/run/docker.sock}"

DRY_RUN="false"
YES="false"
FROM_POOL="false"
POOL_SNAPSHOT_DIR="/srv/econgraph-backups/snapshots"
snap=""
for arg in "$@"; do
    case "$arg" in
        --dry-run|-n) DRY_RUN="true" ;;
        --yes|-y) YES="true" ;;
        --from-pool) FROM_POOL="true" ;;
        -*) echo "usage: $0 [--dry-run] [--yes] [--from-pool] <snapshot>" >&2; exit 2 ;;
        *) snap="$arg" ;;
    esac
done
[ -n "$snap" ] || { echo "usage: $0 [--dry-run] [--yes] [--from-pool] <snapshot>" >&2; exit 2; }

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

# mounted_subvol <mountpoint>: "<uuid> <subvolume path>" of the mount there.
mounted_subvol() {
    findmnt -n -o UUID,FSROOT --mountpoint "$1" | tail -n 1
}
# mount_target: mount $target from /etc/fstab (its options), then check it
# really is $subvol on this filesystem; if not, unmount it again and fail.
mount_target() {
    mount "$target" || return 1
    if [ "$(mounted_subvol "$target")" != "$FS_UUID /$subvol" ]; then
        echo "error: /etc/fstab mounted $target from $(mounted_subvol "$target"), not UUID=$FS_UUID /$subvol" >&2
        umount "$target" || true
        return 1
    fi
}
# Dry run or not, before anything else: the checks below read
# $SNAPSHOT_DIR/$snap and the restore uses @econgraph-snapshots/$snap at the
# top level, so $SNAPSHOT_DIR must be that subvolume (and $target, if
# mounted, the one being replaced).
if [ "$(mounted_subvol "$SNAPSHOT_DIR")" != "$FS_UUID /@econgraph-snapshots" ]; then
    echo "error: $SNAPSHOT_DIR is not @econgraph-snapshots on UUID=$FS_UUID" >&2
    exit 1
fi
if mountpoint -q "$target"; then
    if [ "$(mounted_subvol "$target")" != "$FS_UUID /$subvol" ]; then
        echo "error: $target is not $subvol on UUID=$FS_UUID" >&2
        exit 1
    fi
    echo "= $target is $subvol on UUID=$FS_UUID"
elif [ "$DRY_RUN" = "true" ]; then
    echo "= $target is not mounted, so a dry run cannot confirm $subvol exists"
fi

if [ "$FROM_POOL" = "true" ]; then
    if ! mountpoint -q /srv/econgraph-backups; then
        echo "error: the backup pool is not mounted at /srv/econgraph-backups" >&2
        exit 1
    fi
    if [ "$(btrfs property get "$POOL_SNAPSHOT_DIR/$snap" ro 2>/dev/null)" != "ro=true" ]; then
        echo "error: $POOL_SNAPSHOT_DIR/$snap is not a complete read-only snapshot" >&2
        exit 1
    fi
    if [ -e "$SNAPSHOT_DIR/$snap" ]; then
        echo "= $SNAPSHOT_DIR/$snap already exists locally; using it"
    else
        echo "+ btrfs send $POOL_SNAPSHOT_DIR/$snap | btrfs receive $SNAPSHOT_DIR"
        if [ "$DRY_RUN" = "true" ]; then
            echo "(dry run: stopping here, the remaining checks need the local copy)"
            exit 0
        fi
        if ! btrfs send -q "$POOL_SNAPSHOT_DIR/$snap" | btrfs receive "$SNAPSHOT_DIR"; then
            echo "error: copying $snap back from the backup pool failed" >&2
            if [ -e "$SNAPSHOT_DIR/$snap" ]; then
                # Set the partial copy aside (never delete it), so a rerun
                # receives again instead of finding a writable leftover.
                leftover="$SNAPSHOT_DIR/$snap.incomplete-$(date -u +%Y%m%dT%H%M%SZ)"
                echo "  the partial copy is set aside as $leftover (not deleted)" >&2
                mv "$SNAPSHOT_DIR/$snap" "$leftover" || echo "  could not rename it; move it aside by hand" >&2
            fi
            exit 1
        fi
    fi
fi

if ! btrfs subvolume show "$SNAPSHOT_DIR/$snap" >/dev/null 2>&1; then
    echo "error: $SNAPSHOT_DIR/$snap is not a subvolume" >&2
    exit 1
fi
if [ "$(btrfs property get "$SNAPSHOT_DIR/$snap" ro)" != "ro=true" ]; then
    echo "error: $SNAPSHOT_DIR/$snap is not read-only; refusing to restore from a snapshot that may have been modified" >&2
    exit 1
fi

# Postgres must be stopped: ask the cluster, on the rootful daemon.
pod_state="$(env -u DOCKER_CONTEXT DOCKER_HOST="$DOCKER_SOCK" \
    docker exec "$KIND_NODE" kubectl -n "$NAMESPACE" get pod "$pod" --ignore-not-found -o name 2>&1)" || {
    echo "error: could not ask the cluster ($KIND_NODE on $DOCKER_SOCK) whether $pod is running: $pod_state" >&2
    exit 1
}
if [ -n "$pod_state" ]; then
    echo "error: $pod still exists. Stop it first, e.g.:" >&2
    echo "  docker -H $DOCKER_SOCK exec $KIND_NODE kubectl -n $NAMESPACE scale statefulset/${pod%-0} --replicas=0" >&2
    echo "  (scale the backend and crawler to 0 too, so nothing reconnects mid-restore)" >&2
    exit 1
fi
echo "= $pod is not running"

stamp="$(date -u +%Y%m%dT%H%M%SZ)"
echo "Restore plan:"
echo "  $subvol (mounted at $target) -> renamed $subvol.replaced-$stamp (kept)"
echo "  new writable $subvol <- snapshot of $SNAPSHOT_DIR/$snap"
if [ "$YES" = "false" ] && [ "$DRY_RUN" = "false" ]; then
    if [ ! -t 0 ]; then
        echo "error: no terminal to confirm on; refusing (pass --yes only if you mean it)" >&2
        exit 1
    fi
    read -r -p "Type '$subvol' to replace it: " answer
    if [ "$answer" != "$subvol" ]; then
        echo "aborted; nothing changed"
        exit 1
    fi
fi

if [ "$DRY_RUN" = "true" ]; then
    # No temporary directory and no top-level mount in a dry run.
    echo "+ mount -o subvolid=5 UUID=$FS_UUID <temporary directory under /mnt>   (not in a dry run)"
    top="<top>"
else
    top="$(mktemp -d /mnt/econgraph-top.XXXXXX)"
    cleanup() {
        if mountpoint -q "$top"; then umount "$top"; fi
        rmdir "$top"
    }
    trap cleanup EXIT
    mount -o subvolid=5 "UUID=$FS_UUID" "$top"

    for p in "$top/$subvol" "$top/@econgraph-snapshots/$snap"; do
        if ! btrfs subvolume show "$p" >/dev/null 2>&1; then
            echo "error: $p not found at the filesystem top level" >&2
            exit 1
        fi
    done
fi

if mountpoint -q "$target"; then
    run umount "$target"
fi
run mv "$top/$subvol" "$top/$subvol.replaced-$stamp"
# put_back: rename $subvol.replaced-$stamp back to $subvol, remount $target,
# say that nothing was restored, exit 1. Only renames; nothing is deleted.
put_back() {
    echo "  renaming $subvol.replaced-$stamp back to $subvol" >&2
    if [ -e "$top/$subvol" ] || ! mv "$top/$subvol.replaced-$stamp" "$top/$subvol"; then
        echo "error: could not rename it back. The data is intact in $subvol.replaced-$stamp" >&2
        echo "  at the top level of UUID=$FS_UUID. To put it back by hand:" >&2
        echo "    sudo mount -o subvolid=5 UUID=$FS_UUID /mnt" >&2
        echo "    sudo mv /mnt/$subvol.replaced-$stamp /mnt/$subvol   (after checking /mnt/$subvol does not exist)" >&2
        echo "    sudo umount /mnt && sudo mount $target" >&2
        exit 1
    fi
    if ! mount_target; then
        echo "error: $subvol is back under its name but mounting $target failed; check /etc/fstab, then: sudo mount $target" >&2
    fi
    echo "Nothing was restored; $target holds the data it had before." >&2
    exit 1
}
if ! run btrfs subvolume snapshot "$top/@econgraph-snapshots/$snap" "$top/$subvol"; then
    echo "error: creating $subvol from $snap failed" >&2
    put_back
fi
if [ "$DRY_RUN" = "true" ]; then
    echo "+ mount $target   (from /etc/fstab, then checked to be UUID=$FS_UUID /$subvol)"
elif ! mount_target; then
    # Set the new copy aside (kept, not deleted) and put the original back.
    echo "error: mounting the restored $subvol at $target failed" >&2
    if ! mv "$top/$subvol" "$top/$subvol.failed-restore-$stamp"; then
        echo "error: could not set the restored copy aside; $subvol.replaced-$stamp still holds the previous data" >&2
        exit 1
    fi
    echo "  the restored copy is kept as $subvol.failed-restore-$stamp" >&2
    put_back
fi

if [ "$DRY_RUN" = "false" ]; then
    findmnt "$target"
fi
cat <<EOF

Restored $target from $snap. The previous data is kept as $subvol.replaced-$stamp.
Keep Postgres stopped. Its kind node, $STORAGE_NODE, still bind-mounts the
old subvolume and only picks up the new one when its container restarts.
As $OWNER, no sudo (the rootful daemon, through the docker group):
  docker -H $DOCKER_SOCK restart $STORAGE_NODE
wait for the node to be Ready, then scale Postgres (and the backend/crawler)
back up and check the data.
EOF
