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
#                              through the cloudflared tunnel; sign-in returns
#                              to https://econgraph.com only;
#   PUBLIC_HOST= (empty)       http://localhost only; every public-host piece
#                              left by an earlier deploy is removed;
#   unset                      the same as empty on a cluster that serves no
#                              public host; on one that does, the script stops
#                              rather than silently taking the site down.
# scripts/deploy/render-public-host.py lists what the host changes, and
# scripts/deploy/apply-gateway-routes.sh applies the gateway's part of it.
#
# A public host is served only from a cluster marked production with
# scripts/deploy/mark-production-cluster.sh (see lib/cluster-role.sh): on any
# other cluster a non-empty PUBLIC_HOST stops the deploy, and cloudflared is
# kept at 0 replicas.
#
# Every kubectl call here goes through kc (lib/cluster-role.sh): the same
# binary as the production check, ${KUBECTL:-kubectl}, on the context that
# public_host_init pins, so the check and the applies hit the same cluster.

# shellcheck source=scripts/deploy/lib/cluster-role.sh
source "$(dirname "${BASH_SOURCE[0]}")/cluster-role.sh"
# shellcheck source=scripts/deploy/lib/deploy-config.sh
source "$(dirname "${BASH_SOURCE[0]}")/deploy-config.sh"

PUBLIC_HOST_LABEL="econgraph.io/public-host"

# The public host the cluster serves now: from the public HTTPRoutes, else (a
# cluster still on ingress-nginx) the public Ingresses or the retired
# econ-graph-ssl-ingress; nothing when there is none (also when the namespace
# does not exist yet). Fails when the cluster cannot be asked: an error must
# not look like "no public host" and lead to the public routes being removed.
public_host_in_cluster() {
    local ns hosts=""
    ns="$(kc get namespace econ-graph --ignore-not-found -o name)" || return 1
    [ -n "$ns" ] || return 0
    if kc get crd httproutes.gateway.networking.k8s.io >/dev/null 2>&1; then
        hosts="$(kc -n econ-graph get httproute -l "${PUBLIC_HOST_LABEL}=true" \
            -o jsonpath='{range .items[*]}{.spec.hostnames[0]}{"\n"}{end}')" || return 1
    fi
    if [ -z "$hosts" ]; then
        hosts="$(kc -n econ-graph get ingress -l "${PUBLIC_HOST_LABEL}=true" \
            -o jsonpath='{range .items[*]}{.spec.rules[0].host}{"\n"}{end}')" || return 1
    fi
    if [ -z "$hosts" ]; then
        hosts="$(kc -n econ-graph get ingress econ-graph-ssl-ingress --ignore-not-found \
            -o jsonpath='{range .spec.rules[*]}{.host}{"\n"}{end}')" || return 1
    fi
    printf '%s\n' "$hosts" | sed -n '/./{s/^www\.//;p;q;}'
}

