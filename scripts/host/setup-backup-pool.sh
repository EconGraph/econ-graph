#!/bin/bash

# Make the 3-disk Samsung 870 SATA btrfs pool the local backup target
# (docs/deployment/BACKUPS.md).
#
# The pool (sda1, sdb1, sdc1; UUID a1e61098-087b-4403-8765-b714bf7b240b,
# label home-temp, data RAID1 / metadata RAID1C3) is NOT empty: its top level
# holds @home with nested snapshots @home/.final-snap and
# @home/.final-snap2, an older copy of /home that Joe decides about
# separately. This script never touches them. It only:
#
#   - creates a NEW top-level subvolume @econgraph-backups (if absent)
#   - mounts it at /srv/econgraph-backups via an /etc/fstab line (nofail, so
#     a missing disk never blocks boot)
#   - refuses an existing fstab entry or mount at /srv/econgraph-backups
#     that is not this pool's @econgraph-backups
#   - keeps the subvolume root root:root 0755
#   - creates (or, on a rerun, corrects the owner and mode of) snapshots/
#     (root:root 0700: where backup-snapshots.sh receives the snapshots, out
#     of reach of the jmalicki account) and dumps/ (sudo user, 0700: the
#     nightly logical dumps)
#
# It is a different filesystem from /home, so snapshots reach it only via
# btrfs send/receive (scripts/host/backup-snapshots.sh, as root), never cp.
# Nothing here deletes anything. Safe to re-run.
#
# The next hourly backup-snapshots.sh run starts copying to it by itself.
#
# Usage: sudo scripts/host/setup-backup-pool.sh [--dry-run]

set -euo pipefail

FS_UUID="a1e61098-087b-4403-8765-b714bf7b240b"
SUBVOL="@econgraph-backups"
TARGET="/srv/econgraph-backups"
MOUNT_OPTS="compress=zstd:3,ssd,discard=async,space_cache=v2,noatime,nofail,x-systemd.device-timeout=30s"

DRY_RUN="false"
for arg in "$@"; do
    case "$arg" in
        --dry-run|-n) DRY_RUN="true" ;;
        *) echo "usage: $0 [--dry-run]" >&2; exit 2 ;;
    esac
done

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
    echo "error: run with sudo from the account that writes the dumps (SUDO_USER is unset)" >&2
    exit 1
fi

if ! blkid -U "$FS_UUID" >/dev/null; then
    echo "error: no filesystem with UUID $FS_UUID (try: btrfs device scan)" >&2
    exit 1
fi

top="$(mktemp -d /mnt/econgraph-backups-top.XXXXXX)"
cleanup() {
    if mountpoint -q "$top"; then umount "$top"; fi
    rmdir "$top"
}
trap cleanup EXIT

# The top level is mounted even in a dry run, read-only, so the checks are real.
if [ "$DRY_RUN" = "true" ]; then
    mount -o ro,subvolid=5 "UUID=$FS_UUID" "$top"
else
    mount -o subvolid=5 "UUID=$FS_UUID" "$top"
fi

echo "= existing top-level entries on the pool (left untouched):"
find "$top" -mindepth 1 -maxdepth 1 -printf '    %f\n'

if [ -e "$top/$SUBVOL" ]; then
    if ! btrfs subvolume show "$top/$SUBVOL" >/dev/null 2>&1; then
        echo "error: $SUBVOL exists on the pool but is not a subvolume" >&2
        exit 1
    fi
    echo "= subvolume $SUBVOL already exists"
else
    run btrfs subvolume create "$top/$SUBVOL"
fi

if [ ! -d "$TARGET" ]; then
    run mkdir -p "$TARGET"
fi

line="UUID=$FS_UUID $TARGET btrfs subvol=/$SUBVOL,$MOUNT_OPTS 0 0"
# An existing fstab entry for $TARGET is accepted only if it mounts this
# pool's $SUBVOL; anything else is refused before any ownership changes.
existing="$(awk -v t="$TARGET" '$1 !~ /^#/ && $2 == t' /etc/fstab)"
if [ -n "$existing" ]; then
    if [ "$(wc -l <<<"$existing")" -ne 1 ]; then
        echo "error: /etc/fstab has more than one entry for $TARGET:" >&2
        echo "$existing" >&2
        exit 1
    fi
    read -r fs_src _ fs_type fs_opts _ <<<"$existing"
    if [ "$fs_src" != "UUID=$FS_UUID" ] || [ "$fs_type" != "btrfs" ] \
        || ! grep -qE "(^|,)subvol=/?$SUBVOL(,|$)" <<<"$fs_opts"; then
        echo "error: /etc/fstab already mounts $TARGET from something else:" >&2
        echo "  $existing" >&2
        echo "  expected: $line" >&2
        exit 1
    fi
    echo "= fstab already mounts $TARGET from UUID=$FS_UUID subvol=/$SUBVOL"
else
    echo "+ append to /etc/fstab: $line"
    if [ "$DRY_RUN" = "false" ]; then
        echo "$line" >>/etc/fstab
        systemctl daemon-reload
    fi
fi

# A mount at $TARGET is accepted only if it is this pool's $SUBVOL.
check_mount() {
    local uuid fsroot
    read -r uuid fsroot < <(findmnt -n -o UUID,FSROOT --mountpoint "$TARGET" | tail -n 1)
    if [ "$uuid" != "$FS_UUID" ] || [ "$fsroot" != "/$SUBVOL" ]; then
        echo "error: $TARGET is mounted from UUID=$uuid subvolume $fsroot, not UUID=$FS_UUID /$SUBVOL; refusing" >&2
        exit 1
    fi
}
if mountpoint -q "$TARGET"; then
    check_mount
    echo "= $TARGET already mounted from UUID=$FS_UUID /$SUBVOL"
else
    run mount "$TARGET"
    if [ "$DRY_RUN" = "false" ]; then
        check_mount
    fi
fi

# The subvolume root stays root-owned, so the sudo user cannot rename
# snapshots/ out from under the root backup timer; only dumps/ belongs to
# the sudo user. Owner and mode are set on every run, so a rerun also
# corrects directories that an earlier run or another account created.
for d in snapshots dumps; do
    if [ -L "$TARGET/$d" ] || { [ -e "$TARGET/$d" ] && [ ! -d "$TARGET/$d" ]; }; then
        echo "error: $TARGET/$d exists but is not a directory; refusing" >&2
        exit 1
    fi
done
run chown root:root "$TARGET"
run chmod 0755 "$TARGET"
run install -d -m 0700 -o root -g root "$TARGET/snapshots"
run install -d -m 0700 -o "$OWNER" -g "$OWNER" "$TARGET/dumps"

if [ "$DRY_RUN" = "false" ]; then
    findmnt "$TARGET"
    echo "the next backup-snapshots run copies the snapshots to $TARGET/snapshots"
fi
