#!/bin/bash
#
# Move a running kind cluster from ingress-nginx to the Gateway API gateway
# (Envoy Gateway, k8s/manifests/gateway/), checking every route on the gateway
# before ingress-nginx is removed. For a cluster that still serves through
# ingress-nginx; deploy.sh installs the gateway directly on a new one.
#
# Usage:
#   scripts/deploy/cutover-to-gateway.sh [--dry-run] [--yes] [--context NAME] [--host-url URL]
#   scripts/deploy/cutover-to-gateway.sh --rollback DIR [--dry-run] [--yes] [--context NAME] [--host-url URL]
#
#   --dry-run       Print every step; change nothing.
#   --yes           Answer the confirmation before ingress-nginx is removed.
#   --context NAME  kubectl context (default kind-econ-graph).
#   --host-url URL  Where host ports 80/443 answer (default http://localhost).
#   --rollback DIR  Undo a cut-over from the state it saved in DIR (below).
#
# Steps:
#   1. Read-only checks: the API server answers, namespace econ-graph and the
#      frontend and backend Services exist; report the ingress-nginx
#      controller, the Ingresses it serves and what else to look out for.
#   2. Save what a rollback needs to
#      ${XDG_STATE_HOME:-~/.local/state}/econgraph/gateway-cutover/<UTC time>/
#      (mode 700; the files 600): the Ingresses and NetworkPolicies of
#      namespace econ-graph; the ingress-nginx controller's version, its
#      upstream kind manifest, and its namespace's ConfigMaps, controller
#      Deployment and Secrets (a default TLS certificate, for one), which
#      deleting the namespace removes; and the gateway's release manifests,
#      so steps 3 and 7 and the rollback need no network. If any download
#      fails, stop before changing anything.
#   3. Label namespace econ-graph for the gateway (namespace.yaml), then
#      install the gateway next to ingress-nginx with install-gateway.sh,
#      GATEWAY_HOST_PORTS=false: ingress-nginx keeps host ports 80/443, the
#      proxy is reachable only through its Service. Apply the routes
#      (apply-gateway-routes.sh).
#   4. Check every route through that Service (kubectl port-forward and
#      test-gateway-routes.sh, rate limits included), and that a pod in
#      econ-graph reaches the proxy Service (a short-lived curl pod, image
#      curlimages/curl, which the NetworkPolicies must let through). If a
#      check fails, stop here: nothing has been removed and the site is still
#      on ingress-nginx.
#   5. Ask for a typed "yes" (or --yes).
#   6. Delete the Ingresses in econ-graph, then ingress-nginx itself: its
#      namespace (controller, admission webhook, ConfigMaps and Secrets) and
#      its cluster-wide objects (IngressClass nginx, webhook configuration,
#      RBAC). From here on a failure prints the rollback command.
#   7. Re-run install-gateway.sh with host ports: the proxy restarts on the
#      control-plane node holding ports 80/443 (a few seconds without a
#      proxy, plus the time ingress-nginx takes to stop).
#   8. Check every route again at --host-url.
#
# Rollback (--rollback DIR, with the DIR that step 2 printed):
#   1. Move the gateway proxy off host ports 80/443 (install-gateway.sh with
#      GATEWAY_HOST_PORTS=false, from the manifests saved in DIR); the
#      gateway objects stay, so a later cut-over starts again from step 3.
#   2. Wait for a terminating ingress-nginx namespace to be gone, install
#      ingress-nginx from the kind manifest saved in DIR, then put back its
#      saved ConfigMaps, controller Deployment (its arguments and node
#      selector included) and Secrets. The admission webhook's certificate
#      Secret is the exception: the manifest's own Job issues a new one that
#      matches the new webhook configuration.
#   3. Re-create the saved Ingresses, and put back the saved NetworkPolicies
#      (k8s/manifests/network-policy.yaml admits only the gateway's proxy, so
#      one applied after the cut-over would block ingress-nginx; a policy
#      that did not exist at the cut-over is reported, not deleted).
#   4. Check that --host-url serves the frontend again.
#   The same steps by hand, if this script is not usable:
#     GATEWAY_HOST_PORTS=false scripts/deploy/install-gateway.sh --context kind-econ-graph --manifests-dir DIR/gateway-manifests
#     kubectl apply -f DIR/ingress-nginx.yaml
#     kubectl apply -f DIR/ingress-nginx-objects/<the configmap-* and deployment-* files>
#     kubectl create -f DIR/ingress-nginx-objects/<each secret-* file>
#     kubectl apply -f DIR/ingresses.yaml
#     kubectl apply -f DIR/networkpolicies.yaml       # if it lists any
#
# Never touches PersistentVolumes, databases or the Secrets of econ-graph;
# the only Secrets it deletes are those in namespace ingress-nginx, after
# saving them. A Cloudflare Tunnel whose origin is ingress-nginx's Service
# must be pointed at
# http://econ-graph-gateway-proxy.envoy-gateway-system.svc.cluster.local:80
# (the tunnel's public-hostname settings) when step 6 runs; the script
# reminds you if it finds a cloudflared Deployment.

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
NAMESPACE="econ-graph"
PREFLIGHT_IMAGE="curlimages/curl:8.11.1"

