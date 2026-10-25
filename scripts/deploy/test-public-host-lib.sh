#!/usr/bin/env bash
# The stubs below are called by the sourced library; the scripts given to run_lib are
# expanded there, not here.
# shellcheck disable=SC2329,SC2016
# Tests for scripts/deploy/lib/public-host.sh against a stub kubectl and a stub
# apply-gateway-routes.sh, no cluster:
#   - PUBLIC_HOST unset on a cluster that serves a public host (public HTTPRoutes, or on a
#     cluster still on ingress-nginx the public Ingresses or the retired
#     econ-graph-ssl-ingress) stops the deploy, and so does a failed lookup;
#   - apply_public_edge hands the public host to apply-gateway-routes.sh, or `none`
#     without one;
#   - cloudflared gets 0 replicas without its Secret or without a public host.
#
# Usage: scripts/deploy/test-public-host-lib.sh (from anywhere)
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$repo_root"
failures=0
fail() {
  echo "FAIL $1"
  failures=$((failures + 1))
}

# run_lib SCRIPT: runs SCRIPT in a subshell with the library, a stub kubectl and a stub
# apply-gateway-routes.sh. The library calls ./scripts/deploy/apply-gateway-routes.sh,
# so SCRIPT runs in a copy of the files it needs, with that script replaced.
# STUB_ROUTE_HOST / STUB_INGRESS_HOST / STUB_SSL_HOSTS are what the cluster serves;
# STUB_SECRET=1 means cloudflare-tunnel exists. Every kubectl call (and stdin piped to
# apply) and every apply-gateway-routes.sh call is appended to $log.
log="$(mktemp)"
work="$(mktemp -d)"
trap 'rm -rf "$log" "$work"' EXIT
mkdir -p "$work/scripts/deploy/lib" "$work/k8s/manifests"
cp scripts/deploy/lib/public-host.sh "$work/scripts/deploy/lib/"
cp scripts/deploy/render-public-host.py "$work/scripts/deploy/"
cp k8s/manifests/cloudflared.yaml "$work/k8s/manifests/"
cat >"$work/scripts/deploy/apply-gateway-routes.sh" <<EOF
#!/bin/bash
echo "apply-gateway-routes.sh PUBLIC_HOST=\${PUBLIC_HOST-<unset>}" >>"$log"
EOF
chmod +x "$work/scripts/deploy/apply-gateway-routes.sh"

run_lib() {
  : >"$log"
  (
    cd "$work"
    kubectl() {
      echo "kubectl $*" >>"$log"
      case "$*" in
        *"get namespace econ-graph"*)
          [ "${STUB_LOOKUP_FAILS:-0}" != 1 ] || return 1
          [ "${STUB_NO_NAMESPACE:-0}" = 1 ] || echo namespace/econ-graph ;;
        *"get crd httproutes"*) [ "${STUB_NO_GATEWAY_API:-0}" != 1 ] ;;
        *"get httproute -l"*) printf '%s' "${STUB_ROUTE_HOST:-}" ;;
        *"get ingress -l"*) printf '%s' "${STUB_INGRESS_HOST:-}" ;;
        *"get ingress econ-graph-ssl-ingress"*) printf '%s' "${STUB_SSL_HOSTS:-}" ;;
        *"get secret cloudflare-tunnel"*) [ "${STUB_SECRET:-0}" = 1 ] ;;
        *"apply -f -"*) sed 's/^/  stdin: /' >>"$log" ;;
      esac
    }
    yq() { python3 -c 'import sys,yaml; d=yaml.safe_load(open(sys.argv[1])); d["spec"]["replicas"]=0; print(yaml.safe_dump(d))' "$2"; }
    # shellcheck source=scripts/deploy/lib/public-host.sh
    source scripts/deploy/lib/public-host.sh
    eval "$1"
  ) >/dev/null 2>&1
}

