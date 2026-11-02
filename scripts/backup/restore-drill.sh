#!/bin/bash
# Restore drill: prove the newest dump actually restores.
#
# Starts a throwaway Postgres container (PG_IMAGE, random localhost port,
# removed afterwards), restores the econ_graph dump into it (and the
# econ_graph_app and keycloak dumps next to it, if present), runs
# verification queries and
# prints one PASS/FAIL line. On PASS it records the result in
# ECONGRAPH_STATE_DIR/restore-drill.last for check-freshness.sh.
#
# Never touches the cluster or the live databases.
#
# Usage:
#   restore-drill.sh [--dry-run] [--expect-series N] [--expect-data-points N] [DUMP_FILE]
#
# DUMP_FILE defaults to econ_graph.dump in the newest verified dump
# directory under BACKUP_ROOT; with none, the drill fails rather than pass on
# a dump that dump-databases.sh did not mark VERIFIED. Without --expect-*, the counts recorded in
# that directory's MANIFEST at dump time are compared, as a warning only.
# --dry-run checks the Docker daemon, picks the dump and prints the plan,
# without starting a container or writing state.

set -euo pipefail

# shellcheck source=scripts/backup/lib.sh
source "$(dirname "$(readlink -f "$0")")/lib.sh"

expect_series=""
expect_points=""
dump=""
DRY_RUN="false"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run|-n) DRY_RUN="true"; shift ;;
        --expect-series) expect_series="$2"; shift 2 ;;
        --expect-data-points) expect_points="$2"; shift 2 ;;
        -h|--help) sed -n '2,21p' "$0"; exit 0 ;;
        -*) die "unknown option: $1" ;;
        *) dump="$1"; shift ;;
    esac
done

if [[ -z "${dump}" ]]; then
    dir="$(latest_verified_dump_dir || true)"
    [[ -n "${dir}" ]] || die "no VERIFIED dump directory under ${BACKUP_ROOT} (pass DUMP_FILE to drill an unverified dump)"
    self="$(readlink -f "$0")"
    printf -v open_command '%q %q -- %q %q' "$(dirname "$self")/open-backup.sh" "$dir" "$self" '@BACKUP@/econ_graph.dump'
    [[ ! -f "${dir}/backup.tar.age" ]] || die "encrypted backup: run $open_command at an operator terminal; no unattended restore was performed"
    dump="${dir}/econ_graph.dump"
fi
dump="$(readlink -f "${dump}")"
[[ -f "${dump}" ]] || die "dump not found: ${dump}"
dump_dir="$(dirname "${dump}")"

check_docker_daemon
if [[ "${DRY_RUN}" == "true" ]]; then
    log "dry run: would restore ${dump} into a throwaway ${PG_IMAGE} container on ${DOCKER_HOST}"
    [[ -f "${dump_dir}/econ_graph_app.dump" ]] && log "dry run: and ${dump_dir}/econ_graph_app.dump"
    [[ -f "${dump_dir}/keycloak.dump" ]] && log "dry run: and ${dump_dir}/keycloak.dump"
    log "dry run: then record a PASS in ${ECONGRAPH_STATE_DIR}/restore-drill.last"
    exit 0
fi
manifest="${dump_dir}/MANIFEST"

manifest_value() {
    [[ -f "${manifest}" ]] && sed -n "s/^$1=//p" "${manifest}" | tail -n 1
}

name="econgraph-restore-drill-$$-$(date +%s)"
password="$(head -c 18 /dev/urandom | base64 | tr -dc 'A-Za-z0-9')"
cleanup() { docker rm -f -v "${name}" >/dev/null 2>&1 || true; }
trap cleanup EXIT

log "drill: restoring ${dump} into throwaway container ${name} (${PG_IMAGE})"
# The restored databases are plaintext production data: keep the server's
# data directory in RAM, never in a Docker volume on disk. Every VOLUME the
# image declares gets a tmpfs (otherwise Docker makes an anonymous volume on
# disk there), and PGDATA must lie under one of them.
docker image inspect "${PG_IMAGE}" >/dev/null 2>&1 || docker pull -q "${PG_IMAGE}" >/dev/null ||
    die "could not pull ${PG_IMAGE}"
