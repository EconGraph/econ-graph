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
#      scaled to 0, cronjob/postgres-backup suspended. On exit, also after a failure or a
#      signal, the crawler and the CronJob are put back as they were. The backend is put
#      back only if econ_graph still holds the app tables and no removal or rollback
#      succeeded: a backend on the old setting would otherwise serve the wrong database or
#      create new, empty app tables. The script then prints the deploy that starts it.
#   4. Port-forward both databases on 127.0.0.1 and write a 0600 libpq service file and
#      password file from the Secrets econ-graph-postgres and econ-graph-app-postgres
#      into a private temporary directory, removed on exit. Passwords are never printed.
#   5. Run split-app-data.py with pg_dump/pg_restore from a ${PG_IMAGE} container:
#      its dry run, then the copy (a new verified backup under the backup root; an empty
#      app database first gets the app schema as the backend's role econgraph, in the
#      same transaction), then, with --drop-source-app, the removal; or the rollback
#      with --restore-source-app. After a removal commits, app-database-url is rewritten to
#      the app database (APP_DATABASE=separate create-secrets.sh --app-database-only), and
#      back to the data database after a rollback, so the Secret records
#      where the app tables are and a plain deploy starts the backend on the right one.
#
# The run holds database locks for the length of each step; writers are stopped first.
# Nothing under the backup root is ever deleted. Run it inside tmux or screen: a dropped
# connection is handled, but the typed confirmation needs the terminal.
#
# Environment: PYTHON (default python3), PG_IMAGE (default postgres:18), DB_CHECK_TIMEOUT
# (seconds for the exit-time check of econ_graph, default 60; unknown keeps the backend at 0),
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
# Resolved once: the same absolute path for the rollback check, the children and the
# container mount (a relative or symlinked root would differ between them).
BACKUP_ROOT="$(realpath -m "${BACKUP_ROOT}")"
export ECONGRAPH_BACKUP_ROOT="${BACKUP_ROOT}"
PYTHON="${PYTHON:-python3}"
PG_IMAGE="${PG_IMAGE:-postgres:18}"
DB_CHECK_TIMEOUT="${DB_CHECK_TIMEOUT:-60}"
CONTEXT="kind-${KIND_CLUSTER}"
WRITER_DEPLOYMENTS=(econ-graph-backend crawler-worker)
WRITER_CRONJOBS=(postgres-backup)

# kctl ARGS: the kind cluster's own kubectl, on its own context, in the namespace.
kctl() { kubectl --context "${CONTEXT}" -n "${NAMESPACE}" "$@"; }

# --- 1. checks (read-only) -----------------------------------------------------------

log "Cluster ${KIND_CLUSTER} (context ${CONTEXT}, namespace ${NAMESPACE})"
check_docker_daemon
ensure_kind_kubectl
echo "  kubectl: $(command -v kubectl) --context ${CONTEXT}"
# 0 production, 1 not production, 2 unknown (could not be asked, or conflicting
# markers): unknown stops here, whatever --production says.
role_status=0
kind_cluster_is_production || role_status=$?
case "$role_status" in
    0)
        [ "$PRODUCTION" = true ] ||
            die "${KIND_CLUSTER} carries the production marker; pass --production to convert production"
        echo "  PRODUCTION cluster"
        ;;
    1)
        [ "$PRODUCTION" = false ] ||
            die "--production given, but ${KIND_CLUSTER} is not marked production (${CLUSTER_ROLE_REASON:-no marker})"
        echo "  not a production cluster"
        ;;
    *)
        die "production status unknown for ${KIND_CLUSTER}: ${CLUSTER_ROLE_REASON:-the check failed}; nothing was changed"
        ;;
