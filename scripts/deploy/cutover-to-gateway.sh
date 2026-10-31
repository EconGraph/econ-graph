#!/bin/bash
#
# Move a running kind cluster from ingress-nginx to the Gateway API gateway
# (Envoy Gateway, k8s/manifests/gateway/), checking every route on the gateway
# before ingress-nginx is removed. For a cluster that still serves through
# ingress-nginx; deploy.sh installs the gateway directly on a new one.
#
# Usage:
#   scripts/deploy/cutover-to-gateway.sh [--dry-run] [--yes] [--context NAME] [--host-url URL] [--host-https-url URL]
#   scripts/deploy/cutover-to-gateway.sh --rollback DIR [--dry-run] [--yes] [--context NAME] [--host-url URL]
#
#   --dry-run       Print every step; change nothing.
#   --yes           Answer the confirmation before ingress-nginx is removed.
#   --context NAME  kubectl context (default kind-econ-graph).
#   --host-url URL  Where host port 80 answers (default http://localhost).
#   --host-https-url URL
#                   Where host port 443 answers (default https://localhost);
#                   checked only for a public host.
#   --rollback DIR  Undo a cut-over from the state it saved in DIR (below).
#
# kubectl is KUBECTL, with the KUBECONFIG it finds. Without KUBECTL, it is the
# kind cluster's own kubectl kept in ${XDG_CACHE_HOME:-~/.cache}/econgraph, with
# KUBECONFIG if that is set and the kind cluster's kubeconfig kept there if not
# (cluster_role_default_kubectl in lib/cluster-role.sh); without those copies
# the script stops before anything runs. Never plain `kubectl`: it may be
# another cluster's, such as the MicroK8s snap wrapper. install-gateway.sh and
# apply-gateway-routes.sh get the same one.
#
# Steps:
#   1. Read-only checks: the API server answers, namespace econ-graph and the
#      frontend and backend Services exist; report the ingress-nginx
#      controller, the Ingresses it serves and what else to look out for. The
#      public host (PUBLIC_HOST, scripts/deploy/lib/public-host.sh) is the one
#      the public Ingresses serve, unless PUBLIC_HOST is given; it needs
#      cert-manager. MAIN_HOST and MAIN_HOST_PERMANENT come from the environment, else
#      the host's deploy config (as deploy.sh reads them), else MAIN_HOST is
#      www.PUBLIC_HOST; the cut-over stops when Keycloak's live KC_HOSTNAME is on the
#      other name (scripts/deploy/switch-main-host.sh changes the main host).
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
#      (apply-gateway-routes.sh), with the public host's https listener,
#      certificate, public routes and redirects.
#   4. Check every route through that Service (kubectl port-forward and
#      test-gateway-routes.sh, rate limits included): on port 80 for
#      localhost, and for a public host on port 443 with its Host header, the
#      way the tunnel connector sends it, plus its redirects. Check that a pod
#      in econ-graph reaches the proxy Service, over https too for a public
#      host (a short-lived curl pod, image curlimages/curl, which the
#      NetworkPolicies must let through, as they must cloudflared). If a check
#      fails, stop here: nothing has been removed and the site is still on
#      ingress-nginx.
#   5. Ask for a typed "yes" (or --yes).
#   6. Delete the Ingresses in econ-graph, then ingress-nginx itself: its
#      namespace (controller, admission webhook, ConfigMaps and Secrets) and
#      its cluster-wide objects (IngressClass nginx, webhook configuration,
#      RBAC). From here on a failure prints the rollback command. With a
#      cloudflared Deployment, the tunnel's origin keeps working: namespace
#      ingress-nginx is re-created holding only Service ingress-nginx-controller
#      of type ExternalName, pointing at the gateway's proxy Service (labelled
#      econgraph.io/tunnel-origin-shim). Point the tunnel's public hostnames
#      at https://econ-graph-gateway-proxy.envoy-gateway-system.svc.cluster.local:443
#      (No TLS Verify, as before) in the Cloudflare dashboard, then delete
#      that namespace; the script prints both.
#   7. Re-run install-gateway.sh with host ports: the proxy restarts on the
#      control-plane node holding ports 80/443 (a few seconds without a
#      proxy, plus the time ingress-nginx takes to stop).
#   8. Check every route again at --host-url, and for a public host at
#      --host-https-url and through the tunnel's origin name.
#
# Rollback (--rollback DIR, with the DIR that step 2 printed):
#   1. Move the gateway proxy off host ports 80/443 (install-gateway.sh with
#      GATEWAY_HOST_PORTS=false, from the manifests saved in DIR); the
#      gateway objects stay, so a later cut-over starts again from step 3.
#   2. Delete the tunnel-origin Service of step 6, if any. Wait for a
#      terminating ingress-nginx namespace to be gone, install
#      ingress-nginx from the kind manifest saved in DIR, then put back its
#      saved ConfigMaps, controller Deployment (its arguments and node
#      selector included) and Secrets. The admission webhook's certificate
#      Secret is the exception: the manifest's own Job issues a new one that
#      matches the new webhook configuration.
#   3. Re-create the saved Ingresses, put back the saved NetworkPolicies, and,
#      when the namespace then has any NetworkPolicy, add
#      econ-graph-ingress-nginx-rollback, which admits the ingress-nginx
#      controller to every pod in the namespace on the ports the Ingresses use
#      (3000, 8080, 9876; never Postgres). It is needed when
#      k8s/manifests/network-policy.yaml, which admits only the gateway's
#      proxy, was applied before the cut-over (and so saved) or after it.
#      Without any other policy nothing blocks ingress-nginx, and it is not
#      added: alone, it would deny every other connection to the namespace's
#      pods (the backend's to Postgres among them). The next cut-over deletes
#      it. A policy that did not exist at the cut-over is reported, not deleted.
#   4. Check that --host-url serves the frontend again, and that a GraphQL
#      query through ingress-nginx gets an answer from the database.
#   The same steps by hand, if this script is not usable:
#     GATEWAY_HOST_PORTS=false scripts/deploy/install-gateway.sh --context kind-econ-graph --manifests-dir DIR/gateway-manifests
#     kubectl apply -f DIR/ingress-nginx.yaml
#     kubectl apply -f DIR/ingress-nginx-objects/<the configmap-* and deployment-* files>
#     kubectl create -f DIR/ingress-nginx-objects/<each secret-* file>
#     kubectl apply -f DIR/ingresses.yaml
#     kubectl apply -f DIR/networkpolicies.yaml       # if it lists any
#     (and, if the namespace has any NetworkPolicy, econ-graph-ingress-nginx-rollback,
#     printed by a --dry-run rollback)
#
# Never touches PersistentVolumes, databases or the Secrets of econ-graph;
# the only Secrets it deletes are those in namespace ingress-nginx, after
# saving them. It never changes the Cloudflare side.

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
NAMESPACE="econ-graph"
PREFLIGHT_IMAGE="curlimages/curl:8.11.1"
# Added by --rollback, deleted by the next cut-over (see the header).
ROLLBACK_POLICY="econ-graph-ingress-nginx-rollback"

