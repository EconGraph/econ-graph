#!/usr/bin/env bash
#
# Convert a kind cluster's combined database: copy its app records into the app
# database (scripts/deploy/split-app-data.py), optionally remove them from the
# combined database, or roll that removal back. docs/deployment/APP_DATA_CONVERSION.md
# is the runbook.
#
# Usage:
#   scripts/deploy/convert-app-data.sh --cluster NAME [--production] --source-map FILE \
#       [--drop-source-app] [--previous-evidence FILE] [--dry-run]
#   scripts/deploy/convert-app-data.sh --cluster NAME [--production] \
#       --restore-source-app BACKUP_DIR [--dry-run]
#
#   --cluster NAME        the kind cluster (no default); every kubectl call uses that
#                         cluster's own kubectl and the context kind-NAME
#   --production          required exactly when the cluster carries the production
#                         marker (lib/cluster-role.sh); a mismatch stops before anything
#   --source-map FILE     data_sources UUID -> crawler source code (see the runbook)
#   --drop-source-app     after the verified copy, remove the app tables from the
#                         combined database; asks for a typed phrase on the terminal
#   --previous-evidence F evidence.json of an earlier run, passed to split-app-data.py
#   --restore-source-app DIR
#                         roll back a removal from DIR, the backup directory of the
#                         --drop-source-app run
#   --dry-run             read-only checks, then print every step; changes nothing,
#                         reads no Secret and opens no database connection
#
# Steps of a real run, each stopping the run if it fails:
#   1. Checks: the cluster and its production marker, postgresql-0 and app-postgres-0
#      running, their server major version equal to the pg_dump/pg_restore in
#      ${PG_IMAGE} (default postgres:18), Python with psycopg, no Job running.
#   2. scripts/deploy/backup-kind-databases.sh: data, app and Keycloak dumps plus the
#      Secrets, under ${ECONGRAPH_BACKUP_ROOT:-~/econgraph-backups}.
#   3. Stop the writers: deployment/econ-graph-backend and deployment/crawler-worker
#      scaled to 0, cronjob/postgres-backup suspended. They are put back as they were
#      when the script exits, also on failure, with one exception: after a successful
#      --drop-source-app or --restore-source-app the backend stays at 0, because its
#      database settings must change before it starts (the runbook says how).
#   4. Port-forward both databases on 127.0.0.1 and write a 0600 libpq service file and
#      password file from the Secrets econ-graph-postgres and econ-graph-app-postgres
#      into a private temporary directory, removed on exit. Passwords are never printed.
#   5. Run split-app-data.py with pg_dump/pg_restore from a ${PG_IMAGE} container:
#      its dry run, then the copy (a new verified backup under the backup root), then,
#      with --drop-source-app, the removal; or the rollback with --restore-source-app.
#
# The run holds database locks for the length of each step; writers are stopped first.
# Nothing under the backup root is ever deleted.
#
# Environment: PYTHON (default python3), PG_IMAGE (default postgres:18),
# ECONGRAPH_BACKUP_ROOT, and lib/kind-env.sh's ECONGRAPH_DOCKER_HOST.

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
usage() { awk 'NR > 1 && /^#/ { sub(/^# ?/, ""); print; next } NR > 1 { exit }' "$0"; }

CLUSTER="" PRODUCTION=false SOURCE_MAP="" DROP=false PREVIOUS="" RESTORE="" DRY=false
while [ $# -gt 0 ]; do
    case "$1" in
        --cluster) [ $# -ge 2 ] || { usage >&2; exit 2; }; CLUSTER="$2"; shift 2 ;;
        --production) PRODUCTION=true; shift ;;
        --source-map) [ $# -ge 2 ] || { usage >&2; exit 2; }; SOURCE_MAP="$2"; shift 2 ;;
        --drop-source-app) DROP=true; shift ;;
        --previous-evidence) [ $# -ge 2 ] || { usage >&2; exit 2; }; PREVIOUS="$2"; shift 2 ;;
        --restore-source-app) [ $# -ge 2 ] || { usage >&2; exit 2; }; RESTORE="$2"; shift 2 ;;
        --dry-run | -n) DRY=true; shift ;;
        -h | --help) usage; exit 0 ;;
        *) echo "error: unknown argument: $1" >&2; usage >&2; exit 2 ;;
    esac
