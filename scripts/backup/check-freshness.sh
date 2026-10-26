#!/bin/bash
# Fail when any layer of the backup chain has gone stale, or space runs low.
#
# Freshness:
#   - the newest snapshot of data-plane and of app-prod in SNAPSHOT_DIR
#     (hourly ones from scripts/host/backup-snapshots.sh, or snapshot-now),
#   - the last backup-snapshots.sh run (its status file): recent, and with
#     failures=0,
#   - the newest copy on the backup pool (from the status file that
#     backup-snapshots.sh writes; skipped while the pool is not mounted),
#   - the newest VERIFIED dump directory under BACKUP_ROOT,
#   - the newest passing restore drill (ECONGRAPH_STATE_DIR/restore-drill.last).
#
# Space: nothing is ever deleted automatically, so usage only grows. Fails
# when free space on the /home btrfs filesystem or on the backup pool drops
# below its threshold, and reports the size of the local snapshots, the
# dumps directory and the backup pool's snapshots.
#
# Usage: check-freshness.sh [--skip-snapshots] [--textfile FILE]
#
# Limits (environment or ~/.config/econgraph/backup.env):
#   SNAPSHOT_MAX_AGE_HOURS  (default 3)    snapshots are hourly
#   POOL_COPY_MAX_AGE_HOURS (default 3)    copied to the pool in the same run
#   DUMP_MAX_AGE_HOURS      (default 26)   dumps run nightly
#   DRILL_MAX_AGE_HOURS     (default 192)  the drill runs weekly
#   HOME_MIN_FREE_PCT       (default 15)   /home filesystem (data + snapshots)
#   POOL_MIN_FREE_PCT       (default 15)   backup pool
#
# --skip-snapshots (or FRESHNESS_SKIP_SNAPSHOTS=1 in backup.env) is for the
# period before the hourly snapshot timer is installed.
# --textfile writes Prometheus metrics in node_exporter textfile format
# (written atomically). Nothing scrapes it yet: the repo's monitoring stack
# (k8s/monitoring) has no node_exporter or Pushgateway, so this is a hook
# for when one is added.

set -euo pipefail

# shellcheck source=scripts/backup/lib.sh
source "$(dirname "$(readlink -f "$0")")/lib.sh"

SNAPSHOT_MAX_AGE_HOURS="${SNAPSHOT_MAX_AGE_HOURS:-3}"
DUMP_MAX_AGE_HOURS="${DUMP_MAX_AGE_HOURS:-26}"
DRILL_MAX_AGE_HOURS="${DRILL_MAX_AGE_HOURS:-192}"
POOL_COPY_MAX_AGE_HOURS="${POOL_COPY_MAX_AGE_HOURS:-3}"
HOME_MIN_FREE_PCT="${HOME_MIN_FREE_PCT:-15}"
POOL_MIN_FREE_PCT="${POOL_MIN_FREE_PCT:-15}"
POOL_MOUNT="${POOL_MOUNT:-/srv/econgraph-backups}"
HOST_STATE_DIR="${HOST_STATE_DIR:-/var/lib/econgraph}"

skip_snapshots="${FRESHNESS_SKIP_SNAPSHOTS:-0}"
textfile=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --skip-snapshots) skip_snapshots=1; shift ;;
        --textfile) textfile="$2"; shift 2 ;;
        -h|--help) sed -n '2,35p' "$0"; exit 0 ;;
        *) die "unknown argument: $1" ;;
    esac
done

now="$(date +%s)"
failed=0
metrics=()

# Epoch of the newest snapshot of <name> in SNAPSHOT_DIR, or nothing.
# Snapshot times come from the names (<name>.<UTC stamp> or snapshot-now's
# <name>.pre-<UTC stamp>-<reason>): reading btrfs otime needs root.
newest_snapshot_epoch() {
    local name="$1" best=0 base ts e
    shopt -s nullglob
    for path in "${SNAPSHOT_DIR}/${name}".*; do
        base="${path##*/}"
        ts="${base#"${name}".}"
        ts="${ts#pre-}"
        if [[ "${ts}" =~ ^([0-9]{4})([0-9]{2})([0-9]{2})T([0-9]{2})([0-9]{2})([0-9]{2})?(Z|[+-][0-9]{4})? ]]; then
            e="$(date -d "${BASH_REMATCH[1]}-${BASH_REMATCH[2]}-${BASH_REMATCH[3]} ${BASH_REMATCH[4]}:${BASH_REMATCH[5]}:${BASH_REMATCH[6]:-00} ${BASH_REMATCH[7]:-}" +%s 2>/dev/null)" || continue
            (( e > best )) && best="${e}"
        fi
    done
    shopt -u nullglob
    (( best > 0 )) && echo "${best}"
}

