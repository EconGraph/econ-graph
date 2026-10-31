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
# Run as root, it first takes the lock the hourly backup-snapshots.sh holds
# (/run/econgraph/backup-snapshots.lock) and stops if it is held. It also
# runs as the operator without sudo, taking no lock, provided the operator
# owns the subvolume roots (the kernel only lets a non-root user snapshot
# subvolumes it owns) and can
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
        -h|--help) awk 'NR == 1 { next } /^#/ { sub(/^# ?/, ""); print; next } { exit }' "$0"; exit 0 ;;
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

# As root, the lock the hourly backup-snapshots.sh holds, until exit, so no
# backup run lists, sends or prunes the snapshot directory meanwhile. Without
# root the lock (in /run) can't be taken; the snapshots are made anyway.
if [[ "${DRY_RUN}" != "true" && "$(id -u)" == 0 ]]; then
    lock_file="${ECONGRAPH_TEST_ROOT:-}/run/econgraph/backup-snapshots.lock"
    lock_status=0
    take_backup_lock 0 || lock_status=$?
    [[ "${lock_status}" != 1 ]] || die "${lock_file} is held: a backup-snapshots run, a restore or a delete is in progress; nothing was taken, re-run when it is done"
    [[ "${lock_status}" == 0 ]] || die "could not take ${lock_file}; nothing was taken"
fi

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