done
[ -n "$CLUSTER" ] || { echo "error: --cluster NAME is required; there is no default cluster" >&2; exit 2; }
if [ -n "$RESTORE" ]; then
    [ -z "$SOURCE_MAP" ] && [ "$DROP" = false ] && [ -z "$PREVIOUS" ] ||
        { echo "error: --restore-source-app runs on its own" >&2; exit 2; }
else
    [ -n "$SOURCE_MAP" ] || { echo "error: --source-map FILE is required" >&2; exit 2; }
fi

export KIND_CLUSTER="$CLUSTER"
DRY_RUN="$DRY"
# shellcheck source=scripts/deploy/lib/kind-env.sh
source "${PROJECT_ROOT}/scripts/deploy/lib/kind-env.sh"
export ECONGRAPH_BACKUP_ROOT="${BACKUP_ROOT}"
PYTHON="${PYTHON:-python3}"
PG_IMAGE="${PG_IMAGE:-postgres:18}"
CONTEXT="kind-${KIND_CLUSTER}"
WRITER_DEPLOYMENTS=(econ-graph-backend crawler-worker)
WRITER_CRONJOBS=(postgres-backup)

# kctl ARGS: the kind cluster's own kubectl, on its own context, in the namespace.
kctl() { kubectl --context "${CONTEXT}" -n "${NAMESPACE}" "$@"; }

# --- 1. checks (read-only) -----------------------------------------------------------

log "Cluster ${KIND_CLUSTER} (context ${CONTEXT}, namespace ${NAMESPACE})"
check_docker_daemon
ensure_kind_kubectl
if kind_cluster_is_production; then
    [ "$PRODUCTION" = true ] ||
        die "${KIND_CLUSTER} carries the production marker; pass --production to convert production"
    echo "  PRODUCTION cluster"
else
    [ "$PRODUCTION" = false ] ||
        die "--production given, but ${KIND_CLUSTER} is not marked production (${CLUSTER_ROLE_REASON:-no marker})"
    echo "  not a production cluster"
fi

server_major() {
    local version
    version="$(kctl exec "$1" -c "$2" -- postgres --version)" || die "cannot read the server version in $1"
    [[ "$version" =~ ([0-9]+)\.[0-9]+ ]] || die "unexpected server version '$version' in $1"
    echo "${BASH_REMATCH[1]}"
}
for pod in postgresql-0 app-postgres-0; do
    [ "$(kctl get pod "$pod" -o jsonpath='{.status.phase}' --ignore-not-found)" = Running ] ||
        die "$pod is not running"
done
data_major="$(server_major postgresql-0 postgresql)"
app_major="$(server_major app-postgres-0 postgres)"
[ "$data_major" = "$app_major" ] || die "postgresql-0 runs PostgreSQL $data_major but app-postgres-0 runs $app_major"
check_client() {
    local client
    client="$(docker run --rm "$PG_IMAGE" pg_dump --version)" || die "cannot run pg_dump from $PG_IMAGE"
    [[ "$client" =~ ([0-9]+)\.[0-9]+ ]] || die "unexpected pg_dump version '$client' in $PG_IMAGE"
    [ "${BASH_REMATCH[1]}" = "$data_major" ] ||
        die "$PG_IMAGE has pg_dump ${BASH_REMATCH[1]}, the servers run PostgreSQL $data_major; set PG_IMAGE"
    echo "  PostgreSQL $data_major on both servers; client tools from $PG_IMAGE ($client)"
}
if docker image inspect "$PG_IMAGE" >/dev/null 2>&1; then
    check_client