DRY_RUN=false
ASSUME_YES=false
KUBE_CONTEXT="kind-econ-graph"
HOST_URL="http://localhost"
HOST_HTTPS_URL="https://localhost"
ROLLBACK_DIR=""
while [ $# -gt 0 ]; do
    case "$1" in
        --dry-run) DRY_RUN=true ;;
        --yes | -y) ASSUME_YES=true ;;
        --context | --host-url | --host-https-url | --rollback)
            [ $# -ge 2 ] || { echo "error: $1 needs a value" >&2; exit 2; }
            case "$1" in
                --context) KUBE_CONTEXT="$2" ;;
                --host-url) HOST_URL="$2" ;;
                --host-https-url) HOST_HTTPS_URL="$2" ;;
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
HOST_HTTPS_URL="${HOST_HTTPS_URL%/}"
PROXY_FQDN="econ-graph-gateway-proxy.envoy-gateway-system.svc.cluster.local"
SHIM_LABEL="econgraph.io/tunnel-origin-shim"

log() { echo; echo "==> $*"; }
warn() { echo "warning: $*" >&2; }
die() {
    echo "error: $*" >&2
    exit 1
}
# The kubectl (see the header), exported for install-gateway.sh and
# apply-gateway-routes.sh. Every call names --context, so a context pinned by
# a parent script is not followed.
# lib/public-host.sh brings lib/cluster-role.sh and lib/deploy-config.sh, and
# main_host_init; its kc calls are this script's kc, defined below.
# shellcheck source=scripts/deploy/lib/public-host.sh
source "$PROJECT_ROOT/scripts/deploy/lib/public-host.sh"
unset CLUSTER_ROLE_CONTEXT
cluster_role_default_kubectl || exit 1
export KUBECTL="${KUBECTL:-kubectl}"
kc() { command "$KUBECTL" --context "$KUBE_CONTEXT" "$@"; }

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
    PUBLIC_HOST="${PUBLIC_HOST:-none}" MAIN_HOST="${MAIN_HOST:-}" "$PROJECT_ROOT/scripts/deploy/apply-gateway-routes.sh" "${args[@]}"
}

