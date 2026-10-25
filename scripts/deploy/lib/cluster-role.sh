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