# The PUBLIC_HOST in the host's deploy config (deploy_config_path in
# lib/deploy-config.sh: ${XDG_CONFIG_HOME:-~/.config}/econgraph/deploy.env, or
# ECONGRAPH_CONFIG_DIR/deploy.env, or ECONGRAPH_DEPLOY_ENV): a line
# PUBLIC_HOST=<host name>, the last one when there are several, read by
# deploy_config_read (not sourced; surrounding blanks and one pair of matching
# quotes dropped), then lowercased. Used by release-deploy.sh and
# set-crawler-keys.sh. Sets DEPLOY_ENV, CONFIGURED_PUBLIC_HOST (empty when none is
# configured) and PUBLIC_HOST_CONFIGURED (true or false). Returns 1, with the
# reason in PUBLIC_HOST_CONFIG_ERROR, for a config it can't use: an unreadable
# file, an `export PUBLIC_HOST=` line, CRLF endings, blanks inside the name, or a
# name that is not a public DNS name.
# shellcheck disable=SC2034 # the variables it sets are for the caller
read_configured_public_host() {
    local host
    DEPLOY_ENV="$(deploy_config_path)"
    CONFIGURED_PUBLIC_HOST=""
    PUBLIC_HOST_CONFIGURED=false
    PUBLIC_HOST_CONFIG_ERROR=""
    if ! deploy_config_read PUBLIC_HOST "host name"; then
        PUBLIC_HOST_CONFIG_ERROR="$DEPLOY_CONFIG_ERROR"
        return 1
    fi
    host="$DEPLOY_CONFIG_VALUE"
    # Host names are case-insensitive; the routes, the certificate and the
    # comparisons use the lowercase form.
    if [ "$host" != "${host,,}" ]; then
        echo "  PUBLIC_HOST     ${host} in ${DEPLOY_ENV} is used as ${host,,}"
        host="${host,,}"
    fi
    if [ -n "$host" ] &&
        ! "$(dirname "${BASH_SOURCE[0]}")/../render-public-host.py" "$host" /dev/null >/dev/null; then
        PUBLIC_HOST_CONFIG_ERROR="${DEPLOY_ENV} sets PUBLIC_HOST='${host}', which is not a public DNS name"
        return 1
    fi
    CONFIGURED_PUBLIC_HOST="$host"
    [ -z "$host" ] || PUBLIC_HOST_CONFIGURED=true
}