DRY_RUN=false
ASSUME_YES=false
KUBE_CONTEXT="kind-econ-graph"
HOST_URL="http://localhost"
ROLLBACK_DIR=""
while [ $# -gt 0 ]; do
    case "$1" in
        --dry-run) DRY_RUN=true ;;
        --yes | -y) ASSUME_YES=true ;;
        --context | --host-url | --rollback)
            [ $# -ge 2 ] || { echo "error: $1 needs a value" >&2; exit 2; }
            case "$1" in
                --context) KUBE_CONTEXT="$2" ;;
                --host-url) HOST_URL="$2" ;;
                --rollback) ROLLBACK_DIR="$2" ;;
            esac
            shift
            ;;
        -h | --help)
            sed -n '2,/^$/p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "error: unknown argument: $1" >&2
            exit 2
            ;;
    esac
    shift
done
HOST_URL="${HOST_URL%/}"

log() { echo; echo "==> $*"; }
warn() { echo "warning: $*" >&2; }
die() {
    echo "error: $*" >&2
    exit 1
}
kc() { kubectl --context "$KUBE_CONTEXT" "$@"; }

# Run a changing command, or only print it under --dry-run.
run() {
    if [ "$1" = kc ]; then
        echo "+ kubectl --context ${KUBE_CONTEXT} ${*:2}"
    else
        echo "+ $*"
    fi
    if [ "$DRY_RUN" != true ]; then
        "$@"
    fi
}

confirm() {
    local prompt="$1" reply
    if [ "$ASSUME_YES" = true ]; then
        echo "$prompt (--yes given)"
        return 0
    fi
    if [ "$DRY_RUN" = true ]; then
        echo "$prompt (dry run: would ask for confirmation here)"
        return 0
    fi
    [ -t 0 ] || die "$prompt: needs confirmation; run on a terminal or pass --yes"
    read -r -p "$prompt Type 'yes' to continue: " reply
    [ "$reply" = "yes" ] || die "aborted; nothing more was changed"
}

# install_gateway HOST_PORTS [MANIFESTS_DIR]
install_gateway() {
    local host_ports="$1" manifests="${2:-}" args=(--context "$KUBE_CONTEXT")
    [ -n "$manifests" ] && args+=(--manifests-dir "$manifests")
    [ "$DRY_RUN" = true ] && args+=(--dry-run)
    echo "+ GATEWAY_HOST_PORTS=${host_ports} scripts/deploy/install-gateway.sh ${args[*]}"
    GATEWAY_HOST_PORTS="$host_ports" "$PROJECT_ROOT/scripts/deploy/install-gateway.sh" "${args[@]}"
}

apply_routes() {
    local args=(--context "$KUBE_CONTEXT")
    [ "$DRY_RUN" = true ] && args+=(--dry-run)
    "$PROJECT_ROOT/scripts/deploy/apply-gateway-routes.sh" "${args[@]}"
}

