#!/usr/bin/env bash
#
# Builds a sanitized copy of the production databases (the "golden fixture"
# for release labs and CI) from their dumps, or with --ci-subset a size-capped
# subset of it that may leave the host.
#
# The dumps are restored into a scratch postgres:18 container on the rootless
# Docker daemon (never the rootful one the cluster runs on), sanitized there
# (scripts/fixtures/sql/steps.sql), verified (sql/checks.sql and the scan in
# sql/functions.sql), and dumped again with a MANIFEST and SHA256SUMS. See
# docs/deployment/SANITIZED_FIXTURES.md.
#
# Inputs, read only (mounted read-only into the container; nothing that holds
# a dump or backup is ever written, moved or deleted):
#   --data FILE       econ_graph dump. With APP_DATABASE=shared it also holds
#                     the app tables.
#   --app FILE        econ_graph_app dump (APP_DATABASE=separate); optional.
#   --keycloak FILE   keycloak dump.
#   --from-dir DIR    take the three from DIR instead: a host dump directory
#                     (scripts/backup/dump-databases.sh: econ_graph.dump,
#                     econ_graph_app.dump, keycloak.dump), or a copy of the
#                     postgres-backup volume (latest.dump,
#                     latest-econ_graph_app.dump, latest-keycloak.dump, or else
#                     the newest econ_graph-<stamp>.dump and the app and
#                     Keycloak dumps of the same stamp).
#   --latest          --from-dir with the newest VERIFIED directory of the
#                     host dumps (BACKUP_ROOT, as scripts/backup/lib.sh sets it).
#
# Output (never overwritten, never pruned):
#   --output-dir DIR  default ~/econgraph-fixtures/<UTC stamp>, with
#                     -ci-subset appended in --ci-subset mode. Holds
#                     econ_graph.dump, econ_graph_app.dump (when given),
#                     keycloak.dump, MANIFEST and SHA256SUMS, files 0600.
#                     If DIR already holds a fixture built from the same
#                     inputs in the same mode, the run does nothing and exits 0;
#                     any other existing DIR is refused.
#
# Modes:
#   (default)         the full sanitized copy of every database. Stays on the host.
#   --ci-subset       the data database cut to about --subset-series series
#                     (default 300) spread over every source, each with at
#                     most --subset-max-points data_points rows (default 5000)
#                     and all of them (every vintage), plus the reference
#                     tables they need; the app and Keycloak databases whole.
#                     Fails if the dumps total more than --subset-max-kb
#                     (default 51200).
#   --dry-run         check the inputs and the Docker daemon, print the plan;
#                     start nothing and write nothing.
#   --list-steps      print the sanitize steps and exit.
#
# Any sanitize step that fails, any check that finds something, or any real
# e-mail address or secret-shaped value the scan finds fails the run: the
# exit status is non-zero and nothing is written to the output directory.
# Findings name the table, column and kind and a row count, never a value.
#
# Environment:
#   FIXTURE_DOCKER_HOST          daemon to use (default the rootless socket,
#                                unix:///run/user/<uid>/docker.sock).
#   FIXTURE_ALLOW_ROOTFUL_DOCKER true allows a daemon that isn't rootless
#                                (CI runners have only a rootful one).
#   FIXTURE_PG_IMAGE             default postgres:18.
#   FIXTURE_SALT_FILE            the pseudonym key, created (0600) on first
#                                use; default ~/.config/econgraph/fixture-salt.
#                                The same key gives the same pseudonyms.
#   FIXTURE_TEST_SKIP_STEPS      test only: space-separated steps to leave out,
#                                to show that verification catches each one.
#
# Usage: build-sanitized-fixture.sh [--data F --keycloak F [--app F] |
#            --from-dir DIR | --latest] [--output-dir DIR] [--ci-subset
#            [--subset-series N] [--subset-max-points N] [--subset-max-kb N]]
#            [--dry-run]
#        build-sanitized-fixture.sh --list-steps

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SQL_DIR="${SCRIPT_DIR}/sql"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

log() { printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*" >&2; }
die() { log "ERROR: $*"; exit 1; }

# The step names in steps.sql, in order: "<name> <kind>" per line.
list_steps() {
    awk '/^--@step / { print $2, $3 }' "${SQL_DIR}/steps.sql"
}

# The SQL of the steps of these kinds, without the skipped ones.
steps_sql() {
    local kinds=" $1 " skip=" ${FIXTURE_TEST_SKIP_STEPS:-} "
    awk -v kinds="${kinds}" -v skip="${skip}" '
        /^--@step / {
            on = index(kinds, " " $3 " ") > 0 && index(skip, " " $2 " ") == 0
            if (on) printf "\\echo step %s\n", $2
            next
        }
        on' "${SQL_DIR}/steps.sql"
}

DATA="" APP="" KEYCLOAK="" FROM_DIR="" LATEST=false OUT="" CI_SUBSET=false DRY_RUN=false
SUBSET_SERIES=300 SUBSET_MAX_POINTS=5000 SUBSET_MAX_KB=51200
while [[ $# -gt 0 ]]; do
    case "$1" in
        --data) DATA="${2:?--data needs a file}"; shift 2 ;;
        --app) APP="${2:?--app needs a file}"; shift 2 ;;
        --keycloak) KEYCLOAK="${2:?--keycloak needs a file}"; shift 2 ;;
        --from-dir) FROM_DIR="${2:?--from-dir needs a directory}"; shift 2 ;;
        --latest) LATEST=true; shift ;;
        --output-dir) OUT="${2:?--output-dir needs a directory}"; shift 2 ;;
        --ci-subset) CI_SUBSET=true; shift ;;
        --subset-series) SUBSET_SERIES="${2:?}"; shift 2 ;;
        --subset-max-points) SUBSET_MAX_POINTS="${2:?}"; shift 2 ;;
        --subset-max-kb) SUBSET_MAX_KB="${2:?}"; shift 2 ;;
        --dry-run|-n) DRY_RUN=true; shift ;;
        --list-steps) list_steps; exit 0 ;;
        -h|--help) sed -n '2,/^set -euo/p' "$0" | sed '$d' | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) die "unknown argument: $1 (see --help)" ;;
    esac
done

for n in "${SUBSET_SERIES}" "${SUBSET_MAX_POINTS}" "${SUBSET_MAX_KB}"; do
    [[ "${n}" =~ ^[1-9][0-9]*$ ]] || die "--subset-* values must be positive integers (got ${n})"
done

# ---------------------------------------------------------------- inputs

if [[ "${LATEST}" == true ]]; then
    [[ -z "${FROM_DIR}" ]] || die "--latest and --from-dir are exclusive"
    # lib.sh decides where the host dumps live; in a subshell, since it also
    # points DOCKER_HOST at the rootful daemon.
    FROM_DIR="$(
        # shellcheck source=scripts/backup/lib.sh
        source "${REPO_ROOT}/scripts/backup/lib.sh"
        latest_verified_dump_dir || true
    )"
    [[ -n "${FROM_DIR}" ]] || die "--latest: no VERIFIED dump directory under the host backup root"
    log "--latest: ${FROM_DIR}"
fi

if [[ -n "${FROM_DIR}" ]]; then
    [[ -z "${DATA}${APP}${KEYCLOAK}" ]] || die "--from-dir/--latest and --data/--app/--keycloak are exclusive"
    [[ -d "${FROM_DIR}" ]] || die "not a directory: ${FROM_DIR}"
    if [[ -e "${FROM_DIR}/econ_graph.dump" ]]; then
        # Host layout (dump-databases.sh).
        DATA="${FROM_DIR}/econ_graph.dump"
        [[ -e "${FROM_DIR}/econ_graph_app.dump" ]] && APP="${FROM_DIR}/econ_graph_app.dump"
        KEYCLOAK="${FROM_DIR}/keycloak.dump"
        [[ -e "${FROM_DIR}/VERIFIED" ]] || log "WARNING: ${FROM_DIR} has no VERIFIED marker"
    elif [[ -e "${FROM_DIR}/latest.dump" ]]; then
        # The postgres-backup volume, with its latest*.dump links.
        DATA="${FROM_DIR}/latest.dump"
        [[ -e "${FROM_DIR}/latest-econ_graph_app.dump" ]] && APP="${FROM_DIR}/latest-econ_graph_app.dump"
        KEYCLOAK="${FROM_DIR}/latest-keycloak.dump"
    else
        # A copy without the links (the S3 sync leaves them out): the newest
        # econ_graph-<stamp>.dump, and the others of the same run.
        DATA="$(find "${FROM_DIR}" -maxdepth 1 -name 'econ_graph-[0-9]*-[0-9]*.dump' -printf '%f\n' | sort | tail -n 1)"
        [[ -n "${DATA}" ]] || die "${FROM_DIR} holds no econ_graph dump (econ_graph.dump, latest.dump or econ_graph-<stamp>.dump)"
        stamp="${DATA#econ_graph-}"
        stamp="${stamp%.dump}"
        DATA="${FROM_DIR}/${DATA}"
        [[ -e "${FROM_DIR}/econ_graph_app-${stamp}.dump" ]] && APP="${FROM_DIR}/econ_graph_app-${stamp}.dump"
        KEYCLOAK="${FROM_DIR}/keycloak-${stamp}.dump"
    fi
