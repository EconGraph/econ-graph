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
#   - cloudflared gets 0 replicas without its Secret or without a public host;
#   - only a cluster marked production (scripts/deploy/lib/cluster-role.sh) runs
#     cloudflared or takes a PUBLIC_HOST: unmarked, a marker copied from another cluster
#     (kube-system UID mismatch) and a failed cluster query all mean 0 replicas, and a
#     PUBLIC_HOST there stops the deploy before anything is applied.
# And, with the same stubbed cluster:
#   - scripts/deploy/create-secrets.sh writes the tunnel token only into a marked cluster;
#   - scripts/deploy/mark-production-cluster.sh changes nothing without a terminal and a
#     typed confirmation, and leaves a marked cluster alone.
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

log="$(mktemp)"
out="$(mktemp)"
stub_dir="$(mktemp -d)"
trap 'rm -rf "$log" "$out" "$stub_dir"' EXIT

# The cluster's identity and production marker, for both kubectl stubs below.
# STUB_MARKER: production (made on this cluster, the default), copied (made on another
# cluster), other-role, none, or fails (the lookup fails). STUB_CLUSTER_FAILS=1 makes the
# kube-system lookup fail. Returns 1 when ARGS are not a cluster-role lookup, 2 on a
# stubbed failure.
cat >"$stub_dir/cluster-role-stub.sh" <<'STUB'
stub_cluster_role() {
  case "$*" in
    *"get namespace kube-system"*)
      [ "${STUB_CLUSTER_FAILS:-0}" != 1 ] || return 2
      printf 'uid-this-cluster' ;;
    *"get configmap econgraph-cluster-role"*)
      case "${STUB_MARKER:-production}" in
        production) printf 'production uid-this-cluster' ;;
        copied) printf 'production uid-other-cluster' ;;
        other-role) printf 'staging uid-this-cluster' ;;
        fails) return 2 ;;
      esac ;;
    *) return 1 ;;
  esac
}
STUB
# shellcheck source=/dev/null
source "$stub_dir/cluster-role-stub.sh"

# run_lib SCRIPT: runs SCRIPT in a subshell with the library and a stub kubectl.
# STUB_PUBLIC_HOST is what the cluster serves; STUB_SECRET=1 means cloudflare-tunnel exists,
# STUB_SECRET_FAILS=1 makes looking it up fail.
# Every kubectl call (and stdin piped to apply) is appended to $log, the output to $out.
run_lib() {
  : >"$log"
  (
    kubectl() {
      echo "kubectl $*" >>"$log"
      local rc=0
      stub_cluster_role "$@" || rc=$?
      [ "$rc" = 1 ] || return "$rc"
      case "$*" in
        *"get namespace econ-graph"*)
          [ "${STUB_LOOKUP_FAILS:-0}" != 1 ] || return 1
          [ "${STUB_NO_NAMESPACE:-0}" = 1 ] || echo namespace/econ-graph ;;
        *"get ingress -l"*) printf '%s' "${STUB_PUBLIC_HOST:-}" ;;
        *"get ingress econ-graph-ssl-ingress"*) printf '%s' "${STUB_SSL_HOSTS:-}" ;;
        *"get nodes"*) printf '10.244.0.0/24 10.244.1.0/24' ;;
        *"get secret cloudflare-tunnel"*)
          [ "${STUB_SECRET_FAILS:-0}" != 1 ] || return 1
          [ "${STUB_SECRET:-0}" != 1 ] || echo secret/cloudflare-tunnel ;;
        *"delete ingress econ-graph-ssl-ingress"*) [ "${STUB_DELETE_FAILS:-0}" != 1 ] ;;
        *"apply -f -"*) sed 's/^/  stdin: /' >>"$log" ;;
      esac
    }
    # Only the '.spec.replicas = N' form apply_cloudflared uses.
    yq() {
      python3 -c 'import re, sys, yaml
d = yaml.safe_load(open(sys.argv[2]))
d["spec"]["replicas"] = int(re.fullmatch(r"\.spec\.replicas = (\d+)", sys.argv[1])[1])
print(yaml.safe_dump(d))' "$1" "$2"
    }
    # shellcheck source=scripts/deploy/lib/public-host.sh
    source scripts/deploy/lib/public-host.sh
    eval "$1"
  ) >"$out" 2>&1
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