# Check the routes through the proxy Service, before the proxy has host ports.
check_routes_in_cluster() {
    local port pf status=0
    if [ "$DRY_RUN" = true ]; then
        echo "+ kubectl -n envoy-gateway-system port-forward svc/econ-graph-gateway-proxy <port>:80"
        echo "+ scripts/deploy/test-gateway-routes.sh --rate-limit http://127.0.0.1:<port>"
        return 0
    fi
    port=$((20000 + RANDOM % 20000))
    kc -n envoy-gateway-system port-forward svc/econ-graph-gateway-proxy "${port}:80" >/dev/null 2>&1 &
    pf=$!
    for _ in $(seq 1 30); do
        curl -s -o /dev/null --max-time 2 "http://127.0.0.1:${port}/" && break
        sleep 1
    done
    "$PROJECT_ROOT/scripts/deploy/test-gateway-routes.sh" --rate-limit "http://127.0.0.1:${port}" || status=$?
    kill "$pf" 2>/dev/null || true
    wait "$pf" 2>/dev/null || true
    return "$status"
}

# A pod in econ-graph (only the namespace-wide NetworkPolicy applies to it,
# as to an in-cluster tunnel connector) reaches the gateway through its
# Service.
check_pod_reaches_proxy() {
    local out
    if [ "$DRY_RUN" = true ]; then
        echo "+ kubectl -n ${NAMESPACE} run gateway-preflight --rm -i --restart=Never --image=${PREFLIGHT_IMAGE} -- curl http://econ-graph-gateway-proxy.envoy-gateway-system.svc.cluster.local/health"
        return 0
    fi
    kc -n "$NAMESPACE" delete pod gateway-preflight --ignore-not-found >/dev/null
    out="$(kc -n "$NAMESPACE" run gateway-preflight --rm -i --quiet --restart=Never \
        --image="$PREFLIGHT_IMAGE" --labels=app=gateway-preflight --pod-running-timeout=180s -- \
        curl -s -o /dev/null -w '%{http_code}' --max-time 10 -H 'Host: localhost' \
        http://econ-graph-gateway-proxy.envoy-gateway-system.svc.cluster.local/health 2>&1 || true)"
    if [ "${out: -3}" = 200 ]; then
        echo "  ok    a pod in ${NAMESPACE} reaches econ-graph-gateway-proxy (200)"
    else
        echo "  FAIL  a pod in ${NAMESPACE} does not reach econ-graph-gateway-proxy: ${out}"
        return 1
    fi
}

wait_host_url() {
    [ "$DRY_RUN" = true ] && return 0
    for _ in $(seq 1 60); do
        [ "$(curl -s -o /dev/null -w '%{http_code}' --max-time 3 -H "Host: ${ROUTE_HOST:-localhost}" "${HOST_URL}/")" = 200 ] && return 0
        sleep 2
    done
    return 1
}

nginx_manifest_url() {
    echo "https://raw.githubusercontent.com/kubernetes/ingress-nginx/controller-$1/deploy/static/provider/kind/deploy.yaml"
}

server_reachable() {
    kc version --request-timeout=15s >/dev/null 2>&1 ||
        die "cannot reach the API server of context ${KUBE_CONTEXT}"
}

# save_object FILE NAMESPACE KIND/NAME: the object as YAML without the fields
# the server sets, so the file re-creates it. Fails unless the file holds it.
save_object() {
    local file="$1" ns="$2" obj="$3"
    kc -n "$ns" get "$obj" -o yaml |
        sed -E '/^  (resourceVersion|uid|creationTimestamp|generation): /d; /^status:/,$d' >"$file" ||
        die "could not save ${ns}/${obj}"
    grep -q "^  name: ${obj#*/}$" "$file" || die "saving ${ns}/${obj} to ${file} failed"
}

# --- rollback --------------------------------------------------------------------

