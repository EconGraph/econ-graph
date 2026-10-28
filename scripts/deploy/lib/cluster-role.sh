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

CLUSTER_ROLE_NAMESPACE="kube-system"
CLUSTER_ROLE_CONFIGMAP="econgraph-cluster-role"

# kubectl, as ${KUBECTL:-kubectl}, on the pinned context once there is one.
# `command` runs the binary even where a script wraps kubectl in a function
# that calls kc (deploy.sh, restart-k8s-rollout.sh).
kc() {
    if [ -n "${CLUSTER_ROLE_CONTEXT:-}" ]; then
        command "${KUBECTL:-kubectl}" --context "$CLUSTER_ROLE_CONTEXT" "$@"
    else
        command "${KUBECTL:-kubectl}" "$@"
    fi
}

# For a script run by hand (create-secrets.sh, the backup scripts): choose the
# kubectl, exported as KUBECTL, and say which. Never plain `kubectl` from PATH:
# on the production host that is the MicroK8s snap wrapper, another cluster,
# and it ignores KUBECONFIG. The kind cluster's own copies live in
# ${XDG_CACHE_HOME:-~/.cache}/econgraph (bin/kubectl, kubeconfig-econ-graph),
# copied out of the cluster as the error below says.
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
    # The kind cluster runs on the rootful daemon, whatever DOCKER_HOST the
    # shell has (it may be the rootless one), so the commands name it.
    local fix="copy the kind cluster's kubectl and kubeconfig there from the rootful Docker daemon it runs on:
  mkdir -p ${dir}/bin
  DOCKER_HOST=unix:///var/run/docker.sock docker cp -L econ-graph-control-plane:/usr/bin/kubectl ${bin}
  DOCKER_HOST=unix:///var/run/docker.sock kind get kubeconfig --name econ-graph > ${config}
or set KUBECTL (and KUBECONFIG) to choose the cluster explicitly"
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

# The Docker daemon the kind cluster runs on: the rootful system daemon
# (docs/deployment/HOST_STORAGE.md), as for the release-image scripts
# (lib/release-images.sh). The operator's shell may export DOCKER_HOST=<rootless
# socket> for everyday use, so that variable is deliberately not the override;
# ECONGRAPH_DOCKER_HOST is. Exports DOCKER_HOST, so kind, docker and every script
# run from here use the same daemon, and says which one it chose.
cluster_role_kind_docker_host() {
    local chosen="${ECONGRAPH_DOCKER_HOST:-unix:///var/run/docker.sock}"
    if [ -n "${DOCKER_HOST:-}" ] && [ "$DOCKER_HOST" != "$chosen" ]; then
        echo "Docker daemon of the kind cluster: ${chosen}, not DOCKER_HOST=${DOCKER_HOST}" \
            "(set ECONGRAPH_DOCKER_HOST to choose another)" >&2
    else
        echo "Docker daemon of the kind cluster: ${chosen} (set ECONGRAPH_DOCKER_HOST to choose another)" >&2
    fi
    export DOCKER_HOST="$chosen"
    unset DOCKER_CONTEXT
}

# Run kind as the repo keeps it, like ri_kind in lib/release-images.sh: ${KIND},
# else the copy kept next to the kind cluster's kubectl
# (${XDG_CACHE_HOME:-~/.cache}/econgraph/bin/kind), else kind on PATH. Call
# cluster_role_kind_docker_host first.
cluster_role_kind() {
    local cached="${XDG_CACHE_HOME:-$HOME/.cache}/econgraph/bin/kind"
    if [ -n "${KIND:-}" ]; then
        command "$KIND" "$@"
    elif [ -x "$cached" ]; then
        command "$cached" "$@"
    else
        command kind "$@"
    fi
}

# Pin CLUSTER_ROLE_CONTEXT to the current context, unless it is set already
# (exported, so scripts started from here, like create-secrets.sh from
# deploy.sh, stay on it). Fails when there is no current context.
cluster_role_pin_context() {
    [ -z "${CLUSTER_ROLE_CONTEXT:-}" ] || return 0
    CLUSTER_ROLE_CONTEXT="$(command "${KUBECTL:-kubectl}" config current-context)" || return 1
    [ -n "$CLUSTER_ROLE_CONTEXT" ] || return 1
    export CLUSTER_ROLE_CONTEXT
}

# For the top-level scripts (deploy.sh, restart-k8s-rollout.sh): make CONTEXT
# the current context of ${KUBECTL:-kubectl} and pin it. A CLUSTER_ROLE_CONTEXT
# inherited from the environment is replaced, never followed: it must not send
# the check and the writes to another cluster than the one the script deploys.
cluster_role_use_context() {
    command "${KUBECTL:-kubectl}" config use-context "$1" >/dev/null || return 1
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
# cluster it acted on.
cluster_role_show_identity() {
    local server nodes uid role
    server="$(kc config view --minify -o 'jsonpath={.clusters[0].cluster.server}' 2>/dev/null)" ||
        server=""
    nodes="$(kc get nodes -o 'jsonpath={.items[*].metadata.name}' 2>/dev/null)" || nodes="(unknown)"
    uid="$(kube_system_uid 2>/dev/null)" || uid="(unknown)"
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

# For a script about to destroy a cluster (recreate-kind-cluster.sh): whether
# the marker text $1, "ROLE UID" as cluster_role_marker prints it, calls for the
# kube-system UID prefix before the delete. True for any marker with a role
# other than `none`, whatever its case or UID: a mistyped, copied or stale
# production marker still asks. Deliberately broader than
# cluster_is_marked_production, which decides whether a cluster may run the
# tunnel and so counts only an exact marker made on this cluster.
cluster_role_marker_guards_delete() {
    local role
    read -r role _ <<<"${1:-}" || true
    [ -n "${role}" ] && [ "${role}" != none ]
}
