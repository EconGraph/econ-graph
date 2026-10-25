#!/bin/bash
#
# Install the cluster's HTTP entry point: the Gateway API CRDs (standard
# channel), Envoy Gateway, and the cluster-wide gateway objects in
# k8s/manifests/gateway/ (EnvoyProxy, GatewayClass, Gateway). The per-namespace
# HTTPRoutes and rate-limit policies in k8s/manifests/gateway/routes/ are
# applied by deploy.sh with the rest of the app.
#
# Usage: scripts/deploy/install-gateway.sh [--dry-run] [--context NAME]
#
#   --dry-run       Print what would be applied; change nothing. Still
#                   downloads and checksums the pinned release manifests.
#   --context NAME  kubectl context to use (default: the current context).
#
# Environment:
#   GATEWAY_HTTPS_HOSTNAME  Adds listener `https` (port 443) for this host name,
#                           with a cert-manager certificate in Secret
#                           econ-graph-tls (k8s/manifests/gateway/gateway-https.yaml).
#                           Empty (the default): no https listener, and one
#                           added by an earlier run is removed.
#   GATEWAY_TLS_ISSUER      ClusterIssuer for that certificate (default
#                           letsencrypt-staging, k8s/manifests/letsencrypt-issuer.yaml).
#   GATEWAY_HOST_PORTS      true: the proxy takes host ports 80/443. false: it
#                           doesn't, and is reachable only through its Service
#                           (cutover-to-gateway.sh uses this while ingress-nginx
#                           still holds the ports). auto (the default): true,
#                           unless an ingress-nginx controller is installed, which
#                           holds those ports until cutover-to-gateway.sh removes it.
#
# Idempotent: everything is applied with server-side apply, so a re-run with
# the same versions changes nothing. Versions are exact and the downloads are
# checked against pinned SHA-256 sums; bump all three together.
#
# Version matrix (https://gateway.envoyproxy.io/news/releases/matrix/):
#   Envoy Gateway v1.8.5: Gateway API v1.5.1, Envoy Proxy 1.38.x,
#   Kubernetes v1.32 to v1.35. Envoy Gateway v1.9 needs Kubernetes 1.33+.

set -euo pipefail

GATEWAY_API_VERSION="v1.5.1"
ENVOY_GATEWAY_VERSION="v1.8.5"
GATEWAY_API_URL="https://github.com/kubernetes-sigs/gateway-api/releases/download/${GATEWAY_API_VERSION}/standard-install.yaml"
GATEWAY_API_SHA256="751002b3b91a87f7ae3bd2517c79a47a8d7ed6702901808a1cf9bd97d284f9b8"
EG_CRDS_URL="https://github.com/envoyproxy/gateway/releases/download/${ENVOY_GATEWAY_VERSION}/envoy-gateway-crds.yaml"
EG_CRDS_SHA256="c15e31a987284601b980f5de43ed240a389ad6af961c4e265b10c814b01b35d6"
EG_INSTALL_URL="https://github.com/envoyproxy/gateway/releases/download/${ENVOY_GATEWAY_VERSION}/install.yaml"
EG_INSTALL_SHA256="0639894d98328025b9626e5169fc11a19d107ed99bc1dd7054983978110369c9"

FIELD_MANAGER="econ-graph-install-gateway"
HTTPS_FIELD_MANAGER="econ-graph-gateway-https"

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
GATEWAY_DIR="${PROJECT_ROOT}/k8s/manifests/gateway"

DRY_RUN=false
KUBE_CONTEXT=""
while [ $# -gt 0 ]; do
    case "$1" in
        --dry-run) DRY_RUN=true ;;
        --context)
            [ $# -ge 2 ] || { echo "error: --context needs a value" >&2; exit 2; }
            KUBE_CONTEXT="$2"
            shift
            ;;
        --context=*) KUBE_CONTEXT="${1#--context=}" ;;
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

GATEWAY_HTTPS_HOSTNAME="${GATEWAY_HTTPS_HOSTNAME:-}"
GATEWAY_TLS_ISSUER="${GATEWAY_TLS_ISSUER:-letsencrypt-staging}"
GATEWAY_HOST_PORTS="${GATEWAY_HOST_PORTS:-auto}"
case "$GATEWAY_HOST_PORTS" in
    true | false | auto) ;;
    *)
        echo "error: GATEWAY_HOST_PORTS must be true, false or auto" >&2
        exit 2
        ;;
esac

log() { echo "==> $*"; }
die() {
    echo "error: $*" >&2
    exit 1
}

