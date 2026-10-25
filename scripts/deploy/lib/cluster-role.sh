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
# Source it; don't run it. Uses ${KUBECTL:-kubectl} against the current context.

CLUSTER_ROLE_NAMESPACE="kube-system"
CLUSTER_ROLE_CONFIGMAP="econgraph-cluster-role"

# Print the UID of the kube-system namespace. Fails when the cluster cannot be
# asked or answers with nothing.
kube_system_uid() {
    local uid
    uid="$("${KUBECTL:-kubectl}" get namespace "$CLUSTER_ROLE_NAMESPACE" \
        -o 'jsonpath={.metadata.uid}')" || return 1
    [ -n "$uid" ] || return 1
    printf '%s\n' "$uid"
}

# Print the marker as "ROLE UID", or nothing when there is no marker. Fails when
# the cluster cannot be asked.
cluster_role_marker() {
    "${KUBECTL:-kubectl}" -n "$CLUSTER_ROLE_NAMESPACE" get configmap "$CLUSTER_ROLE_CONFIGMAP" \
        --ignore-not-found -o 'jsonpath={.data.role}{" "}{.data.kube-system-uid}'
}

# True only when the current cluster carries a production marker made on this
# very cluster. Anything else, including a failure to ask the cluster, is "not
# production" (fail closed); CLUSTER_ROLE_REASON then says why.
# shellcheck disable=SC2034 # CLUSTER_ROLE_REASON is read by the callers.
cluster_is_marked_production() {
    local uid marker role marker_uid
    CLUSTER_ROLE_REASON=""
    if ! uid="$(kube_system_uid)"; then
        CLUSTER_ROLE_REASON="could not read the ${CLUSTER_ROLE_NAMESPACE} namespace UID from the cluster, so it is treated as not production"
        return 1
    fi
    if ! marker="$(cluster_role_marker)"; then
        CLUSTER_ROLE_REASON="could not read ConfigMap ${CLUSTER_ROLE_NAMESPACE}/${CLUSTER_ROLE_CONFIGMAP} from the cluster, so it is treated as not production"
        return 1
    fi
    read -r role marker_uid <<<"$marker" || true
    if [ -z "$role" ] && [ -z "$marker_uid" ]; then
        CLUSTER_ROLE_REASON="the cluster is not marked production (no ConfigMap ${CLUSTER_ROLE_NAMESPACE}/${CLUSTER_ROLE_CONFIGMAP})"
        return 1
    fi
    if [ "$role" != production ]; then
        CLUSTER_ROLE_REASON="ConfigMap ${CLUSTER_ROLE_NAMESPACE}/${CLUSTER_ROLE_CONFIGMAP} does not say role=production"
        return 1
    fi
    if [ "$marker_uid" != "$uid" ]; then
        CLUSTER_ROLE_REASON="the production marker was made on another cluster (${CLUSTER_ROLE_NAMESPACE} UID ${marker_uid:-missing}, this cluster's is ${uid}); a copied or restored marker does not count"
        return 1
    fi
}
