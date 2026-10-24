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
#   - chowns the subvolume root to the sudo user
#   - creates snapshots/ (root:root 0700: where backup-snapshots.sh receives
#     the snapshots, out of reach of the jmalicki account) and dumps/ (sudo
#     user: the nightly logical dumps)
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
if grep -qE "^[^#]*[[:space:]]${TARGET}[[:space:]]" /etc/fstab; then
    echo "= fstab already mounts $TARGET"
else
    echo "+ append to /etc/fstab: $line"
    if [ "$DRY_RUN" = "false" ]; then
        echo "$line" >>/etc/fstab
        systemctl daemon-reload
    fi
fi

if mountpoint -q "$TARGET"; then
    echo "= $TARGET already mounted"
else
    run mount "$TARGET"
fi

# Only the subvolume root and the two directories below change hands.
run chown "$OWNER:$OWNER" "$TARGET"
if [ -d "$TARGET/snapshots" ]; then
    echo "= $TARGET/snapshots exists"
else
    run install -d -m 0700 -o root -g root "$TARGET/snapshots"
fi
if [ -d "$TARGET/dumps" ]; then
    echo "= $TARGET/dumps exists"
else
    run install -d -m 0750 -o "$OWNER" -g "$OWNER" "$TARGET/dumps"
fi

if [ "$DRY_RUN" = "false" ]; then
    findmnt "$TARGET"
    echo "the next backup-snapshots run copies the snapshots to $TARGET/snapshots"
fi
