#!/usr/bin/env bash
# Stage a private origin CA and public trust bundle for the remotely managed tunnel.
# Dry run is the default and contacts no cluster. --apply creates the cert-manager
# resources, copies only the public CA certificate into a ConfigMap, and patches
# the cloudflared mount without changing its replicas or token. It does not switch
# the gateway issuer or Cloudflare dashboard TLS settings.
#
# Usage: stage-tunnel-origin-ca.sh --context NAME [--dry-run | --apply]
set -euo pipefail
umask 077

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
resources="$project_root/k8s/manifests/tunnel-trust"
context=""
apply=false
while (($#)); do
    case "$1" in
        --context)
            (($# >= 2)) && [[ -n "$2" && "$2" != --* ]] || {
                echo 'error: --context needs a value' >&2; exit 2;
            }
            context="$2"; shift ;;
        --dry-run) apply=false ;;
        --apply) apply=true ;;
        -h|--help) sed -n '2,/^set /{ /^set /d; s/^# \{0,1\}//; p; }' "$0"; exit 0 ;;
        *) echo "error: unknown argument: $1" >&2; exit 2 ;;
    esac
    shift
done
[[ -n "$context" ]] || { echo 'error: choose an explicit --context' >&2; exit 2; }
kc() { command "${KUBECTL:-kubectl}" --context "$context" "$@"; }

if [[ "$apply" != true ]]; then
    printf 'Dry run on context %q: apply %q\n' "$context" "$resources/ca.yaml"
    echo 'Wait for cert-manager CA readiness; copy only Secret tls.crt to ConfigMap econ-graph/cloudflared-origin-ca.'
    echo 'Patch only the cloudflared public-CA mount; leave replicas, token, gateway issuer and dashboard unchanged.'
    echo 'After staging, follow docs/deployment/TUNNEL_ORIGIN_TLS.md for the verified cutover.'
    exit 0
fi

for tool in "${KUBECTL:-kubectl}" openssl base64; do
    command -v "$tool" >/dev/null || { echo "error: missing tool: $tool" >&2; exit 1; }
done
# An explicitly selected context is still required; never silently switch contexts.
kc -n econ-graph get deployment cloudflared -o name >/dev/null
kc apply -f "$resources/ca.yaml"
kc -n cert-manager wait --for=condition=Ready certificate/econ-graph-origin-ca --timeout=120s
kc wait --for=condition=Ready clusterissuer/econ-graph-origin-ca --timeout=120s
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
# Fetch only the public certificate field. Never request the full Secret or tls.key.
kc -n cert-manager get secret econ-graph-origin-ca -o 'jsonpath={.data.tls\.crt}' |
    base64 --decode >"$work/ca.crt"
openssl x509 -in "$work/ca.crt" -noout -checkend 7776000 >/dev/null
openssl x509 -in "$work/ca.crt" -noout -ext basicConstraints | grep -q 'CA:TRUE' || {
    echo 'error: signing certificate is not a CA' >&2; exit 1;
}
openssl verify -CAfile "$work/ca.crt" "$work/ca.crt" >/dev/null
# Replacing an active trust root before the gateway leaf is rotated can break the
# running tunnel. A changed root needs the documented two-root rotation procedure.
kc -n econ-graph get configmap cloudflared-origin-ca --ignore-not-found \
    -o 'jsonpath={.data.ca\.crt}' >"$work/existing-ca.crt"
if [[ -s "$work/existing-ca.crt" ]] && ! cmp -s "$work/ca.crt" "$work/existing-ca.crt"; then
    echo 'error: existing trust bundle differs; follow the two-root rotation runbook' >&2
    exit 1
fi
kc -n econ-graph create configmap cloudflared-origin-ca --from-file="ca.crt=$work/ca.crt" \
    --dry-run=client -o yaml | kc apply -f -
kc -n econ-graph patch deployment cloudflared --type=strategic \
    --patch-file "$resources/cloudflared-mount.yaml"
kc -n econ-graph rollout status deployment/cloudflared --timeout=120s
cat <<'NEXT'
Public CA staged. No gateway issuer or Cloudflare dashboard setting was changed.
Follow docs/deployment/TUNNEL_ORIGIN_TLS.md to select the gateway issuer, verify
certificate chain/name, and then set Origin Server Name, CA Pool and TLS verification.
NEXT