# cloudflared on a marked cluster: 0 replicas without the Secret or without a public
# host, 1 with both.
PUBLIC_HOST=econgraph.com STUB_SECRET=0 run_lib 'public_host_init; apply_cloudflared'
grep -q 'stdin: .*replicas: 0' "$log" || fail "no Secret: cloudflared not at 0 replicas"
PUBLIC_HOST='' STUB_SECRET=1 run_lib 'public_host_init; apply_cloudflared'
grep -q 'stdin: .*replicas: 0' "$log" || fail "no public host: cloudflared not at 0 replicas"
PUBLIC_HOST=econgraph.com STUB_SECRET=1 run_lib 'public_host_init; apply_cloudflared; [ "$CLOUDFLARED_DEPLOYED" = true ]' ||
  fail "marked, Secret and host: apply_cloudflared failed: $(cat "$out")"
grep -q 'stdin: .*replicas: 1' "$log" || fail "marked, Secret and host: cloudflared not at 1 replica"
grep -q 'apply -f k8s/manifests/cloudflared.yaml' "$log" &&
  fail "cloudflared.yaml applied as committed instead of with the replica count"
# The Secret cannot be looked up: stop, and leave cloudflared as it is (a failed lookup
# must not scale a running connector to 0).
if PUBLIC_HOST=econgraph.com STUB_SECRET=1 STUB_SECRET_FAILS=1 run_lib apply_cloudflared; then
  fail "a failed Secret lookup did not stop the deploy"
fi
grep -q 'apply' "$log" && fail "a failed Secret lookup changed cloudflared: $(grep apply "$log")"
grep -q 'Could not look up the Secret cloudflare-tunnel' "$out" || fail "a failed Secret lookup was not reported"

# The committed manifest never starts the connector by itself.
grep -q '^  replicas: 0$' k8s/manifests/cloudflared.yaml || fail "cloudflared.yaml is not committed with replicas: 0"

# Not production: 0 replicas even with the Secret, and a PUBLIC_HOST stops the deploy
# before anything is applied.
for marker in none copied other-role; do
  PUBLIC_HOST='' STUB_SECRET=1 STUB_MARKER=$marker run_lib 'public_host_init; apply_cloudflared; [ "$CLOUDFLARED_DEPLOYED" = false ]' ||
    fail "marker $marker, no public host: apply_cloudflared failed: $(cat "$out")"
  grep -q 'stdin: .*replicas: 0' "$log" || fail "marker $marker: cloudflared not at 0 replicas"
  grep -q 'stdin: .*replicas: [1-9]' "$log" && fail "marker $marker: cloudflared started"

  if PUBLIC_HOST=econgraph.com STUB_SECRET=1 STUB_MARKER=$marker run_lib public_host_init; then
    fail "marker $marker: PUBLIC_HOST accepted on a cluster that is not production"
  fi
  grep -q 'mark-production-cluster.sh' "$out" || fail "marker $marker: the refusal does not name mark-production-cluster.sh"
  grep -Eq 'apply|delete|patch' "$log" && fail "marker $marker: the refused deploy changed the cluster: $(grep -E 'apply|delete|patch' "$log")"

  # apply_cloudflared on its own refuses too, after stopping the connector.
  if PUBLIC_HOST=econgraph.com STUB_SECRET=1 STUB_MARKER=$marker run_lib apply_cloudflared; then
    fail "marker $marker: apply_cloudflared accepted a PUBLIC_HOST"
  fi
  grep -q 'stdin: .*replicas: 0' "$log" || fail "marker $marker, PUBLIC_HOST: cloudflared not at 0 replicas"
  grep -q 'stdin: .*replicas: [1-9]' "$log" && fail "marker $marker, PUBLIC_HOST: cloudflared started"