# Unset on a cluster whose public routes serve econgraph.com: stop.
if (unset PUBLIC_HOST; STUB_ROUTE_HOST=econgraph.com run_lib public_host_init); then
  fail "PUBLIC_HOST unset on a public cluster did not stop"
fi
# Unset on a cluster still on ingress-nginx with public Ingresses: stop.
if (unset PUBLIC_HOST; STUB_INGRESS_HOST=econgraph.com run_lib public_host_init); then
  fail "PUBLIC_HOST unset on a cluster with public Ingresses did not stop"
fi
# ... or without the Gateway API CRDs yet.
if (unset PUBLIC_HOST; STUB_NO_GATEWAY_API=1 STUB_INGRESS_HOST=econgraph.com run_lib public_host_init); then
  fail "PUBLIC_HOST unset on an ingress-nginx cluster without the Gateway API did not stop"
fi
# Unset on a legacy cluster whose retired econ-graph-ssl-ingress serves the public host: stop.
if (unset PUBLIC_HOST; STUB_SSL_HOSTS=$'www.econgraph.com\necongraph.com' run_lib public_host_init); then
  fail "PUBLIC_HOST unset on a cluster with econ-graph-ssl-ingress did not stop"
fi
# The cluster cannot be asked: stop, even with PUBLIC_HOST set to empty.
if PUBLIC_HOST='' STUB_LOOKUP_FAILS=1 run_lib public_host_init; then
  fail "a failed lookup did not stop"
fi
# A new cluster without the namespace: localhost only is fine.
(unset PUBLIC_HOST; STUB_NO_NAMESPACE=1 run_lib 'public_host_init; [ -z "$PUBLIC_HOST" ]') ||
  fail "a new cluster without the namespace did not mean localhost-only"
# Unset on a localhost-only cluster: fine, localhost only.
(unset PUBLIC_HOST; run_lib 'public_host_init; [ -z "$PUBLIC_HOST" ]') ||
  fail "PUBLIC_HOST unset on a localhost cluster did not mean localhost-only"
# A bad name stops.
if PUBLIC_HOST="not a host" run_lib public_host_init; then
  fail "a bad PUBLIC_HOST was accepted"
fi

# With a public host: the gateway's part gets it.
PUBLIC_HOST=econgraph.com run_lib 'public_host_init; apply_public_edge'
grep -qx 'apply-gateway-routes.sh PUBLIC_HOST=econgraph.com' "$log" ||
  fail "public host: apply-gateway-routes.sh not given it: $(grep apply-gateway "$log" || true)"
# Without one: `none`, so a public host served before is removed.
PUBLIC_HOST='' run_lib 'public_host_init; apply_public_edge'
grep -qx 'apply-gateway-routes.sh PUBLIC_HOST=none' "$log" ||
  fail "no public host: apply-gateway-routes.sh not told none: $(grep apply-gateway "$log" || true)"
# Nothing patches ingress-nginx any more.
grep -q 'ingress-nginx' "$log" && fail "the library still touches ingress-nginx"

# cloudflared: 0 replicas without the Secret or without a public host, 1 with both.
PUBLIC_HOST=econgraph.com STUB_SECRET=0 run_lib 'public_host_init; apply_cloudflared'
grep -q 'stdin: .*replicas: 0' "$log" || fail "no Secret: cloudflared not at 0 replicas"
PUBLIC_HOST='' STUB_SECRET=1 run_lib 'public_host_init; apply_cloudflared'
grep -q 'stdin: .*replicas: 0' "$log" || fail "no public host: cloudflared not at 0 replicas"
PUBLIC_HOST=econgraph.com STUB_SECRET=1 run_lib 'public_host_init; apply_cloudflared'
grep -q 'kubectl apply -f k8s/manifests/cloudflared.yaml' "$log" || fail "Secret and host: cloudflared not applied"

if [ "$failures" -gt 0 ]; then
  echo "$failures check(s) failed"
  exit 1
fi
echo "public-host.sh: all checks passed"