volumes="$(docker image inspect --format '{{range $path, $_ := .Config.Volumes}}{{println $path}}{{end}}' "${PG_IMAGE}")" ||
    die "could not read the volumes of ${PG_IMAGE}"
pgdata="$(docker image inspect --format '{{range .Config.Env}}{{println .}}{{end}}' "${PG_IMAGE}" | sed -n 's/^PGDATA=//p')" ||
    die "could not read PGDATA of ${PG_IMAGE}"
tmpfs_args=()
covered=false
while IFS= read -r path; do
    [[ -n "${path}" ]] || continue
    [[ "${path}" == /* ]] || die "${PG_IMAGE} declares a relative VOLUME ${path}; refusing"
    tmpfs_args+=(--tmpfs "${path}")
    [[ -z "${pgdata}" || "${pgdata%/}/" != "${path%/}/"* ]] || covered=true
done < <(printf '%s\n' /var/lib/postgresql "${volumes}" | sort -u)
[[ -n "${pgdata}" && "${covered}" == true ]] ||
    die "${PG_IMAGE}'s PGDATA (${pgdata:-unset}) is not under a tmpfs mount; refusing to restore plaintext onto disk"
docker run -d --rm --name "${name}" \
    -e POSTGRES_PASSWORD="${password}" \
    "${tmpfs_args[@]}" \
    -p 127.0.0.1::5432 \
    -v "${dump_dir}:/dumps:ro" \
    "${PG_IMAGE}" >/dev/null
port="$(docker port "${name}" 5432/tcp | head -n 1 | sed 's/.*://')"
log "drill: container listening on 127.0.0.1:${port}"

psql_in() { docker exec -i "${name}" psql -X -v ON_ERROR_STOP=1 -U postgres "$@"; }

# The image's entrypoint restarts the server once after initdb; wait for the
# final one by requiring a real query to succeed.
for _ in $(seq 1 60); do
    if docker exec "${name}" pg_isready -U postgres -q 2>/dev/null \
        && psql_in -d postgres -Atc 'select 1' >/dev/null 2>&1; then
        break
    fi
    sleep 1
done
psql_in -d postgres -Atc 'select 1' >/dev/null || die "throwaway Postgres never became ready"

fail() {
    echo "RESTORE DRILL FAIL: $* (dump ${dump})"
    exit 1
}

restore_one() {
    local db="$1" file="$2"
    psql_in -d postgres -qc "create database ${db}" || return 1
    # --no-owner/--no-privileges: the production roles don't exist here.
    docker exec "${name}" pg_restore -U postgres --dbname="${db}" \
        --no-owner --no-privileges --exit-on-error "/dumps/${file}"
}

start=$(date +%s)
restore_one econ_graph "$(basename "${dump}")" || fail "pg_restore of econ_graph failed"
log "drill: econ_graph restored in $(( $(date +%s) - start ))s"

q() { psql_in -d econ_graph -Atc "$1"; }

series="$(q 'select count(*) from economic_series')"
points="$(q 'select count(*) from data_points')"
sources="$(q 'select count(*) from data_sources')"
orphans="$(q 'select count(*) from data_points dp where not exists (select 1 from economic_series s where s.id = dp.series_id)')"
series_with_points="$(q 'select count(distinct series_id) from data_points')"
null_dates="$(q 'select count(*) from data_points where date is null')"
migrations="$(q 'select count(*) from __diesel_schema_migrations')"
latest_migration="$(q 'select max(version) from __diesel_schema_migrations')"
date_range="$(q "select min(date) || '..' || max(date) from data_points")"
per_source="$(q "select string_agg(ds.name || '=' || n, ', ' order by n desc) from (select source_id, count(*) n from economic_series group by 1) c join data_sources ds on ds.id = c.source_id")"

log "drill: economic_series=${series} data_points=${points} data_sources=${sources}"
log "drill: series_with_points=${series_with_points} orphan_points=${orphans} null_dates=${null_dates}"
log "drill: diesel migrations=${migrations} latest=${latest_migration} data_points.date=${date_range}"
log "drill: series per source: ${per_source}"

(( series > 0 )) || fail "economic_series is empty"
(( points > 0 )) || fail "data_points is empty"
(( sources > 0 )) || fail "data_sources is empty"
(( orphans == 0 )) || fail "${orphans} data_points reference missing series"
(( null_dates == 0 )) || fail "${null_dates} data_points have no date"
(( migrations > 0 )) || fail "__diesel_schema_migrations is empty"
[[ -z "${expect_series}" || "${series}" == "${expect_series}" ]] \
    || fail "economic_series=${series}, expected ${expect_series}"
[[ -z "${expect_points}" || "${points}" == "${expect_points}" ]] \
    || fail "data_points=${points}, expected ${expect_points}"

m_series="$(manifest_value econ_graph.series || true)"
m_points="$(manifest_value econ_graph.data_points || true)"
if [[ -n "${m_series}" && "${m_series}" != "${series}" ]] || [[ -n "${m_points}" && "${m_points}" != "${points}" ]]; then
    log "drill: WARNING counts differ from MANIFEST (series ${m_series}, data_points ${m_points});" \
        "expected only if the crawler wrote during the dump"
fi

app_note="app: no dump"
if [[ -f "${dump_dir}/econ_graph_app.dump" ]]; then
    restore_one econ_graph_app econ_graph_app.dump || fail "pg_restore of econ_graph_app failed"
    app_q() { psql_in -d econ_graph_app -Atc "$1"; }
    if [[ "$(app_q "select to_regclass('public.users') is not null")" == "t" ]]; then
        app_users="$(app_q 'select count(*) from users')"
        app_annotations="$(app_q 'select count(*) from chart_annotations')"
        app_migrations="$(app_q 'select count(*) from __diesel_schema_migrations')"
        (( app_migrations > 0 )) || fail "econ_graph_app restored with no diesel migrations recorded"
        m_users="$(manifest_value econ_graph_app.users || true)"
        m_annotations="$(manifest_value econ_graph_app.chart_annotations || true)"
        if [[ -n "${m_users}" && "${m_users}" != "${app_users}" ]] \
            || [[ -n "${m_annotations}" && "${m_annotations}" != "${app_annotations}" ]]; then
            log "drill: WARNING app counts differ from MANIFEST (users ${m_users}, chart_annotations ${m_annotations})"
        fi
        app_note="app: users=${app_users} chart_annotations=${app_annotations}"
    else
        app_note="app: no tables"
    fi
fi

keycloak_note="keycloak: no dump"
if [[ -f "${dump_dir}/keycloak.dump" ]]; then
    restore_one keycloak keycloak.dump || fail "pg_restore of keycloak failed"
    realms="$(psql_in -d keycloak -Atc 'select count(*) from realm')"
    users="$(psql_in -d keycloak -Atc 'select count(*) from user_entity')"
    (( realms > 0 )) || fail "keycloak restored with no realms"
    keycloak_note="keycloak: realms=${realms} users=${users}"
fi

summary="series=${series} data_points=${points} sources=${sources} ${app_note} ${keycloak_note}"
echo "RESTORE DRILL PASS: ${summary} (dump ${dump}, $(( $(date +%s) - start ))s)"

mkdir -p "${ECONGRAPH_STATE_DIR}"
{
    echo "passed_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "passed_epoch=$(date +%s)"
    echo "dump=${dump}"
    echo "summary=${summary}"
} >"${ECONGRAPH_STATE_DIR}/restore-drill.last.tmp"
mv "${ECONGRAPH_STATE_DIR}/restore-drill.last.tmp" "${ECONGRAPH_STATE_DIR}/restore-drill.last"