# The public host's redirects: plain http for HOST and www.HOST, and https for
# the alternate host, answer to https://MAIN_HOST with render-public-host.py's status:
# 301 to the apex or to the main host MAIN_HOST_PERMANENT names, else 302. BASE_HTTP and
# BASE_HTTPS are where the gateway's ports 80 and 443 answer.
check_public_redirects() {
    local base_http="$1" base_https="$2" status=0 got spec scheme name base alternate code=301
    [ "$MAIN_HOST" = "$PUBLIC_HOST" ] || [ "${MAIN_HOST_PERMANENT:-}" = "$MAIN_HOST" ] || code=302
    alternate="$PUBLIC_HOST"
    [ "$MAIN_HOST" != "$PUBLIC_HOST" ] || alternate="www.${PUBLIC_HOST}"
    for spec in "http ${PUBLIC_HOST} ${base_http}" "http www.${PUBLIC_HOST} ${base_http}" \
        "https ${alternate} ${base_https}"; do
        read -r scheme name base <<<"$spec"
        got="$(curl -sk -o /dev/null -w '%{http_code} %{redirect_url}' --max-time 10 -H "Host: ${name}" "${base}/some/path?from=redirect")"
        if [ "$got" = "${code} https://${MAIN_HOST}/some/path?from=redirect" ]; then
            echo "  ok    ${scheme}://${name}/some/path?from=redirect -> ${code} https://${MAIN_HOST}/some/path?from=redirect"
        else
            echo "  FAIL  ${scheme}://${name}/some/path?from=redirect: got '${got}'"
            status=1
        fi
    done
    return "$status"
}

# main_host_matches_keycloak: fails, saying why, when Keycloak's live public issuer is on
# another name than MAIN_HOST, so a cut-over never serves www while sign-in stays on the
# apex (or the reverse); changing the main host is scripts/deploy/switch-main-host.sh's.
# A Keycloak without a public KC_HOSTNAME (none yet, or localhost) is no conflict.
main_host_matches_keycloak() {
    local live
    if ! live="$(kc -n "$NAMESPACE" get deployment keycloak --ignore-not-found \
        -o 'jsonpath={.spec.template.spec.containers[?(@.name=="keycloak")].env[?(@.name=="KC_HOSTNAME")].value}')"; then
        echo "could not read Keycloak's KC_HOSTNAME to compare it with MAIN_HOST=${MAIN_HOST}"
        return 1
    fi
    case "$live" in
        https://*)
            if [ "$live" != "https://${MAIN_HOST}/idp" ]; then
                echo "Keycloak's live KC_HOSTNAME is ${live}, but MAIN_HOST=${MAIN_HOST} would serve https://${MAIN_HOST}: set MAIN_HOST to the host Keycloak serves (deploy config or environment), or change it first with scripts/deploy/switch-main-host.sh"
                return 1
            fi ;;
    esac
}

