#!/bin/bash
#
# Install the cluster's HTTP entry point: the Gateway API CRDs (standard
# channel), Envoy Gateway, and the cluster-wide gateway objects in
# k8s/manifests/gateway/ (EnvoyProxy, GatewayClass, Gateway). The per-namespace
# HTTPRoutes and rate-limit policies in k8s/manifests/gateway/routes/ are
# applied by deploy.sh with the rest of the app.
#
# Usage: scripts/deploy/install-gateway.sh [--dry-run] [--context NAME]
#            [--manifests-dir DIR] [--fetch-only]
#
#   --dry-run            Print what would be applied; change nothing. Still
#                        downloads and checksums the pinned release manifests.
#   --context NAME       kubectl context to use (default: the current context).
#   --manifests-dir DIR  Keep the release manifests in DIR: use the copies
#                        there when their checksums match, download (and
#                        store) them otherwise. With all three in DIR, the
#                        script needs no network.
#   --fetch-only         Only download and check the manifests (into
#                        --manifests-dir); touch no cluster.
#
# Environment:
#   GATEWAY_HOST_PORTS      true: the proxy takes host ports 80/443. false: it
#                           doesn't, and is reachable only through its Service
#                           (cutover-to-gateway.sh uses this while ingress-nginx
#                           still holds the ports). auto (the default): true,
#                           unless another pod holds host port 80 or 443 (kind's
#                           ingress-nginx until cutover-to-gateway.sh removes it,
#                           or MicroK8s's ingress add-on).
#   GATEWAY_POD_CIDRS       The pod address ranges, space-separated, whose
#                           X-Forwarded-For the gateway trusts (the tunnel
#                           connector runs there). Default: the nodes'
#                           spec.podCIDRs, which kind sets; set it for a CNI
#                           that does not (MicroK8s's Calico: 10.1.0.0/16).
#
# The routes, and for a public host the https listener, its certificate and
# the public routes, are applied separately, by
# scripts/deploy/apply-gateway-routes.sh.
#
# Idempotent: everything is applied with server-side apply, so a re-run with
# the same versions changes nothing. Versions are exact and the downloads are
# checked against pinned SHA-256 sums; bump all three together.
#
# Version matrix (https://gateway.envoyproxy.io/news/releases/matrix/):
#   Envoy Gateway v1.9.2: built with Gateway API v1.6.1, Envoy Proxy 1.39.x,
#   Kubernetes v1.33 to v1.36, supported until 2027-02-14. The Gateway API
#   CRDs are the newest v1.6 patch, v1.6.3 (patch releases of a Gateway API
#   minor only fix the CRDs).

set -euo pipefail

GATEWAY_API_VERSION="v1.6.3"
ENVOY_GATEWAY_VERSION="v1.9.2"
GATEWAY_API_URL="https://github.com/kubernetes-sigs/gateway-api/releases/download/${GATEWAY_API_VERSION}/standard-install.yaml"
GATEWAY_API_SHA256="356c2b8e286a964dc20995435659ad1095386329974793af10bbfe3eff977fe5"
EG_CRDS_URL="https://github.com/envoyproxy/gateway/releases/download/${ENVOY_GATEWAY_VERSION}/envoy-gateway-crds.yaml"
EG_CRDS_SHA256="b339119f8f3a4046688fc96837b4129553f42c9998976913ef9dff25694d85cc"
EG_INSTALL_URL="https://github.com/envoyproxy/gateway/releases/download/${ENVOY_GATEWAY_VERSION}/install.yaml"
EG_INSTALL_SHA256="0412a72907e57ff9b73c56a7bf6df5190bf0f6e4f8bb4bba34e38630bbab5778"

FIELD_MANAGER="econ-graph-install-gateway"

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
GATEWAY_DIR="${PROJECT_ROOT}/k8s/manifests/gateway"

