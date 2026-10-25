# shellcheck shell=bash
#
# Shared by scripts/deploy/backup-keycloak-db.sh and restore-keycloak-db.sh:
# Keycloak's own database (`keycloak` in the keycloak-postgres StatefulSet, see
# k8s/manifests/keycloak/postgres.yaml), reached with kubectl exec.
#
# Source it; don't run it. Every kubectl call goes through kc from
# lib/cluster-role.sh, on the context kdb_use_context pins, without changing
# the current context of ${KUBECTL:-kubectl}.

# The KDB_ settings are read by the scripts that source this.
# shellcheck disable=SC2034

KDB_NAMESPACE="econ-graph"
KDB_POD="keycloak-postgres-0"
KDB_USER="keycloak"
KDB_DATABASE="keycloak"
KDB_DEPLOYMENT="keycloak"
# Where the dumps go: one directory per dump, named by its UTC time. Nothing here
# is ever deleted by these scripts.
KDB_BACKUP_ROOT="${KEYCLOAK_BACKUP_ROOT:-${HOME}/econgraph-backups/keycloak}"

# Exact row count of every table in the public schema of database $1, one
# "table<TAB>count" line per table, sorted by table name.
KDB_COUNT_SQL="select table_name, (xpath('/row/c/text()', query_to_xml(format('select count(*) as c from %I.%I', table_schema, table_name), false, true, '')))[1]::text from information_schema.tables where table_schema = 'public' and table_type = 'BASE TABLE' order by table_name"

# Pin every kc call to context $1, which must exist. Unlike
# cluster_role_use_context, the current context stays as it is.
kdb_use_context() {
    if ! command "${KUBECTL:-kubectl}" config get-contexts -o name | grep -qxF -- "$1"; then
        echo "${KDB_SCRIPT}: error: ${KUBECTL:-kubectl} has no context $1" >&2
        return 1
    fi
    CLUSTER_ROLE_CONTEXT="$1"
    export CLUSTER_ROLE_CONTEXT
}

# kubectl exec in the Keycloak Postgres pod; stdin is passed through.
kdb_exec() {
    kc -n "$KDB_NAMESPACE" exec -i "$KDB_POD" -- "$@"
}

# Fails unless the Keycloak Postgres pod is ready.
kdb_check_pod() {
    local ready
    ready="$(kc -n "$KDB_NAMESPACE" get pod "$KDB_POD" \
        -o 'jsonpath={.status.containerStatuses[0].ready}')" || return 1
    if [ "$ready" != true ]; then
        echo "${KDB_SCRIPT}: error: pod ${KDB_NAMESPACE}/${KDB_POD} is not ready" >&2
        return 1
    fi
}

# Print the Keycloak Deployment's replicas, or "none" when there is no Deployment.
kdb_keycloak_replicas() {
    local out
    out="$(kc -n "$KDB_NAMESPACE" get deployment "$KDB_DEPLOYMENT" --ignore-not-found \
        -o 'jsonpath={.spec.replicas}')" || return 1
    printf '%s\n' "${out:-none}"
}

# Print the image of the Keycloak Deployment's first container ("none" without one).
kdb_keycloak_image() {
    local out
    out="$(kc -n "$KDB_NAMESPACE" get deployment "$KDB_DEPLOYMENT" --ignore-not-found \
        -o 'jsonpath={.spec.template.spec.containers[0].image}')" || return 1
    printf '%s\n' "${out:-none}"
}

# Scale Keycloak to 0 and wait until its pods are gone, so nothing writes to the
# database while it is dumped or restored. A no-op when it is already at 0.
kdb_stop_keycloak() {
    local replicas pods
    replicas="$(kdb_keycloak_replicas)" || return 1
    if [ "$replicas" != none ] && [ "$replicas" != 0 ]; then
        echo "${KDB_SCRIPT}: scaling deployment/${KDB_DEPLOYMENT} from ${replicas} to 0"
        kc -n "$KDB_NAMESPACE" scale "deployment/${KDB_DEPLOYMENT}" --replicas=0 || return 1
    fi
    pods="$(kc -n "$KDB_NAMESPACE" get pod -l "app=${KDB_DEPLOYMENT}" -o name)" || return 1
    if [ -n "$pods" ]; then
        echo "${KDB_SCRIPT}: waiting for the Keycloak pods to stop"
        kc -n "$KDB_NAMESPACE" wait --for=delete pod -l "app=${KDB_DEPLOYMENT}" --timeout=300s || return 1
    fi
}

# Row counts of database $1 (see KDB_COUNT_SQL).
kdb_counts() {
    kdb_exec psql -U "$KDB_USER" -d "$1" -v ON_ERROR_STOP=1 -X -A -t -F $'\t' -c "$KDB_COUNT_SQL"
}

# Restore dump file $2 into the existing, empty database $1 and check that its
# row counts equal the counts file $3. Prints the difference on a mismatch.
kdb_restore_and_compare() {
    local db="$1" dump="$2" counts="$3" got
    kdb_exec pg_restore -U "$KDB_USER" -d "$db" --no-owner --exit-on-error <"$dump" || return 1
    got="$(kdb_counts "$db")" || return 1
    if [ "$got" != "$(cat "$counts")" ]; then
        echo "${KDB_SCRIPT}: error: row counts in ${db} differ from ${counts}:" >&2
        diff <(cat "$counts") <(printf '%s\n' "$got") >&2 || true
        return 1
    fi
}