fi

[[ -n "${DATA}" ]] || die "no data dump: give --data, --from-dir or --latest"
[[ -n "${KEYCLOAK}" ]] || die "no Keycloak dump: give --keycloak"

# role -> resolved input file (latest*.dump links resolved once, so every step
# reads the same dump).
declare -A INPUT=()
ROLES=(econ_graph)
[[ -n "${APP}" ]] && ROLES+=(econ_graph_app)
ROLES+=(keycloak)
for role in "${ROLES[@]}"; do
    case "${role}" in
        econ_graph) f="${DATA}" ;;
        econ_graph_app) f="${APP}" ;;
        keycloak) f="${KEYCLOAK}" ;;
    esac
    [[ -e "${f}" ]] || die "${role} dump not found: ${f}"
    f="$(readlink -f -- "${f}")"
    [[ -f "${f}" && -r "${f}" && -s "${f}" ]] || die "${role} dump is not a readable, non-empty file: ${f}"
    INPUT[${role}]="${f}"
done

# ---------------------------------------------------------------- output

STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
MODE=full
[[ "${CI_SUBSET}" == true ]] && MODE=ci-subset
if [[ -z "${OUT}" ]]; then
    OUT="${HOME}/econgraph-fixtures/${STAMP}"
    [[ "${MODE}" == ci-subset ]] && OUT="${OUT}-ci-subset"
fi
OUT="$(realpath -m -- "${OUT}")"
OUT_PARENT="$(dirname "${OUT}")"

# What identifies a build: the mode, its parameters and every input's sha256.
build_key() {
    local role
    echo "mode=${MODE}"
    if [[ "${MODE}" == ci-subset ]]; then
        echo "subset.series=${SUBSET_SERIES}"
        echo "subset.max_points=${SUBSET_MAX_POINTS}"
    fi
    for role in "${ROLES[@]}"; do
        echo "input.${role}.sha256=$(sha256sum <"${INPUT[${role}]}" | cut -d' ' -f1)"
    done
}

# ---------------------------------------------------------------- docker

# (The --latest subshell's DOCKER_HOST never reaches here, as intended.)
# shellcheck disable=SC2031
export DOCKER_HOST="${FIXTURE_DOCKER_HOST:-unix:///run/user/$(id -u)/docker.sock}"
unset DOCKER_CONTEXT
PG_IMAGE="${FIXTURE_PG_IMAGE:-postgres:18}"

docker_info="$(docker info --format '{{.ServerVersion}} {{json .SecurityOptions}}' 2>/dev/null)" \
    || die "cannot reach the Docker daemon at ${DOCKER_HOST}"
if [[ "${docker_info}" != *rootless* ]]; then
    [[ "${FIXTURE_ALLOW_ROOTFUL_DOCKER:-}" == true ]] \
        || die "${DOCKER_HOST} is not a rootless Docker daemon; this script runs only on the rootless one (FIXTURE_DOCKER_HOST), not on the daemon the cluster runs on"
    log "docker: ${DOCKER_HOST} (rootful, allowed by FIXTURE_ALLOW_ROOTFUL_DOCKER)"
else
    log "docker: ${DOCKER_HOST} (rootless)"
fi

