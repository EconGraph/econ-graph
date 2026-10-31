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
# take_backup_lock WAIT: take the backup lock, then the old one, waiting up
# to WAIT seconds in all, and keep both until the script exits. Returns 1
# when one is held (BACKUP_LOCK_HELD names it), 2 on any other failure.
#
# The lock is /run/econgraph/backup-snapshots.lock, on fd 9. /run/econgraph
# is root's, mode 0700, so no other user can open the file to hold it; a
# directory or file that is not as this function makes them is refused,
# never used. The hourly job installed by install-backup-timer.sh must take
# this lock too: an installed copy without the BACKUP_LOCK_VERSION line below
# is refused until the installer is re-run.
#
# The old lock, /run/econgraph-backup-snapshots.lock on fd 7, is what copies
# from before this one take (an older checkout's restore-snapshot.sh, say),
# so it is always held as well: created root's and 0600 if missing, made so
# if not. Older copies made it 0644, so a user may have it open: held by a
# process whose real uid is root it is waited for; held by anyone else, the
# file is replaced by a new one this run holds (only runs holding the new
# lock replace it, one at a time).
BACKUP_LOCK_VERSION=2
# backup_lock_root_opened FILE: whether a process whose real uid is root's
# has FILE open. (/proc/locks names the process that took a lock, often a
# flock(1) that has since exited; an effective uid could be a setuid program
# a user started holding the file.) Call with FILE closed here.
backup_lock_root_opened() {
    local file="$1" root_uid="0" fd pid uid
    # Under the test: the test user, unless BACKUP_LOCK_ROOT_UID says otherwise.
    [ -z "${ECONGRAPH_TEST_ROOT:-}" ] || root_uid="${BACKUP_LOCK_ROOT_UID:-${BACKUP_LOCK_OWNER%%:*}}"
    while IFS= read -r fd; do
        pid="${fd#/proc/}"
        pid="${pid%%/*}"
        uid="$(sed -n 's/^Uid:[[:space:]]*\([0-9]*\).*/\1/p' "/proc/$pid/status" 2>/dev/null)" || continue
        [ "$uid" != "$root_uid" ] || return 0
    done < <(find /proc/[0-9]*/fd -maxdepth 1 -lname "$file" 2>/dev/null)
    return 1
}
# backup_lock_fd FD FILE DEADLINE: flock FD (open on FILE), waiting until
# DEADLINE (epoch seconds) at most; on 1 sets BACKUP_LOCK_HELD to FILE and
# says, for the old lock, what holds it.
backup_lock_fd() {
    local fd="$1" file="$2" deadline="$3" left status=0
    left=$((deadline - $(date +%s)))
    [ "$left" -ge 0 ] || left=0
    flock -w "$left" "$fd" || status=$?
    if [ "$status" -eq 1 ]; then
        BACKUP_LOCK_HELD="$file"
        if [ "$fd" = 7 ]; then
            echo "note: $file is the lock older copies of these scripts take: an older checkout's" >&2
            echo "  restore-snapshot.sh, or an installed job from before it moved, is running" >&2
        fi
        return 1
    elif [ "$status" -ne 0 ]; then
        echo "error: could not lock $file (flock exited $status)" >&2
        return 2
    fi
}
take_backup_lock() {
    local wait="$1" root="${ECONGRAPH_TEST_ROOT:-}" owner="0:0" old_umask status deadline
    local dir file installed legacy tmp f
    dir="$root/run/econgraph"
    file="$dir/backup-snapshots.lock"
    installed="$root/usr/local/sbin/econgraph-backup-snapshots"
    legacy="$root/run/econgraph-backup-snapshots.lock"
    deadline=$(($(date +%s) + wait))
    BACKUP_LOCK_HELD=""
    if [ -n "$root" ]; then
        # Test runs only (scripts/host/test-backup-scripts.sh, never root):
        # the test user's own files.
        if [ "$EUID" -eq 0 ]; then
            echo "error: ECONGRAPH_TEST_ROOT is set; refusing to run as root with it" >&2
            return 2
        fi
        owner="${BACKUP_LOCK_OWNER:-0:0}"
    fi
    if [ -e "$installed" ] && ! grep -qx "BACKUP_LOCK_VERSION=$BACKUP_LOCK_VERSION" "$installed"; then
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
    for f in "$file" "$legacy"; do
        if [ -L "$f" ] || { [ -e "$f" ] && [ ! -f "$f" ]; }; then
            echo "error: $f is not a regular file; refusing to use it as a lock" >&2
            return 2
        fi
    done
    old_umask="$(umask)"
    umask 077
    if ! exec 9>>"$file"; then
        umask "$old_umask"
        echo "error: could not open $file" >&2
        return 2
    fi
    status=0
    backup_lock_fd 9 "$file" "$deadline" || status=$?
    if [ "$status" -ne 0 ]; then
        umask "$old_umask"
        return "$status"
    fi
    # The old lock (see above), under the new one.
    if ! exec 7>>"$legacy"; then
        umask "$old_umask"
        echo "error: could not open $legacy" >&2
        return 2
    fi
    if ! flock -n 7; then
        exec 7>&-
        if backup_lock_root_opened "$legacy"; then
            if ! exec 7>>"$legacy"; then
                umask "$old_umask"
                echo "error: could not open $legacy" >&2
                return 2
            fi
            status=0
            backup_lock_fd 7 "$legacy" "$deadline" || status=$?
            if [ "$status" -ne 0 ]; then
                umask "$old_umask"
                return "$status"
            fi
        else
            tmp=""
            if ! { tmp="$(mktemp "$legacy.XXXXXX")" && exec 7>>"$tmp" && flock -n 7 &&
                mv -f "$tmp" "$legacy"; }; then
                [ -z "$tmp" ] || rm -f "$tmp"
                umask "$old_umask"
                echo "error: could not replace $legacy, which a user holds" >&2
                return 2
            fi
        fi
    fi
    umask "$old_umask"
    if [ "$(stat -c '%u:%g %a' "$legacy")" != "$owner 600" ] &&
        ! { chown "$owner" "$legacy" && chmod 0600 "$legacy"; }; then
        echo "error: could not make $legacy $owner 0600" >&2
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
    [[ "${lock_status}" != 1 ]] || die "${BACKUP_LOCK_HELD} is held: a backup-snapshots run, a restore or a delete is in progress; nothing was taken, re-run when it is done"
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
