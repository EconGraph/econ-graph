#!/bin/bash
# Fail when any layer of the backup chain has gone stale:
#   - the newest btrfs snapshot of data-plane and of app-prod (btrbk's
#     hourly ones or a snapshot-now one) in SNAPSHOT_DIR,
#   - the newest VERIFIED dump directory under BACKUP_ROOT,
#   - the newest passing restore drill (ECONGRAPH_STATE_DIR/restore-drill.last).
#
# Usage: check-freshness.sh [--skip-snapshots] [--textfile FILE]
#
# Limits (environment or ~/.config/econgraph/backup.env):
#   SNAPSHOT_MAX_AGE_HOURS (default 3)   btrbk runs hourly
#   DUMP_MAX_AGE_HOURS     (default 26)  dumps run nightly
#   DRILL_MAX_AGE_HOURS    (default 192) the drill runs weekly
#
# --skip-snapshots (or FRESHNESS_SKIP_SNAPSHOTS=1 in backup.env) is for the
# period before the subvolumes and btrbk exist.
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

skip_snapshots="${FRESHNESS_SKIP_SNAPSHOTS:-0}"
textfile=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --skip-snapshots) skip_snapshots=1; shift ;;
        --textfile) textfile="$2"; shift 2 ;;
        -h|--help) sed -n '2,21p' "$0"; exit 0 ;;
        *) die "unknown argument: $1" ;;
    esac
done

now="$(date +%s)"
failed=0
metrics=()

# Epoch of the newest snapshot of <name> in SNAPSHOT_DIR, or nothing.
# Snapshot times come from the names (btrbk long-iso, or snapshot-now's
# UTC stamp): reading btrfs otime needs root.
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

if [[ -n "${textfile}" ]]; then
    mkdir -p "$(dirname "${textfile}")"
    {
        echo "# HELP econgraph_backup_last_success_timestamp_seconds Time of the newest good snapshot, dump or restore drill."
        echo "# TYPE econgraph_backup_last_success_timestamp_seconds gauge"
        echo "# HELP econgraph_backup_fresh 1 if the newest snapshot, dump or drill is within its age limit."
        echo "# TYPE econgraph_backup_fresh gauge"
        printf '%s\n' "${metrics[@]}"
    } >"${textfile}.tmp"
    mv "${textfile}.tmp" "${textfile}"
fi

if (( failed )); then
    echo "BACKUP FRESHNESS FAIL"
    exit 1
fi
echo "BACKUP FRESHNESS OK"