esac

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
        "${BACKUP_ROOT}"/*) ;;
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
    # econgraph: the role the backend applies the app migrations as (app-postgres-init.yaml).
    ARGS=(--source-service conversion_source --app-service conversion_app --source-map "$SOURCE_MAP"
        --init-app-schema econgraph)
    [ -z "$PREVIOUS" ] || ARGS+=(--previous-evidence "$PREVIOUS")
fi

if [ "$DRY" = true ]; then
    log "Dry run: nothing changed. A real run would:"
    echo "+ scripts/deploy/backup-kind-databases.sh   (data, app and Keycloak dumps and Secrets under ${BACKUP_ROOT})"
    for d in "${WRITER_DEPLOYMENTS[@]}"; do
        [ -z "${REPLICAS[$d]}" ] || echo "+ kubectl --context ${CONTEXT} -n ${NAMESPACE} scale deployment/$d --replicas=0   (back to ${REPLICAS[$d]} on exit; see below for the backend)"
    done
    for c in "${CRONJOBS_PRESENT[@]}"; do
        echo "+ kubectl --context ${CONTEXT} -n ${NAMESPACE} patch cronjob/$c suspend=true   (back to ${SUSPENDED[$c]} on exit)"
    done
    echo "+ kubectl port-forward pod/postgresql-0 and pod/app-postgres-0 on 127.0.0.1"
    echo "+ write a 0600 libpq service and password file from Secrets econ-graph-postgres and econ-graph-app-postgres (temporary)"
    echo "  On exit the backend is started again only if econ_graph still holds the app tables and"
    echo "  no removal or rollback succeeded; otherwise it stays at 0 until its database setting changes."
    if [ -n "$RESTORE" ]; then
        echo "+ split-app-data.py ${ARGS[*]}"
        echo "+ APP_DATABASE=shared scripts/deploy/create-secrets.sh --app-database-only --context=${CONTEXT}   (app-database-url back to econ_graph)"
        echo "  (the backend stays at 0 afterwards; deploy.sh starts it)"
    else
        echo "+ split-app-data.py ${ARGS[*]} --dry-run   (locks both databases while it hashes them)"
        echo "+ split-app-data.py ${ARGS[*]} --writers-stopped   (new verified backup, then the copy)"
        if [ "$DROP" = true ]; then
            echo "+ split-app-data.py ${ARGS[*]} --writers-stopped --drop-source-app   (asks for the typed phrase)"
            echo "+ APP_DATABASE=separate scripts/deploy/create-secrets.sh --app-database-only --context=${CONTEXT}   (app-database-url to econ_graph_app)"
            echo "  (the backend stays at 0 afterwards; deploy.sh starts it)"
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

# Whether econ_graph still holds the app tables. Anything but a clear yes counts as no, so a
# backend is never started on the combined database once its app tables may be gone (it
# would create new, empty ones).
# Bounded with SIGKILL: cleanup ignores TERM, and its children inherit that, so a hanging
# exec must be killed outright.
source_has_app_tables() {
    [ "$(timeout -s KILL "${DB_CHECK_TIMEOUT}" kubectl --context "${CONTEXT}" -n "${NAMESPACE}" exec postgresql-0 \
        -c postgresql -- psql -U postgres -d econ_graph -Atc \
        "select to_regclass('public.users') is not null" 2>/dev/null)" = t ]
}

# Put the writers back. Nothing here may stop it: errors are ignored, signals and a closed
# terminal or pipe too, and every change is made before anything is printed.
cleanup() {
    local status=$? d c restart_backend=true why="" pid
    set +e
    trap '' PIPE HUP INT TERM
    for pid in "${PF_PIDS[@]}"; do kill "$pid" 2>/dev/null; done
    [ -z "$WORK" ] || rm -rf "$WORK"
    [ "$STOPPED" = true ] || exit "$status"
    declare -A result
    # The CronJob and the crawler first: they do not depend on where the app tables are.
    for c in "${CRONJOBS_PRESENT[@]}"; do
        kctl patch cronjob "$c" --type=merge -p "{\"spec\":{\"suspend\":${SUSPENDED[$c]}}}" \
            --request-timeout=30s >/dev/null 2>&1
        result[cronjob/$c]=$?
    done
    for d in "${WRITER_DEPLOYMENTS[@]}"; do
        [ -n "${REPLICAS[$d]}" ] && [ "$d" != econ-graph-backend ] || continue
        kctl scale deployment "$d" --replicas="${REPLICAS[$d]}" --request-timeout=30s >/dev/null 2>&1
        result[deployment/$d]=$?
    done
    # The backend only when econ_graph certainly still holds the app tables.
    if [ "$KEEP_BACKEND_STOPPED" = true ]; then
        restart_backend=false why="$NEXT_STEP"
    elif ! source_has_app_tables; then
        restart_backend=false
        why="econ_graph no longer holds the app tables, or that could not be checked within ${DB_CHECK_TIMEOUT}s. Check before starting the backend on it: if the tables are gone, roll back with --restore-source-app <the removal's backup directory>, or start it on the app database with KIND_CLUSTER=${KIND_CLUSTER} APP_DATABASE=separate scripts/deploy/deploy.sh (docs/deployment/APP_DATA_CONVERSION.md)"
    fi
    if [ -n "${REPLICAS[econ-graph-backend]}" ] && [ "$restart_backend" = true ]; then
        kctl scale deployment econ-graph-backend --replicas="${REPLICAS[econ-graph-backend]}" \
            --request-timeout=30s >/dev/null 2>&1
        result[deployment/econ-graph-backend]=$?
    fi
    for c in "${CRONJOBS_PRESENT[@]}"; do
        if [ "${result[cronjob/$c]}" = 0 ]; then
            echo "  cronjob/$c back to suspend=${SUSPENDED[$c]}" || true
        else
            warn "could not restore cronjob/$c suspend=${SUSPENDED[$c]}; patch it by hand" || true
        fi
    done
    for d in "${WRITER_DEPLOYMENTS[@]}"; do
        [ -n "${REPLICAS[$d]}" ] || continue
        if [ -z "${result[deployment/$d]+set}" ]; then
            warn "deployment/$d stays at 0 replicas (was ${REPLICAS[$d]}): $why" || true
        elif [ "${result[deployment/$d]}" = 0 ]; then
            echo "  deployment/$d back to ${REPLICAS[$d]} replicas" || true
        else
            warn "could not scale deployment/$d back to ${REPLICAS[$d]}; scale it by hand" || true
        fi
    done
    exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT TERM HUP
trap 'exit 141' PIPE

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
    for _ in $(seq 1 150); do
        pods="$(kctl get pods -l "app=$d" -o name)" || die "cannot list the pods of deployment/$d"
        [ -n "$pods" ] || break
        sleep 2
    done
    [ -z "$pods" ] || die "deployment/$d still has pods after 300s"
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
# The command to run by hand if the switch fails: the same narrow mode, same kubectl and context.
switch_command() {
    echo "APP_DATABASE=$1 KUBECTL=$(command -v kubectl) KUBECONFIG=${KUBECONFIG} scripts/deploy/create-secrets.sh --app-database-only --context=${CONTEXT}"
}

# switch_app_database_url separate|shared: rewrite only app-database-url, with
# create-secrets.sh --app-database-only, whose guard checks the direction against the
# databases, so the Secret records where the app tables are. The URL is built from the
# passwords already in the cluster; the environment is cut down to the basics anyway (as
# set-crawler-keys.sh does), and no value is printed.
switch_app_database_url() {
    (
        keep=" PATH HOME USER LOGNAME LANG LC_ALL TERM TMPDIR KUBECONFIG "
        while IFS= read -r name; do
            case "$keep" in
                *" $name "*) ;;
                *) [[ "$name" == LC_* ]] || unset "$name" 2>/dev/null || true ;;
            esac
        done < <(compgen -e)
        KUBECTL="$(command -v kubectl)"
        export APP_DATABASE="$1" NAMESPACE KUBECTL
        exec "${PROJECT_ROOT}/scripts/deploy/create-secrets.sh" --app-database-only --context="${CONTEXT}"
    )
}
NEXT_STEP=""
PUBLIC_HINT=""
[ "$PRODUCTION" = false ] || PUBLIC_HINT="PUBLIC_HOST=<the public host> "
if [ -n "$RESTORE" ]; then
    log "Restoring the app tables into the combined database from $RESTORE"
    "${SPLIT[@]}" "${ARGS[@]}"
    KEEP_BACKEND_STOPPED=true
    NEXT_STEP="the app tables are back in econ_graph, but app-database-url could not be pointed at it; run $(switch_command shared), then start the backend with KIND_CLUSTER=${KIND_CLUSTER} ${PUBLIC_HINT}scripts/deploy/deploy.sh"
    log "Pointing app-database-url back at econ_graph"
    switch_app_database_url shared
    NEXT_STEP="the app tables are back in econ_graph and app-database-url names it; start the backend with KIND_CLUSTER=${KIND_CLUSTER} ${PUBLIC_HINT}scripts/deploy/deploy.sh"
    log "Done." || true
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
    NEXT_STEP="the app tables are now only in econ_graph_app, but app-database-url still names econ_graph and could not be changed; run $(switch_command separate) before anything starts the backend (create-secrets.sh refuses shared from now on)"
    log "Pointing app-database-url at the app database"
    switch_app_database_url separate
    NEXT_STEP="the app tables are now only in econ_graph_app and app-database-url names it; start the backend with KIND_CLUSTER=${KIND_CLUSTER} ${PUBLIC_HINT}scripts/deploy/deploy.sh (APP_DATABASE unset or separate)"
    log "Done." || true
fi