DRY_RUN=false
KUBE_CONTEXT=""
MANIFESTS_DIR=""
FETCH_ONLY=false
while [ $# -gt 0 ]; do
    case "$1" in
        --dry-run) DRY_RUN=true ;;
        --fetch-only) FETCH_ONLY=true ;;
        --manifests-dir)
            [ $# -ge 2 ] || { echo "error: --manifests-dir needs a value" >&2; exit 2; }
            MANIFESTS_DIR="$2"
            shift
            ;;
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

if [ "$FETCH_ONLY" = true ] && [ -z "$MANIFESTS_DIR" ]; then
    echo "error: --fetch-only needs --manifests-dir" >&2
    exit 2
fi
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

sum_ok() { [ -s "$1" ] && [ "$("${SHA256[@]}" "$1" | cut -d' ' -f1)" = "$2" ]; }

# fetch URL SHA256 FILE: FILE checked against SHA256; from --manifests-dir when
# a matching copy is there, else downloaded (and stored there).
fetch() {
    local url="$1" sum="$2" out="$3" cached=""
    if [ -n "$MANIFESTS_DIR" ]; then
        cached="$MANIFESTS_DIR/$(basename "$out")"
        if sum_ok "$cached" "$sum"; then
            cp "$cached" "$out"
            return 0
        fi
    fi
    curl -fsSL --retry 3 -o "$out" "$url" || die "download failed: $url"
    sum_ok "$out" "$sum" || die "checksum mismatch for $url"
    if [ -n "$cached" ]; then
        mkdir -p "$MANIFESTS_DIR"
        cp "$out" "$cached"
    fi
}

fetch_all() {
    fetch "$GATEWAY_API_URL" "$GATEWAY_API_SHA256" "$WORK/gateway-api-standard.yaml"
    fetch "$EG_CRDS_URL" "$EG_CRDS_SHA256" "$WORK/envoy-gateway-crds.yaml"
    fetch "$EG_INSTALL_URL" "$EG_INSTALL_SHA256" "$WORK/envoy-gateway-install.yaml"
}

if [ "$FETCH_ONLY" = true ]; then
    fetch_all
    echo "Gateway API ${GATEWAY_API_VERSION} and Envoy Gateway ${ENVOY_GATEWAY_VERSION} manifests checked in ${MANIFESTS_DIR}"
    exit 0
fi

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
if [ "$minor" -lt 33 ] || [ "$minor" -gt 36 ]; then
    die "Envoy Gateway ${ENVOY_GATEWAY_VERSION} supports Kubernetes v1.33 to v1.36, not ${server_version}"
fi

# Pods other than this gateway's proxy that hold host port 80 or 443 on a node
# labelled ingress-ready=true (where the proxy runs): kind's ingress-nginx
# until the cut-over, MicroK8s's ingress add-on (DaemonSet
# nginx-ingress-microk8s-controller in namespace ingress), or anything else. A
# proxy asking for those ports would never be scheduled.
ingress_nodes="$(kc get nodes -l ingress-ready=true -o jsonpath='{.items[*].metadata.name}' 2>/dev/null || true)"
port_holders="$(kc get pods -A -o jsonpath='{range .items[*]}{.metadata.namespace}/{.metadata.name}{" node="}{.spec.nodeName}{" owner="}{.metadata.labels.gateway\.envoyproxy\.io/owning-gateway-name}{" ports"}{range .spec.containers[*].ports[*]}{" "}{.hostPort}{end}{"\n"}{end}' 2>/dev/null |
    awk -v nodes=" ${ingress_nodes} " '
        $3 != "owner=econ-graph-gateway" && / (80|443)( |$)/ {
            node = substr($2, 6)
            if (index(nodes, " " node " ") > 0) printf "%s ", $1
        }' || true)"
port_holders="${port_holders% }"
port_advice="move the cluster to the gateway with scripts/deploy/cutover-to-gateway.sh"
case " $port_holders" in
    *" ingress/nginx-ingress-microk8s-controller"*) port_advice="run 'microk8s disable ingress', then this script again" ;;
