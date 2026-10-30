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
# --retire-other-roots ends a two-root rotation: it republishes the bundle with
# only the root that the gateway certificate econ-graph-tls is issued from (its
# ClusterIssuer's CA Secret, public tls.crt only) and restarts cloudflared. Before
# changing anything it refuses an empty or multi-certificate root, one that is not
# a self-signed CA, a gateway certificate that does not verify against it, and a
# bundle that does not already contain it. Its dry run (the default) reads the
# cluster, makes the same checks and lists the roots it would drop.
#
# Usage: stage-tunnel-origin-ca.sh --context NAME [--retire-other-roots] [--dry-run | --apply]
set -euo pipefail
umask 077

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
resources="$project_root/k8s/manifests/tunnel-trust"
context=""
apply=false
retire=false
while (($#)); do
    case "$1" in
        --context)
            (($# >= 2)) && [[ -n "$2" && "$2" != --* ]] || {
                echo 'error: --context needs a value' >&2; exit 2;
            }
            context="$2"; shift ;;
        --dry-run) apply=false ;;
        --apply) apply=true ;;
        --retire-other-roots) retire=true ;;
        -h|--help) sed -n '2,/^set /{ /^set /d; s/^# \{0,1\}//; p; }' "$0"; exit 0 ;;
        *) echo "error: unknown argument: $1" >&2; exit 2 ;;
    esac
    shift
done
[[ -n "$context" ]] || { echo 'error: choose an explicit --context' >&2; exit 2; }

