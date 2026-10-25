#!/bin/bash

# Delete snapshots BY HAND (docs/deployment/BACKUPS.md, "Deleting by hand").
# Nothing deletes snapshots automatically; this is the only tool that does,
# and it is never run from a timer: it refuses to run without a terminal
# and asks for typed confirmation every time.
#
# Usage:
#   sudo scripts/host/delete-snapshots.sh [--dry-run] [--pool] <name>...
#   sudo scripts/host/delete-snapshots.sh --list [--pool]
#
# <name> is one of:
#   data-plane.* / app-prod.*      a snapshot in /srv/econgraph/snapshots
#                                  (with --pool: in /srv/econgraph-backups/snapshots,
#                                  including *.incomplete-* leftovers)
#   @econgraph-data-plane.replaced-* / @econgraph-app-prod.replaced-*
#                                  a subvolume set aside by restore-snapshot.sh
# The live subvolumes can never be named here.
#
# It prints exactly what it will delete, warns when a name is the newest
# snapshot of its series (the next hourly copy to the pool then has to fall
# back to an older common parent, or a full send), and proceeds only after
# you type "delete <count>".
#
# --dry-run (-n) writes nothing and mounts nothing: it checks each name and
# prints what would be deleted. Set-aside @econgraph-* subvolumes are looked
# up with `btrfs subvolume list` on the existing /srv/econgraph/snapshots
# mount instead of mounting the filesystem top level.

set -euo pipefail

FS_UUID="03ee0fcb-5f19-4f44-8596-e227f1fb647c"
LOCAL_DIR="/srv/econgraph/snapshots"
POOL_DIR="/srv/econgraph-backups/snapshots"

DRY_RUN="false"
POOL="false"
LIST="false"
names=()
for arg in "$@"; do
    case "$arg" in
        --dry-run|-n) DRY_RUN="true" ;;
        --pool) POOL="true" ;;
        --list) LIST="true" ;;
        -*) echo "usage: $0 [--dry-run] [--pool] <name>... | --list [--pool]" >&2; exit 2 ;;
        *) names+=("$(basename "$arg")") ;;
    esac
done

if [ "$(id -u)" -ne 0 ]; then
    echo "error: run with sudo" >&2
    exit 1
fi

dir="$LOCAL_DIR"
if [ "$POOL" = "true" ]; then
    dir="$POOL_DIR"
    mountpoint -q /srv/econgraph-backups || { echo "error: the backup pool is not mounted" >&2; exit 1; }
fi

if [ "$LIST" = "true" ]; then
    echo "Snapshots in $dir (oldest first):"
    find "$dir" -mindepth 1 -maxdepth 1 -type d -printf '  %f\n' | sort
    exit 0
fi

[ "${#names[@]}" -gt 0 ] || { echo "usage: $0 [--dry-run] [--pool] <name>... | --list [--pool]" >&2; exit 2; }

if [ "$DRY_RUN" = "false" ] && [ ! -t 0 ]; then
    echo "error: this is a manual tool; run it from a terminal (it asks for confirmation)" >&2
    exit 1
fi

for n in "${names[@]}"; do
    case "$n" in
        data-plane.?*|app-prod.?*|@econgraph-data-plane.replaced-?*|@econgraph-app-prod.replaced-?*) ;;
        *) echo "error: refusing $n: not a snapshot or set-aside subvolume name" >&2; exit 1 ;;
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

paths=()
for n in "${names[@]}"; do
    case "$n" in
        @econgraph-*)
            if [ "$DRY_RUN" = "true" ]; then
                # Top-level subvolumes, read through an existing mount.
                if [ "$(findmnt -n -o UUID --mountpoint "$LOCAL_DIR" | tail -n 1)" != "$FS_UUID" ]; then
                    echo "error: $LOCAL_DIR is not mounted from UUID=$FS_UUID; cannot look up $n" >&2
                    exit 1
                fi
                if btrfs subvolume list "$LOCAL_DIR" | sed -nE 's/.* top level 5 path (<FS_TREE>\/)?//p' | grep -qxF "$n"; then
                    echo "= $n: a top-level subvolume of UUID=$FS_UUID"
                    paths+=("<top level of UUID=$FS_UUID>/$n")
                else
                    echo "= $n: not a subvolume here, skipped"
                fi
                continue
            fi
            if [ -z "$top" ]; then
                top="$(mktemp -d /mnt/econgraph-top.XXXXXX)"
                mount -o subvolid=5 "UUID=$FS_UUID" "$top"
            fi
            path="$top/$n"
            ;;
        *)
            path="$dir/$n"
            ;;
    esac
    if [ -L "$path" ] || ! btrfs subvolume show "$path" >/dev/null 2>&1; then
        echo "= $n: not a subvolume here, skipped"
        continue
    fi
    paths+=("$path")
done

if [ "${#paths[@]}" -eq 0 ]; then
    echo "nothing to delete"
    exit 0
fi

echo "Will PERMANENTLY delete ${#paths[@]} subvolume(s):"
for p in "${paths[@]}"; do
    echo "  $p"
    base="$(basename "$p")"
    series="${base%%.*}"
    case "$base" in
        *.incomplete-*|@*) ;;
        *)
            newest="$(find "$(dirname "$p")" -mindepth 1 -maxdepth 1 -type d -name "$series.*" \
                -not -name '*.incomplete-*' -printf '%f\n' \
                | sed -E "s/^${series}\.(pre-)?([0-9]{8}T[0-9]{6}Z).*/\2 &/" | sort | tail -n 1 | cut -d' ' -f2)"
            if [ "$newest" = "$base" ]; then
                echo "    warning: this is the newest $series snapshot here"
            fi
            ;;
    esac
done

if [ "$DRY_RUN" = "true" ]; then
    echo "(dry run: nothing deleted)"
    exit 0
fi

read -r -p "Type 'delete ${#paths[@]}' to proceed: " answer
if [ "$answer" != "delete ${#paths[@]}" ]; then
    echo "aborted; nothing deleted"
    exit 1
fi

for p in "${paths[@]}"; do
    echo "+ btrfs subvolume delete $p"
    btrfs subvolume delete "$p"
done