elif [ "$DRY" = true ]; then
    echo "  PostgreSQL $data_major on both servers; $PG_IMAGE is not pulled yet (a real run pulls it, then checks its version)"
else
    docker pull -q "$PG_IMAGE" >/dev/null || die "cannot pull $PG_IMAGE"
    check_client
fi
"$PYTHON" -c 'import psycopg' 2>/dev/null || die "$PYTHON cannot import psycopg (pip install 'psycopg[binary]==3.3.6')"
if [ -n "$SOURCE_MAP" ]; then
    [ -r "$SOURCE_MAP" ] || die "cannot read the source map $SOURCE_MAP"
fi
if [ -n "$RESTORE" ]; then
    [ -s "$RESTORE/combined.dump" ] && [ -s "$RESTORE/evidence.json" ] ||
        die "$RESTORE is not a conversion backup directory (no combined.dump and evidence.json)"
    RESTORE="$(realpath "$RESTORE")"
    case "$RESTORE" in
        "$(realpath -m "${BACKUP_ROOT}")"/*) ;;
        *) die "$RESTORE is not under the backup root ${BACKUP_ROOT}" ;;
    esac
fi
active="$(kctl get jobs -o jsonpath='{range .items[?(@.status.active)]}{.metadata.name}{" "}{end}')" ||
    die "cannot list the Jobs in ${NAMESPACE}"
[ -z "$active" ] || die "Jobs are running (${active% }); wait for them to finish"
declare -A REPLICAS SUSPENDED
CRONJOBS_PRESENT=()
for d in "${WRITER_DEPLOYMENTS[@]}"; do
    REPLICAS[$d]="$(kctl get deployment "$d" -o jsonpath='{.spec.replicas}' --ignore-not-found)" ||
        die "cannot read deployment/$d"
    echo "  writer deployment/$d: ${REPLICAS[$d]:-not deployed} replicas"
done
for c in "${WRITER_CRONJOBS[@]}"; do
    found="$(kctl get cronjob "$c" -o name --ignore-not-found)" || die "cannot read cronjob/$c"
    [ -n "$found" ] || { echo "  writer cronjob/$c: not deployed"; continue; }
    CRONJOBS_PRESENT+=("$c")
    SUSPENDED[$c]="$(kctl get cronjob "$c" -o jsonpath='{.spec.suspend}')" || die "cannot read cronjob/$c"
    [ "${SUSPENDED[$c]}" = true ] || SUSPENDED[$c]=false
    echo "  writer cronjob/$c: suspend=${SUSPENDED[$c]}"
done

if [ -n "$RESTORE" ]; then
    ARGS=(--source-service conversion_source --restore-source-app "$RESTORE" --writers-stopped)
else
    ARGS=(--source-service conversion_source --app-service conversion_app --source-map "$SOURCE_MAP")
    [ -z "$PREVIOUS" ] || ARGS+=(--previous-evidence "$PREVIOUS")
fi

if [ "$DRY" = true ]; then
    log "Dry run: nothing changed. A real run would:"
    echo "+ scripts/deploy/backup-kind-databases.sh   (data, app and Keycloak dumps and Secrets under ${BACKUP_ROOT})"
    for d in "${WRITER_DEPLOYMENTS[@]}"; do
        [ -z "${REPLICAS[$d]}" ] || echo "+ kubectl --context ${CONTEXT} -n ${NAMESPACE} scale deployment/$d --replicas=0   (back to ${REPLICAS[$d]} on exit)"
    done
    for c in "${CRONJOBS_PRESENT[@]}"; do
        echo "+ kubectl --context ${CONTEXT} -n ${NAMESPACE} patch cronjob/$c suspend=true   (back to ${SUSPENDED[$c]} on exit)"
    done
    echo "+ kubectl port-forward pod/postgresql-0 and pod/app-postgres-0 on 127.0.0.1"
    echo "+ write a 0600 libpq service and password file from Secrets econ-graph-postgres and econ-graph-app-postgres (temporary)"
    if [ -n "$RESTORE" ]; then
        echo "+ split-app-data.py ${ARGS[*]}"
        echo "  (the backend stays at 0 afterwards: point it back at the combined database first)"
    else
        echo "+ split-app-data.py ${ARGS[*]} --dry-run   (locks both databases while it hashes them)"
        echo "+ split-app-data.py ${ARGS[*]} --writers-stopped   (new verified backup, then the copy)"
        if [ "$DROP" = true ]; then
            echo "+ split-app-data.py ${ARGS[*]} --writers-stopped --drop-source-app   (asks for the typed phrase)"
            echo "  (the backend stays at 0 afterwards: point it at the app database first)"
        fi
    fi
    exit 0
fi

# --- 2. full cluster backup ---------------------------------------------------------

log "Backing up every database and the Secrets first"
"${PROJECT_ROOT}/scripts/deploy/backup-kind-databases.sh" ||
    die "backup-kind-databases.sh failed; nothing was stopped or changed"

# --- 3. stop the writers, put them back on exit ---------------------------------------

WORK="" PF_PIDS=() KEEP_BACKEND_STOPPED=false STOPPED=false
cleanup() {
    local status=$? d c
    for pid in "${PF_PIDS[@]}"; do kill "$pid" 2>/dev/null || true; done
    [ -z "$WORK" ] || rm -rf "$WORK"
    if [ "$STOPPED" = true ]; then
        for c in "${CRONJOBS_PRESENT[@]}"; do
            kctl patch cronjob "$c" --type=merge -p "{\"spec\":{\"suspend\":${SUSPENDED[$c]}}}" >/dev/null ||
                warn "could not restore cronjob/$c suspend=${SUSPENDED[$c]}"
            echo "  cronjob/$c back to suspend=${SUSPENDED[$c]}"
        done
        for d in "${WRITER_DEPLOYMENTS[@]}"; do
            [ -n "${REPLICAS[$d]}" ] || continue
            if [ "$d" = econ-graph-backend ] && [ "$KEEP_BACKEND_STOPPED" = true ]; then
                warn "deployment/$d stays at 0 replicas (was ${REPLICAS[$d]}): change its database settings first (docs/deployment/APP_DATA_CONVERSION.md)"
                continue
            fi
            kctl scale deployment "$d" --replicas="${REPLICAS[$d]}" >/dev/null ||
                warn "could not scale deployment/$d back to ${REPLICAS[$d]}"
            echo "  deployment/$d back to ${REPLICAS[$d]} replicas"
        done
    fi
    exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT TERM

log "Stopping the writers"
STOPPED=true
for c in "${CRONJOBS_PRESENT[@]}"; do
    kctl patch cronjob "$c" --type=merge -p '{"spec":{"suspend":true}}' >/dev/null
done
for d in "${WRITER_DEPLOYMENTS[@]}"; do
    [ -n "${REPLICAS[$d]}" ] || continue
    kctl scale deployment "$d" --replicas=0 >/dev/null
done
for d in "${WRITER_DEPLOYMENTS[@]}"; do
    [ -n "${REPLICAS[$d]}" ] || continue
    kctl wait --for=delete pod -l "app=$d" --timeout=300s >/dev/null
    echo "  deployment/$d stopped"
done
active="$(kctl get jobs -o jsonpath='{range .items[?(@.status.active)]}{.metadata.name}{" "}{end}')"
[ -z "$active" ] || die "Jobs started meanwhile (${active% }); stopping"

# --- 4. port-forwards and credentials -------------------------------------------------

umask 077
WORK="$(mktemp -d "${TMPDIR:-/tmp}/econgraph-convert.XXXXXX")"
# forward POD: sets PORT to the local end of a new port-forward to POD's 5432.
forward() {
    local pod="$1" out="$WORK/pf-$1.log"
    PORT=""
    kubectl --context "${CONTEXT}" -n "${NAMESPACE}" port-forward --address 127.0.0.1 "pod/$pod" :5432 >"$out" 2>&1 &
    PF_PIDS+=("$!")
    for _ in $(seq 1 60); do
        PORT="$(sed -n 's/^Forwarding from 127\.0\.0\.1:\([0-9]*\) .*/\1/p' "$out" | head -n1)"
        [ -z "$PORT" ] || return 0
        sleep 0.5
    done
    die "port-forward to $pod did not start"
}
forward postgresql-0
DATA_PORT="$PORT"
forward app-postgres-0
APP_PORT="$PORT"
echo "  postgresql-0 on 127.0.0.1:${DATA_PORT}, app-postgres-0 on 127.0.0.1:${APP_PORT}"