# check <metric label> <epoch or empty> <limit hours> <description>
check() {
    local what="$1" epoch="$2" limit_h="$3" desc="$4" age_h
    if [[ -z "${epoch}" ]]; then
        echo "MISSING ${what}: ${desc}"
        failed=1
        metrics+=("econgraph_backup_last_success_timestamp_seconds{kind=\"${what}\"} 0")
        metrics+=("econgraph_backup_fresh{kind=\"${what}\"} 0")
        return
    fi
    age_h=$(( (now - epoch) / 3600 ))
    metrics+=("econgraph_backup_last_success_timestamp_seconds{kind=\"${what}\"} ${epoch}")
    if (( now - epoch > limit_h * 3600 )); then
        echo "STALE   ${what}: ${desc} is ${age_h}h old (limit ${limit_h}h)"
        failed=1
        metrics+=("econgraph_backup_fresh{kind=\"${what}\"} 0")
    else
        echo "OK      ${what}: ${desc} is ${age_h}h old (limit ${limit_h}h)"
        metrics+=("econgraph_backup_fresh{kind=\"${what}\"} 1")
    fi
}

if (( skip_snapshots )); then
    echo "SKIP    snapshots (--skip-snapshots)"
elif [[ ! -d "${SNAPSHOT_DIR}" ]]; then
    echo "MISSING snapshots: ${SNAPSHOT_DIR} does not exist"
    failed=1
else
    for name in data-plane app-prod; do
        check "snapshot_${name//-/_}" "$(newest_snapshot_epoch "${name}" || true)" \
            "${SNAPSHOT_MAX_AGE_HOURS}" "newest ${name} snapshot in ${SNAPSHOT_DIR}"
    done
fi

dump_dir="$(latest_verified_dump_dir || true)"
dump_epoch=""
if [[ -n "${dump_dir}" ]]; then
    dump_epoch="$(date -d "$(cat "${dump_dir}/VERIFIED")" +%s 2>/dev/null || true)"
fi
check dump "${dump_epoch}" "${DUMP_MAX_AGE_HOURS}" "newest verified dump ${dump_dir:-under ${BACKUP_ROOT}}"

drill_epoch=""
if [[ -f "${ECONGRAPH_STATE_DIR}/restore-drill.last" ]]; then
    drill_epoch="$(sed -n 's/^passed_epoch=//p' "${ECONGRAPH_STATE_DIR}/restore-drill.last")"
fi
check restore_drill "${drill_epoch}" "${DRILL_MAX_AGE_HOURS}" "newest passing restore drill"

# Copy on the backup pool: its directory is root-only, so read the status
# file backup-snapshots.sh leaves behind.
status_file="${HOST_STATE_DIR}/backup-snapshots.status"
status_value() { [[ -f "${status_file}" ]] && sed -n "s/^$1=//p" "${status_file}" | tail -n 1; }
# The hourly run itself: it must have run recently (a status file that
# stopped updating means the timer stopped), and without failures (one
# dataset's failed snapshot would otherwise pass while the other's is fresh).
if (( ! skip_snapshots )); then
    run_epoch="$(status_value run_epoch || true)"
    [[ "${run_epoch}" =~ ^[0-9]+$ ]] || run_epoch=""
    check snapshot_run "${run_epoch}" "${SNAPSHOT_MAX_AGE_HOURS}" "last backup-snapshots run (${status_file})"
    run_failures="$(status_value failures || true)"
    if [[ -n "${run_epoch}" ]] && [[ ! "${run_failures}" =~ ^[0-9]+$ || "${run_failures}" -gt 0 ]]; then
        echo "FAIL    snapshot_run: the last backup-snapshots run had failures=${run_failures:-(missing)} (journalctl -u econgraph-backup-snapshots)"
        failed=1
    fi
fi
if ! mountpoint -q "${POOL_MOUNT}"; then
    echo "SKIP    pool_copy: ${POOL_MOUNT} is not mounted (scripts/host/setup-backup-pool.sh)"
elif (( skip_snapshots )); then
    echo "SKIP    pool_copy (--skip-snapshots)"
else
    pool_epoch="$(status_value pool_newest_epoch || true)"
    [[ "${pool_epoch}" == 0 ]] && pool_epoch=""
    check pool_copy "${pool_epoch}" "${POOL_COPY_MAX_AGE_HOURS}" "newest snapshot copied to ${POOL_MOUNT}/snapshots"
    if [[ "$(status_value pool_status || true)" == "error" ]]; then
        echo "FAIL    pool_copy: the last backup-snapshots run failed to copy (journalctl -u econgraph-backup-snapshots)"
        failed=1
    fi
