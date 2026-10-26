#!/usr/bin/env bash
#
# Check that the database is quiet enough for a deploy whose migrations lock
# tables, before running scripts/deploy/deploy.sh.
#
# Usage:
#   scripts/deploy/precheck-db-migration.sh [--dry-run] [--stop-crawler]
#
# Run it immediately before scripts/deploy/deploy.sh, on the cluster deploy.sh
# deploys to: context kind-econ-graph, always.
#
#   --dry-run       check, and show what --stop-crawler would change, but change
#                   nothing
#   --stop-crawler  scale deployment/crawler-worker to 0 and wait for its pods
#                   to go. deploy.sh applies crawler-worker.yaml, bringing it
#                   back at 1 replica on the new image, only after the new
#                   backend (which applies the migrations) has rolled out. If a
#                   later check fails, the command to scale it back is printed.
#
# Checks, in order; any failure exits non-zero:
#   1. No postgres-backup Job is running (the nightly CronJob runs at 08:00
#      UTC; a migration waiting behind the backup's long transaction would time
#      out, and one taking an ACCESS EXCLUSIVE lock would stall the backup).
#   2. With --stop-crawler, the crawler worker is stopped.
#   3. No session on the data database has been in its transaction for more
#      than 5 seconds or is idle in a transaction: such a session holds locks
#      the migration waits for. Each one is listed with how to find its owner.
#      Sessions are never cancelled or terminated by this script.
#
# Idempotent: it reads the cluster, and --stop-crawler only scales down a
# worker that is running. It deletes nothing.
#
# Environment: KUBECTL (default kubectl) and its KUBECONFIG. With neither set,
# it uses the kind cluster's own kubectl and kubeconfig when copies are kept in
# ~/.cache/econgraph (bin/kubectl, kubeconfig-econ-graph), and says so.
# NAMESPACE (default econ-graph), POSTGRES_POD (default postgresql-0),
# DATA_DB_NAME (default econ_graph), DB_USER (default postgres).

set -euo pipefail

SCRIPT="precheck-db-migration"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/deploy/lib/cluster-role.sh
source "$PROJECT_ROOT/scripts/deploy/lib/cluster-role.sh"

NAMESPACE="${NAMESPACE:-econ-graph}"
POSTGRES_POD="${POSTGRES_POD:-postgresql-0}"
DATA_DB_NAME="${DATA_DB_NAME:-econ_graph}"
DB_USER="${DB_USER:-postgres}"
CRAWLER="crawler-worker"
BACKUP_PREFIX="postgres-backup"
# A transaction older than this blocks the migration's locks long enough to matter.
MAX_XACT_SECONDS=5

DRY_RUN=false
STOP_CRAWLER=false
# The context deploy.sh deploys to.
CONTEXT="kind-econ-graph"
while [ $# -gt 0 ]; do
    case "$1" in
        --dry-run) DRY_RUN=true; shift ;;
        --stop-crawler) STOP_CRAWLER=true; shift ;;
        -h | --help) awk 'NR > 1 && /^#/ { sub(/^# ?/, ""); print; next } NR > 1 { exit }' "$0"; exit 0 ;;
        *) echo "${SCRIPT}: error: unknown argument: $1" >&2; exit 2 ;;
    esac
done

die() {
    echo "${SCRIPT}: error: $*" >&2
    exit 1
}

log() {
    echo "${SCRIPT}: $*"
}

# One kubectl and one context for every call (kc), without changing the current
# context: the kind cluster's kubectl when no KUBECTL/KUBECONFIG is set.
unset CLUSTER_ROLE_CONTEXT
cluster_role_default_kubectl
KUBECTL="${KUBECTL:-kubectl}"
command -v "$KUBECTL" >/dev/null 2>&1 || die "$KUBECTL not found"
contexts="$(command "$KUBECTL" config get-contexts -o name)" || die "could not list the contexts of $KUBECTL"
grep -qxF -- "$CONTEXT" <<<"$contexts" || die "$KUBECTL has no context $CONTEXT"
CLUSTER_ROLE_CONTEXT="$CONTEXT"
export CLUSTER_ROLE_CONTEXT
role_status=0
cluster_is_marked_production || role_status=$?
case "$role_status" in
    0) cluster_role="PRODUCTION" ;;
    1) cluster_role="not marked production" ;;
    *) cluster_role="unknown: ${CLUSTER_ROLE_REASON}" ;;
esac
log "context ${CONTEXT} (${cluster_role}), kubectl ${KUBECTL}, KUBECONFIG=${KUBECONFIG:-default}"

failed=false

# Once this run has scaled the crawler down, a failed check says how to bring it back.
SCALED_FROM=""
crawler_restore_hint() {
    local status=$?
    if [ "$status" != 0 ] && [ -n "$SCALED_FROM" ]; then
        echo "${SCRIPT}: crawler-worker was scaled from ${SCALED_FROM} to 0 by this run and stays" \
            "stopped. To bring it back without deploying:" >&2
        echo "  ${KUBECONFIG:+KUBECONFIG=${KUBECONFIG} }${KUBECTL} --context ${CONTEXT} -n ${NAMESPACE}" \
            "scale deployment/${CRAWLER} --replicas=${SCALED_FROM}" >&2
    fi
    return "$status"
}
trap crawler_restore_hint EXIT

# The crawler pods, one name per line.
crawler_pods() {
    kc -n "$NAMESPACE" get pods -l "app=${CRAWLER}" -o name
}

