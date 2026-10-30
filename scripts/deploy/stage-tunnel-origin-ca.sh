#!/usr/bin/env bash
# Stage a private origin CA and public trust bundle for the remotely managed tunnel.
# Dry run is the default and contacts no cluster. --apply creates the cert-manager
# resources, copies only the public CA certificate into a ConfigMap, and patches
# the cloudflared mount, then restarts connectors and waits for their rollout. It
# leaves replicas and token intact. When the ConfigMap already holds that
# certificate and the mount is in place, it changes nothing and restarts nothing.
# Without a cloudflared Deployment yet, it publishes the CA and ConfigMap only; the
# normal connector manifest carries the same optional mount.
# A published root is replaced only by a renewal of itself (same subject and public
# key); a published bundle that still contains this root among others (a two-root
# rotation) is left unchanged; any other root needs the two-root rotation runbook.
# It does not switch the gateway issuer or Cloudflare dashboard TLS settings;
# apply-gateway-routes.sh runs it when that issuer is econ-graph-origin-ca.
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

if [[ "$apply" != true ]]; then
    printf 'Dry run on context %q: apply %q\n' "$context" "$resources/ca.yaml"
    echo 'Wait for cert-manager CA readiness; copy only Secret tls.crt to ConfigMap econ-graph/cloudflared-origin-ca.'
    echo 'Refuse a different published root unless it is a renewal (same subject and public key).'
    echo 'Patch only the cloudflared public-CA mount; leave replicas, token, gateway issuer and dashboard unchanged.'
    echo 'Restart cloudflared and wait for its rollout so every connector receives the public CA file,'
    echo 'unless the ConfigMap and mount were already in place, or there is no cloudflared Deployment yet.'
    echo 'After staging, follow docs/deployment/TUNNEL_ORIGIN_TLS.md for the verified cutover.'
    exit 0
fi

# kubectl is KUBECTL, with the KUBECONFIG it finds. Without KUBECTL it is the
# kind cluster's own kubectl kept in ${XDG_CACHE_HOME:-~/.cache}/econgraph
# (cluster_role_default_kubectl in lib/cluster-role.sh); without those copies the
# script stops. Never plain `kubectl`: it may be another cluster's, such as the
# MicroK8s snap wrapper. kc below replaces the library's.
# shellcheck source=scripts/deploy/lib/cluster-role.sh
source "$project_root/scripts/deploy/lib/cluster-role.sh"
cluster_role_default_kubectl || exit 1
# Run kubectl only against the context explicitly selected by the operator.
kc() { command "$KUBECTL" --context "$context" "$@"; }

for tool in "$KUBECTL" openssl base64; do
    command -v "$tool" >/dev/null || { echo "error: missing tool: $tool" >&2; exit 1; }
done
# A failed lookup stops staging; a missing Deployment only skips the mount and restart.
connector="$(kc -n econ-graph get deployment cloudflared --ignore-not-found -o name)"
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
# running tunnel. A renewal of the published root (rotationPolicy: Never keeps its
# key) has the same subject and public key, so leaves it signed still verify and it
# may replace the published one. A bundle of several roots that still contains this
# one exactly (a two-root rotation in progress) is left as it is. Anything else
# needs the documented two-root rotation procedure.
kc -n econ-graph get configmap cloudflared-origin-ca --ignore-not-found \
    -o 'jsonpath={.data.ca\.crt}' >"$work/existing-ca.crt"
# Prints the subject and public key of a file holding exactly one certificate.
root_identity() {
    [[ "$(grep -c -- '-----BEGIN CERTIFICATE-----' "$1")" == 1 ]] || return 1
    openssl x509 -in "$1" -noout -subject -nameopt RFC2253 &&
        openssl x509 -in "$1" -noout -pubkey
}
# Split the published bundle into one file per certificate, in $work/bundle.
rm -rf "$work/bundle" && mkdir "$work/bundle"
awk -v dir="$work/bundle" '
    /-----BEGIN CERTIFICATE-----/ { n++; out = sprintf("%s/%03d.pem", dir, n); inside = 1 }
    inside { print > out }
    /-----END CERTIFICATE-----/ { inside = 0; close(out) }' "$work/existing-ca.crt"