fi

# Free space. df on btrfs is an estimate (RAID profiles), good enough for a
# threshold.
human() { numfmt --to=iec --suffix=B "$1" 2>/dev/null || echo "$1 bytes"; }
check_space() {
    local what="$1" path="$2" min_pct="$3" size avail pct
    read -r size avail < <(df -B1 --output=size,avail "${path}" | tail -n 1)
    pct=$(( avail * 100 / size ))
    metrics+=("econgraph_backup_free_bytes{fs=\"${what}\"} ${avail}")
    metrics+=("econgraph_backup_free_ratio{fs=\"${what}\"} $(awk -v a="${avail}" -v s="${size}" 'BEGIN { printf "%.4f", a / s }')")
    if (( pct < min_pct )); then
        echo "LOWSPACE ${what}: $(human "${avail}") free of $(human "${size}") (${pct}%, minimum ${min_pct}%)"
        failed=1
    else
        echo "OK      ${what}: $(human "${avail}") free of $(human "${size}") (${pct}%, minimum ${min_pct}%)"
    fi
}
space_path="${SNAPSHOT_DIR}"
[[ -d "${space_path}" ]] || space_path="${HOME}"
check_space home_fs "${space_path}" "${HOME_MIN_FREE_PCT}"
if mountpoint -q "${POOL_MOUNT}"; then
    check_space backup_pool "${POOL_MOUNT}" "${POOL_MIN_FREE_PCT}"
fi

# Sizes (information only). Snapshot sizes come from the daily report that
# backup-snapshots.sh writes as root (btrfs filesystem du, shared extents
# counted once); the snapshot contents are not readable by this account.
sizes_file="${HOST_STATE_DIR}/backup-sizes.env"
size_value() { [[ -f "${sizes_file}" ]] && sed -n "s/^$1=//p" "${sizes_file}" | tail -n 1; }
report_size() {
    local what="$1" bytes="$2" desc="$3"
    if [[ "${bytes}" =~ ^[0-9]+$ ]]; then
        echo "SIZE    ${what}: $(human "${bytes}") ${desc}"
        metrics+=("econgraph_backup_size_bytes{what=\"${what}\"} ${bytes}")
    else
        echo "SIZE    ${what}: unknown ${desc}"
    fi
}
sizes_when="(no size report yet)"
if [[ -f "${sizes_file}" ]]; then
    sizes_when="(as of $(date -u -d "@$(size_value computed_epoch)" +%Y-%m-%dT%H:%MZ 2>/dev/null || echo '?'))"
fi
report_size local_snapshots "$(size_value local_snapshots_bytes || true)" \
    "in ${SNAPSHOT_DIR}, $(find "${SNAPSHOT_DIR}" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | wc -l) snapshots now, size ${sizes_when}"
dumps_bytes="$(du -sb "${BACKUP_ROOT}" 2>/dev/null | cut -f1 || true)"
report_size dumps "${dumps_bytes}" "in ${BACKUP_ROOT}"
if mountpoint -q "${POOL_MOUNT}"; then
    report_size pool_snapshots "$(size_value pool_snapshots_bytes || true)" \
        "in ${POOL_MOUNT}/snapshots, $(size_value pool_snapshots_count || echo '?') snapshots ${sizes_when}"
fi

if [[ -n "${textfile}" ]]; then
    mkdir -p "$(dirname "${textfile}")"
    {
        echo "# HELP econgraph_backup_last_success_timestamp_seconds Time of the newest good snapshot, dump or restore drill."
        echo "# TYPE econgraph_backup_last_success_timestamp_seconds gauge"
        echo "# HELP econgraph_backup_fresh 1 if the newest snapshot, dump or drill is within its age limit."
        echo "# TYPE econgraph_backup_fresh gauge"
        echo "# HELP econgraph_backup_free_bytes Free bytes on the filesystem holding the data or the backups."
        echo "# TYPE econgraph_backup_free_bytes gauge"
        echo "# HELP econgraph_backup_free_ratio Free fraction of the filesystem holding the data or the backups."
        echo "# TYPE econgraph_backup_free_ratio gauge"
        echo "# HELP econgraph_backup_size_bytes Disk used by snapshots or dumps."
        echo "# TYPE econgraph_backup_size_bytes gauge"
        printf '%s\n' "${metrics[@]}"
    } >"${textfile}.tmp"
    mv "${textfile}.tmp" "${textfile}"
fi

if (( failed )); then
    echo "BACKUP CHECK FAIL"
    exit 1
fi
echo "BACKUP CHECK OK"