if [ -n "$ROLLBACK_DIR" ]; then
    [ -s "$ROLLBACK_DIR/ingress-nginx-version" ] || die "$ROLLBACK_DIR/ingress-nginx-version is missing"
    version="$(cat "$ROLLBACK_DIR/ingress-nginx-version")"
    [[ "$version" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "bad ingress-nginx version '$version' in $ROLLBACK_DIR"
    # The manifest saved at the cut-over; the upstream URL only for state saved
    # without one.
    manifest="$ROLLBACK_DIR/ingress-nginx.yaml"
    [ -s "$manifest" ] || manifest="$(nginx_manifest_url "$version")"
    gateway_manifests=""
    [ -d "$ROLLBACK_DIR/gateway-manifests" ] && gateway_manifests="$ROLLBACK_DIR/gateway-manifests"
    # Every saved object must be readable before anything is changed.
    for f in "$ROLLBACK_DIR"/ingress-nginx-objects/*.yaml; do
        [ -e "$f" ] || continue
        grep -q '^kind: ' "$f" || die "$f holds no object; not rolling back from $ROLLBACK_DIR"
    done
    server_reachable
    log "Rollback to ingress-nginx ${version} on context ${KUBE_CONTEXT}"
    confirm "Move the gateway off ports 80/443 and reinstall ingress-nginx?"

    log "1/4 Gateway proxy off the host ports"
    install_gateway false "$gateway_manifests"

    log "2/4 ingress-nginx ${version}"
    if [ "$DRY_RUN" != true ] && [ "$(kc get namespace ingress-nginx -o jsonpath='{.status.phase}' 2>/dev/null)" = Terminating ]; then
        echo "  waiting for namespace ingress-nginx to finish terminating"
        kc wait --for=delete namespace/ingress-nginx --timeout=600s
    fi
    run kc apply -f "$manifest"
    if [ -d "$ROLLBACK_DIR/ingress-nginx-objects" ]; then
        for f in "$ROLLBACK_DIR"/ingress-nginx-objects/*.yaml; do
            [ -e "$f" ] || continue
            case "${f##*/}" in
                # `create`, not `apply`: no last-applied annotation holding the
                # values. One the manifest already created is left alone.
                secret-*)
                    if [ "$DRY_RUN" = true ]; then
                        echo "+ kubectl --context ${KUBE_CONTEXT} create -f $f"
                    elif ! kc -n ingress-nginx get secret "$(basename "$f" .yaml | sed 's/^secret-//')" >/dev/null 2>&1; then
                        kc create -f "$f"
                    fi
                    ;;
                *) run kc apply -f "$f" ;;
            esac
        done
    else
        run kc -n ingress-nginx patch deployment ingress-nginx-controller --type=merge \
            -p '{"spec":{"template":{"spec":{"nodeSelector":{"ingress-ready":"true"}}}}}'
    fi
    if [ "$DRY_RUN" != true ]; then
        kc -n ingress-nginx rollout status deployment/ingress-nginx-controller --timeout=300s
        kc -n ingress-nginx wait --for=condition=Complete --timeout=120s job --all >/dev/null 2>&1 || true
    fi

    log "3/4 Ingresses and NetworkPolicies"
    if [ -s "$ROLLBACK_DIR/ingresses.yaml" ]; then
        # The admission webhook may take a moment after the controller is up.
        if [ "$DRY_RUN" = true ]; then
            echo "+ kubectl apply -f $ROLLBACK_DIR/ingresses.yaml"
        else
            for attempt in 1 2 3 4 5 6; do
                kc apply -f "$ROLLBACK_DIR/ingresses.yaml" && break
                [ "$attempt" = 6 ] && die "could not re-create the Ingresses from $ROLLBACK_DIR/ingresses.yaml"
                sleep 10
            done
        fi
    else
        warn "no saved Ingresses in $ROLLBACK_DIR"
    fi
    if grep -q '^  kind: NetworkPolicy' "$ROLLBACK_DIR/networkpolicies.yaml" 2>/dev/null; then
        run kc apply -f "$ROLLBACK_DIR/networkpolicies.yaml"
    fi
    for policy in $(kc -n "$NAMESPACE" get networkpolicy -o name 2>/dev/null); do
        grep -qx "  name: ${policy#*/}" <(sed -n 's/^  \{0,2\}  name: /  name: /p' "$ROLLBACK_DIR/networkpolicies.yaml" 2>/dev/null) ||
            warn "${policy} did not exist at the cut-over and was kept; it may block ingress-nginx (kubectl -n ${NAMESPACE} describe ${policy})"
    done

    log "4/4 ${HOST_URL}/ answers"
    wait_host_url || die "${HOST_URL}/ does not answer 200 after the rollback (check the NetworkPolicies in ${NAMESPACE} and the ingress-nginx controller's log)"
    echo "Rolled back: ${HOST_URL} is served by ingress-nginx again."
    exit 0
