# shellcheck shell=bash
#
# Which cluster is production (ECO-468). Only production may run the Cloudflare
# Tunnel connector or hold its token: the tunnel is token-authenticated, and
# every cloudflared that runs with the token becomes one more connector of the
# same tunnel, so Cloudflare silently sends part of the public site's traffic to
# it. A test cluster that got the token would serve real visitors.
#
# A cluster is production only while it carries the marker that
# scripts/deploy/mark-production-cluster.sh writes: the ConfigMap
# kube-system/econgraph-cluster-role with role=production and the UID of the
# cluster's kube-system namespace at marking time. The UID is new for every
# cluster, so a marker copied or restored into another cluster (or into a
# recreated one) does not count; mark the cluster again after a recreate.
#
# Source it; don't run it. Every kubectl call of the check, and of the writes it
# allows, goes through kc: one binary, ${KUBECTL:-kubectl}, and one context,
# pinned on the first check (cluster_role_pin_context), so the check and the
# write cannot reach different clusters.
#
# CLUSTER_ROLE_DRY_RUN=true, set only by deploy.sh --dry-run after sourcing this
# file (which resets it to false), makes kc print each call instead of running
# it, reads as well as writes, and makes the production check report that it
# would run instead of asking the cluster: a dry run never changes a cluster
# and never decides whether one is production. (deploy.sh --dry-run still runs
# docker info and kind-env.sh's read-only kubectl version check.)

CLUSTER_ROLE_NAMESPACE="kube-system"
CLUSTER_ROLE_CONFIGMAP="econgraph-cluster-role"
# Never taken from the environment: a stray CLUSTER_ROLE_DRY_RUN=true would make
# kc print instead of run while child scripts run for real, and make every
# cluster look production. Only deploy.sh --dry-run sets it, after sourcing.
CLUSTER_ROLE_DRY_RUN=false

# kubectl, as ${KUBECTL:-kubectl}, on the pinned context once there is one.
# `command` runs the binary even where a script wraps kubectl in a function
# that calls kc (deploy.sh, restart-k8s-rollout.sh).
kc() {
    if [ "${CLUSTER_ROLE_DRY_RUN:-false}" = true ]; then
        echo "[dry-run] kubectl${CLUSTER_ROLE_CONTEXT:+ --context ${CLUSTER_ROLE_CONTEXT}} $*" >&2
        # What a real apply would read from a pipe is read and dropped.
        case " $* " in *" -f - "*) cat >/dev/null ;; esac
        return 0
    fi
    if [ -n "${CLUSTER_ROLE_CONTEXT:-}" ]; then
        command "${KUBECTL:-kubectl}" --context "$CLUSTER_ROLE_CONTEXT" "$@"
    else
        command "${KUBECTL:-kubectl}" "$@"
    fi
}

# For a script run by hand (create-secrets.sh, the gateway scripts): choose the
# kubectl, exported as KUBECTL, and say which. Never plain `kubectl` from PATH:
# on the production host that is the MicroK8s snap wrapper, another cluster,
# and it ignores KUBECONFIG. The kind cluster's own copies live in
# ${XDG_CACHE_HOME:-~/.cache}/econgraph (bin/kubectl, kubeconfig-econ-graph),
# kept by the kind-env.sh scripts (ensure_kind_kubectl).
#
#   - KUBECTL set: kept as it is. (deploy.sh exports its KUBECTL with the
#     context it pins, so scripts it runs keep both.)
#   - KUBECONFIG set: the kind kubectl binary with that KUBECONFIG; fails when
#     there is no kind kubectl (set KUBECTL as well).
#   - neither: the kind kubectl and its kubeconfig; fails, naming the missing
#     file, when either is missing.
#
# A pinned CLUSTER_ROLE_CONTEXT without KUBECTL is kept, and the kubectl is
# still chosen as above: the context names a cluster, not the kubectl to reach
# it with.
#
# A failure prints how to go on and returns 1, having changed nothing.
cluster_role_default_kubectl() {
    local dir="${XDG_CACHE_HOME:-$HOME/.cache}/econgraph"
    local bin="$dir/bin/kubectl" config="$dir/kubeconfig-econ-graph"
    local fix="reach the kind cluster once with a script that keeps its kubectl there (any kind-env.sh script, e.g. scripts/deploy/precheck-db-migration.sh --dry-run, or scripts/deploy/recreate-kind-cluster.sh --if-missing to create it), or set KUBECTL (and KUBECONFIG) to choose the cluster explicitly"
    if [ -n "${KUBECTL:-}" ]; then
        return 0
    fi
    if [ -n "${KUBECONFIG:-}" ]; then
        if [ ! -x "$bin" ]; then
            echo "error: KUBECONFIG is set (${KUBECONFIG}) but KUBECTL is not, and there is no kind" \
                "kubectl at ${bin}; plain kubectl may ignore KUBECONFIG (the MicroK8s wrapper does)." \
                "Set KUBECTL too, or ${fix}" >&2
            return 1
        fi
        KUBECTL="$bin"
        export KUBECTL KUBECONFIG
        echo "Using the kind cluster's kubectl ${KUBECTL} with KUBECONFIG=${KUBECONFIG}" \
            "(from the environment; set KUBECTL to choose another kubectl)" >&2
        return 0
    fi
    if [ -x "$bin" ] && [ -s "$config" ]; then
        KUBECTL="$bin"
        KUBECONFIG="$config"
        export KUBECTL KUBECONFIG
        echo "Using the kind cluster's kubectl ${KUBECTL} and KUBECONFIG=${KUBECONFIG}" \
            "(set KUBECTL and KUBECONFIG to choose another cluster)" >&2
        return 0
    fi
    local missing="${bin} and ${config}"
    if [ -x "$bin" ]; then
        missing="$config"
    elif [ -s "$config" ]; then
        missing="$bin"
    fi
    echo "error: no KUBECTL is set and the kind cluster's kubectl is not kept here (no ${missing});" \
        "plain kubectl may be another cluster's. ${fix^}" >&2
    return 1
}

