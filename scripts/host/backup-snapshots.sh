#!/bin/bash

# Hourly btrfs backups of the production data (docs/deployment/BACKUPS.md).
#
#   1. Takes a read-only snapshot of /srv/econgraph/data-plane and of
#      /srv/econgraph/app-prod into /srv/econgraph/snapshots, named
#      <name>.<UTC YYYYMMDDTHHMMSSZ>.
#   2. If the backup pool is mounted at /srv/econgraph-backups, copies every
#      read-only snapshot that is not there yet into
#      /srv/econgraph-backups/snapshots (root only, 0700) with
#      `btrfs send | btrfs receive`, incrementally (-p the newest snapshot
#      already on both sides). The pool is a different filesystem, so this is
#      the only way across. If it is not mounted, the copy is skipped with a
#      message. A read-only subvolume of the same name on the pool counts as
#      the copy only if its Received UUID is the local snapshot's UUID (or,
#      for a local snapshot that was itself received, its Received UUID);
#      one that does not match is renamed <name>.<stamp>.mismatch-<when>
#      (not deleted) and received again.
#   3. Writes a status file and (at most daily) a size report to
#      /var/lib/econgraph/ for scripts/backup/check-freshness.sh.
#
# It never deletes anything: snapshots accumulate on both sides until the operator
# removes some by hand with scripts/host/delete-snapshots.sh. A receive that
# fails half-way is renamed <name>.<stamp>.incomplete-<when> (not deleted)
# and retried on the next run.
#
# Installed as /usr/local/sbin/econgraph-backup-snapshots and run by
# econgraph-backup-snapshots.timer (scripts/host/install-backup-timer.sh).
# Safe to run by hand at any time. It holds /run/econgraph-backup-snapshots.lock
# while it runs; if another run, or scripts/host/restore-snapshot.sh, holds
# it, it stops at once with an error (that hour is skipped).
#
# Usage: sudo scripts/host/backup-snapshots.sh [--dry-run]
#
# --dry-run (-n) prints every snapshot and copy it would make and writes
# nothing: no snapshot, no lock file, no status file.
#
# ECONGRAPH_TEST_ROOT, if set, is prefixed to every absolute path below, so
# scripts/host/test-backup-scripts.sh can run this against stubs in a
# temporary directory.

set -euo pipefail

TEST_ROOT="${ECONGRAPH_TEST_ROOT:-}"
NAMES=(data-plane app-prod)
SRV_ROOT="$TEST_ROOT/srv/econgraph"
SNAPSHOT_DIR="$SRV_ROOT/snapshots"
POOL_MOUNT="$TEST_ROOT/srv/econgraph-backups"
POOL_SNAPSHOT_DIR="$POOL_MOUNT/snapshots"
STATE_DIR="$TEST_ROOT/var/lib/econgraph"
LOCK_FILE="$TEST_ROOT/run/econgraph-backup-snapshots.lock"
SIZE_REPORT_EVERY_HOURS=24

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

# >>> backup lock: the same in backup-snapshots.sh, restore-snapshot.sh,
# delete-snapshots.sh and snapshot-now.sh (test-backup-scripts.sh checks).
# take_backup_lock FILE WAIT: take FILE's flock on fd 9, waiting up to WAIT
# seconds, and keep it until the script exits. FILE is root's and mode 0600,
# so no other user can open it to hold the lock; one that is not is replaced,
# under the lock, by one that is (a chmod would not close a descriptor some
# user already has open). After locking, the file must still be the one at
# FILE (another run may have replaced it meanwhile), or it starts over.
# Returns 1 when the lock is held, 2 on any other failure.
take_backup_lock() {
    local file="$1" wait="$2" owner="0:0" tries=0 old_umask status tmp
    # Under scripts/host/test-backup-scripts.sh only: the test user's own files.
    [ -z "${ECONGRAPH_TEST_ROOT:-}" ] || owner="${BACKUP_LOCK_OWNER:-0:0}"
    while :; do
        tries=$((tries + 1))
        if [ "$tries" -gt 5 ]; then
            echo "error: $file keeps being replaced; could not lock it" >&2
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
        [ "$(stat -L -c %d:%i /proc/self/fd/9)" != "$(stat -c %d:%i "$file" 2>/dev/null)" ] || break
        exec 9>&-
    done
    if [ "$(stat -c '%u:%g %a' "$file")" != "$owner 600" ]; then
        echo "note: $file is $(stat -c '%u:%g %a' "$file"), not $owner 600; replacing it" >&2
        tmp=""
        if ! { tmp="$(mktemp "$file.XXXXXX")" && chown "$owner" "$tmp" && chmod 0600 "$tmp" &&
            exec 8>>"$tmp" && flock -n 8 && mv -f "$tmp" "$file"; }; then
            [ -z "$tmp" ] || rm -f "$tmp"
            echo "error: could not replace $file with a root-only one" >&2
            return 2
        fi
        exec 9>&8 8>&-
    fi
}
# <<< backup lock