# Check the routes through the proxy Service, before the proxy has host ports.
check_routes_in_cluster() {
    local port pf status=0
    if [ "$DRY_RUN" = true ]; then
        echo "+ kubectl -n envoy-gateway-system port-forward svc/econ-graph-gateway-proxy <port>:80 <port2>:443"
        echo "+ scripts/deploy/test-gateway-routes.sh --rate-limit http://127.0.0.1:<port>"
        if [ -n "${PUBLIC_HOST:-}" ]; then
            echo "+ ROUTE_HOST=${MAIN_HOST} scripts/deploy/test-gateway-routes.sh https://127.0.0.1:<port2>, and the redirects"
        fi
        return 0
    fi
    port=$((20000 + RANDOM % 20000))
    if [ -n "${PUBLIC_HOST:-}" ]; then
        kc -n envoy-gateway-system port-forward svc/econ-graph-gateway-proxy "${port}:80" "$((port + 1)):443" >/dev/null 2>&1 &
    else
        kc -n envoy-gateway-system port-forward svc/econ-graph-gateway-proxy "${port}:80" >/dev/null 2>&1 &
    fi
    pf=$!
    for _ in $(seq 1 30); do
        curl -s -o /dev/null --max-time 2 "http://127.0.0.1:${port}/" && break
        sleep 1
    done
    "$PROJECT_ROOT/scripts/deploy/test-gateway-routes.sh" --rate-limit "http://127.0.0.1:${port}" || status=$?
    if [ -n "${PUBLIC_HOST:-}" ]; then
        # The certificate is issued by cert-manager; give it a moment.
        for _ in $(seq 1 60); do
            [ "$(kc -n econ-graph-gateway get certificate econ-graph-tls \
                -o jsonpath='{.status.conditions[?(@.type=="Ready")].status}' 2>/dev/null)" = True ] && break
            sleep 2
        done
        echo "Main host ${MAIN_HOST}, over https as the tunnel connector sends it:"
        ROUTE_HOST="$MAIN_HOST" "$PROJECT_ROOT/scripts/deploy/test-gateway-routes.sh" "https://127.0.0.1:$((port + 1))" || status=$?
        check_public_redirects "http://127.0.0.1:${port}" "https://127.0.0.1:$((port + 1))" || status=$?
    fi
    kill "$pf" 2>/dev/null || true
    wait "$pf" 2>/dev/null || true
    return "$status"
}

# A pod in econ-graph (only the namespace-wide NetworkPolicy applies to it,
# as to an in-cluster tunnel connector) reaches the gateway through its
# Service.
# check_pod_reaches_proxy URL HOST: a pod in econ-graph (only the
# namespace-wide NetworkPolicy applies to it, as to the tunnel connector)
# gets 200 for URL/health with Host HOST.
# NetworkPolicy ROLLBACK_POLICY: the ingress-nginx controller may reach every
# pod in NAMESPACE on the ports the Ingresses use (frontend 3000, Keycloak
# 8080, backend 9876), and nothing else (never Postgres).
rollback_policy_yaml() {
    cat <<EOF
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: ${ROLLBACK_POLICY}
  namespace: ${NAMESPACE}
spec:
  podSelector: {}
  policyTypes:
  - Ingress
  ingress:
  - from:
    - namespaceSelector:
        matchLabels:
          kubernetes.io/metadata.name: ingress-nginx
      podSelector:
        matchLabels:
          app.kubernetes.io/name: ingress-nginx
    ports:
    - protocol: TCP
      port: 3000
    - protocol: TCP
      port: 8080
    - protocol: TCP
      port: 9876
EOF
}

# Add ROLLBACK_POLICY when NAMESPACE has any other NetworkPolicy (the live
# ones; in a dry run also those in the saved file SAVED, which a real run
# would have applied by now). Without one, nothing blocks ingress-nginx, and a
# podSelector {} policy would become the only ingress policy of every pod,
# denying all other connections to them; a left-over one is deleted then.
apply_rollback_policy() {
    local saved="$1" live others
    live="$(kc -n "$NAMESPACE" get networkpolicy -o name)" ||
        die "could not list the NetworkPolicies of ${NAMESPACE}"
    others="$(
        {
            printf '%s\n' "$live" | sed 's|^[^/]*/||'
            if [ "$DRY_RUN" = true ] && [ -f "$saved" ]; then
                sed -n 's/^  \{0,2\}  name: //p' "$saved"
            fi
        } | grep -v -x -e "$ROLLBACK_POLICY" -e '' | sort -u | tr '\n' ' '
    )" || true
    if [ -n "$others" ]; then
        echo "  NetworkPolicies in ${NAMESPACE} (${others% }): adding ${ROLLBACK_POLICY} so ingress-nginx gets through"
        if [ "$DRY_RUN" = true ]; then
            echo "+ kubectl --context ${KUBE_CONTEXT} apply -f - <<"
            rollback_policy_yaml | sed 's/^/    /'
        else
            rollback_policy_yaml | kc apply -f -
        fi
    else
        echo "  no NetworkPolicy in ${NAMESPACE}: nothing blocks ingress-nginx, so ${ROLLBACK_POLICY} is not added"
        run kc -n "$NAMESPACE" delete networkpolicy "$ROLLBACK_POLICY" --ignore-not-found
    fi
}