kc() {
    if [ -n "$KUBE_CONTEXT" ]; then
        kubectl --context "$KUBE_CONTEXT" "$@"
    else
        kubectl "$@"
    fi
}

# Apply a file with server-side apply, or print what would be applied.
ssa() {
    local manager="$1" file="$2"
    shift 2
    if [ "$DRY_RUN" = true ]; then
        echo "+ kubectl${KUBE_CONTEXT:+ --context ${KUBE_CONTEXT}} apply --server-side --field-manager=${manager}${*:+ $*} -f ${file#"${PROJECT_ROOT}/"}"
        # One line per object: kind and metadata.name.
        awk '
            /^---$/ { if (k != "") print "    " k " " n; k = ""; n = ""; m = 0; next }
            /^kind: / && k == "" { k = $2 }
            /^metadata:/ { m = 1; next }
            m && /^  name: / && n == "" { n = $2; gsub(/\x27/, "", n) }
            /^[a-z]/ && !/^metadata:/ { m = 0 }
            END { if (k != "") print "    " k " " n }
        ' "$file"
        return 0
    fi
    kc apply --server-side --force-conflicts --field-manager="$manager" "$@" -f "$file"
}

if [ -n "$GATEWAY_HTTPS_HOSTNAME" ]; then
    # One DNS name, lower case, at least one dot; no wildcards.
    [[ "$GATEWAY_HTTPS_HOSTNAME" =~ ^([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]([a-z0-9-]{0,61}[a-z0-9])?$ ]] ||
        die "GATEWAY_HTTPS_HOSTNAME='${GATEWAY_HTTPS_HOSTNAME}' is not a DNS host name"
    [[ "$GATEWAY_TLS_ISSUER" =~ ^[a-z0-9]([a-z0-9.-]*[a-z0-9])?$ ]] ||
        die "GATEWAY_TLS_ISSUER='${GATEWAY_TLS_ISSUER}' is not a resource name"
fi

command -v kubectl >/dev/null || die "kubectl not found on PATH"
command -v curl >/dev/null || die "curl not found on PATH"
if command -v sha256sum >/dev/null; then
    SHA256=(sha256sum)
elif command -v shasum >/dev/null; then
    SHA256=(shasum -a 256)
else
    die "neither sha256sum nor shasum found on PATH"
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

fetch() {
    local url="$1" sum="$2" out="$3"
    curl -fsSL --retry 3 -o "$out" "$url" || die "download failed: $url"
    [ "$("${SHA256[@]}" "$out" | cut -d' ' -f1)" = "$sum" ] || die "checksum mismatch for $url"
}

log "Gateway API ${GATEWAY_API_VERSION} (standard channel), Envoy Gateway ${ENVOY_GATEWAY_VERSION}"
if [ "$DRY_RUN" = true ]; then
    log "Dry run: nothing is changed"
fi

# The server's version only: without a reachable server, kubectl prints just
# its own.
server_version="$(kc version -o json 2>/dev/null |
    sed -n '/"serverVersion"/,$ s/.*"gitVersion": *"\(v[0-9.]*\).*/\1/p' | head -1 || true)"
[ -n "$server_version" ] || die "cannot reach the Kubernetes API server${KUBE_CONTEXT:+ (context ${KUBE_CONTEXT})}"
minor="$(echo "$server_version" | cut -d. -f2)"
echo "  Kubernetes ${server_version}"
if [ "$minor" -lt 32 ] || [ "$minor" -gt 35 ]; then
    die "Envoy Gateway ${ENVOY_GATEWAY_VERSION} supports Kubernetes v1.32 to v1.35, not ${server_version}"
fi

# Host ports 80/443 are held by ingress-nginx's controller while it is
# installed; a proxy asking for them would never be scheduled.
nginx_present=false
if [ -n "$(kc -n ingress-nginx get deployment ingress-nginx-controller -o name --ignore-not-found 2>/dev/null)" ]; then
    nginx_present=true
fi
if [ "$GATEWAY_HOST_PORTS" = auto ]; then
    if [ "$nginx_present" = true ]; then
        GATEWAY_HOST_PORTS=false
        echo "warning: ingress-nginx is installed and holds host ports 80/443; the gateway proxy runs without them. Move the cluster to the gateway with scripts/deploy/cutover-to-gateway.sh." >&2
    else
        GATEWAY_HOST_PORTS=true
    fi