esac
if [ "$GATEWAY_HOST_PORTS" = auto ]; then
    if [ -n "$port_holders" ]; then
        GATEWAY_HOST_PORTS=false
        echo "warning: ${port_holders} hold(s) host port 80/443; the gateway proxy runs without them. To give the ports to the gateway, ${port_advice}." >&2
    else
        GATEWAY_HOST_PORTS=true
    fi
elif [ "$GATEWAY_HOST_PORTS" = true ] && [ -n "$port_holders" ]; then
    if [ "$DRY_RUN" = true ]; then
        # A dry run of a cut-over gets here before ingress-nginx is gone.
        echo "  (dry run) ${port_holders} hold(s) host port 80/443 now; the proxy can take them only once they are gone"
    else
        die "${port_holders} hold(s) host port 80/443; ${port_advice}"
    fi
fi

fetch_all

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
# The certgen Job's template is immutable: one left from another Envoy
# Gateway version has to go before this version's can be applied.
certgen_image="$(kc -n envoy-gateway-system get job eg-gateway-helm-certgen \
    -o jsonpath='{.spec.template.spec.containers[0].image}' --ignore-not-found 2>/dev/null || true)"
if [ -n "$certgen_image" ] && [ "$certgen_image" != "envoyproxy/gateway:${ENVOY_GATEWAY_VERSION}" ]; then
    if [ "$DRY_RUN" = true ]; then
        echo "+ kubectl -n envoy-gateway-system delete job eg-gateway-helm-certgen (image ${certgen_image})"
    else
        kc -n envoy-gateway-system delete job eg-gateway-helm-certgen --wait=true
    fi
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

log "EnvoyProxy, GatewayClass, Gateway, client headers and the rate-limit burst patch"
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
# The client-address policy, trusting X-Forwarded-For from the nodes' pod
# CIDRs only.
# GATEWAY_POD_CIDRS overrides them, for a CNI that leaves the Node fields
# empty (MicroK8s's Calico).
pod_cidrs="${GATEWAY_POD_CIDRS:-}"
if [ -z "$pod_cidrs" ]; then
    pod_cidrs="$(kc get nodes -o jsonpath='{.items[*].spec.podCIDRs[*]}' 2>/dev/null || true)"
fi
[ -n "$pod_cidrs" ] ||
    die "could not read the nodes' pod CIDRs (spec.podCIDRs) for client-traffic-policy.yaml; set GATEWAY_POD_CIDRS (space-separated)"
awk -v cidrs="$pod_cidrs" '
    $0 == "      - __POD_CIDRS__" { n = split(cidrs, c, " "); for (i = 1; i <= n; i++) print "      - " c[i]; next }
    { print }' "$GATEWAY_DIR/client-traffic-policy.yaml" >"$WORK/client-traffic-policy.yaml"
grep -q '^ *- __POD_CIDRS__' "$WORK/client-traffic-policy.yaml" && die "unreplaced placeholder in client-traffic-policy.yaml"
ssa "$FIELD_MANAGER" "$WORK/client-traffic-policy.yaml"
ssa "$FIELD_MANAGER" "$GATEWAY_DIR/ratelimit-burst.yaml"

if [ "$DRY_RUN" != true ]; then
    kc -n econ-graph-gateway wait --for=condition=Programmed --timeout=300s gateway/econ-graph-gateway
    kc -n envoy-gateway-system rollout status deployment \
        -l gateway.envoyproxy.io/owning-gateway-name=econ-graph-gateway --timeout=300s 2>/dev/null ||
        kc -n envoy-gateway-system wait --for=condition=Available --timeout=300s deployment \
            -l gateway.envoyproxy.io/owning-gateway-name=econ-graph-gateway
    log "Gateway econ-graph-gateway is programmed"
    kc -n econ-graph-gateway get gateway econ-graph-gateway
fi