fi

# --- cut-over --------------------------------------------------------------------

log "1/8 Checks (read-only), context ${KUBE_CONTEXT}"
server_reachable
kc get namespace "$NAMESPACE" >/dev/null || die "namespace $NAMESPACE not found"
for svc in econ-graph-frontend-service econ-graph-backend-service; do
    kc -n "$NAMESPACE" get service "$svc" >/dev/null || die "Service $svc not found in $NAMESPACE"
done
SKIP_KEYCLOAK=false
if ! kc -n "$NAMESPACE" get service keycloak-service >/dev/null 2>&1; then
    warn "keycloak-service not found; the /idp checks are skipped"
    SKIP_KEYCLOAK=true
fi
export SKIP_KEYCLOAK
nginx_image="$(kc -n ingress-nginx get deployment ingress-nginx-controller \
    -o jsonpath='{.spec.template.spec.containers[0].image}' 2>/dev/null || true)"
nginx_version="$(sed -n 's/.*:\(v[0-9][0-9.]*\).*/\1/p' <<<"$nginx_image")"
if [ -n "$nginx_image" ]; then
    echo "  ingress-nginx controller: ${nginx_image}"
    [ -n "$nginx_version" ] || die "cannot read the ingress-nginx version from image '${nginx_image}'"
else
    echo "  ingress-nginx: not installed"
fi
if kc -n ingress get daemonset nginx-ingress-microk8s-controller >/dev/null 2>&1; then
    die "MicroK8s's ingress add-on holds host ports 80/443; this script removes kind's ingress-nginx only. Run 'microk8s disable ingress', then scripts/deploy/install-gateway.sh"
fi
echo "  Ingresses in ${NAMESPACE}:"
kc -n "$NAMESPACE" get ingress 2>/dev/null | sed 's/^/    /' || true
# Host names the Ingresses serve that the HTTPRoutes don't: after step 6 the
# gateway answers them with 404.
https_host="$(kc -n econ-graph-gateway get gateway econ-graph-gateway \
    -o jsonpath='{.spec.listeners[?(@.name=="https")].hostname}' 2>/dev/null || true)"
route_hosts="$(printf 'localhost\n%s\n' "$https_host")"
unserved=""
for host in $(kc -n "$NAMESPACE" get ingress -o jsonpath='{.items[*].spec.rules[*].host}' 2>/dev/null | tr ' ' '\n' | sort -u); do
    grep -qx "$host" <<<"$route_hosts" || unserved="${unserved} ${host}"
done
if [ -n "$unserved" ]; then
    warn "the Ingresses also serve${unserved}, which the HTTPRoutes do not; the gateway will not answer for those host names"
fi
if [ -n "$nginx_image" ]; then
    custom="$(kc -n ingress-nginx get configmap ingress-nginx-controller -o jsonpath='{.data}' 2>/dev/null || true)"
    if [ -n "$custom" ] && [ "$custom" != "{}" ] && [ "$custom" != '{"allow-snippet-annotations":"false"}' ]; then
        warn "ingress-nginx's ConfigMap has settings (${custom}); the gateway does not take them over (they are saved for a rollback)"
    fi
