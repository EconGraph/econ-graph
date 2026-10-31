#!/bin/bash

# Delete snapshots BY HAND (docs/deployment/BACKUPS.md, "Deleting by hand").
# Nothing deletes snapshots automatically; this is the only tool that does,
# and it is never run from a timer: it refuses to run without a terminal
# and asks for typed confirmation every time. Before it looks at any name
# it takes the lock the hourly backup-snapshots.sh holds
# (/run/econgraph/backup-snapshots.lock), and keeps it until it exits; if
# the lock is held it stops, deleting nothing.
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
#   @econgraph-data-plane.failed-restore-* / @econgraph-app-prod.failed-restore-*
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
# ECONGRAPH_TEST_ROOT, if set, is prefixed to the lock file only, so
# scripts/host/test-backup-scripts.sh can check the lock against stubs.
LOCK_FILE="${ECONGRAPH_TEST_ROOT:-}/run/econgraph/backup-snapshots.lock"
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

# >>> backup lock: the same in backup-snapshots.sh, restore-snapshot.sh,
# delete-snapshots.sh and snapshot-now.sh (test-backup-scripts.sh checks).
# take_backup_lock WAIT: take the backup lock, /run/econgraph/backup-snapshots.lock,
# on fd 9, waiting up to WAIT seconds, and keep it until the script exits.
# /run/econgraph is root's, mode 0700, so no other user can open the file to
# hold the lock; a directory or file that is not as this function makes them
# is refused, never used. The hourly job installed by install-backup-timer.sh
# must take this same lock: an installed copy that does not (one installed
# before it moved here, which locks /run/econgraph-backup-snapshots.lock) is
# refused until the installer is re-run. Returns 1 when the lock is held, 2
# on any other failure.
take_backup_lock() {
    local wait="$1" root="${ECONGRAPH_TEST_ROOT:-}" owner="0:0" old_umask status
    local dir file installed
    dir="$root/run/econgraph"
    file="$dir/backup-snapshots.lock"
    installed="$root/usr/local/sbin/econgraph-backup-snapshots"
    if [ -n "$root" ]; then
        # Test runs only (scripts/host/test-backup-scripts.sh, never root):
        # the test user's own files.
        if [ "$EUID" -eq 0 ]; then
            echo "error: ECONGRAPH_TEST_ROOT is set; refusing to run as root with it" >&2
            return 2
        fi
        owner="${BACKUP_LOCK_OWNER:-0:0}"
    fi
    if [ -e "$installed" ] && ! grep -qF 'run/econgraph/backup-snapshots.lock' "$installed"; then
        echo "error: the installed hourly job $installed takes an older lock, so it would not" >&2
        echo "  wait for this run. Re-run sudo scripts/host/install-backup-timer.sh first." >&2
        return 2
    fi
    if [ ! -e "$dir" ] && ! mkdir -m 0700 "$dir" 2>/dev/null && [ ! -d "$dir" ]; then
        echo "error: could not create $dir" >&2
        return 2
    fi
    if [ -L "$dir" ] || [ ! -d "$dir" ] || [ "$(stat -c '%u:%g %a' "$dir")" != "$owner 700" ]; then
        echo "error: $dir is not a directory owned by $owner with mode 0700; refusing to lock in it." >&2
        echo "  Check what made it; if no backup, restore or delete is running, remove it and re-run." >&2
        return 2
    fi
    if [ -L "$file" ] || { [ -e "$file" ] && [ ! -f "$file" ]; }; then
        echo "error: $file is not a regular file; refusing to use it as the lock" >&2
        return 2
    fi
    old_umask="$(umask)"
    umask 077
    if ! exec 9>>"$file"; then
        umask "$old_umask"
        echo "error: could not open $file" >&2
        return 2
    fi
    umask "$old_umask"
    status=0
    flock -w "$wait" 9 || status=$?
    if [ "$status" -eq 1 ]; then
        return 1
    elif [ "$status" -ne 0 ]; then
        echo "error: could not lock $file (flock exited $status)" >&2
        return 2
    fi
}
# <<< backup lock

# The backup lock, held until this script exits, before the first check of
# a name: no backup run sends, or restore uses, a snapshot while it is
# deleted. A dry run takes none.
if [ "$DRY_RUN" = "false" ]; then
    lock_status=0
    take_backup_lock 0 || lock_status=$?
    if [ "$lock_status" -eq 1 ]; then
        echo "error: $LOCK_FILE is held: a backup-snapshots run (econgraph-backup-snapshots.service)," >&2
        echo "  a restore or another delete is in progress. Nothing was deleted; re-run when it is done." >&2
    fi
    [ "$lock_status" -eq 0 ] || exit 1
fi

if [ "$DRY_RUN" = "false" ] && [ ! -t 0 ]; then
    echo "error: this is a manual tool; run it from a terminal (it asks for confirmation)" >&2
    exit 1
fi

for n in "${names[@]}"; do
    case "$n" in
        data-plane.?*|app-prod.?*|@econgraph-data-plane.replaced-?*|@econgraph-app-prod.replaced-?*) ;;
        @econgraph-data-plane.failed-restore-?*|@econgraph-app-prod.failed-restore-?*) ;;
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
                # The whole list is read first (no grep -q in a pipeline,
                # which could end it early and trip pipefail).
                if ! subvol_list="$(btrfs subvolume list "$LOCAL_DIR")"; then
                    echo "error: btrfs subvolume list $LOCAL_DIR failed; cannot look up $n" >&2
                    exit 1
                fi
                if sed -nE 's/.* top level 5 path (<FS_TREE>\/)?//p' <<<"$subvol_list" | grep -xF "$n" >/dev/null; then
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
