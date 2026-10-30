#!/bin/bash
#
# Apply namespace econ-graph's routes and rate limits on the gateway
# (k8s/manifests/gateway/routes/), and the public host's part of the gateway.
# Every script that applies the routes uses this one, after install-gateway.sh
# and, for a public host, after cert-manager is installed.
#
# With a public host (PUBLIC_HOST, see scripts/deploy/lib/public-host.sh):
#   - listener https on the Gateway, its certificate (Secret econ-graph-tls,
#     from ClusterIssuer GATEWAY_TLS_ISSUER) and its rate-limit bursts
#     (k8s/manifests/gateway/gateway-https.yaml);
#   - the public copies of the routes, the http-to-https and www redirects,
#     and the rate limits on them (scripts/deploy/render-public-host.py).
# Without one, all of that is removed. The localhost routes are applied either
# way.
#
# Usage: scripts/deploy/apply-gateway-routes.sh [--dry-run] [--context NAME]
#
#   --dry-run       Print what would be applied or removed; change nothing.
#   --context NAME  kubectl context to use (default: the current context).
#
# Environment:
#   PUBLIC_HOST         A host name: serve it. `none` or empty: no public host;
#                       one served before is removed. Unset: keep what the
#                       cluster serves now (its public routes).
#   GATEWAY_TLS_ISSUER  ClusterIssuer for the https listener's certificate. An
#                       explicit choice: it may change the certificate's issuer.
#                       Without it: a GATEWAY_TLS_ISSUER= line in the host's deploy
#                       config (~/.config/econgraph/deploy.env), only when it matches
#                       the certificate's current issuer or there is no certificate
#                       (a recreated cluster), and refused when it differs; else the
#                       current issuer; else econ-graph-selfsigned
#                       (k8s/manifests/gateway/selfsigned-issuer.yaml, which the tunnel
#                       connector accepts unverified). See tls_issuer_choose in
#                       lib/deploy-config.sh. Read only with a public host. For
#                       econ-graph-origin-ca, the private CA and the connector's
#                       public trust bundle are staged first
#                       (scripts/deploy/stage-tunnel-origin-ca.sh --apply; idempotent),
#                       so a fresh cluster gets them before its certificate does; if
#                       staging fails, the certificate is not changed.

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
GATEWAY_DIR="${PROJECT_ROOT}/k8s/manifests/gateway"
ROUTES_DIR="${GATEWAY_DIR}/routes"
HTTPS_FIELD_MANAGER="econ-graph-gateway-https"
PUBLIC_LABEL="econgraph.io/public-host"

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

die() {
    echo "error: $*" >&2
    exit 1
}
# kubectl is KUBECTL, with the KUBECONFIG it finds. Run by hand without
# KUBECTL, it is the kind cluster's own kubectl kept in
# ${XDG_CACHE_HOME:-~/.cache}/econgraph, with KUBECONFIG if that is set and the
# kind cluster's kubeconfig kept there if not (cluster_role_default_kubectl in
# lib/cluster-role.sh); without those copies the script stops. Never plain
# `kubectl`: it may be another cluster's, such as the MicroK8s snap wrapper.
# kc below replaces the library's.
# shellcheck source=scripts/deploy/lib/cluster-role.sh
source "$PROJECT_ROOT/scripts/deploy/lib/cluster-role.sh"
cluster_role_default_kubectl || exit 1
kc() {
    if [ -n "$KUBE_CONTEXT" ]; then
        command "${KUBECTL:-kubectl}" --context "$KUBE_CONTEXT" "$@"
    else
        command "${KUBECTL:-kubectl}" "$@"
    fi
}
# Run a changing kubectl command, or only print it under --dry-run.
run_kc() {
    echo "+ kubectl${KUBE_CONTEXT:+ --context ${KUBE_CONTEXT}} $*"
    if [ "$DRY_RUN" != true ]; then
        kc "$@"
    fi
}

# What the cluster serves now; fails when it cannot be asked. Without the
# Gateway API CRDs (a dry run before install-gateway.sh) there are no routes.
current_host=""
if kc get crd httproutes.gateway.networking.k8s.io >/dev/null 2>&1; then
    current_host="$(kc -n econ-graph get httproute -l "${PUBLIC_LABEL}=true" \
        -o jsonpath='{range .items[*]}{.spec.hostnames[0]}{"\n"}{end}' 2>/dev/null |
        sed -n '/./{s/^www\.//;p;q;}')" || die "could not read the public routes"
fi
if [ -z "${PUBLIC_HOST+x}" ]; then
    host="$current_host"