elif [ "$GATEWAY_HOST_PORTS" = true ] && [ "$nginx_present" = true ]; then
    if [ "$DRY_RUN" = true ]; then
        # A dry run of a cut-over gets here before ingress-nginx is gone.
        echo "  (dry run) ingress-nginx is installed now; the proxy can take host ports 80/443 only once it is removed"
    else
        die "ingress-nginx is installed and holds host ports 80/443; remove it first (scripts/deploy/cutover-to-gateway.sh)"
    fi
fi

fetch "$GATEWAY_API_URL" "$GATEWAY_API_SHA256" "$WORK/gateway-api-standard.yaml"
fetch "$EG_CRDS_URL" "$EG_CRDS_SHA256" "$WORK/envoy-gateway-crds.yaml"
fetch "$EG_INSTALL_URL" "$EG_INSTALL_SHA256" "$WORK/envoy-gateway-install.yaml"

# install.yaml bundles the Gateway API CRDs from the experimental channel, plus
# Envoy Gateway's own CRDs. Drop every CRD (and the Gateway API's
# safe-upgrades admission policy, which the standard bundle carries) so that
# the cluster gets the standard channel only; the Envoy Gateway CRDs come from
# envoy-gateway-crds.yaml. Documents are separated by lines of exactly `---`.
awk '
    function flush() {
        if (doc != "" && doc !~ /\nkind: (CustomResourceDefinition|ValidatingAdmissionPolicy|ValidatingAdmissionPolicyBinding)\n/)
            printf "---%s", doc
        doc = ""
    }
    /^---$/ { flush(); doc = "\n"; next }
    { doc = doc $0 "\n" }
    END { flush() }
' "$WORK/envoy-gateway-install.yaml" >"$WORK/envoy-gateway-controller.yaml"
if grep -q '^kind: CustomResourceDefinition' "$WORK/envoy-gateway-controller.yaml"; then
    die "filtering the CRDs out of install.yaml failed"
fi
grep -q '^kind: Deployment' "$WORK/envoy-gateway-controller.yaml" ||
    die "the filtered install.yaml has no Deployment; its layout changed"
# Envoy Gateway's own configuration (ConfigMap envoy-gateway-config): allow
# EnvoyPatchPolicy, which k8s/manifests/gateway/ratelimit-burst.yaml needs.
awk '$0 == "    extensionApis: {}" { print "    extensionApis:"; print "      enableEnvoyPatchPolicy: true"; next } { print }' \
    "$WORK/envoy-gateway-controller.yaml" >"$WORK/envoy-gateway-controller.yaml.new"
mv "$WORK/envoy-gateway-controller.yaml.new" "$WORK/envoy-gateway-controller.yaml"
grep -q '^      enableEnvoyPatchPolicy: true$' "$WORK/envoy-gateway-controller.yaml" ||
    die "could not enable EnvoyPatchPolicy in envoy-gateway-config; install.yaml's layout changed"

# An experimental-channel CRD already in the cluster would be silently kept by
# the standard bundle's server-side apply only for fields it does not set;
# refuse instead, so the channel is never mixed.
if [ "$DRY_RUN" != true ]; then
    channel="$(kc get crd gateways.gateway.networking.k8s.io \
        -o jsonpath='{.metadata.annotations.gateway\.networking\.k8s\.io/channel}' 2>/dev/null || true)"
    if [ -n "$channel" ] && [ "$channel" != "standard" ]; then
        die "the Gateway API CRDs in this cluster are from the '${channel}' channel; remove them before installing the standard channel"
    fi
fi

log "Gateway API CRDs"
ssa "$FIELD_MANAGER" "$WORK/gateway-api-standard.yaml"
log "Envoy Gateway CRDs"
ssa "$FIELD_MANAGER" "$WORK/envoy-gateway-crds.yaml"
if [ "$DRY_RUN" != true ]; then
    kc wait --for=condition=Established --timeout=120s \
        crd/gateways.gateway.networking.k8s.io crd/httproutes.gateway.networking.k8s.io \
        crd/envoyproxies.gateway.envoyproxy.io crd/backendtrafficpolicies.gateway.envoyproxy.io
fi

log "Envoy Gateway controller (namespace envoy-gateway-system)"
config_before=""
if [ "$DRY_RUN" != true ]; then
    config_before="$(kc -n envoy-gateway-system get configmap envoy-gateway-config \
        -o jsonpath='{.data.envoy-gateway\.yaml}' --ignore-not-found 2>/dev/null || true)"