done
PUBLIC_HOST=econgraph.com STUB_MARKER=copied run_lib public_host_init || true
grep -q 'made on another cluster' "$out" || fail "copied marker: the refusal does not say it was made on another cluster"

# The cluster cannot be asked for its identity or its marker: not production, and said
# so. A PUBLIC_HOST stops the deploy before anything is applied, and apply_cloudflared
# stops without touching cloudflared, so a failed lookup neither starts the connector nor
# takes the site down.
for case_env in STUB_CLUSTER_FAILS=1 STUB_MARKER=fails; do
  if (export "${case_env?}"; PUBLIC_HOST=econgraph.com STUB_SECRET=1 run_lib public_host_init); then
    fail "$case_env: a failed lookup counted as production"
  fi
  grep -q 'treated as not production' "$out" || fail "$case_env: a failed lookup was not reported"
  grep -Eq 'apply|delete|patch' "$log" && fail "$case_env: the refused deploy changed the cluster"
  for host in '' econgraph.com; do
    if (export "${case_env?}"; PUBLIC_HOST=$host STUB_SECRET=1 run_lib apply_cloudflared); then
      fail "$case_env, PUBLIC_HOST=$host: apply_cloudflared went on after a failed lookup"
    fi
    grep -q 'apply' "$log" && fail "$case_env, PUBLIC_HOST=$host: a failed lookup changed cloudflared"
  done
done

# --- an executable stub kubectl, for the scripts below (passed as KUBECTL) -------
# Every Secret is absent, except that STUB_TUNNEL_SECRET=1 gives cloudflare-tunnel a
# (fake) token. STUB_CLOUDFLARED_REPLICAS is the cloudflared Deployment's replica count
# (unset: no Deployment); STUB_SCALE_FAILS=1 makes scaling it fail. Writes are logged to
# $STUB_LOG, never made.
{
  echo '#!/usr/bin/env bash'
  cat "$stub_dir/cluster-role-stub.sh"
  cat <<'STUB'
echo "kubectl $*" >>"$STUB_LOG"
rc=0
stub_cluster_role "$@" || rc=$?
[ "$rc" = 1 ] || exit "$rc"
case "$*" in
  "config current-context") echo kind-test ;;
  *"get secret cloudflare-tunnel"*"{.data.token}"*)
    # base64 of a fake token, not a real one.
    [ "${STUB_TUNNEL_SECRET:-0}" != 1 ] || printf 'ZmFrZS1leGlzdGluZy10dW5uZWwtdG9rZW4tMDAwMDAwMDAwMDAwMDAwMDAwMDAwMA==' ;;
  *"create secret generic"* | *"create configmap"*) echo "kind: Stub" ;;
  *"get deployment cloudflared"*) printf '%s' "${STUB_CLOUDFLARED_REPLICAS:-}" ;;
  *"scale deployment/cloudflared"*) [ "${STUB_SCALE_FAILS:-0}" != 1 ] || exit 1 ;;
  *apply*) cat >/dev/null ;;
esac
exit 0
STUB
} >"$stub_dir/kubectl"
chmod +x "$stub_dir/kubectl"

# run_stubbed [VAR=value ...] COMMAND...: COMMAND with the stub kubectl, no terminal.
run_stubbed() {
  : >"$log"
  env STUB_LOG="$log" KUBECTL="$stub_dir/kubectl" "$@" </dev/null >"$out" 2>&1
}
# run_in_terminal [VAR=value ...] COMMAND-STRING: the same under a terminal (script(1)),
# with stdin fed to it.
run_in_terminal() {
  local cmd="${*: -1}"
  : >"$log"
  env STUB_LOG="$log" KUBECTL="$stub_dir/kubectl" "${@:1:$#-1}" \
    script -qec "$cmd" /dev/null >"$out" 2>&1
}
have_script=false
command -v script >/dev/null 2>&1 && have_script=true