# Pin CLUSTER_ROLE_CONTEXT to the current context, unless it is set already
# (exported, so scripts started from here, like create-secrets.sh from
# deploy.sh, stay on it). Fails when there is no current context.
cluster_role_pin_context() {
    [ -z "${CLUSTER_ROLE_CONTEXT:-}" ] || return 0
    if [ "${CLUSTER_ROLE_DRY_RUN:-false}" = true ]; then
        echo "[dry-run] kubectl config current-context (pinned)" >&2
        CLUSTER_ROLE_CONTEXT="current-context"
        return 0
    fi
    CLUSTER_ROLE_CONTEXT="$(command "${KUBECTL:-kubectl}" config current-context)" || return 1
    [ -n "$CLUSTER_ROLE_CONTEXT" ] || return 1
    export CLUSTER_ROLE_CONTEXT
}

# For the top-level scripts (deploy.sh, restart-k8s-rollout.sh): make CONTEXT
# the current context of ${KUBECTL:-kubectl} and pin it. A CLUSTER_ROLE_CONTEXT
# inherited from the environment is replaced, never followed: it must not send
# the check and the writes to another cluster than the one the script deploys.
cluster_role_use_context() {
    if [ "${CLUSTER_ROLE_DRY_RUN:-false}" = true ]; then
        echo "[dry-run] kubectl config use-context $1 (pinned)" >&2
    else
        command "${KUBECTL:-kubectl}" config use-context "$1" >/dev/null || return 1
    fi
    CLUSTER_ROLE_CONTEXT="$1"
    export CLUSTER_ROLE_CONTEXT
}

# For an operator script that acts on one named context (the database backup
# and restore scripts): pin CLUSTER_ROLE_CONTEXT to context $1 without changing
# the current context of ${KUBECTL:-kubectl}, replacing any inherited one.
# Fails when ${KUBECTL:-kubectl}'s kubeconfig has no such context. The check is
# itself a kc call, so it carries the context like every later call.
cluster_role_pin_named_context() {
    local server
    # An empty name would make kc fall back to the current context.
    [ -n "${1:-}" ] || return 1
    CLUSTER_ROLE_CONTEXT="$1"
    export CLUSTER_ROLE_CONTEXT
    server="$(kc config view --minify -o 'jsonpath={.clusters[0].cluster.server}' 2>/dev/null)" &&
        [ -n "$server" ] || return 1
}

# Print, indented, which cluster the pinned context reaches: this host, the
# kubectl and KUBECONFIG, the context, its API server, its nodes, its
# kube-system UID (new for every cluster) and whether it carries the production
# marker. Shown before a script changes anything, so the output says which
# cluster it acted on. Prints what it could read, and fails when the API
# server, the nodes or the kube-system UID could not be read: the identity is
# then not verified.
cluster_role_show_identity() {
    local server nodes uid role verified=0
    server="$(kc config view --minify -o 'jsonpath={.clusters[0].cluster.server}' 2>/dev/null)" ||
        server=""
    [ -n "$server" ] || verified=1
    nodes="$(kc get nodes -o 'jsonpath={.items[*].metadata.name}' 2>/dev/null)" || { nodes="(unknown)"; verified=1; }
    uid="$(kube_system_uid 2>/dev/null)" || { uid="(unknown)"; verified=1; }
    if cluster_is_marked_production; then
        role="PRODUCTION (marked by mark-production-cluster.sh)"
    else
        role="not production: ${CLUSTER_ROLE_REASON}"
    fi
    echo "  this host       $(hostname 2>/dev/null || echo '(unknown)')"
    echo "  kubectl         ${KUBECTL:-kubectl}"
    echo "  KUBECONFIG      ${KUBECONFIG:-(unset)}"
    echo "  context         ${CLUSTER_ROLE_CONTEXT:-(none)}"
    echo "  API server      ${server:-(unknown)}"
    echo "  nodes           ${nodes:-(none)}"
    echo "  kube-system UID ${uid}"
    echo "  cluster role    ${role}"
    return "$verified"
}

