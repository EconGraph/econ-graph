#!/usr/bin/env bash
# The stubs below are called by the sourced library; the scripts given to run_lib are
# expanded there, not here.
# shellcheck disable=SC2329,SC2016
# Tests for scripts/deploy/lib/public-host.sh against a stub kubectl, no cluster:
#   - PUBLIC_HOST unset on a cluster that serves a public host (public Ingresses, or the
#     retired econ-graph-ssl-ingress) stops the deploy, and so does a failed lookup;
#   - a public host sets only the real-ip settings in the ingress-nginx ConfigMap and
#     removes use-forwarded-headers; no public host removes all of them again and
#     deletes the public Ingresses; the retired econ-graph-ssl-ingress goes either way,
#     before any Ingress is applied (in public_host_init);
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

# run_lib SCRIPT: runs SCRIPT in a subshell with the library and a stub kubectl.
# STUB_PUBLIC_HOST is what the cluster serves; STUB_SECRET=1 means cloudflare-tunnel exists.
# Every kubectl call (and stdin piped to apply) is appended to $log.
log="$(mktemp)"
trap 'rm -f "$log"' EXIT
run_lib() {
  : >"$log"
  (
    kubectl() {
      echo "kubectl $*" >>"$log"
      case "$*" in
        *"get namespace econ-graph"*)
          [ "${STUB_LOOKUP_FAILS:-0}" != 1 ] || return 1
          [ "${STUB_NO_NAMESPACE:-0}" = 1 ] || echo namespace/econ-graph ;;
        *"get ingress -l"*) printf '%s' "${STUB_PUBLIC_HOST:-}" ;;
        *"get ingress econ-graph-ssl-ingress"*) printf '%s' "${STUB_SSL_HOSTS:-}" ;;
        *"get nodes"*) printf '10.244.0.0/24 10.244.1.0/24' ;;
        *"get secret cloudflare-tunnel"*) [ "${STUB_SECRET:-0}" = 1 ] ;;
        *"delete ingress econ-graph-ssl-ingress"*) [ "${STUB_DELETE_FAILS:-0}" != 1 ] ;;
        *"apply -f -"*) sed 's/^/  stdin: /' >>"$log" ;;
      esac
    }
    yq() { python3 -c 'import sys,yaml; d=yaml.safe_load(open(sys.argv[1])); d["spec"]["replicas"]=0; print(yaml.safe_dump(d))' "$2"; }
    # shellcheck source=scripts/deploy/lib/public-host.sh
    source scripts/deploy/lib/public-host.sh
    eval "$1"
  ) >/dev/null 2>&1
}

# Unset on a cluster serving econgraph.com: stop.
if (unset PUBLIC_HOST; STUB_PUBLIC_HOST=econgraph.com run_lib public_host_init); then
  fail "PUBLIC_HOST unset on a public cluster did not stop"
fi
# Unset on a legacy cluster whose retired econ-graph-ssl-ingress serves the public host: stop.
if (unset PUBLIC_HOST; STUB_SSL_HOSTS=$'www.econgraph.com\necongraph.com' run_lib public_host_init); then
  fail "PUBLIC_HOST unset on a cluster with econ-graph-ssl-ingress did not stop"
fi
# The cluster cannot be asked: stop, even with PUBLIC_HOST set to empty.
if PUBLIC_HOST='' STUB_LOOKUP_FAILS=1 run_lib public_host_init; then
  fail "a failed Ingress lookup did not stop"
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

# The retired Ingress goes in public_host_init, before anything applies an Ingress.
PUBLIC_HOST=econgraph.com run_lib public_host_init
grep -q 'delete ingress econ-graph-ssl-ingress' "$log" || fail "public_host_init: econ-graph-ssl-ingress not deleted"

# A failed delete stops: the old Ingress would keep serving the public names.
if PUBLIC_HOST='' STUB_DELETE_FAILS=1 run_lib public_host_init; then
  fail "public_host_init went on after failing to delete econ-graph-ssl-ingress"
fi

# With a public host: real-ip settings only, use-forwarded-headers removed.
PUBLIC_HOST=econgraph.com run_lib 'public_host_init; apply_public_edge'
grep -q 'patch configmap ingress-nginx-controller' "$log" || fail "public host: ConfigMap not patched"
grep -q '"enable-real-ip":"true","forwarded-for-header":"CF-Connecting-IP","proxy-real-ip-cidr":"10.244.0.0/24,10.244.1.0/24","use-forwarded-headers":null' "$log" ||
  fail "public host: wrong ingress-nginx settings: $(grep patch "$log")"
grep -q 'delete ingress econ-graph-ssl-ingress' "$log" || fail "public host: econ-graph-ssl-ingress not deleted"
grep -q 'delete ingress -l' "$log" && fail "public host: public Ingresses deleted"

# Without one: every setting removed, public Ingresses deleted.
PUBLIC_HOST='' run_lib 'public_host_init; apply_public_edge'
grep -q '"enable-real-ip":null,"forwarded-for-header":null,"proxy-real-ip-cidr":null,"use-forwarded-headers":null' "$log" ||
  fail "no public host: settings not removed: $(grep patch "$log")"
grep -q 'delete ingress -l econgraph.io/public-host=true' "$log" || fail "no public host: public Ingresses kept"
grep -q 'delete ingress econ-graph-ssl-ingress' "$log" || fail "no public host: econ-graph-ssl-ingress not deleted"

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
