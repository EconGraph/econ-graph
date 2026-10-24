#!/bin/bash

# Delete snapshots that btrbk does not prune by itself
# (docs/deployment/BACKUPS.md):
#   - pre-change snapshots from scripts/backup/snapshot-now.sh
#     (/srv/econgraph/snapshots/<name>.pre-*), once the change is accepted
#   - subvolumes set aside by scripts/host/restore-snapshot.sh
#     (@econgraph-<name>.replaced-*), once the restore is accepted
#
# Usage: sudo scripts/host/delete-snapshots.sh [--dry-run] <name>...
#
# Each <name> is a snapshot name in /srv/econgraph/snapshots matching
# data-plane.pre-* / app-prod.pre-*, or a top-level subvolume name matching
# @econgraph-data-plane.replaced-* / @econgraph-app-prod.replaced-*.
# Anything else is refused: btrbk owns its own <name>.<timestamp> snapshots,
# and the live subvolumes are never deletable from here.

set -euo pipefail

FS_UUID="03ee0fcb-5f19-4f44-8596-e227f1fb647c"
SNAPSHOT_DIR="/srv/econgraph/snapshots"

DRY_RUN="false"
names=()
for arg in "$@"; do
    case "$arg" in
        --dry-run|-n) DRY_RUN="true" ;;
        -*) echo "usage: $0 [--dry-run] <name>..." >&2; exit 2 ;;
        *) names+=("$(basename "$arg")") ;;
    esac
done
[ "${#names[@]}" -gt 0 ] || { echo "usage: $0 [--dry-run] <name>..." >&2; exit 2; }

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

for n in "${names[@]}"; do
    case "$n" in
        data-plane.pre-*|app-prod.pre-*|@econgraph-data-plane.replaced-*|@econgraph-app-prod.replaced-*) ;;
        *) echo "error: refusing to delete $n (only *.pre-* snapshots and @econgraph-*.replaced-* subvolumes)" >&2; exit 1 ;;
    esac
done

top=""
cleanup() {
    if [ -n "$top" ]; then
        if mountpoint -q "$top"; then umount "$top"; fi
        rmdir "$top"
    fi
}
trap cleanup EXIT

for n in "${names[@]}"; do
    case "$n" in
        @econgraph-*)
            if [ -z "$top" ]; then
                top="$(mktemp -d /mnt/econgraph-top.XXXXXX)"
                mount -o subvolid=5 "UUID=$FS_UUID" "$top"
            fi
            path="$top/$n"
            ;;
        *)
            path="$SNAPSHOT_DIR/$n"
            ;;
    esac
    if ! btrfs subvolume show "$path" >/dev/null 2>&1; then
        echo "= $n does not exist"
        continue
    fi
    run btrfs subvolume delete "$path"
done