fi
if [ -z "${EXPECTED_ISSUER:-}" ]; then
    kc_hostname="$(kc -n "$NAMESPACE" get deployment keycloak \
        -o jsonpath='{.spec.template.spec.containers[0].env[?(@.name=="KC_HOSTNAME")].value}' 2>/dev/null || true)"
    if [ -n "$kc_hostname" ]; then
        export EXPECTED_ISSUER="${kc_hostname%/}/realms/econ-graph"
        echo "  Keycloak issuer: ${EXPECTED_ISSUER}"
    fi
fi
if [ -n "$(kc -n "$NAMESPACE" get networkpolicy -o name 2>/dev/null)" ]; then
    warn "NetworkPolicies exist in ${NAMESPACE}; they must admit the Envoy proxy pods and let pods reach them (k8s/manifests/network-policy.yaml does), or the checks in step 4 fail"
fi
if kc -n "$NAMESPACE" get deployment cloudflared >/dev/null 2>&1; then
    warn "a cloudflared Deployment runs in ${NAMESPACE}: after step 6, point the tunnel's public hostnames at http://econ-graph-gateway-proxy.envoy-gateway-system.svc.cluster.local:80"
fi

log "2/8 Saving rollback state"
STATE_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/econgraph/gateway-cutover/$(date -u +%Y%m%dT%H%M%SZ)"
GATEWAY_MANIFESTS=""
if [ -z "$nginx_image" ]; then
    # Nothing to go back to: the run only (re)applies the gateway.
    STATE_DIR=""
    echo "  ingress-nginx is not installed; no rollback state saved"
elif [ "$DRY_RUN" = true ]; then
    echo "+ save to ${STATE_DIR}/: the Ingresses and NetworkPolicies of ${NAMESPACE}; ingress-nginx's version,"
    echo "  kind manifest ($(nginx_manifest_url "$nginx_version")), ConfigMaps, controller Deployment and Secrets;"
    echo "  the gateway's release manifests"
else
    (umask 077 && mkdir -p "$STATE_DIR/ingress-nginx-objects")
    GATEWAY_MANIFESTS="$STATE_DIR/gateway-manifests"
    (
        umask 077
        echo "${nginx_version}" >"$STATE_DIR/ingress-nginx-version"
        # The downloads first: without them there is no way back, so stop
        # before anything changes.
        curl -fsSL --retry 3 -o "$STATE_DIR/ingress-nginx.yaml" "$(nginx_manifest_url "$nginx_version")" ||
            die "cannot download the ingress-nginx ${nginx_version} kind manifest for the rollback; nothing was changed"
        "$PROJECT_ROOT/scripts/deploy/install-gateway.sh" --fetch-only --manifests-dir "$GATEWAY_MANIFESTS" ||
            die "cannot download the gateway's release manifests; nothing was changed"
        # Server-set fields removed, so the files re-create the objects.
        kc -n "$NAMESPACE" get ingress -o yaml |
            sed -E '/^    (resourceVersion|uid|creationTimestamp|generation): /d' >"$STATE_DIR/ingresses.yaml"
        kc -n "$NAMESPACE" get networkpolicy -o yaml |
            sed -E '/^    (resourceVersion|uid|creationTimestamp|generation): /d' >"$STATE_DIR/networkpolicies.yaml"
        # What deleting the ingress-nginx namespace removes and the stock
        # manifest does not bring back. Not the admission certificate (the
        # manifest's Job issues one matching the new webhook) and not
        # service-account tokens.
        save_object "$STATE_DIR/ingress-nginx-objects/deployment-ingress-nginx-controller.yaml" \
            ingress-nginx deployment/ingress-nginx-controller
        for cm in $(kc -n ingress-nginx get configmap -o name); do
            [ "$cm" = configmap/kube-root-ca.crt ] && continue
            save_object "$STATE_DIR/ingress-nginx-objects/configmap-${cm#*/}.yaml" ingress-nginx "$cm"
        done
        for secret in $(kc -n ingress-nginx get secret -o name --field-selector type!=kubernetes.io/service-account-token); do
            [ "$secret" = secret/ingress-nginx-admission ] && continue
            save_object "$STATE_DIR/ingress-nginx-objects/secret-${secret#*/}.yaml" ingress-nginx "$secret"
        done
    ) || exit 1
    echo "  ${STATE_DIR}"
    for f in "$STATE_DIR"/ingress-nginx-objects/*; do echo "    ingress-nginx-objects/${f##*/}"; done
    echo "  rollback: scripts/deploy/cutover-to-gateway.sh --rollback ${STATE_DIR} --context ${KUBE_CONTEXT}"