# A dry run takes no lock: opening the lock file would create it.
if [ "$DRY_RUN" = "false" ]; then
    lock_status=0
    take_backup_lock "$LOCK_FILE" 0 || lock_status=$?
    if [ "$lock_status" -eq 1 ]; then
        echo "error: another backup-snapshots run, or a restore-snapshot.sh or delete-snapshots.sh," >&2
        echo "  holds $LOCK_FILE" >&2
    fi
    [ "$lock_status" -eq 0 ] || exit 1
fi

# "20261024T051500Z" -> epoch, or empty.
stamp_epoch() {
    local s="$1"
    if [[ "$s" =~ ^([0-9]{4})([0-9]{2})([0-9]{2})T([0-9]{2})([0-9]{2})([0-9]{2})Z$ ]]; then
        date -u -d "${BASH_REMATCH[1]}-${BASH_REMATCH[2]}-${BASH_REMATCH[3]} ${BASH_REMATCH[4]}:${BASH_REMATCH[5]}:${BASH_REMATCH[6]}" +%s
    fi
}

# Snapshot names of <name> in <dir>, oldest first, one per line. Covers both
# the hourly <name>.<stamp> and snapshot-now's <name>.pre-<stamp>-<reason>.
# Names outside a safe character set are ignored (the local snapshot
# directory is writable by the operator).
list_snapshots() {
    local dir="$1" name="$2"
    find "$dir" -mindepth 1 -maxdepth 1 -type d -name "$name.*" -printf '%f\n' 2>/dev/null \
        | grep -E "^${name}\.(pre-)?[0-9]{8}T[0-9]{6}Z(-[A-Za-z0-9._-]+)?$" \
        | sed -E "s/^${name}\.(pre-)?([0-9]{8}T[0-9]{6}Z).*/\2 &/" \
        | sort | cut -d' ' -f2
}

is_ro_subvolume() {
    local p="$1"
    [ ! -L "$p" ] && [ -d "$p" ] && [ "$(stat -c %i "$p")" = 256 ] \
        && [ "$(btrfs property get "$p" ro 2>/dev/null)" = "ro=true" ]
}

# subvol_field <path> <field>: a field of `btrfs subvolume show <path>`
# ("UUID" or "Received UUID"); "-" when unset, empty when unreadable.
subvol_field() {
    # || true: an unreadable subvolume must not abort the run under pipefail.
    { btrfs subvolume show "$1" 2>/dev/null \
        | sed -nE "s/^[[:space:]]*$2:[[:space:]]+([^[:space:]]+).*/\1/p" | head -n 1; } || true
}

# send_uuid <path>: the UUID a send of <path> carries, which the receiving
# side records as its Received UUID: <path>'s own Received UUID if it was
# itself received, otherwise its UUID. Empty when unreadable.
send_uuid() {
    local received
    received="$(subvol_field "$1" "Received UUID")"
    if [ -n "$received" ] && [ "$received" != "-" ]; then
        echo "$received"
    else
        subvol_field "$1" "UUID"
    fi
}

newest_epoch() {
    local dir="$1" name newest="" s e best=0
    for name in "${NAMES[@]}"; do
        newest="$(list_snapshots "$dir" "$name" | tail -n 1)"
        [ -n "$newest" ] || continue
        s="$(sed -E 's/^[^.]+\.(pre-)?([0-9]{8}T[0-9]{6}Z).*/\2/' <<<"$newest")"
        e="$(stamp_epoch "$s")"
        if [ -n "$e" ] && [ "$e" -gt "$best" ]; then best="$e"; fi
    done
    echo "$best"
}

failures=0
stamp="$(date -u +%Y%m%dT%H%M%SZ)"

# 1. local read-only snapshots
if ! mountpoint -q "$SNAPSHOT_DIR"; then
    echo "error: $SNAPSHOT_DIR is not mounted" >&2
    exit 1
fi
snap_fs="$(findmnt -n -o UUID -T "$SNAPSHOT_DIR")"
for name in "${NAMES[@]}"; do
    src="$SRV_ROOT/$name"
    if ! mountpoint -q "$src" || [ "$(findmnt -n -o UUID -T "$src")" != "$snap_fs" ]; then
        echo "error: $src is not mounted from the same filesystem as $SNAPSHOT_DIR" >&2
        failures=$((failures + 1))
        continue
    fi
    dest="$SNAPSHOT_DIR/$name.$stamp"
    if [ -e "$dest" ]; then
        echo "= $dest already exists"
    elif ! run btrfs subvolume snapshot -r "$src" "$dest"; then
        # Count it and carry on, so the other name, the pool copy and the
        # status file still happen.
        echo "error: snapshot of $src failed" >&2
        failures=$((failures + 1))
    fi
done

# 2. copy to the backup pool
pool_status="not-mounted"
if ! mountpoint -q "$POOL_MOUNT"; then
    echo "= $POOL_MOUNT is not mounted; skipping the copy to the backup pool"
    echo "  (scripts/host/setup-backup-pool.sh sets it up)"
