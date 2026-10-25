#!/bin/bash
# Nightly logical dumps of the production databases.
#
# Runs `pg_dump -Fc` for each database in BACKUP_DATABASES inside its pod
# (reached through the kind node, since the host's kubectl is microk8s),
# writes the dumps to BACKUP_ROOT/<UTC stamp>/, verifies every archive with
# `pg_restore --list` and a full read-through, and only then gives the
# directory its final name and a VERIFIED marker.
#
# It only ever adds: no dump is ever deleted or pruned. A run that fails
# keeps what it wrote as <stamp>.failed for inspection. Watch the space
# with check-freshness.sh; remove old dumps by hand if ever needed.
#
# Exits non-zero if any database fails to dump or verify. Nothing in the
# cluster is modified.
#
# Usage: dump-databases.sh [--dry-run] [--target-dir DIR]
#
# --dry-run checks the Docker daemon, the kind node and the pods, prints what
# would be dumped and where, and writes nothing.

set -euo pipefail

# shellcheck source=scripts/backup/lib.sh
source "$(dirname "$(readlink -f "$0")")/lib.sh"

# The dumps hold every row, password hashes and Keycloak secrets included:
# files 0600, directories 0700.
umask 077

DRY_RUN="false"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run|-n) DRY_RUN="true"; shift ;;
        --target-dir) BACKUP_ROOT="$2"; shift 2 ;;
        -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
        *) die "unknown argument: $1" ;;
    esac
done

check_docker_daemon
docker inspect "${KIND_NODE}" >/dev/null 2>&1 \
    || die "kind node container ${KIND_NODE} not found on ${DOCKER_HOST}"

if [[ "${DRY_RUN}" == "true" ]]; then
    log "dry run: would dump into ${BACKUP_ROOT}/$(utc_stamp) (via .partial, then VERIFIED)"
    for entry in ${BACKUP_DATABASES}; do
        IFS=: read -r db pod user <<<"${entry}"
        if phase="$(kube get pod "${pod}" -o jsonpath='{.status.phase}' --ignore-not-found 2>/dev/null)"; then
            phase="${phase:-not found}"
        else
            phase="API server not reachable through ${KIND_NODE}"
        fi
        log "dry run: pg_dump --format=custom ${db} as ${user} from pod ${pod} (${phase})"
    done
    log "dry run: verify with: $(pg_restore_cmd "${BACKUP_ROOT}") --list / --file=/dev/null"
    exit 0
fi

mkdir -p "${BACKUP_ROOT}"

# One dump at a time (a manual run racing the timer would otherwise collide).
exec 9>"${BACKUP_ROOT}/.lock"
flock -n 9 || die "another dump is already running (lock ${BACKUP_ROOT}/.lock)"

STAMP="$(utc_stamp)"
FINAL="${BACKUP_ROOT}/${STAMP}"
WORK="${FINAL}.partial"
[[ -e "${FINAL}" || -e "${WORK}" ]] && die "${FINAL} (or .partial) already exists"
mkdir -p "${WORK}"
# On any failure, keep what was written under a name that can't be mistaken
# for a good dump.
trap 'mv "${WORK}" "${FINAL}.failed" 2>/dev/null || true' EXIT

read -r -a RESTORE <<<"$(pg_restore_cmd "${WORK}")"
log "pg_restore for verification: ${RESTORE[*]}"

{
    echo "stamp=${STAMP}"
    echo "started_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "host=$(hostname)"
    echo "kind_node=${KIND_NODE}"
    echo "namespace=${K8S_NAMESPACE}"
} >"${WORK}/MANIFEST"

failures=0
for entry in ${BACKUP_DATABASES}; do
    IFS=: read -r db pod user <<<"${entry}"
    out="${WORK}/${db}.dump"
    log "dumping ${db} from pod ${pod} as ${user}"
    if ! kube exec "${pod}" -- pg_dump --format=custom --username="${user}" --dbname="${db}" >"${out}"; then
        log "pg_dump of ${db} failed"
        failures=$((failures + 1))
        continue
    fi
    if [[ ! -s "${out}" ]]; then
        log "pg_dump of ${db} produced an empty file"
        failures=$((failures + 1))
        continue
    fi

    log "verifying ${db}.dump (pg_restore --list, then a full read)"
    if ! "${RESTORE[@]}" --list "${out}" >"${out}.toc"; then
        log "pg_restore --list failed for ${db}"
        failures=$((failures + 1))
        continue
    fi
    if ! "${RESTORE[@]}" --file=/dev/null "${out}"; then
        log "full archive read failed for ${db}"
        failures=$((failures + 1))
        continue
    fi
    entries="$(grep -cv '^;' "${out}.toc" || true)"
    size="$(stat -c %s "${out}")"
    echo "${db}.bytes=${size}" >>"${WORK}/MANIFEST"
    echo "${db}.toc_entries=${entries}" >>"${WORK}/MANIFEST"

    # Row counts at dump time, for the restore drill to compare against.
    # Taken just after the dump, so a busy crawler can make them differ a
    # little from what the dump's own snapshot contains.
    if [[ "${db}" == "econ_graph" ]]; then
        counts="$(kube exec "${pod}" -- psql -X -At --username="${user}" --dbname="${db}" \
            -c "select (select count(*) from economic_series) || ' ' || (select count(*) from data_points)" || true)"
        if [[ "${counts}" =~ ^([0-9]+)\ ([0-9]+)$ ]]; then
            echo "econ_graph.series=${BASH_REMATCH[1]}" >>"${WORK}/MANIFEST"
            echo "econ_graph.data_points=${BASH_REMATCH[2]}" >>"${WORK}/MANIFEST"
        fi
    fi
    log "${db}: OK (${size} bytes, ${entries} TOC entries)"
done

if (( failures > 0 )); then
    die "${failures} database(s) failed to dump or verify; this run's files are in ${FINAL}.failed"
fi

(cd "${WORK}" && sha256sum -- *.dump >SHA256SUMS)
echo "finished_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)" >>"${WORK}/MANIFEST"
date -u +%Y-%m-%dT%H:%M:%SZ >"${WORK}/VERIFIED"
mv "${WORK}" "${FINAL}"
trap - EXIT
log "dump OK: ${FINAL}"