# Whether a GraphQL query sent to HOST_URL (through ingress-nginx after a
# rollback) gets an answer the backend read from the database: the frontend's
# root alone would not show a NetworkPolicy that keeps the backend from
# Postgres.
wait_backend_db() {
    local body
    [ "$DRY_RUN" = true ] && { echo "+ POST ${HOST_URL}/graphql {dataSources {id}} until it answers from the database"; return 0; }
    for _ in $(seq 1 60); do
        body="$(curl -s --max-time 10 -H "Host: ${ROUTE_HOST:-localhost}" -H 'Content-Type: application/json' \
            --data '{"query":"{ dataSources { id } }"}' "${HOST_URL}/graphql" || true)"
        case "$body" in
            *'"dataSources"'*) case "$body" in *'"errors"'*) ;; *) return 0 ;; esac ;;
        esac
        sleep 2
    done
    echo "  last answer: $(printf '%s' "$body" | head -c 300)" >&2
    return 1
}

check_pod_reaches_proxy() {
    local url="$1" host="$2" out
    if [ "$DRY_RUN" = true ]; then
        echo "+ kubectl -n ${NAMESPACE} run gateway-preflight --image=${PREFLIGHT_IMAGE} (sleeping); kubectl exec gateway-preflight -- curl -k -H 'Host: ${host}' ${url}/health; delete it"
        return 0
    fi
    # A pod that waits, then curl through kubectl exec: the output of
    # `kubectl run --rm -i` can be lost when the container exits first.
    kc -n "$NAMESPACE" delete pod gateway-preflight --ignore-not-found --wait=true >/dev/null
    kc -n "$NAMESPACE" run gateway-preflight --restart=Never --image="$PREFLIGHT_IMAGE" \
        --labels=app=gateway-preflight --command -- sleep 600 >/dev/null
    out="$(kc -n "$NAMESPACE" wait --for=condition=Ready --timeout=180s pod/gateway-preflight >/dev/null 2>&1 &&
        kc -n "$NAMESPACE" exec gateway-preflight -- \
            curl -sk -o /dev/null -w '%{http_code}' --max-time 10 -H "Host: ${host}" "${url}/health" 2>&1 || true)"
    kc -n "$NAMESPACE" delete pod gateway-preflight --ignore-not-found --wait=false >/dev/null
    if [ "${out: -3}" = 200 ]; then
        echo "  ok    a pod in ${NAMESPACE} reaches ${url} for ${host} (200)"
    else
        echo "  FAIL  a pod in ${NAMESPACE} does not reach ${url} for ${host}: ${out}"
        return 1
    fi
}