else
    pool_status="ok"
    if [ ! -d "$POOL_SNAPSHOT_DIR" ]; then
        run install -d -m 0700 -o root -g root "$POOL_SNAPSHOT_DIR"
    fi
    for name in "${NAMES[@]}"; do
        parent=""
        while IFS= read -r snap; do
            [ -n "$snap" ] || continue
            local_path="$SNAPSHOT_DIR/$snap"
            target_path="$POOL_SNAPSHOT_DIR/$snap"
            if ! is_ro_subvolume "$local_path"; then
                echo "warning: $local_path is not a read-only subvolume; not copying it" >&2
                continue
            fi
            if [ -e "$target_path" ]; then
                if is_ro_subvolume "$target_path"; then
                    # It must have been received from this snapshot: a stale
                    # or unrelated copy would otherwise become the next send
                    # parent, or be restored with --from-pool.
                    want="$(send_uuid "$local_path")"
                    if [ -z "$want" ] || [ "$want" = "-" ]; then
                        echo "error: cannot read the UUID of $local_path" >&2
                        failures=$((failures + 1))
                        pool_status="error"
                        break
                    fi
                    got="$(subvol_field "$target_path" "Received UUID")"
                    if [ "$got" = "$want" ]; then
                        parent="$snap"
                        continue
                    fi
                    # Set it aside (never delete) and receive again.
                    echo "warning: $target_path has Received UUID ${got:-(unreadable)}, not $want from $local_path; setting it aside" >&2
                    run mv "$target_path" "$target_path.mismatch-$stamp"
                else
                    # A receive that died half-way leaves a writable subvolume.
                    # Set it aside (never delete) and receive again.
                    run mv "$target_path" "$target_path.incomplete-$stamp"
                fi
            fi
            if [ -n "$parent" ]; then
                echo "+ btrfs send -p $SNAPSHOT_DIR/$parent $local_path | btrfs receive $POOL_SNAPSHOT_DIR"
                ok="true"
                if [ "$DRY_RUN" = "false" ]; then
                    btrfs send -q -p "$SNAPSHOT_DIR/$parent" "$local_path" \
                        | btrfs receive "$POOL_SNAPSHOT_DIR" || ok="false"
                fi
            else
                echo "+ btrfs send $local_path | btrfs receive $POOL_SNAPSHOT_DIR   (full: no common parent yet)"
                ok="true"
                if [ "$DRY_RUN" = "false" ]; then
                    btrfs send -q "$local_path" | btrfs receive "$POOL_SNAPSHOT_DIR" || ok="false"
                fi
            fi
            if [ "$ok" = "true" ]; then
                parent="$snap"
            else
                echo "error: copying $snap to the backup pool failed" >&2
                failures=$((failures + 1))
                pool_status="error"
                if [ -e "$target_path" ]; then
                    mv "$target_path" "$target_path.incomplete-$stamp"
                fi
                break
            fi
        done < <(list_snapshots "$SNAPSHOT_DIR" "$name")
    done
fi

# 3. status and size report
if [ "$DRY_RUN" = "false" ]; then
    install -d -m 0755 "$STATE_DIR"
    pool_newest=0
    if [ "$pool_status" != "not-mounted" ]; then
        pool_newest="$(newest_epoch "$POOL_SNAPSHOT_DIR")"
    fi
    {
        echo "run_epoch=$(date +%s)"
        echo "failures=$failures"
        echo "local_newest_epoch=$(newest_epoch "$SNAPSHOT_DIR")"
        echo "pool_status=$pool_status"
        echo "pool_newest_epoch=$pool_newest"
    } >"$STATE_DIR/backup-snapshots.status.tmp"
    chmod 0644 "$STATE_DIR/backup-snapshots.status.tmp"
    mv "$STATE_DIR/backup-snapshots.status.tmp" "$STATE_DIR/backup-snapshots.status"

    sizes="$STATE_DIR/backup-sizes.env"
    if [ ! -f "$sizes" ] || [ $(( $(date +%s) - $(stat -c %Y "$sizes") )) -ge $(( SIZE_REPORT_EVERY_HOURS * 3600 )) ]; then
        # btrfs filesystem du counts shared extents once: exclusive + set
        # shared is what the whole snapshot directory really occupies.
        disk_bytes() {
            btrfs filesystem du -s --raw "$1" 2>/dev/null \
                | awk 'NR == 2 { print $2 + $3 }'
        }
        {
            echo "computed_epoch=$(date +%s)"
            echo "local_snapshots_bytes=$(disk_bytes "$SNAPSHOT_DIR" || true)"
            echo "local_snapshots_count=$(find "$SNAPSHOT_DIR" -mindepth 1 -maxdepth 1 -type d | wc -l)"
            if [ "$pool_status" != "not-mounted" ]; then
                echo "pool_snapshots_bytes=$(disk_bytes "$POOL_SNAPSHOT_DIR" || true)"
                echo "pool_snapshots_count=$(find "$POOL_SNAPSHOT_DIR" -mindepth 1 -maxdepth 1 -type d | wc -l)"
            fi
        } >"$sizes.tmp"
        chmod 0644 "$sizes.tmp"
        mv "$sizes.tmp" "$sizes"
    fi
fi

if [ "$failures" -gt 0 ]; then
    echo "backup-snapshots: $failures failure(s)" >&2
    exit 1
fi
echo "backup-snapshots: OK ($stamp, pool: $pool_status)"