# --- create-secrets.sh: the tunnel token goes only into a marked cluster ---------
fake_token="fake-tunnel-token-for-tests-00000000000000000000000"
# Refused before anything is written, whichever way the marker check fails.
for case_env in STUB_MARKER=none STUB_MARKER=copied STUB_MARKER=fails STUB_CLUSTER_FAILS=1; do
  if run_stubbed "$case_env" CLOUDFLARE_TUNNEL_TOKEN="$fake_token" scripts/deploy/create-secrets.sh; then
    fail "create-secrets ($case_env): wrote the tunnel token into a cluster that is not production"
  fi
  grep -q 'create secret' "$log" && fail "create-secrets ($case_env): wrote Secrets before refusing the token"
  grep -q 'mark-production-cluster.sh' "$out" ||
    fail "create-secrets ($case_env): the refusal does not name mark-production-cluster.sh: $(cat "$out")"
done
# Marked: the token is written.
run_stubbed STUB_MARKER=production CLOUDFLARE_TUNNEL_TOKEN="$fake_token" scripts/deploy/create-secrets.sh ||
  fail "create-secrets: refused the tunnel token on a marked cluster: $(cat "$out")"
grep -q 'create secret generic cloudflare-tunnel --from-file=token=' "$log" ||
  fail "create-secrets: the tunnel token was not written on a marked cluster"
# No token given: as before on an unmarked cluster; a token already there is reported.
run_stubbed STUB_MARKER=none scripts/deploy/create-secrets.sh ||
  fail "create-secrets: failed without a token on an unmarked cluster: $(cat "$out")"
grep -q 'create secret generic cloudflare-tunnel' "$log" && fail "create-secrets: wrote cloudflare-tunnel without a token"
run_stubbed STUB_MARKER=copied STUB_TUNNEL_SECRET=1 scripts/deploy/create-secrets.sh ||
  fail "create-secrets: failed keeping a token on an unmarked cluster: $(cat "$out")"
grep -q 'warning: cloudflare-tunnel holds a tunnel token' "$out" ||
  fail "create-secrets: a token in an unmarked cluster was not reported"
# --prompt-tunnel-token refuses before it prompts.
if [ "$have_script" = true ]; then
  if run_in_terminal STUB_MARKER=copied 'scripts/deploy/create-secrets.sh --prompt-tunnel-token' </dev/null; then
    fail "create-secrets --prompt-tunnel-token: accepted an unmarked cluster"
  fi
  grep -q 'input hidden' "$out" && fail "create-secrets --prompt-tunnel-token: prompted on an unmarked cluster"
  grep -q 'mark-production-cluster.sh' "$out" ||
    fail "create-secrets --prompt-tunnel-token: the refusal is not shown: $(cat "$out")"
fi

# --- mark-production-cluster.sh ---------------------------------------------------
mark=scripts/deploy/mark-production-cluster.sh
writes='apply|delete|scale'
run_stubbed STUB_MARKER=production "$mark" || fail "mark: failed on a marked cluster: $(cat "$out")"
grep -q 'already marked' "$out" || fail "mark: a marked cluster was not reported as marked"
grep -Eq "$writes" "$log" && fail "mark: changed a marked cluster"
run_stubbed STUB_MARKER=copied "$mark" --dry-run || fail "mark --dry-run failed: $(cat "$out")"
grep -q 'made on another cluster' "$out" || fail "mark --dry-run: a copied marker was not reported"
grep -q 'kube-system-uid=uid-this-cluster' "$out" || fail "mark --dry-run: does not show this cluster's UID"
grep -Eq "$writes" "$log" && fail "mark --dry-run: changed the cluster"
if run_stubbed STUB_MARKER=none "$mark"; then
  fail "mark: went on without a terminal"