# Decide PUBLIC_HOST (see above) and validate it. Exits on a bad name, when the
# cluster cannot be asked, or when it is unset while the cluster serves a
# public host. Exports it for apply-gateway-routes.sh.
public_host_init() {
    local current
    if ! cluster_role_pin_context; then
        echo "❌ ${KUBECTL:-kubectl} has no current context"
        exit 1
    fi
    echo "☸️  Context: ${CLUSTER_ROLE_CONTEXT} (${KUBECTL:-kubectl})"
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
        # Checked before anything is applied, so a refused deploy changes nothing.
        if ! cluster_is_marked_production; then
            refuse_public_host
        fi
        echo "🌐 Public host: https://${PUBLIC_HOST} (www.${PUBLIC_HOST} redirects to it)"
        # The certificate's issuer, chosen as apply-gateway-routes.sh will
        # (tls_issuer_choose): a bad GATEWAY_TLS_ISSUER line in the deploy config, or
        # one that would change the issuer implicitly, stops the deploy here, before
        # anything is applied. Without a public host the line is not read. A dry run
        # does not ask the cluster (kc only prints), so it cannot know the current
        # issuer: it checks only that the line can be read, and says so.
        local issuer_now
        if [ "${CLUSTER_ROLE_DRY_RUN:-false}" = true ]; then
            deploy_config_value GATEWAY_TLS_ISSUER "issuer name" >/dev/null || exit 1
            echo "🔐 Certificate issuer: chosen when applied; not checked against the cluster in a dry run"
        else
            if ! issuer_now="$(gateway_tls_current_issuer)"; then
                echo "❌ Could not read the issuer of certificate econ-graph-tls"
                exit 1
            fi
            tls_issuer_choose "$issuer_now" || exit 1
            echo "🔐 Certificate issuer: ${TLS_ISSUER} (from ${TLS_ISSUER_SOURCE})"
        fi
    else
        echo "🌐 No PUBLIC_HOST: serving http://localhost only"
    fi
    # The retired ssl-ingress.yaml's Ingress served the public host names without
    # the /graphql rate limits, on a cluster still on ingress-nginx. It goes
    # first, so it never serves the public host names next to the gateway.
    # --ignore-not-found also covers a new cluster without the namespace; any
    # other failure stops the deploy, since the old Ingress would keep serving
    # the public host names.
    if ! kc -n econ-graph delete ingress econ-graph-ssl-ingress --ignore-not-found; then
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

# The gateway's side of the public host, both ways: run after install-gateway.sh
# and cert-manager. With a public host: the https listener, its certificate
# (Secret econ-graph-tls), the public copies of the routes and the http-to-https
# and www redirects. Without one, all of that is removed. The localhost routes
# are applied either way. See scripts/deploy/apply-gateway-routes.sh, which runs
# on the context public_host_init pinned, with ${KUBECTL:-kubectl}.
#
# The visitor's address: cloudflared forwards it in X-Forwarded-For, which the
# gateway trusts only from pod addresses (ClientTrafficPolicy
# econ-graph-client-headers), so the GraphQL per-client limits apply per
# visitor instead of to every visitor as the cloudflared pod. Any pod can still
# claim an address that way, so addresses logged for in-cluster callers are not
# audit-grade.
apply_public_edge() {
    local args=(--context "$CLUSTER_ROLE_CONTEXT")
    [ "${CLUSTER_ROLE_DRY_RUN:-false}" != true ] || args+=(--dry-run)
    PUBLIC_HOST="${PUBLIC_HOST:-none}" ./scripts/deploy/apply-gateway-routes.sh "${args[@]}"
}

# Stop the deploy: PUBLIC_HOST is set on a cluster that is not production
# (CLUSTER_ROLE_REASON says why).
refuse_public_host() {
    echo "❌ PUBLIC_HOST=${PUBLIC_HOST}, but ${CLUSTER_ROLE_REASON}."
    echo "   Only the production cluster serves the public host: a second tunnel connector"
    echo "   would silently take a share of the site's traffic."
    echo "   Only if this is the cluster serving ${PUBLIC_HOST}: run"
    echo "   scripts/deploy/mark-production-cluster.sh once (again after a recreate)."
    echo "   On any other cluster, deploy with PUBLIC_HOST= (empty)."
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
        yq '.spec.replicas = 0' k8s/manifests/cloudflared.yaml | kc apply -f -
        [ -z "$PUBLIC_HOST" ] || refuse_public_host
        return 0
    fi
    # A failed lookup (API error, access denied) must not look like a missing
    # Secret: that would scale the connector to 0 and take the site down.
    if ! secret="$(kc -n econ-graph get secret cloudflare-tunnel --ignore-not-found -o name)"; then
        echo "❌ Could not look up the Secret cloudflare-tunnel; cloudflared was left as it is."
        exit 1
    fi
    if [ -z "$secret" ]; then
        echo "⚠️  Secret cloudflare-tunnel not found: cloudflared applied with 0 replicas,"
        echo "   so the site is not served publicly. Enter the tunnel token with"
        echo "   scripts/deploy/create-secrets.sh --prompt-tunnel-token, then deploy again."
        yq '.spec.replicas = 0' k8s/manifests/cloudflared.yaml | kc apply -f -
    elif [ -z "$PUBLIC_HOST" ]; then
        echo "⚠️  No PUBLIC_HOST: cloudflared applied with 0 replicas (no route serves"
        echo "   the tunnel's host names)."
        yq '.spec.replicas = 0' k8s/manifests/cloudflared.yaml | kc apply -f -
    else
        echo "🚇 Deploying cloudflared (Cloudflare Tunnel), ${CLOUDFLARED_REPLICAS} replica(s)..."
        yq ".spec.replicas = ${CLOUDFLARED_REPLICAS}" k8s/manifests/cloudflared.yaml | kc apply -f -
        CLOUDFLARED_DEPLOYED=true
    fi
}

# Wait for cloudflared, without failing the deploy: a rejected token or an edge
# outage is cloudflared's to report (its logs, and the CloudflareTunnel* alerts).
wait_cloudflared() {
    if [ "${CLOUDFLARED_DEPLOYED:-false}" = true ]; then
        echo "Waiting for cloudflared to connect to Cloudflare..."
        if ! kc -n econ-graph rollout status deployment/cloudflared --timeout=120s; then
            echo "⚠️  cloudflared is not ready; check: kubectl -n econ-graph logs deployment/cloudflared"
        fi
    fi
}