if [[ "$apply" != true && "$retire" != true ]]; then
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
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# split_bundle FILE: one file per certificate in FILE, in $work/bundle.
split_bundle() {
    rm -rf "$work/bundle" && mkdir "$work/bundle"
    awk -v dir="$work/bundle" '
        /-----BEGIN CERTIFICATE-----/ { n++; out = sprintf("%s/%03d.pem", dir, n); inside = 1 }
        inside { print > out }
        /-----END CERTIFICATE-----/ { inside = 0; close(out) }' "$1"
}
fingerprint_of() { openssl x509 -in "$1" -noout -fingerprint -sha256 2>/dev/null; }
# bundle_has FINGERPRINT: whether the split bundle holds that certificate.
bundle_has() {
    local cert
    for cert in "$work/bundle"/*.pem; do
        [[ -e "$cert" ]] || continue
        [[ "$(fingerprint_of "$cert" || true)" == "$1" ]] && return 0
    done
    return 1
}
# list_other_roots FINGERPRINT HEADING: print the subject, fingerprint and expiry of
# every certificate in the split bundle other than FINGERPRINT, under HEADING; sets
# other_roots to their number.
list_other_roots() {
    local cert fingerprint
    other_roots=0
    for cert in "$work/bundle"/*.pem; do
        [[ -e "$cert" ]] || continue
        fingerprint="$(fingerprint_of "$cert")" || fingerprint="(unreadable certificate)"
        [[ "$fingerprint" != "$1" ]] || continue
        ((other_roots += 1))
        ((other_roots > 1)) || echo "$2"
        echo "  subject:     $(openssl x509 -in "$cert" -noout -subject -nameopt RFC2253 2>/dev/null)"
        echo "  fingerprint: ${fingerprint}"
        echo "  expires:     $(openssl x509 -in "$cert" -noout -enddate 2>/dev/null)"
    done
}
# is_self_signed_ca FILE: exactly one certificate, CA:TRUE, verifying against itself.
is_self_signed_ca() {
    [[ "$(grep -c -- '-----BEGIN CERTIFICATE-----' "$1")" == 1 ]] &&
        openssl x509 -in "$1" -noout -ext basicConstraints 2>/dev/null | grep -q 'CA:TRUE' &&
        openssl verify -CAfile "$1" "$1" >/dev/null 2>&1
}
publish_and_restart() { # FILE: publish FILE as the bundle, restart and wait.
    kc -n econ-graph create configmap cloudflared-origin-ca --from-file="ca.crt=$1" \
        --dry-run=client -o yaml | kc apply -f -
    kc -n econ-graph rollout restart deployment/cloudflared
    kc -n econ-graph rollout status deployment/cloudflared --timeout=120s
}

if [[ "$retire" == true ]]; then
    refuse() { echo "error: $*; nothing was changed" >&2; exit 1; }
    # The root the live gateway certificate is issued from: its ClusterIssuer's CA Secret.
    issuer_kind="$(kc -n econ-graph-gateway get certificate econ-graph-tls -o 'jsonpath={.spec.issuerRef.kind}')"
    issuer="$(kc -n econ-graph-gateway get certificate econ-graph-tls -o 'jsonpath={.spec.issuerRef.name}')"
    [[ "$issuer_kind" == ClusterIssuer && -n "$issuer" ]] ||
        refuse "certificate econ-graph-tls is not issued by a ClusterIssuer (${issuer_kind:-no kind}/${issuer:-no name})"
    ca_secret="$(kc get clusterissuer "$issuer" -o 'jsonpath={.spec.ca.secretName}')"
    [[ -n "$ca_secret" ]] || refuse "ClusterIssuer ${issuer} is not a CA issuer (no spec.ca.secretName)"
    # Only the public certificate field, decoded to a file; an empty one is refused.
    kc -n cert-manager get secret "$ca_secret" -o 'jsonpath={.data.tls\.crt}' |
        base64 --decode >"$work/root.crt" ||
        refuse "could not read the public certificate of Secret cert-manager/${ca_secret}"
    [[ -s "$work/root.crt" ]] || refuse "Secret cert-manager/${ca_secret} has no public certificate"
    is_self_signed_ca "$work/root.crt" ||
        refuse "Secret cert-manager/${ca_secret} does not hold exactly one self-signed CA certificate"
    # The live gateway certificate (public tls.crt only) must verify against it.
    kc -n econ-graph-gateway get secret econ-graph-tls -o 'jsonpath={.data.tls\.crt}' |
        base64 --decode >"$work/gateway.crt" ||
        refuse "could not read the public certificate of Secret econ-graph-gateway/econ-graph-tls"
    [[ -s "$work/gateway.crt" ]] || refuse "Secret econ-graph-gateway/econ-graph-tls has no certificate"
    openssl verify -CAfile "$work/root.crt" "$work/gateway.crt" >/dev/null 2>&1 ||
        refuse "the gateway certificate econ-graph-tls does not verify against the root of ClusterIssuer ${issuer}"
    kc -n econ-graph get configmap cloudflared-origin-ca --ignore-not-found \
        -o 'jsonpath={.data.ca\.crt}' >"$work/existing-ca.crt"
    split_bundle "$work/existing-ca.crt"
    keep="$(fingerprint_of "$work/root.crt")"
    bundle_has "$keep" ||
        refuse "the published bundle does not contain the root of ClusterIssuer ${issuer}; connectors do not trust it yet"
    echo "Keeping the root of ClusterIssuer ${issuer} (${keep})."
    list_other_roots "$keep" 'Roots to drop from the published trust bundle:'
    if ((other_roots == 0)); then
        echo 'The bundle holds only that root; nothing to retire.'
        exit 0
    fi
    if [[ "$apply" != true ]]; then
        echo 'Dry run: would publish only that root to ConfigMap econ-graph/cloudflared-origin-ca, restart cloudflared and wait for its rollout. Rerun with --apply.'
        exit 0
    fi
    publish_and_restart "$work/root.crt"
    echo 'Retired the other roots. Repeat the positive origin check (docs/deployment/TUNNEL_ORIGIN_TLS.md).'
    exit 0
fi

# A failed lookup stops staging; a missing Deployment only skips the mount and restart.
connector="$(kc -n econ-graph get deployment cloudflared --ignore-not-found -o name)"
kc apply -f "$resources/ca.yaml"
kc -n cert-manager wait --for=condition=Ready certificate/econ-graph-origin-ca --timeout=120s
kc wait --for=condition=Ready clusterissuer/econ-graph-origin-ca --timeout=120s
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
split_bundle "$work/existing-ca.crt"
current_fingerprint="$(fingerprint_of "$work/ca.crt")"
bundle_changed=false
if [[ ! -s "$work/existing-ca.crt" ]]; then
    bundle_changed=true
elif ! cmp -s "$work/ca.crt" "$work/existing-ca.crt"; then
    if new_id="$(root_identity "$work/ca.crt" 2>/dev/null)" &&
        old_id="$(root_identity "$work/existing-ca.crt" 2>/dev/null)" &&
        [[ "$new_id" == "$old_id" ]]; then
        echo 'Published root renewed (same subject and public key); replacing it.'
        bundle_changed=true
    elif bundle_has "$current_fingerprint"; then
        echo 'Trust bundle holds this root among others (a two-root rotation); left unchanged.'
        # Nothing removes a root from the bundle by itself: list the others, so a
        # retired one is noticed (--retire-other-roots drops them).
        list_other_roots "$current_fingerprint" \
            'Published trust bundle also holds roots other than the current one:'
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