# For an operator script run by hand on one named context: one kubectl and one
# context for every later call, never a CLUSTER_ROLE_CONTEXT inherited from the
# environment. Picks the kubectl (cluster_role_default_kubectl), pins context
# $1 (cluster_role_pin_named_context) and prints the cluster's identity. Fails,
# having changed nothing, when the kubectl is missing, has no such context, or
# the identity can't be read.
cluster_role_operator_context() {
    unset CLUSTER_ROLE_CONTEXT
    cluster_role_default_kubectl || return 1
    export KUBECTL
    if ! command -v "${KUBECTL}" >/dev/null 2>&1; then
        echo "${KUBECTL} not found" >&2
        return 1
    fi
    if ! cluster_role_pin_named_context "${1:-}"; then
        echo "${KUBECTL} has no context ${1:-(empty)} (KUBECONFIG=${KUBECONFIG:-unset})" >&2
        return 1
    fi
    echo "Cluster:"
    if ! cluster_role_show_identity; then
        echo "could not read the identity (API server, nodes, kube-system UID) of context $1" >&2
        return 1
    fi
}

# Print the UID of the kube-system namespace. Fails when the cluster cannot be
# asked or answers with nothing.
kube_system_uid() {
    local uid
    uid="$(kc get namespace "$CLUSTER_ROLE_NAMESPACE" -o 'jsonpath={.metadata.uid}')" || return 1
    [ -n "$uid" ] || return 1
    printf '%s\n' "$uid"
}

# Print the marker as "ROLE UID", or nothing when there is no marker. Fails when
# the cluster cannot be asked.
cluster_role_marker() {
    kc -n "$CLUSTER_ROLE_NAMESPACE" get configmap "$CLUSTER_ROLE_CONFIGMAP" \
        --ignore-not-found -o 'jsonpath={.data.role}{" "}{.data.kube-system-uid}'
}

# True only when the cluster (on the pinned context) carries a production
# marker made on this very cluster. Anything else, including a failure to ask
# the cluster, is "not production" (fail closed); CLUSTER_ROLE_REASON then says
# why. Returns 1 when the cluster answered and is not production, 2 when it
# could not be asked.
# shellcheck disable=SC2034 # CLUSTER_ROLE_REASON is read by the callers.
cluster_is_marked_production() {
    local uid marker role marker_uid
    CLUSTER_ROLE_REASON=""
    if [ "${CLUSTER_ROLE_DRY_RUN:-false}" = true ]; then
        cluster_role_pin_context
        echo "[dry-run] check that context ${CLUSTER_ROLE_CONTEXT} is marked production" \
            "(ConfigMap ${CLUSTER_ROLE_NAMESPACE}/${CLUSTER_ROLE_CONFIGMAP}); a real run stops or" \
            "keeps cloudflared at 0 replicas if it is not" >&2
        return 0
    fi
    if ! cluster_role_pin_context; then
        CLUSTER_ROLE_REASON="kubectl has no current context, so the cluster is treated as not production"
        return 2
    fi
    if ! uid="$(kube_system_uid)"; then
        CLUSTER_ROLE_REASON="could not read the ${CLUSTER_ROLE_NAMESPACE} namespace UID from context ${CLUSTER_ROLE_CONTEXT}, so it is treated as not production"
        return 2
    fi
    if ! marker="$(cluster_role_marker)"; then
        CLUSTER_ROLE_REASON="could not read ConfigMap ${CLUSTER_ROLE_NAMESPACE}/${CLUSTER_ROLE_CONFIGMAP} from context ${CLUSTER_ROLE_CONTEXT}, so it is treated as not production"
        return 2
    fi
    read -r role marker_uid <<<"$marker" || true
    if [ -z "$role" ] && [ -z "$marker_uid" ]; then
        CLUSTER_ROLE_REASON="context ${CLUSTER_ROLE_CONTEXT} is not marked production (no ConfigMap ${CLUSTER_ROLE_NAMESPACE}/${CLUSTER_ROLE_CONFIGMAP})"
        return 1
    fi
    if [ "$role" != production ]; then
        CLUSTER_ROLE_REASON="ConfigMap ${CLUSTER_ROLE_NAMESPACE}/${CLUSTER_ROLE_CONFIGMAP} on context ${CLUSTER_ROLE_CONTEXT} does not say role=production"
        return 1
    fi
    if [ "$marker_uid" != "$uid" ]; then
        CLUSTER_ROLE_REASON="the production marker on context ${CLUSTER_ROLE_CONTEXT} was made on another cluster (${CLUSTER_ROLE_NAMESPACE} UID ${marker_uid:-missing}, this cluster's is ${uid}); a copied or restored marker does not count"
        return 1
    fi
}