fi
grep -Eq "$writes" "$log" && fail "mark without a terminal: changed the cluster"
if run_stubbed STUB_MARKER=production "$mark" --unmark; then
  fail "mark --unmark: went on without a terminal"
fi
grep -Eq "$writes" "$log" && fail "mark --unmark without a terminal: changed the cluster"
run_stubbed STUB_MARKER=none STUB_CLOUDFLARED_REPLICAS=0 "$mark" --unmark ||
  fail "mark --unmark: failed on an unmarked cluster: $(cat "$out")"
grep -q 'nothing to do' "$out" || fail "mark --unmark: an unmarked cluster without cloudflared running was not left alone"
grep -Eq "$writes" "$log" && fail "mark --unmark: changed an unmarked cluster"
# No marker but cloudflared still running: --unmark still stops it (a deploy with
# PUBLIC_HOST would refuse before scaling it down).
run_stubbed STUB_MARKER=none STUB_CLOUDFLARED_REPLICAS=1 "$mark" --unmark --dry-run ||
  fail "mark --unmark --dry-run failed: $(cat "$out")"
grep -q 'would scale deployment/cloudflared' "$out" || fail "mark --unmark --dry-run: a running cloudflared without a marker is not stopped"
grep -q 'would delete ConfigMap' "$out" && fail "mark --unmark --dry-run: would delete a marker that does not exist"
grep -Eq "$writes" "$log" && fail "mark --unmark --dry-run: changed the cluster"
if run_stubbed STUB_MARKER=none STUB_CLOUDFLARED_REPLICAS=1 "$mark" --unmark; then
  fail "mark --unmark: stopped cloudflared without a terminal, or did nothing"
fi
if run_stubbed STUB_CLUSTER_FAILS=1 "$mark" --dry-run; then
  fail "mark: went on without the cluster's kube-system UID"
fi
if [ "$have_script" = true ]; then
  # Typed confirmation: a wrong name changes nothing; the context name writes the marker
  # (the stub cluster then still reports no marker, so the final check fails, which is
  # what a marker that does not check out must do).
  if printf 'wrong\n' | run_in_terminal STUB_MARKER=none "$mark"; then
    fail "mark: a wrong confirmation was accepted"
  fi
  grep -Eq "$writes" "$log" && fail "mark: a wrong confirmation changed the cluster"
  printf 'kind-test\n' | run_in_terminal STUB_MARKER=none "$mark" || true
  grep -q 'create configmap econgraph-cluster-role --from-literal=role=production --from-literal=kube-system-uid=uid-this-cluster' "$log" ||
    fail "mark: the confirmed marker was not written: $(cat "$out")"

  # --unmark scales cloudflared to 0 before it deletes the marker...
  printf 'kind-test\n' | run_in_terminal STUB_MARKER=production STUB_CLOUDFLARED_REPLICAS=1 "$mark --unmark" ||
    fail "mark --unmark failed: $(cat "$out")"
  grep -Eo 'scale deployment/cloudflared --replicas=0|delete configmap econgraph-cluster-role' "$log" | tr '\n' ' ' |
    grep -q '^scale deployment/cloudflared --replicas=0 delete configmap econgraph-cluster-role $' ||
    fail "mark --unmark: did not scale cloudflared, then delete the marker: $(grep -E 'scale|delete' "$log")"
  # ...and keeps the marker when scaling fails.
  if printf 'kind-test\n' | run_in_terminal STUB_MARKER=production STUB_CLOUDFLARED_REPLICAS=1 STUB_SCALE_FAILS=1 "$mark --unmark"; then
    fail "mark --unmark: went on after failing to scale cloudflared"
  fi
  grep -q 'delete configmap' "$log" && fail "mark --unmark: deleted the marker although cloudflared was not scaled to 0"
fi

if [ "$failures" -gt 0 ]; then
  echo "$failures check(s) failed"
  exit 1
fi
echo "public-host.sh, create-secrets.sh and mark-production-cluster.sh: all checks passed"