check_pods_reach_proxy() {
    check_pod_reaches_proxy "http://${PROXY_FQDN}" localhost || return 1
    if [ -n "${PUBLIC_HOST:-}" ]; then
        check_pod_reaches_proxy "https://${PROXY_FQDN}" "$MAIN_HOST" || return 1
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
    # The tunnel-origin Service of the cut-over gives way to ingress-nginx's own.
    if [ -n "$(kc -n ingress-nginx get service -l "${SHIM_LABEL}=true" -o name 2>/dev/null)" ]; then
        run kc -n ingress-nginx delete service -l "${SHIM_LABEL}=true"
    fi
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
    if grep -q '^  kind: Ingress$' "$ROLLBACK_DIR/ingresses.yaml" 2>/dev/null; then
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
    # NetworkPolicies add up: this one lets ingress-nginx through whatever the
    # others admit (a saved or later network-policy.yaml admits only Envoy).
    apply_rollback_policy "$ROLLBACK_DIR/networkpolicies.yaml"
    for policy in $(kc -n "$NAMESPACE" get networkpolicy -o name 2>/dev/null); do
        [ "${policy#*/}" != "$ROLLBACK_POLICY" ] || continue
        grep -qx "  name: ${policy#*/}" <(sed -n 's/^  \{0,2\}  name: /  name: /p' "$ROLLBACK_DIR/networkpolicies.yaml" 2>/dev/null) ||
            warn "${policy} did not exist at the cut-over and was kept; ${ROLLBACK_POLICY} admits ingress-nginx on 3000/8080/9876, check it allows anything else the Ingresses need (kubectl -n ${NAMESPACE} describe ${policy})"
    done

    log "4/4 ${HOST_URL}/ answers, and the backend reaches its database"
    wait_host_url || die "${HOST_URL}/ does not answer 200 after the rollback (check the NetworkPolicies in ${NAMESPACE} and the ingress-nginx controller's log)"
    wait_backend_db || die "POST ${HOST_URL}/graphql gets no answer from the database after the rollback (check the NetworkPolicies in ${NAMESPACE}: the backend must reach Postgres)"
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
# The public host: given, or the one the public Ingresses serve
# (scripts/deploy/lib/public-host.sh).
if [ -z "${PUBLIC_HOST+x}" ]; then
    PUBLIC_HOST="$(kc -n "$NAMESPACE" get ingress -l econgraph.io/public-host=true \
        -o jsonpath='{range .items[*]}{.spec.rules[0].host}{"\n"}{end}' 2>/dev/null | sed -n '/./{s/^www\.//;p;q;}')"
fi
[ "${PUBLIC_HOST:-}" = none ] && PUBLIC_HOST=""
if [ -n "$PUBLIC_HOST" ]; then
    PUBLIC_HOST="${PUBLIC_HOST,,}"
    # MAIN_HOST and MAIN_HOST_PERMANENT as deploy.sh resolves them (main_host_init): the
    # environment, else the deploy config for the same base domain, else www.PUBLIC_HOST.
    main_host_init || die "MAIN_HOST must be ${PUBLIC_HOST} or www.${PUBLIC_HOST}"
    reason="$(main_host_matches_keycloak)" || die "$reason; nothing was changed"
    echo "  public host: ${PUBLIC_HOST} (PUBLIC_HOST); main host: ${MAIN_HOST} (MAIN_HOST)${MAIN_HOST_PERMANENT:+; permanent redirects to ${MAIN_HOST_PERMANENT} (MAIN_HOST_PERMANENT)}"
    kc get crd certificates.cert-manager.io >/dev/null 2>&1 ||
        die "the public host needs cert-manager (for the gateway's https listener certificate); install it first"
else
    MAIN_HOST=""
    echo "  public host: none"
fi
# Host names the Ingresses serve that the HTTPRoutes don't: after step 6 the
# gateway answers them with 404.
route_hosts="$(printf 'localhost\n%s\n%s\n' "${PUBLIC_HOST}" "www.${PUBLIC_HOST}")"
unserved=""
for host in $(kc -n "$NAMESPACE" get ingress -o jsonpath='{.items[*].spec.rules[*].host}' 2>/dev/null | tr ' ' '\n' | sort -u); do
    grep -qx "$host" <<<"$route_hosts" || unserved="${unserved} ${host}"
done
if [ -n "$unserved" ]; then
    warn "the Ingresses also serve${unserved}, which the HTTPRoutes do not; the gateway will not answer for those host names"
fi
if [ -n "$nginx_image" ]; then
    # The public host's client-address settings (enable-real-ip and the rest) have
    # their counterpart in the gateway's client-traffic-policy.yaml; anything else
    # is not carried over. ($k and $v are go-template variables.)
    # shellcheck disable=SC2016
    custom="$(kc -n ingress-nginx get configmap ingress-nginx-controller \
        -o go-template='{{range $k, $v := .data}}{{$k}}={{$v}}{{"\n"}}{{end}}' 2>/dev/null |
        grep -Ev '^(allow-snippet-annotations=false|enable-real-ip=|forwarded-for-header=|proxy-real-ip-cidr=)' |
        tr '\n' ' ' || true)"
    if [ -n "$custom" ]; then
        warn "ingress-nginx's ConfigMap has settings the gateway does not take over: ${custom}(they are saved for a rollback)"
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
TUNNEL=false
if kc -n "$NAMESPACE" get deployment cloudflared >/dev/null 2>&1; then
    TUNNEL=true
    echo "  cloudflared: the tunnel's origin is kept working through step 6 (see the header)"
    [ -n "$PUBLIC_HOST" ] || warn "a cloudflared Deployment runs, but there is no public host for it to serve"
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
        # Lists in variables first: a failing command substitution in a for
        # list would not stop the script.
        cms="$(kc -n ingress-nginx get configmap -o name)"
        for cm in $cms; do
            [ "$cm" = configmap/kube-root-ca.crt ] && continue
            save_object "$STATE_DIR/ingress-nginx-objects/configmap-${cm#*/}.yaml" ingress-nginx "$cm"
        done
        secrets="$(kc -n ingress-nginx get secret -o name --field-selector type!=kubernetes.io/service-account-token)"
        for secret in $secrets; do
            [ "$secret" = secret/ingress-nginx-admission ] && continue
            save_object "$STATE_DIR/ingress-nginx-objects/secret-${secret#*/}.yaml" ingress-nginx "$secret"
        done
    )
    echo "  ${STATE_DIR}"
    for f in "$STATE_DIR"/ingress-nginx-objects/*; do echo "    ingress-nginx-objects/${f##*/}"; done
    echo "  rollback: scripts/deploy/cutover-to-gateway.sh --rollback ${STATE_DIR} --context ${KUBE_CONTEXT}"