# pgpass escaping: backslash and colon.
pgpass_line() {
    local password
    password="$(kctl get secret "$4" -o jsonpath='{.data.postgres-password}' | base64 -d)" ||
        die "cannot read the password from Secret $4"
    [ -n "$password" ] || die "Secret $4 has an empty postgres-password"
    password="${password//\\/\\\\}"
    printf '127.0.0.1:%s:%s:%s:%s\n' "$1" "$2" "$3" "${password//:/\\:}"
}
(
    {
        pgpass_line "$DATA_PORT" econ_graph postgres econ-graph-postgres
        pgpass_line "$APP_PORT" econ_graph_app postgres econ-graph-app-postgres
    } >"$WORK/pgpass"
    printf '[conversion_source]\nhost=127.0.0.1\nport=%s\ndbname=econ_graph\nuser=postgres\n\n[conversion_app]\nhost=127.0.0.1\nport=%s\ndbname=econ_graph_app\nuser=postgres\n' \
        "$DATA_PORT" "$APP_PORT" >"$WORK/services.conf"
)
export PGSERVICEFILE="$WORK/services.conf" PGPASSFILE="$WORK/pgpass"

# pg_dump and pg_restore of the servers' major version, from ${PG_IMAGE}, on the host network.
mkdir -p "$WORK/bin" "${BACKUP_ROOT}"
for tool in pg_dump pg_restore; do
    cat >"$WORK/bin/$tool" <<EOF