fi

log "3/8 Gateway next to ingress-nginx (no host ports yet)"
run kc label namespace "$NAMESPACE" --overwrite econgraph.io/gateway-access=true econgraph.io/env=production
install_gateway false "$GATEWAY_MANIFESTS"
apply_routes
if [ "$DRY_RUN" != true ]; then
    for route in econ-graph-frontend econ-graph-backend econ-graph-graphql econ-graph-data-graphql econ-graph-idp; do
        accepted=""
        for _ in $(seq 1 60); do
            accepted="$(kc -n "$NAMESPACE" get httproute "$route" \
                -o jsonpath='{.status.parents[0].conditions[?(@.type=="Accepted")].status}' 2>/dev/null || true)"
            [ "$accepted" = True ] && break
            sleep 2
        done
        [ "$accepted" = True ] || die "HTTPRoute $route was not accepted by the gateway"
    done
fi

log "4/8 Checking every route through the proxy Service"
if ! check_routes_in_cluster || ! check_pod_reaches_proxy; then
    die "a check failed; ingress-nginx is untouched and still serves the site. The gateway can stay installed (it holds no host ports)."
fi

log "5/8 Confirm"
[ -z "$unserved" ] || warn "not served by the gateway after the switch:${unserved}"
confirm "All routes answer on the gateway. Delete the Ingresses and ingress-nginx, and give ports 80/443 to the gateway?"

log "6/8 Removing the Ingresses and ingress-nginx"
# From here on, a failure leaves the site between the two; say how to go back.
if [ "$DRY_RUN" != true ] && [ -n "$STATE_DIR" ]; then
    trap 'status=$?; if [ "$status" -ne 0 ]; then echo; echo "The cut-over stopped after ingress-nginx was removed. To go back to ingress-nginx:"; echo "  scripts/deploy/cutover-to-gateway.sh --rollback ${STATE_DIR} --context ${KUBE_CONTEXT}"; fi' EXIT
fi
if [ -n "$nginx_image" ]; then
    run kc -n "$NAMESPACE" delete ingress --all --wait=true
    run kc delete namespace ingress-nginx --wait=true --timeout=300s
    run kc delete ingressclass,validatingwebhookconfiguration,clusterrole,clusterrolebinding \
        -l app.kubernetes.io/name=ingress-nginx --ignore-not-found
fi

log "7/8 Gateway proxy onto host ports 80/443"
install_gateway true "$GATEWAY_MANIFESTS"

log "8/8 Checking every route at ${HOST_URL}"
if [ "$DRY_RUN" = true ]; then
    echo "+ scripts/deploy/test-gateway-routes.sh ${HOST_URL}"
else
    wait_host_url || warn "${HOST_URL}/ is not answering yet"
    if ! "$PROJECT_ROOT/scripts/deploy/test-gateway-routes.sh" "$HOST_URL"; then
        echo "A route check failed after the switch."
        exit 1
    fi
fi

echo
if [ "$DRY_RUN" = true ]; then
    echo "Dry run complete: nothing was changed."
    exit 0
fi
echo "Done: ${HOST_URL} is served by the gateway; ingress-nginx is removed."
[ -z "$STATE_DIR" ] || echo "Rollback, if needed: scripts/deploy/cutover-to-gateway.sh --rollback ${STATE_DIR} --context ${KUBE_CONTEXT}"