fi
ssa "$FIELD_MANAGER" "$WORK/envoy-gateway-controller.yaml"
if [ "$DRY_RUN" != true ]; then
    config_after="$(kc -n envoy-gateway-system get configmap envoy-gateway-config -o jsonpath='{.data.envoy-gateway\.yaml}')"
    # The controller reads its configuration only at start.
    if [ -n "$config_before" ] && [ "$config_before" != "$config_after" ]; then
        log "envoy-gateway-config changed: restarting the controller"
        kc -n envoy-gateway-system rollout restart deployment/envoy-gateway
    fi
    kc -n envoy-gateway-system rollout status deployment/envoy-gateway --timeout=300s
fi

log "EnvoyProxy, GatewayClass, Gateway and the rate-limit burst patch"
if [ "$GATEWAY_HOST_PORTS" = true ]; then
    ssa "$FIELD_MANAGER" "$GATEWAY_DIR/envoy-proxy.yaml"
else
    # The same EnvoyProxy without its Deployment patch, which is what adds
    # the hostPorts (the patch is the last block of the file).
    log "GATEWAY_HOST_PORTS=false: the proxy does not take host ports 80/443"
    sed '/^        patch:$/,$d' "$GATEWAY_DIR/envoy-proxy.yaml" >"$WORK/envoy-proxy-no-host-ports.yaml"
    if grep -q '^ *hostPort:' "$WORK/envoy-proxy-no-host-ports.yaml" ||
        ! grep -q 'name: econ-graph-gateway-proxy' "$WORK/envoy-proxy-no-host-ports.yaml"; then
        die "could not remove the host ports from envoy-proxy.yaml; its layout changed"
    fi
    ssa "$FIELD_MANAGER" "$WORK/envoy-proxy-no-host-ports.yaml"
fi
ssa "$FIELD_MANAGER" "$GATEWAY_DIR/gatewayclass.yaml"
ssa "$FIELD_MANAGER" "$GATEWAY_DIR/gateway.yaml"
ssa "$FIELD_MANAGER" "$GATEWAY_DIR/ratelimit-burst.yaml"

# The https listener is a second field manager's part of the same Gateway:
# server-side apply merges listeners by name, so gateway.yaml never removes
# it, and applying the manager's empty part removes it again.
if [ -n "$GATEWAY_HTTPS_HOSTNAME" ]; then
    log "Listener https for ${GATEWAY_HTTPS_HOSTNAME} (certificate from ClusterIssuer ${GATEWAY_TLS_ISSUER})"
    sed -e "s/__GATEWAY_HTTPS_HOSTNAME__/${GATEWAY_HTTPS_HOSTNAME}/g" \
        -e "s/__GATEWAY_TLS_ISSUER__/${GATEWAY_TLS_ISSUER}/g" \
        "$GATEWAY_DIR/gateway-https.yaml" >"$WORK/gateway-https.yaml"
    if grep -q '__GATEWAY_' "$WORK/gateway-https.yaml"; then
        die "unreplaced placeholder in gateway-https.yaml"
    fi
    ssa "$HTTPS_FIELD_MANAGER" "$WORK/gateway-https.yaml"
else
    log "No GATEWAY_HTTPS_HOSTNAME: listener https not configured"
    cat >"$WORK/gateway-https-none.yaml" <<'EOF'
apiVersion: gateway.networking.k8s.io/v1
kind: Gateway
metadata:
  name: econ-graph-gateway
  namespace: econ-graph-gateway
EOF
    ssa "$HTTPS_FIELD_MANAGER" "$WORK/gateway-https-none.yaml"
    if [ "$DRY_RUN" = true ]; then
        echo "+ kubectl${KUBE_CONTEXT:+ --context ${KUBE_CONTEXT}} -n econ-graph-gateway delete envoypatchpolicy econ-graph-ratelimit-burst-https --ignore-not-found"
    else
        kc -n econ-graph-gateway delete envoypatchpolicy econ-graph-ratelimit-burst-https --ignore-not-found
    fi
fi

if [ "$DRY_RUN" != true ]; then
    kc -n econ-graph-gateway wait --for=condition=Programmed --timeout=300s gateway/econ-graph-gateway
    kc -n envoy-gateway-system rollout status deployment \
        -l gateway.envoyproxy.io/owning-gateway-name=econ-graph-gateway --timeout=300s 2>/dev/null ||
        kc -n envoy-gateway-system wait --for=condition=Available --timeout=300s deployment \
            -l gateway.envoyproxy.io/owning-gateway-name=econ-graph-gateway
    log "Gateway econ-graph-gateway is programmed"
    kc -n econ-graph-gateway get gateway econ-graph-gateway
fi
