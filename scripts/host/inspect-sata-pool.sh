#!/bin/bash

# Show what is on the 3-disk Samsung 870 SATA btrfs pool (sda1, sdb1, sdc1)
# before it becomes the local backup target (docs/deployment/BACKUPS.md).
# Mounts it read-only on a temporary directory, lists the top level, its
# subvolumes and space usage, then unmounts. Changes nothing.
#
# Usage: sudo scripts/host/inspect-sata-pool.sh

set -euo pipefail

FS_UUID="a1e61098-087b-4403-8765-b714bf7b240b"

if [ "$(id -u)" -ne 0 ]; then
    echo "error: run with sudo" >&2
    exit 1
fi

if findmnt -rn -S "UUID=$FS_UUID" >/dev/null; then
    echo "note: the pool is already mounted:"
    findmnt -S "UUID=$FS_UUID"
fi

dir="$(mktemp -d /mnt/sata-pool.XXXXXX)"
cleanup() {
    if mountpoint -q "$dir"; then umount "$dir"; fi
    rmdir "$dir"
}
trap cleanup EXIT

mount -o ro,subvolid=5 "UUID=$FS_UUID" "$dir"

echo "== devices"
btrfs filesystem show "$dir"
echo
echo "== usage"
btrfs filesystem usage -h "$dir" | sed -n '1,12p'
echo
echo "== subvolumes"
btrfs subvolume list "$dir" || true
echo
echo "== top level"
ls -la "$dir"
echo
echo "== largest directories (two levels)"
# No -x: every subvolume is its own device to du, so -x would stop at each one.
du -h --max-depth=2 "$dir" 2>/dev/null | sort -rh | head -20