fi

export PUBLIC_HOST MAIN_HOST
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
if ! check_routes_in_cluster || ! check_pods_reach_proxy; then
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
# A rollback's opening for ingress-nginx (see --rollback) is no longer needed.
run kc -n "$NAMESPACE" delete networkpolicy "$ROLLBACK_POLICY" --ignore-not-found
if [ "$TUNNEL" = true ]; then
    # The tunnel's dashboard origin names ingress-nginx's Service; keep that
    # name answering, now from the gateway's proxy (port 443 is passed on).
    echo "+ kubectl --context ${KUBE_CONTEXT} create namespace ingress-nginx; Service ingress-nginx-controller -> ${PROXY_FQDN} (ExternalName)"
    if [ "$DRY_RUN" != true ]; then
        cat <<EOF | kc apply -f -
apiVersion: v1
kind: Namespace
metadata:
  name: ingress-nginx
  labels:
    ${SHIM_LABEL}: "true"
---
apiVersion: v1
kind: Service
metadata:
  name: ingress-nginx-controller
  namespace: ingress-nginx
  labels:
    ${SHIM_LABEL}: "true"
spec:
  type: ExternalName
  externalName: ${PROXY_FQDN}
  ports:
  - name: http
    port: 80
  - name: https
    port: 443
EOF
    fi
fi

log "7/8 Gateway proxy onto host ports 80/443"
install_gateway true "$GATEWAY_MANIFESTS"

log "8/8 Checking every route at ${HOST_URL}"
if [ "$DRY_RUN" = true ]; then
    echo "+ scripts/deploy/test-gateway-routes.sh ${HOST_URL}"
else
    wait_host_url || warn "${HOST_URL}/ is not answering yet"
    failed=false
    "$PROJECT_ROOT/scripts/deploy/test-gateway-routes.sh" "$HOST_URL" || failed=true
    if [ -n "$PUBLIC_HOST" ]; then
        ROUTE_HOST="$MAIN_HOST" "$PROJECT_ROOT/scripts/deploy/test-gateway-routes.sh" "$HOST_HTTPS_URL" || failed=true
        check_public_redirects "$HOST_URL" "$HOST_HTTPS_URL" || failed=true
        if [ "$TUNNEL" = true ]; then
            check_pod_reaches_proxy "https://ingress-nginx-controller.ingress-nginx.svc.cluster.local" "$MAIN_HOST" || failed=true
        fi
    fi
    if [ "$failed" = true ]; then
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
if [ "$TUNNEL" = true ]; then
    echo "The tunnel reaches the gateway through Service ingress-nginx/ingress-nginx-controller (ExternalName)."
    echo "In the Cloudflare dashboard (Zero Trust > Networks > Tunnels > the tunnel > Public hostnames),"
    echo "set the service of each public hostname to"
    echo "  HTTPS  ${PROXY_FQDN}:443   (keep TLS > No TLS Verify on)"
    echo "then remove the stand-in:"
    echo "  kubectl --context ${KUBE_CONTEXT} delete namespace ingress-nginx"
fi
[ -z "$STATE_DIR" ] || echo "Rollback, if needed: scripts/deploy/cutover-to-gateway.sh --rollback ${STATE_DIR} --context ${KUBE_CONTEXT}"
