# shellcheck shell=bash
#
# The public host (ECO-457), shared by scripts/deploy/deploy.sh and
# scripts/deploy/restart-k8s-rollout.sh so that both apply the manifests the
# same way. Source it from the repository root after kubectl points at the
# cluster; don't run it.
#
# PUBLIC_HOST decides what the cluster serves:
#   PUBLIC_HOST=econgraph.com  http://localhost and https://econgraph.com
#                              (www.econgraph.com redirects there), reached
#                              through the cloudflared tunnel;
#   PUBLIC_HOST= (empty)       http://localhost only; every public-host piece
#                              left by an earlier deploy is removed;
#   unset                      the same as empty on a cluster that serves no
#                              public host; on one that does, the script stops
#                              rather than silently taking the site down.
# scripts/deploy/render-public-host.py lists what the host changes.
#
# A public host is served only from a cluster marked production with
# scripts/deploy/mark-production-cluster.sh (see lib/cluster-role.sh): on any
# other cluster a non-empty PUBLIC_HOST stops the deploy, and cloudflared is
# kept at 0 replicas.

# shellcheck source=scripts/deploy/lib/cluster-role.sh
source "$(dirname "${BASH_SOURCE[0]}")/cluster-role.sh"

PUBLIC_HOST_LABEL="econgraph.io/public-host"
# Settings this file owns in the ingress-nginx controller ConfigMap. Listed so
# that a deploy without a public host removes them again; use-forwarded-headers
# is here only so that an older value is removed: it must never be set (it
# makes ingress-nginx take the scheme, host and port from client headers).
INGRESS_NGINX_PUBLIC_KEYS=(enable-real-ip forwarded-for-header proxy-real-ip-cidr use-forwarded-headers)

# The public host the cluster serves now, from the public Ingresses or the
# retired econ-graph-ssl-ingress, or nothing (also when the namespace does not
# exist yet). Fails when the cluster cannot be asked: an error must not look
# like "no public host" and lead to the public routes being removed.
public_host_in_cluster() {
    local ns hosts
    ns="$(kubectl get namespace econ-graph --ignore-not-found -o name)" || return 1
    [ -n "$ns" ] || return 0
    hosts="$(kubectl -n econ-graph get ingress -l "${PUBLIC_HOST_LABEL}=true" \
        -o jsonpath='{range .items[*]}{.spec.rules[0].host}{"\n"}{end}')" || return 1
    if [ -z "$hosts" ]; then
        hosts="$(kubectl -n econ-graph get ingress econ-graph-ssl-ingress --ignore-not-found \
            -o jsonpath='{range .spec.rules[*]}{.host}{"\n"}{end}')" || return 1
    fi
    printf '%s\n' "$hosts" | sed -n '/./{s/^www\.//;p;q;}'
}

# Decide PUBLIC_HOST (see above) and validate it. Exits on a bad name, when the
# cluster cannot be asked, or when it is unset while the cluster serves a
# public host.
public_host_init() {
    local current
    if ! current="$(public_host_in_cluster)"; then
        echo "❌ Could not read the cluster's Ingresses to see which public host it serves"
        exit 1
    fi
    if [ -z "${PUBLIC_HOST+x}" ] && [ -n "$current" ]; then
        echo "❌ The cluster serves https://${current}, but PUBLIC_HOST is not set."
        echo "   Rerun with PUBLIC_HOST=${current} to keep it, or PUBLIC_HOST= to serve localhost only."
        exit 1
    fi
    PUBLIC_HOST="${PUBLIC_HOST:-}"
    if [ -n "$PUBLIC_HOST" ]; then
        ./scripts/deploy/render-public-host.py "$PUBLIC_HOST" /dev/null >/dev/null || exit 1
        # Checked before anything is applied, so a refused deploy changes nothing.
        if ! cluster_is_marked_production; then
            refuse_public_host
        fi
        echo "🌐 Public host: https://${PUBLIC_HOST} (www.${PUBLIC_HOST} redirects to it)"
    else
        echo "🌐 No PUBLIC_HOST: serving http://localhost only"
    fi
    # The retired ssl-ingress.yaml's Ingress served the public host names without
    # the /graphql rate limits. It goes before any Ingress is applied: it holds
    # econgraph.com /, and the admission webhook refuses a second Ingress for the
    # same host and path. --ignore-not-found also covers a new cluster without the
    # namespace; any other failure stops the deploy, since the old Ingress would
    # keep serving the public host names.
    if ! kubectl -n econ-graph delete ingress econ-graph-ssl-ingress --ignore-not-found; then
        echo "❌ Could not delete the retired Ingress econ-graph-ssl-ingress; it still serves the public host names"
        exit 1
    fi
}

# Print manifest(s) (or stdin, "-") with the public host applied, if any.
render_manifest() {
    if [ -n "$PUBLIC_HOST" ]; then
        ./scripts/deploy/render-public-host.py "$PUBLIC_HOST" "$@"
    else
        cat "$@"
    fi
}