current_fingerprint="$(openssl x509 -in "$work/ca.crt" -noout -fingerprint -sha256)"
# Nothing removes a root from the bundle by itself: list every published root other
# than the current one, so a retired root is noticed (TUNNEL_ORIGIN_TLS.md says how
# to drop it).
other_roots=0
for cert in "$work/bundle"/*.pem; do
    [[ -e "$cert" ]] || continue
    fingerprint="$(openssl x509 -in "$cert" -noout -fingerprint -sha256 2>/dev/null)" ||
        fingerprint="(unreadable certificate)"
    [[ "$fingerprint" != "$current_fingerprint" ]] || continue
    ((other_roots += 1))
    ((other_roots > 1)) || echo 'Published trust bundle also holds roots other than the current one:'
    echo "  subject:     $(openssl x509 -in "$cert" -noout -subject -nameopt RFC2253 2>/dev/null)"
    echo "  fingerprint: ${fingerprint}"
    echo "  expires:     $(openssl x509 -in "$cert" -noout -enddate 2>/dev/null)"
done
# Whether the published bundle contains the current root (the same SHA-256 fingerprint).
bundle_contains_current() {
    local cert fingerprint
    for cert in "$work/bundle"/*.pem; do
        [[ -e "$cert" ]] || continue
        fingerprint="$(openssl x509 -in "$cert" -noout -fingerprint -sha256 2>/dev/null)" || continue
        [[ "$fingerprint" == "$current_fingerprint" ]] && return 0
    done
    return 1
}
bundle_changed=false
if [[ ! -s "$work/existing-ca.crt" ]]; then
    bundle_changed=true
elif ! cmp -s "$work/ca.crt" "$work/existing-ca.crt"; then
    if new_id="$(root_identity "$work/ca.crt" 2>/dev/null)" &&
        old_id="$(root_identity "$work/existing-ca.crt" 2>/dev/null)" &&
        [[ "$new_id" == "$old_id" ]]; then
        echo 'Published root renewed (same subject and public key); replacing it.'
        bundle_changed=true
    elif bundle_contains_current; then
        echo 'Trust bundle holds this root among others (a two-root rotation); left unchanged.'
    else
        echo 'error: existing trust bundle differs; follow the two-root rotation runbook' >&2
        exit 1
    fi
fi
if [[ "$bundle_changed" == true ]]; then
    kc -n econ-graph create configmap cloudflared-origin-ca --from-file="ca.crt=$work/ca.crt" \
        --dry-run=client -o yaml | kc apply -f -
fi
if [[ -z "$connector" ]]; then
    echo 'No cloudflared Deployment yet: its manifest mounts this ConfigMap when it is applied.'
else
    volume="$(kc -n econ-graph get deployment cloudflared \
        -o 'jsonpath={.spec.template.spec.volumes[?(@.name=="origin-ca")].configMap.name}')"
    mount="$(kc -n econ-graph get deployment cloudflared \
        -o 'jsonpath={.spec.template.spec.containers[?(@.name=="cloudflared")].volumeMounts[?(@.name=="origin-ca")].mountPath}')"
    if [[ "$bundle_changed" == false && "$volume" == cloudflared-origin-ca &&
        "$mount" == /etc/cloudflared/origin-ca ]]; then
        echo 'Trust bundle and connector mount already in place; cloudflared not restarted.'
    else
        kc -n econ-graph patch deployment cloudflared --type=strategic \
            --patch-file "$resources/cloudflared-mount.yaml"
        # The mount may already exist, so patching alone need not create new Pods.
        # Restart after publishing the ConfigMap so staging cannot finish before
        # kubelet projects the certificate into every running connector.
        kc -n econ-graph rollout restart deployment/cloudflared
        kc -n econ-graph rollout status deployment/cloudflared --timeout=120s
    fi
fi
cat <<'NEXT'
Public CA staged. No gateway issuer or Cloudflare dashboard setting was changed.
Follow docs/deployment/TUNNEL_ORIGIN_TLS.md to select the gateway issuer, verify
certificate chain/name, and then set Origin Server Name, CA Pool and TLS verification.
NEXT