else
    host="${PUBLIC_HOST}"
    [ "$host" = none ] && host=""
fi
if [ -n "$host" ]; then
    # Validates the name, as the deploy scripts do.
    "$PROJECT_ROOT/scripts/deploy/render-public-host.py" "$host" /dev/null >/dev/null ||
        die "PUBLIC_HOST='${host}' is not a public DNS name"
    kc get crd certificates.cert-manager.io >/dev/null 2>&1 ||
        die "cert-manager is not installed; the https listener's certificate needs it"
    # shellcheck source=scripts/deploy/lib/deploy-config.sh
    source "$PROJECT_ROOT/scripts/deploy/lib/deploy-config.sh"
    current_issuer="$(gateway_tls_current_issuer)" ||
        die "could not read the issuer of certificate econ-graph-tls"
    tls_issuer_choose "$current_issuer" || exit 1
    issuer="$TLS_ISSUER"
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

if [ -n "$host" ]; then
    echo "==> Public host ${host}: listener https, certificate (ClusterIssuer ${issuer}, from ${TLS_ISSUER_SOURCE}), public routes"
    if [ "$issuer" = econ-graph-selfsigned ]; then
        run_kc apply -f "$GATEWAY_DIR/selfsigned-issuer.yaml"
    elif [ "$issuer" = econ-graph-origin-ca ]; then
        # The tunnel verifies this certificate against the staged public root, so
        # the CA and trust bundle must exist before the certificate names them.
        stage_context="$KUBE_CONTEXT"
        [ -n "$stage_context" ] || stage_context="$(kc config current-context)" ||
            die "could not read the current kubectl context for staging the origin CA"
        stage=(scripts/deploy/stage-tunnel-origin-ca.sh --context "$stage_context" --apply)
        echo "+ ${stage[*]}"
        if [ "$DRY_RUN" != true ]; then
            "$PROJECT_ROOT/${stage[0]}" "${stage[@]:1}" ||
                die "staging the origin CA failed, so certificate econ-graph-tls was not changed; fix it and rerun, or run ${stage[*]} first"
        fi
    fi
    sed -e "s/__PUBLIC_HOST__/${host}/g" -e "s/__TLS_ISSUER__/${issuer}/g" \
        "$GATEWAY_DIR/gateway-https.yaml" >"$WORK/gateway-https.yaml"
    grep -q '__[A-Z_]*__' "$WORK/gateway-https.yaml" && die "unreplaced placeholder in gateway-https.yaml"
    # Server-side apply under its own field manager: adds listener https to the
    # Gateway without owning the rest (install-gateway.sh owns listener http).
    run_kc apply --server-side --force-conflicts --field-manager="$HTTPS_FIELD_MANAGER" -f "$WORK/gateway-https.yaml"
    "$PROJECT_ROOT/scripts/deploy/render-public-host.py" "$host" \
        "$ROUTES_DIR/httproutes.yaml" "$ROUTES_DIR/ratelimit-policies.yaml" >"$WORK/routes.yaml"
    run_kc apply -f "$WORK/routes.yaml"
    # Public routes for another host name, from an earlier deploy, go.
    if [ -n "$current_host" ] && [ "$current_host" != "$host" ]; then
        echo "  (the public host was ${current_host}; its routes are replaced)"
    fi
else
    echo "==> No public host: localhost routes only"
    run_kc apply -f "$ROUTES_DIR/"
    run_kc -n econ-graph delete httproute -l "${PUBLIC_LABEL}=true" --ignore-not-found
    if kc -n econ-graph-gateway get gateway econ-graph-gateway \
        -o jsonpath='{.spec.listeners[*].name}' 2>/dev/null | grep -qw https; then
        # The manager's empty part removes the listener it applied.
        cat >"$WORK/gateway-https-none.yaml" <<'EOF'
apiVersion: gateway.networking.k8s.io/v1
kind: Gateway
metadata:
  name: econ-graph-gateway
  namespace: econ-graph-gateway
EOF
        run_kc apply --server-side --force-conflicts --field-manager="$HTTPS_FIELD_MANAGER" -f "$WORK/gateway-https-none.yaml"
    fi
    run_kc -n econ-graph-gateway delete envoypatchpolicy econ-graph-ratelimit-burst-https --ignore-not-found
    if kc get crd certificates.cert-manager.io >/dev/null 2>&1; then
        run_kc -n econ-graph-gateway delete certificate econ-graph-tls --ignore-not-found
    fi
    run_kc -n econ-graph-gateway delete secret econ-graph-tls --ignore-not-found
fi