#!/bin/sh
exec docker run --rm -i --network=host --user "$(id -u):$(id -g)" \\
    -e PGSERVICEFILE -e PGPASSFILE -v "$WORK:$WORK:ro" -v "${BACKUP_ROOT}:${BACKUP_ROOT}:ro" \\
    "$PG_IMAGE" $tool "\$@"
EOF
    chmod 0700 "$WORK/bin/$tool"
done
export PATH="$WORK/bin:$PATH"

# --- 5. the conversion ----------------------------------------------------------------

SPLIT=("$PYTHON" "${PROJECT_ROOT}/scripts/deploy/split-app-data.py")
if [ -n "$RESTORE" ]; then
    log "Restoring the app tables into the combined database from $RESTORE"
    "${SPLIT[@]}" "${ARGS[@]}"
    KEEP_BACKEND_STOPPED=true
    log "Done. Point the backend back at the combined database (APP_DATABASE=shared scripts/deploy/create-secrets.sh), then start it."
    exit 0
fi
log "Conversion dry run (locks both databases while it hashes them)"
"${SPLIT[@]}" "${ARGS[@]}" --dry-run
log "Verified backup and copy"
"${SPLIT[@]}" "${ARGS[@]}" --writers-stopped
if [ "$DROP" = true ]; then
    log "Removing the app tables from the combined database (typed confirmation)"
    "${SPLIT[@]}" "${ARGS[@]}" --writers-stopped --drop-source-app
    KEEP_BACKEND_STOPPED=true
    log "Done. Point the backend at the app database (APP_DATABASE=separate scripts/deploy/create-secrets.sh), then start it."
fi