if [[ "${DRY_RUN}" == true ]]; then
    log "dry run: mode ${MODE}; output ${OUT} (via a scratch directory in ${OUT_PARENT}, moved there only if verification passes)"
    [[ -e "${OUT}" ]] && log "dry run: ${OUT} already exists; a run would do nothing if it holds this build, and refuse otherwise"
    for role in "${ROLES[@]}"; do
        log "dry run: ${role} <- ${INPUT[${role}]} ($(stat -c %s "${INPUT[${role}]}") bytes, read only)"
    done
    [[ -n "${APP}" ]] || log "dry run: no app dump: the app tables are expected in econ_graph (APP_DATABASE=shared)"
    log "dry run: scratch container ${PG_IMAGE}, no network; steps: $(list_steps | awk '{ printf "%s%s", sep, $1; sep = " " }')"
    if [[ "${MODE}" == ci-subset ]]; then
        log "dry run: subset of ${SUBSET_SERIES} series, at most ${SUBSET_MAX_POINTS} data points each, at most ${SUBSET_MAX_KB} KiB"
    fi
    log "dry run: pseudonym key ${FIXTURE_SALT_FILE:-${XDG_CONFIG_HOME:-${HOME}/.config}/econgraph/fixture-salt}"
    log "dry run: nothing started, nothing written"
    exit 0
fi

# ---------------------------------------------------------------- build

umask 077
mkdir -p "${OUT_PARENT}"
exec 9>"${OUT_PARENT}/.fixture-build.lock"
flock -n 9 || die "another fixture build is running (lock ${OUT_PARENT}/.fixture-build.lock)"

KEY="$(build_key)"
if [[ -e "${OUT}" ]]; then
    if [[ -f "${OUT}/MANIFEST" ]] && (cd "${OUT}" && sha256sum --quiet -c SHA256SUMS >/dev/null 2>&1) \
        && diff -q <(printf '%s\n' "${KEY}") <(grep -E '^(mode|subset\.series|subset\.max_points|input\.[a-z_]+\.sha256)=' "${OUT}/MANIFEST") >/dev/null; then
        log "${OUT} already holds this fixture (same mode and inputs); nothing to do"
        exit 0
    fi
    die "${OUT} already exists and is not this build; choose another --output-dir (fixtures are never overwritten)"
fi

SALT_FILE="${FIXTURE_SALT_FILE:-${XDG_CONFIG_HOME:-${HOME}/.config}/econgraph/fixture-salt}"
if [[ ! -s "${SALT_FILE}" ]]; then
    mkdir -p "$(dirname "${SALT_FILE}")"
    head -c 32 /dev/urandom | sha256sum | cut -d' ' -f1 >"${SALT_FILE}.new"
    mv -n "${SALT_FILE}.new" "${SALT_FILE}"
    rm -f "${SALT_FILE}.new"
    log "created the pseudonym key ${SALT_FILE}"
