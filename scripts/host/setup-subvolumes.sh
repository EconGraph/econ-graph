#!/bin/bash

# Create the btrfs subvolumes that hold production data on this host and
# mount them under /srv/econgraph (docs/deployment/HOST_STORAGE.md).
#
#   @econgraph-data-plane -> /srv/econgraph/data-plane  (pg/)
#   @econgraph-app-prod   -> /srv/econgraph/app-prod    (keycloak-pg/)
#   @econgraph-snapshots  -> /srv/econgraph/snapshots   (read-only snapshots)
#
# The subvolumes sit at the top level of the /home btrfs filesystem, next to
# @home, so snapshots of /home never include them and theirs never include
# /home. Safe to re-run: existing subvolumes, fstab lines and mounts are left
# alone. Nothing existing is changed or deleted.
#
# Usage: sudo [ECONGRAPH_BTRFS_UUID=<uuid>] scripts/host/setup-subvolumes.sh [--dry-run]
# ECONGRAPH_BTRFS_UUID is the btrfs filesystem to create them on; the default
# is the /home filesystem of the production host (`findmnt -no UUID /home`).

set -euo pipefail

FS_UUID="${ECONGRAPH_BTRFS_UUID:-03ee0fcb-5f19-4f44-8596-e227f1fb647c}"
MOUNT_OPTS="compress=zstd:3,ssd,discard=async,space_cache=v2,noatime,nofail"
SRV_ROOT="/srv/econgraph"
NAMES=(data-plane app-prod snapshots)

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
    echo "error: run with sudo from the account that will own the data (SUDO_USER is unset)" >&2
    exit 1
fi

if ! blkid -U "$FS_UUID" >/dev/null; then
    echo "error: no filesystem with UUID $FS_UUID" >&2
    exit 1
fi

top="$(mktemp -d /mnt/econgraph-top.XXXXXX)"
cleanup() {
    if mountpoint -q "$top"; then umount "$top"; fi
    rmdir "$top"
}
trap cleanup EXIT

# The top level (subvolid 5) is mounted even in a dry run, read-only, so the
# existence checks below are real.
if [ "$DRY_RUN" = "true" ]; then
    mount -o ro,subvolid=5 "UUID=$FS_UUID" "$top"
else
    mount -o subvolid=5 "UUID=$FS_UUID" "$top"
fi

for name in "${NAMES[@]}"; do
    subvol="@econgraph-$name"
    target="$SRV_ROOT/$name"

    if [ -e "$top/$subvol" ]; then
        if ! btrfs subvolume show "$top/$subvol" >/dev/null 2>&1; then
            echo "error: $subvol exists at the top level but is not a subvolume" >&2
            exit 1
        fi
        echo "= subvolume $subvol already exists"
    else
        run btrfs subvolume create "$top/$subvol"
    fi

    if [ ! -d "$target" ]; then
        run mkdir -p "$target"
    fi

    line="UUID=$FS_UUID $target btrfs subvol=/$subvol,$MOUNT_OPTS 0 0"
    if grep -qE "^[^#]*[[:space:]]${target}[[:space:]]" /etc/fstab; then
        echo "= fstab already mounts $target"
    else
        echo "+ append to /etc/fstab: $line"
        if [ "$DRY_RUN" = "false" ]; then
            echo "$line" >> /etc/fstab
        fi
    fi
done

if [ "$DRY_RUN" = "false" ]; then
    systemctl daemon-reload
fi

for name in "${NAMES[@]}"; do
    target="$SRV_ROOT/$name"
    if mountpoint -q "$target"; then
        echo "= $target already mounted"
    else
        run mount "$target"
    fi
    # Only the subvolume roots change hands; anything already inside keeps
    # its owner (Postgres data belongs to uid 999; docs/deployment/HOST_STORAGE.md).
    run chown "$OWNER:$OWNER" "$target"
done

if [ "$DRY_RUN" = "false" ]; then
    findmnt -t btrfs -o TARGET,SOURCE,OPTIONS | grep -E "TARGET|$SRV_ROOT"
fi