# 1. No backup running.
jobs="$(kc -n "$NAMESPACE" get jobs \
    -o 'jsonpath={range .items[*]}{.metadata.name}{" "}{.status.active}{"\n"}{end}')" ||
    die "could not list the Jobs in ${NAMESPACE}"
active_backups="$(awk -v p="$BACKUP_PREFIX" 'index($1, p) == 1 && $2 + 0 > 0 { print $1 }' <<<"$jobs")"
if [ -n "$active_backups" ]; then
    echo "${SCRIPT}: BLOCKED: a postgres backup is running: $(tr '\n' ' ' <<<"$active_backups")" >&2
    echo "  Wait for it to finish (kubectl -n ${NAMESPACE} get jobs), then run this again." >&2
    echo "  The nightly backup starts at 08:00 UTC; deploy outside that time." >&2
    exit 1
fi
log "ok: no postgres-backup Job is running"

# 2. Crawler worker.
if [ "$STOP_CRAWLER" = true ]; then
    replicas="$(kc -n "$NAMESPACE" get deployment "$CRAWLER" --ignore-not-found \
        -o 'jsonpath={.spec.replicas}')" || die "could not read deployment/${CRAWLER}"
    pods="$(crawler_pods)" || die "could not list the ${CRAWLER} pods"
    if [ -z "$replicas" ]; then
        log "ok: there is no deployment/${CRAWLER}"
    elif [ "$DRY_RUN" = true ]; then
        if [ "$replicas" != 0 ] || [ -n "$pods" ]; then
            log "dry run: would scale deployment/${CRAWLER} in ${NAMESPACE} from ${replicas} to 0" \
                "and wait for its pods to go (now: $(wc -w <<<"$pods") pod(s))"
        else
            log "dry run: deployment/${CRAWLER} is already at 0 with no pods; nothing to change"
        fi
    else
        if [ "$replicas" != 0 ]; then
            log "scaling deployment/${CRAWLER} from ${replicas} to 0"
            kc -n "$NAMESPACE" scale "deployment/${CRAWLER}" --replicas=0 ||
                die "could not scale deployment/${CRAWLER} to 0"
            SCALED_FROM="$replicas"
        fi
        # `wait --for=delete` fails when no pod matches, so only wait for pods still there,
        # and count a failed wait as success when they are gone by then.
        pods="$(crawler_pods)" || die "could not list the ${CRAWLER} pods"
        if [ -n "$pods" ] &&
            ! kc -n "$NAMESPACE" wait --for=delete pod -l "app=${CRAWLER}" --timeout=300s; then
            pods="$(crawler_pods)" || die "could not list the ${CRAWLER} pods"
            [ -z "$pods" ] || die "the ${CRAWLER} pods did not go within 5 minutes: ${pods}"
        fi
        log "ok: ${CRAWLER} is stopped; deploy.sh brings it back at 1 replica on the new image"
    fi
fi

# 3. Sessions that hold locks the migration would wait for. Read-only.
read -r -d '' sessions_sql <<SQL || true
SELECT pid, coalesce(usename, '-'), coalesce(nullif(application_name, ''), '-'),
       coalesce(host(client_addr), 'local'), coalesce(state, '-'),
       extract(epoch FROM now() - xact_start)::int, coalesce(wait_event_type, '-'),
       left(regexp_replace(coalesce(query, ''), '\\s+', ' ', 'g'), 100)
FROM pg_stat_activity
WHERE datname = current_database() AND pid <> pg_backend_pid()
  AND xact_start IS NOT NULL
  AND (state LIKE 'idle in transaction%'
       OR now() - xact_start > interval '${MAX_XACT_SECONDS} seconds')
ORDER BY xact_start
SQL
sessions="$(kc -n "$NAMESPACE" exec -i "$POSTGRES_POD" -- \
    psql -U "$DB_USER" -d "$DATA_DB_NAME" -v ON_ERROR_STOP=1 -X -A -t -F $'\t' -c "$sessions_sql")" ||
    die "could not read pg_stat_activity of ${DATA_DB_NAME} in ${NAMESPACE}/${POSTGRES_POD}"
if [ -n "$sessions" ]; then
    failed=true
    echo "${SCRIPT}: BLOCKED: sessions on ${DATA_DB_NAME} in a transaction for more than" \
        "${MAX_XACT_SECONDS}s or idle in a transaction:" >&2
    printf '  %-8s %-10s %-22s %-12s %-28s %8s %-8s %s\n' \
        PID USER APPLICATION CLIENT STATE "XACT_S" WAIT QUERY >&2
    while IFS=$'\t' read -r pid user app client state age wait query; do
        printf '  %-8s %-10s %-22s %-12s %-28s %8s %-8s %s\n' \
            "$pid" "$user" "$app" "$client" "$state" "$age" "$wait" "$query" >&2
    done <<<"$sessions"
    cat >&2 <<EOF
  The migration would wait behind these and give up after 10 seconds. Find each
  owner by its application and client address (kubectl -n ${NAMESPACE} get pods -o wide
  maps a pod IP to its pod) and stop or finish that work, then run this again.
  This script never ends sessions. If a session is certainly abandoned, end it
  yourself with: SELECT pg_terminate_backend(<pid>);
EOF
else
    log "ok: no session on ${DATA_DB_NAME} has been in a transaction for more than ${MAX_XACT_SECONDS}s or is idle in one"
fi

if [ "$failed" = true ]; then
    exit 1
fi
if [ "$DRY_RUN" = true ]; then
    log "dry run: checks passed, nothing changed"
else
    log "checks passed: deploy now (scripts/deploy/deploy.sh)"
fi