fi
SALT="$(tr -dc '0-9a-f' <"${SALT_FILE}")"
[[ ${#SALT} -ge 32 ]] || die "${SALT_FILE} must hold at least 32 hex digits"

WORK="$(mktemp -d "${OUT_PARENT}/.build-${STAMP}-XXXXXX")"
CONTAINER="econgraph-fixture-${STAMP,,}-$$"
cleanup() {
    docker rm -f "${CONTAINER}" >/dev/null 2>&1 || true
    # Only this run's scratch directory: what it holds was never verified.
    rm -rf -- "${WORK}"
}
trap cleanup EXIT

mounts=()
for role in "${ROLES[@]}"; do
    mounts+=(-v "${INPUT[${role}]}:/in/${role}.dump:ro")
done
log "starting scratch container ${CONTAINER} (${PG_IMAGE}, no network)"
docker run -d --name "${CONTAINER}" --network none \
    -e POSTGRES_HOST_AUTH_METHOD=trust \
    "${mounts[@]}" "${PG_IMAGE}" \
    -c fsync=off -c synchronous_commit=off -c full_page_writes=off \
    -c max_wal_size=8GB -c maintenance_work_mem=1GB -c max_locks_per_transaction=1024 >/dev/null

pg() { docker exec -i "${CONTAINER}" "$@"; }
psql_db() { pg psql -X -q -v ON_ERROR_STOP=1 -U postgres -d "$1" "${@:2}"; }

for _ in $(seq 1 60); do
    # The image's entrypoint restarts the server after initdb; wait for the
    # final one (listening on TCP inside the container).
    if pg pg_isready -q -h 127.0.0.1 -U postgres 2>/dev/null; then break; fi
    sleep 1
done
pg pg_isready -q -h 127.0.0.1 -U postgres || die "the scratch PostgreSQL did not start"
log "scratch server: $(psql_db postgres -At -c 'show server_version')"

declare -A ARCHIVE_CREATED=() ARCHIVE_DBNAME=() KINDS=()
for role in "${ROLES[@]}"; do
    header="$(pg pg_restore --list "/in/${role}.dump" | head -n 20)" || die "${role}: ${INPUT[${role}]} is not a pg_dump archive"
    ARCHIVE_CREATED[${role}]="$(sed -n 's/^;[[:space:]]*Archive created at //p' <<<"${header}")"
    ARCHIVE_DBNAME[${role}]="$(sed -n 's/^;[[:space:]]*dbname: //p' <<<"${header}")"
    log "${role}: restoring ${INPUT[${role}]} (dumped from ${ARCHIVE_DBNAME[${role}]:-?} at ${ARCHIVE_CREATED[${role}]:-?})"
    psql_db postgres -c "CREATE DATABASE ${role}"
    pg pg_restore --no-owner --no-privileges --exit-on-error -U postgres -d "${role}" "/in/${role}.dump" \
        || die "${role}: pg_restore failed"
    kinds=""
    for k in app:public.users keycloak:public.user_entity data:public.economic_series; do
        t="$(psql_db "${role}" -At -c "select to_regclass('${k#*:}') is not null")"
        [[ "${t}" == t ]] && kinds+="${k%%:*} "
    done
    KINDS[${role}]="${kinds% }"
    log "${role}: holds ${KINDS[${role}]:-no tables this script knows}"
done

# Every kind must be somewhere, and Keycloak in the keycloak dump.
all_kinds=" ${KINDS[*]} "
for k in app data keycloak; do
    [[ "${all_kinds}" == *" ${k} "* ]] || die "no restored database holds the ${k} tables; are these the right dumps?"
done
[[ " ${KINDS[keycloak]} " == *" keycloak "* ]] || die "the keycloak dump holds no Keycloak tables"
[[ " ${KINDS[econ_graph]} " == *" data "* ]] || die "the econ_graph dump holds no data tables"

if [[ -n "${FIXTURE_TEST_SKIP_STEPS:-}" ]]; then
    log "WARNING: FIXTURE_TEST_SKIP_STEPS leaves out: ${FIXTURE_TEST_SKIP_STEPS} (test only)"
fi

# Sanitize: per database, the helpers and every step of its kinds, in one
# transaction. The key goes in on stdin, never on a command line.
for role in "${ROLES[@]}"; do
    log "${role}: sanitizing (${KINDS[${role}]})"
    {
        echo "SELECT set_config('fixture.salt', '${SALT}', false) \\g /dev/null"
        cat "${SQL_DIR}/functions.sql" "${SQL_DIR}/checks.sql"
        echo "BEGIN;"
        echo "SET LOCAL session_replication_role = replica;"
        steps_sql "${KINDS[${role}]} all"
        echo "COMMIT;"
    } | psql_db "${role}" -o /dev/null 2>&1 | sed "s/^/  [${role}] /" >&2 || die "${role}: a sanitize step failed"
done

# Verification: the per-table checks and the scan of every text and JSON
# column, on every database. Any line fails the run.
verify() {
    local role failed=0 out
    for role in "${ROLES[@]}"; do
        out="$(psql_db "${role}" -At -F $'\t' <<'SQL'
SELECT 'CHECK', check_name, n FROM fixture_sanitize.checks();
SELECT 'FOUND', finding || ' in ' || tbl || '.' || column_name, n FROM fixture_sanitize.scan();
SQL
)" || { log "${role}: verification could not run"; failed=1; continue; }
        if [[ -n "${out}" ]]; then
            while IFS=$'\t' read -r what detail n; do
                log "VERIFICATION FAILED [${role}] ${what}: ${detail} (${n} rows)"
            done <<<"${out}"
            failed=1
        else
            log "${role}: verification passed"
        fi
    done
    return "${failed}"
}
verify || die "verification failed; nothing written to ${OUT}"

if [[ "${MODE}" == ci-subset ]]; then
    log "econ_graph: cutting the CI subset (${SUBSET_SERIES} series, at most ${SUBSET_MAX_POINTS} data points each)"
    psql_db econ_graph -At -F $'\t' -v series_target="${SUBSET_SERIES}" -v max_points="${SUBSET_MAX_POINTS}" \
        <"${SQL_DIR}/subset.sql" 2>&1 | sed 's/^/  [econ_graph] /' >&2 || die "cutting the subset failed"
    verify || die "verification of the subset failed; nothing written to ${OUT}"
fi

# Row counts, then the helpers go and each database is dumped.
declare -A COUNTS=()
for role in "${ROLES[@]}"; do
    COUNTS[${role}]="$(psql_db "${role}" -At -F '=' -c 'SELECT tbl, n FROM fixture_sanitize.row_counts()')"
    psql_db "${role}" -c 'DROP SCHEMA fixture_sanitize CASCADE'
    log "${role}: dumping"
    pg pg_dump --format=custom --no-owner --no-privileges -U postgres -d "${role}" >"${WORK}/${role}.dump" \
        || die "${role}: pg_dump failed"
    docker cp "${WORK}/${role}.dump" "${CONTAINER}:/tmp/check.dump" >/dev/null
    pg pg_restore --list /tmp/check.dump >/dev/null || die "${role}: the sanitized dump does not read back"
    pg rm -f /tmp/check.dump
done

total_kb=$(( ($(cat "${WORK}"/*.dump | wc -c) + 1023) / 1024 ))
if [[ "${MODE}" == ci-subset ]] && (( total_kb > SUBSET_MAX_KB )); then
    die "the CI subset is ${total_kb} KiB, over --subset-max-kb ${SUBSET_MAX_KB}; nothing written to ${OUT}"
fi

{
    echo "format=1"
    printf '%s\n' "${KEY}" | grep -v '^input\.'
    echo "built_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "host=$(hostname)"
    echo "script_commit=$(git -C "${REPO_ROOT}" rev-parse HEAD 2>/dev/null || echo unknown)"
    echo "pg_image=${PG_IMAGE}"
    echo "pg_server=$(psql_db postgres -At -c 'show server_version')"
    echo "pseudonym_key_id=$(printf '%s' "${SALT}" | sha256sum | cut -c1-16)"
    echo "app_database=$([[ -n "${APP}" ]] && echo separate || echo shared)"
    echo "total_kib=${total_kb}"
    for role in "${ROLES[@]}"; do
        f="${INPUT[${role}]}"
        echo "input.${role}.file=$(basename "$(dirname "${f}")")/$(basename "${f}")"
        echo "input.${role}.bytes=$(stat -c %s "${f}")"
        echo "input.${role}.mtime=$(date -u -d "@$(stat -c %Y "${f}")" +%Y-%m-%dT%H:%M:%SZ)"
        echo "input.${role}.archive_created=${ARCHIVE_CREATED[${role}]}"
        echo "input.${role}.archive_dbname=${ARCHIVE_DBNAME[${role}]}"
        printf '%s\n' "${KEY}" | grep "^input\.${role}\.sha256="
        echo "input.${role}.kinds=${KINDS[${role}]}"
    done
    for role in "${ROLES[@]}"; do
        echo "output.${role}.file=${role}.dump"
        echo "output.${role}.bytes=$(stat -c %s "${WORK}/${role}.dump")"
        echo "output.${role}.sha256=$(sha256sum <"${WORK}/${role}.dump" | cut -d' ' -f1)"
        # shellcheck disable=SC2001 # prefix every line
        sed "s/^/rows.${role}./" <<<"${COUNTS[${role}]}"
    done
} >"${WORK}/MANIFEST"
(cd "${WORK}" && sha256sum -- *.dump MANIFEST >SHA256SUMS)

mv -T -- "${WORK}" "${OUT}"
trap 'docker rm -f "${CONTAINER}" >/dev/null 2>&1 || true' EXIT
log "fixture OK: ${OUT} (${MODE}, ${total_kb} KiB)"
for role in "${ROLES[@]}"; do
    log "  ${role}.dump $(stat -c %s "${OUT}/${role}.dump") bytes"
done
log "the first fixture from real dumps is for the operator to review before anything uses it"