# The edge in front of the app, both ways: run after the Ingresses are applied.
#
# With a public host, ingress-nginx takes the visitor's address from
# CF-Connecting-IP (the real_ip module, enable-real-ip), trusted only from pod
# addresses, so graphql-ingress.yaml's per-client limits apply per visitor
# instead of to every visitor as the cloudflared pod. That is all it takes from
# client headers: use-forwarded-headers stays off, so X-Forwarded-Proto/-Host/
# -Port sent by a client are ignored and overwritten with the connection's own
# scheme, Host and port. Any pod can still claim an address in CF-Connecting-IP,
# so addresses logged for in-cluster callers are not audit-grade.
#
# Without one, those settings and any public Ingress are removed again.
apply_public_edge() {
    local key patch pod_cidrs
    if [ -n "$PUBLIC_HOST" ]; then
        pod_cidrs="$(kubectl get nodes -o jsonpath='{.items[*].spec.podCIDR}' | tr ' ' ',')"
        if [ -z "$pod_cidrs" ]; then
            echo "❌ Could not read the nodes' pod CIDRs for ingress-nginx's proxy-real-ip-cidr"
            exit 1
        fi
        echo "🔧 ingress-nginx: client address from CF-Connecting-IP, trusted from ${pod_cidrs}"
        kubectl -n ingress-nginx patch configmap ingress-nginx-controller --type merge -p \
            "{\"data\":{\"enable-real-ip\":\"true\",\"forwarded-for-header\":\"CF-Connecting-IP\",\"proxy-real-ip-cidr\":\"${pod_cidrs}\",\"use-forwarded-headers\":null}}"
    else
        patch=""
        for key in "${INGRESS_NGINX_PUBLIC_KEYS[@]}"; do
            patch+="\"${key}\":null,"
        done
        kubectl -n ingress-nginx patch configmap ingress-nginx-controller --type merge -p \
            "{\"data\":{${patch%,}}}"
        kubectl -n econ-graph delete ingress -l "${PUBLIC_HOST_LABEL}=true" --ignore-not-found
    fi
}

# Stop the deploy: PUBLIC_HOST is set on a cluster that is not production
# (CLUSTER_ROLE_REASON says why).
refuse_public_host() {
    echo "❌ PUBLIC_HOST=${PUBLIC_HOST}, but ${CLUSTER_ROLE_REASON}."
    echo "   Only the production cluster serves the public host: a second tunnel connector"
    echo "   would silently take a share of the site's traffic. On the production cluster,"
    echo "   run scripts/deploy/mark-production-cluster.sh once (again after a recreate);"
    echo "   on any other cluster, deploy with PUBLIC_HOST= (empty)."
    exit 1
}

# The Cloudflare Tunnel connector (k8s/manifests/cloudflared.yaml), which is
# committed with replicas: 0, so applying k8s/manifests wholesale never starts
# it. This is the one place that scales it up, to CLOUDFLARED_REPLICAS, and only
# on a cluster marked production that has a public host and the token. The
# token is the Secret cloudflare-tunnel, written by create-secrets.sh (enter it
# with `scripts/deploy/create-secrets.sh --prompt-tunnel-token`, hidden input).
# Otherwise it is applied with 0 replicas, which also stops a connector already
# running; on a cluster that is not production with a PUBLIC_HOST, the deploy
# then stops. When the marker or the Secret cannot be looked up, the deploy
# stops and cloudflared is left as it is: a failed lookup never starts the
# connector, and never takes the site down either. Sets CLOUDFLARED_DEPLOYED.
#
# One replica while the cluster has one worker node. On several nodes, use 2+
# with the pod anti-affinity in the manifest: every replica registers its own
# connections for the same tunnel, so losing a node does not take the site down.
CLOUDFLARED_REPLICAS=1
apply_cloudflared() {
    local secret role=0
    CLOUDFLARED_DEPLOYED=false
    cluster_is_marked_production || role=$?
    if [ "$role" = 2 ]; then
        # Not known to be production, so never started; but not known not to be
        # either, so a running connector is not stopped on a failed lookup.
        echo "❌ ${CLUSTER_ROLE_REASON}; cloudflared was left as it is."
        exit 1
    elif [ "$role" != 0 ]; then
        echo "⚠️  cloudflared applied with 0 replicas: ${CLUSTER_ROLE_REASON}."
        yq '.spec.replicas = 0' k8s/manifests/cloudflared.yaml | kubectl apply -f -
        [ -z "$PUBLIC_HOST" ] || refuse_public_host
        return 0
    fi
    # A failed lookup (API error, access denied) must not look like a missing
    # Secret: that would scale the connector to 0 and take the site down.
    if ! secret="$(kubectl -n econ-graph get secret cloudflare-tunnel --ignore-not-found -o name)"; then
        echo "❌ Could not look up the Secret cloudflare-tunnel; cloudflared was left as it is."
        exit 1
    fi
    if [ -z "$secret" ]; then
        echo "⚠️  Secret cloudflare-tunnel not found: cloudflared applied with 0 replicas,"
        echo "   so the site is not served publicly. Enter the tunnel token with"
        echo "   scripts/deploy/create-secrets.sh --prompt-tunnel-token, then deploy again."
        yq '.spec.replicas = 0' k8s/manifests/cloudflared.yaml | kubectl apply -f -
    elif [ -z "$PUBLIC_HOST" ]; then
        echo "⚠️  No PUBLIC_HOST: cloudflared applied with 0 replicas (no Ingress serves"
        echo "   the tunnel's host names)."
        yq '.spec.replicas = 0' k8s/manifests/cloudflared.yaml | kubectl apply -f -
    else
        echo "🚇 Deploying cloudflared (Cloudflare Tunnel), ${CLOUDFLARED_REPLICAS} replica(s)..."
        yq ".spec.replicas = ${CLOUDFLARED_REPLICAS}" k8s/manifests/cloudflared.yaml | kubectl apply -f -
        CLOUDFLARED_DEPLOYED=true
    fi
}

# Wait for cloudflared, without failing the deploy: a rejected token or an edge
# outage is cloudflared's to report (its logs, and the CloudflareTunnel* alerts).
wait_cloudflared() {
    if [ "${CLOUDFLARED_DEPLOYED:-false}" = true ]; then
        echo "Waiting for cloudflared to connect to Cloudflare..."
        if ! kubectl -n econ-graph rollout status deployment/cloudflared --timeout=120s; then
            echo "⚠️  cloudflared is not ready; check: kubectl -n econ-graph logs deployment/cloudflared"
        fi
    fi
}
