# shellcheck shell=bash
#
# Shared by scripts/deploy/backup-keycloak-db.sh and restore-keycloak-db.sh:
# Keycloak's own database (`keycloak` in the keycloak-postgres StatefulSet, see
# k8s/manifests/keycloak/postgres.yaml), reached with kubectl exec.
#
# Source it; don't run it. It sources lib/kind-env.sh, which sources
# lib/cluster-role.sh. Every kubectl call goes through kc from
# lib/cluster-role.sh, on the context kdb_use_context pins, without changing
# the current context of ${KUBECTL:-kubectl}. Unless KUBECTL is set, kubectl is
# the kind cluster's own binary and kubeconfig (ensure_kind_kubectl in
# lib/kind-env.sh): on this host plain `kubectl` is the microk8s snap wrapper.

# The KDB_ settings are read by the scripts that source this.
# shellcheck disable=SC2034

# shellcheck source=scripts/deploy/lib/kind-env.sh
source "$(dirname "${BASH_SOURCE[0]}")/kind-env.sh"

KDB_NAMESPACE="${NAMESPACE}"
KDB_POD="keycloak-postgres-0"
# The pod also has the init container init-data-dir; exec in this one.
KDB_CONTAINER="postgres"
KDB_USER="keycloak"
KDB_DATABASE="keycloak"
KDB_DEPLOYMENT="keycloak"
# The context deploy.sh deploys to.
KDB_DEFAULT_CONTEXT="kind-${KIND_CLUSTER}"
# Where the dumps go: one directory per dump, named by its UTC time, under the
# backup root of lib/kind-env.sh (ECONGRAPH_BACKUP_ROOT). Nothing here is ever
# deleted by these scripts.
KDB_BACKUP_ROOT="${KEYCLOAK_BACKUP_ROOT:-${BACKUP_ROOT}/keycloak}"

# Exact row count of every table in the public schema of database $1, one
# "table<TAB>count" line per table, sorted by table name.
KDB_COUNT_SQL="select table_name, (xpath('/row/c/text()', query_to_xml(format('select count(*) as c from %I.%I', table_schema, table_name), false, true, '')))[1]::text from information_schema.tables where table_schema = 'public' and table_type = 'BASE TABLE' order by table_name"

# Pin every kc call to context $1, which must exist. Without KUBECTL, first put
# the kind cluster's kubectl and kubeconfig on PATH and KUBECONFIG
# (ensure_kind_kubectl; under DRY_RUN=true nothing is cached), on the Docker
# daemon kind-env.sh chose, which it names. Unlike cluster_role_use_context,
# the current context stays as it is.
kdb_use_context() {
    local names
    if [ -z "${KUBECTL:-}" ]; then
        check_docker_daemon
        ensure_kind_kubectl_quietly || {
            echo "${KDB_SCRIPT}: error: cannot reach the API server of kind cluster '${KIND_CLUSTER}' on ${DOCKER_HOST}" >&2
            return 1
        }
    fi
    # Captured first: grep -q exiting early could SIGPIPE kubectl and fail the pipeline.
    names="$(command "${KUBECTL:-kubectl}" config get-contexts -o name)" || return 1
    if ! grep -qxF -- "$1" <<<"$names"; then
        echo "${KDB_SCRIPT}: error: ${KUBECTL:-kubectl} has no context $1" >&2
        return 1
    fi
    CLUSTER_ROLE_CONTEXT="$1"
    export CLUSTER_ROLE_CONTEXT
}

# kubectl exec in the Keycloak Postgres container; stdin is passed through.
kdb_exec() {
    kc -n "$KDB_NAMESPACE" exec -i "$KDB_POD" -c "$KDB_CONTAINER" -- "$@"
}

# Fails unless the Keycloak Postgres container is ready.
kdb_check_pod() {
    local ready
    ready="$(kc -n "$KDB_NAMESPACE" get pod "$KDB_POD" \
        -o "jsonpath={.status.containerStatuses[?(@.name==\"${KDB_CONTAINER}\")].ready}")" || return 1
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

# Restore dump file $2 into the existing, empty database $1 in one transaction,
# stopping at the first error, and check that its row counts equal the counts
# file $3. Prints the difference on a mismatch.
kdb_restore_and_compare() {
    local db="$1" dump="$2" counts="$3" got
    kdb_exec pg_restore -U "$KDB_USER" -d "$db" --no-owner --exit-on-error --single-transaction \
        <"$dump" || return 1
    got="$(kdb_counts "$db")" || return 1
    if [ "$got" != "$(cat "$counts")" ]; then
        echo "${KDB_SCRIPT}: error: row counts in ${db} differ from ${counts}:" >&2
        diff <(cat "$counts") <(printf '%s\n' "$got") >&2 || true
        return 1
    fi
}

# Drop scratch database $1. Refuses any name but keycloak_verify_* (backup) and
# keycloak_restore_* (restore): these scripts never drop `keycloak` itself.
kdb_drop_scratch() {
    local db="$1"
    if [[ "$db" != keycloak_verify_* && "$db" != keycloak_restore_* ]]; then
        echo "${KDB_SCRIPT}: error: refusing to drop ${db}: not a scratch database" >&2
        return 1
    fi
    kdb_exec dropdb -U "$KDB_USER" --if-exists "$db"
}

# The cluster's identity: the UID of its kube-system namespace (as in
# lib/cluster-role.sh), new for every cluster.
kdb_cluster_uid() {
    kube_system_uid
}
