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
# scripts/deploy/render-public-host.py lists what the host changes, and
# scripts/deploy/apply-gateway-routes.sh applies the gateway's part of it.

PUBLIC_HOST_LABEL="econgraph.io/public-host"

# The public host the cluster serves now: from the public HTTPRoutes, else (a
# cluster still on ingress-nginx) the public Ingresses or the retired
# econ-graph-ssl-ingress; nothing when there is none (also when the namespace
# does not exist yet). Fails when the cluster cannot be asked: an error must
# not look like "no public host" and lead to the public routes being removed.
public_host_in_cluster() {
    local ns hosts
    ns="$(kubectl get namespace econ-graph --ignore-not-found -o name)" || return 1
    [ -n "$ns" ] || return 0
    hosts=""
    if kubectl get crd httproutes.gateway.networking.k8s.io >/dev/null 2>&1; then
        hosts="$(kubectl -n econ-graph get httproute -l "${PUBLIC_HOST_LABEL}=true" \
            -o jsonpath='{range .items[*]}{.spec.hostnames[0]}{"\n"}{end}')" || return 1
    fi
    if [ -z "$hosts" ]; then
        hosts="$(kubectl -n econ-graph get ingress -l "${PUBLIC_HOST_LABEL}=true" \
            -o jsonpath='{range .items[*]}{.spec.rules[0].host}{"\n"}{end}')" || return 1
    fi
    if [ -z "$hosts" ]; then
        hosts="$(kubectl -n econ-graph get ingress econ-graph-ssl-ingress --ignore-not-found \
            -o jsonpath='{range .spec.rules[*]}{.host}{"\n"}{end}')" || return 1
    fi
    printf '%s\n' "$hosts" | sed -n '/./{s/^www\.//;p;q;}'
}

# Decide PUBLIC_HOST (see above) and validate it. Exits on a bad name, when the
# cluster cannot be asked, or when it is unset while the cluster serves a
# public host. Exports it for apply-gateway-routes.sh.
public_host_init() {
    local current
    if ! current="$(public_host_in_cluster)"; then
        echo "❌ Could not read the cluster's routes to see which public host it serves"
        exit 1
    fi
    if [ -z "${PUBLIC_HOST+x}" ] && [ -n "$current" ]; then
        echo "❌ The cluster serves https://${current}, but PUBLIC_HOST is not set."
        echo "   Rerun with PUBLIC_HOST=${current} to keep it, or PUBLIC_HOST= to serve localhost only."
        exit 1
    fi
    PUBLIC_HOST="${PUBLIC_HOST:-}"
    export PUBLIC_HOST
    if [ -n "$PUBLIC_HOST" ]; then
        ./scripts/deploy/render-public-host.py "$PUBLIC_HOST" /dev/null >/dev/null || exit 1
        echo "🌐 Public host: https://${PUBLIC_HOST} (www.${PUBLIC_HOST} redirects to it)"
    else
        echo "🌐 No PUBLIC_HOST: serving http://localhost only"
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

# The gateway's side of the public host, both ways: run after install-gateway.sh
# and cert-manager. With a public host: the https listener, its certificate
# (Secret econ-graph-tls), the public copies of the routes and the http-to-https
# and www redirects. Without one, all of that is removed. The localhost routes
# are applied either way. See scripts/deploy/apply-gateway-routes.sh.
#
# The visitor's address: cloudflared forwards it in X-Forwarded-For, which the
# gateway trusts only from pod addresses (ClientTrafficPolicy
# econ-graph-client-headers), so the GraphQL per-client limits apply per
# visitor instead of to every visitor as the cloudflared pod. Any pod can still
# claim an address that way, so addresses logged for in-cluster callers are not
# audit-grade.
apply_public_edge() {
    if [ -n "$PUBLIC_HOST" ]; then
        ./scripts/deploy/apply-gateway-routes.sh
    else
        PUBLIC_HOST=none ./scripts/deploy/apply-gateway-routes.sh
    fi
}

# The Cloudflare Tunnel connector (k8s/manifests/cloudflared.yaml). Its token is
# the Secret cloudflare-tunnel, written by create-secrets.sh (enter it with
# `scripts/deploy/create-secrets.sh --prompt-tunnel-token`, hidden input).
# Without the Secret, or without a public host for it to serve, it is applied
# with 0 replicas, so nothing crash-loops. Sets CLOUDFLARED_DEPLOYED.
apply_cloudflared() {
    CLOUDFLARED_DEPLOYED=false
    if ! kubectl -n econ-graph get secret cloudflare-tunnel >/dev/null 2>&1; then
        echo "⚠️  Secret cloudflare-tunnel not found: cloudflared applied with 0 replicas,"
        echo "   so the site is not served publicly. Enter the tunnel token with"
        echo "   scripts/deploy/create-secrets.sh --prompt-tunnel-token, then deploy again."
        yq '.spec.replicas = 0' k8s/manifests/cloudflared.yaml | kubectl apply -f -
    elif [ -z "$PUBLIC_HOST" ]; then
        echo "⚠️  No PUBLIC_HOST: cloudflared applied with 0 replicas (no route serves"
        echo "   the tunnel's host names)."
        yq '.spec.replicas = 0' k8s/manifests/cloudflared.yaml | kubectl apply -f -
    else
        echo "🚇 Deploying cloudflared (Cloudflare Tunnel)..."
        kubectl apply -f k8s/manifests/cloudflared.yaml
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
