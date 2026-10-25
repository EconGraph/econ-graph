#!/bin/bash
# Take a read-only btrfs snapshot of the production data before a risky
# change: a data migration, a deliberate recrawl, a schema change, a restore.
# Such changes are "critical" under the repo's rules and need the maintainer's sign-off
# plus a dump first; run this together with dump-databases.sh.
#
# Usage: snapshot-now.sh [--dry-run] [--only data-plane|app-prod] <reason>
#
# --dry-run runs every check, prints the snapshots it would take and
# changes nothing (no snapshot, no log line).
#
# Creates SNAPSHOT_DIR/<name>.pre-<UTC stamp>-<reason> for each of
# data-plane and app-prod (default: both). Like every snapshot here it is
# never deleted automatically; scripts/host/backup-snapshots.sh copies it to
# the backup pool with the hourly ones. Delete by hand, if ever, with
# scripts/host/delete-snapshots.sh (see docs/deployment/BACKUPS.md).
#
# Runs as the operator without sudo, provided the operator owns the subvolume roots
# (the kernel only lets a non-root user snapshot subvolumes it owns) and can
# write to SNAPSHOT_DIR. Postgres can keep running: each data directory,
# WAL included, is inside one subvolume, so the snapshot is crash-consistent.

set -euo pipefail

# shellcheck source=scripts/backup/lib.sh
source "$(dirname "$(readlink -f "$0")")/lib.sh"

only=""
DRY_RUN="false"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --only) [[ $# -ge 2 ]] || die "--only needs data-plane or app-prod"; only="$2"; shift 2 ;;
        --dry-run|-n) DRY_RUN="true"; shift ;;
        -h|--help) sed -n '2,21p' "$0"; exit 0 ;;
        -*) die "unknown option: $1" ;;
        *) break ;;
    esac
done
[[ $# -ge 1 ]] || die "usage: $(basename "$0") [--dry-run] [--only data-plane|app-prod] <reason>"

reason="$*"
slug="$(printf '%s' "${reason}" | tr -cs 'A-Za-z0-9' '-' | sed 's/^-*//; s/-*$//' | cut -c1-60)"
[[ -n "${slug}" ]] || die "reason must contain letters or digits"

declare -A sources=([data-plane]="${DATA_PLANE_DIR}" [app-prod]="${APP_PROD_DIR}")
case "${only}" in
    "") names=(data-plane app-prod) ;;
    data-plane|app-prod) names=("${only}") ;;
    *) die "--only takes data-plane or app-prod" ;;
esac

[[ -d "${SNAPSHOT_DIR}" ]] || die "${SNAPSHOT_DIR} does not exist (see 'Needs sudo' in BACKUPS.md)"
[[ -w "${SNAPSHOT_DIR}" ]] || die "${SNAPSHOT_DIR} is not writable by $(id -un)"
snap_fs="$(findmnt -n -o UUID -T "${SNAPSHOT_DIR}")"

stamp="$(date -u +%Y%m%dT%H%M%SZ)"
[[ "${DRY_RUN}" == "true" ]] || mkdir -p "${ECONGRAPH_STATE_DIR}"
made=()
for name in "${names[@]}"; do
    src="${sources[${name}]}"
    [[ -d "${src}" ]] || die "${src} does not exist"
    # A btrfs subvolume root always has inode number 256; this works
    # without root, unlike `btrfs subvolume show`.
    [[ "$(stat -c %i "${src}")" == 256 ]] || die "${src} is not a btrfs subvolume root"
    [[ "$(findmnt -n -o UUID -T "${src}")" == "${snap_fs}" ]] \
        || die "${src} and ${SNAPSHOT_DIR} are on different filesystems"
    dest="${SNAPSHOT_DIR}/${name}.pre-${stamp}-${slug}"
    if [[ "${DRY_RUN}" == "true" ]]; then
        echo "+ btrfs subvolume snapshot -r ${src} ${dest}"
        echo "+ append to ${ECONGRAPH_STATE_DIR}/snapshots.log"
        continue
    fi
    btrfs subvolume snapshot -r "${src}" "${dest}" >/dev/null \
        || die "snapshot of ${src} failed (does $(id -un) own it?)"
    made+=("${dest}")
    printf '%s\t%s\t%s\t%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$(id -un)" "${dest}" "${reason}" \
        >>"${ECONGRAPH_STATE_DIR}/snapshots.log"
    log "snapshot: ${dest}"
done

if [[ "${DRY_RUN}" == "true" ]]; then
    echo "(dry run: no snapshot taken)"
    exit 0
fi
echo "Pre-change snapshot(s) taken for: ${reason}"
printf '  %s\n' "${made[@]}"
echo "Also take a dump before the change: dump-databases.sh"
