#!/usr/bin/env bash
# The scripts given to run_lib are expanded in its subshell, not here.
# The yq stub is called by the sourced library.
# shellcheck disable=SC2016,SC2329
# Tests for scripts/deploy/lib/public-host.sh against a stub kubectl, no cluster:
#   - PUBLIC_HOST unset on a cluster that serves a public host (public Ingresses, or the
#     retired econ-graph-ssl-ingress) stops the deploy, and so does a failed lookup;
#   - a public host sets only the real-ip settings in the ingress-nginx ConfigMap and
#     removes use-forwarded-headers; no public host removes all of them again and
#     deletes the public Ingresses; the retired econ-graph-ssl-ingress goes either way,
#     before any Ingress is applied (in public_host_init);
#   - cloudflared gets 0 replicas without its Secret or without a public host;
#   - only a cluster marked production (scripts/deploy/lib/cluster-role.sh) runs
#     cloudflared or takes a PUBLIC_HOST: unmarked or a marker copied from another cluster
#     (kube-system UID mismatch) mean 0 replicas, a PUBLIC_HOST there stops the deploy
#     before anything is applied, and a failed or empty lookup stops it without touching
#     cloudflared;
#   - every kubectl call goes to $KUBECTL (never the kubectl on PATH) on one pinned context;
#   - deploy.sh and restart-k8s-rollout.sh leave cloudflared.yaml to apply_cloudflared,
#     and no script applies it or the whole k8s/manifests directory itself.
# And, with the same stubbed cluster:
#   - scripts/deploy/create-secrets.sh writes the tunnel token only into a marked cluster;
#   - scripts/deploy/mark-production-cluster.sh changes nothing without a terminal and the
#     typed kube-system UID prefix, leaves a marked cluster alone, and with --unmark stops
#     cloudflared before removing the marker and deletes the token only when told to;
#   - scripts/deploy/set-crawler-keys.sh --tunnel refuses an unmarked cluster before it
#     asks for the token, and on a marked one starts cloudflared only through
#     apply_cloudflared, with the kind kubectl and context (lib/kind-env.sh) throughout.
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
path_log="$(mktemp)"
stub_dir="$(mktemp -d)"
trap 'rm -rf "$log" "$log.ctx" "$out" "$path_log" "$stub_dir"' EXIT

# The stub cluster, as the executable $stub_dir/kubectl, passed as KUBECTL. Every call is
# appended to $STUB_LOG (and stdin piped to `apply -f -` too); writes are never made.
#   STUB_MARKER          production (made on this cluster, the default), copied (made on
#                        another cluster), other-role, none, or fails (the lookup fails)
#   STUB_CLUSTER_FAILS=1 the kube-system lookup fails; STUB_EMPTY_UID=1 it answers nothing
#   STUB_NO_CONTEXT=1    there is no current context
#   STUB_PUBLIC_HOST     the host the public Ingresses serve; STUB_SSL_HOSTS the retired one's
#   STUB_SECRET=1        cloudflare-tunnel exists (with a fake token); STUB_SECRET_FAILS=1
#                        looking it up fails
#   STUB_CLOUDFLARED_REPLICAS  the cloudflared Deployment's replicas (unset: no Deployment);
#                        STUB_SCALE_FAILS=1 scaling it fails
#   STUB_LOOKUP_FAILS=1, STUB_NO_NAMESPACE=1, STUB_DELETE_FAILS=1  the econ-graph namespace
#                        lookup fails / finds nothing; deleting the retired Ingress fails
cat >"$stub_dir/kubectl" <<'STUB'
#!/usr/bin/env bash
echo "kubectl $*" >>"$STUB_LOG"
case "$*" in
  *"config current-context"*) [ "${STUB_NO_CONTEXT:-0}" = 1 ] && exit 1; cat "$STUB_LOG.ctx" 2>/dev/null || echo kind-test ;;
  *"config use-context "*) echo "${!#}" >"$STUB_LOG.ctx" ;;
  *"config view"*) echo https://127.0.0.1:6443 ;;
  *"get namespace kube-system"*)
    [ "${STUB_CLUSTER_FAILS:-0}" != 1 ] || exit 1
    [ "${STUB_EMPTY_UID:-0}" = 1 ] || printf 'uid-this-cluster' ;;
  *"get configmap econgraph-cluster-role"*)
    case "${STUB_MARKER:-production}" in
      production) printf 'production uid-this-cluster' ;;
      copied) printf 'production uid-other-cluster' ;;
      other-role) printf 'staging uid-this-cluster' ;;
      fails) exit 1 ;;
    esac ;;
  *"get namespace econ-graph"*)
    [ "${STUB_LOOKUP_FAILS:-0}" != 1 ] || exit 1
    [ "${STUB_NO_NAMESPACE:-0}" = 1 ] || echo namespace/econ-graph ;;
  *"get ingress -l"*) printf '%s' "${STUB_PUBLIC_HOST:-}" ;;
  *"get ingress econ-graph-ssl-ingress"*) printf '%s' "${STUB_SSL_HOSTS:-}" ;;
  *"delete ingress econ-graph-ssl-ingress"*) [ "${STUB_DELETE_FAILS:-0}" != 1 ] || exit 1 ;;
  *"get nodes"*"podCIDR"*) printf '10.244.0.0/24 10.244.1.0/24' ;;
  *"get nodes"*) printf 'node-a node-b' ;;
  *"get secret cloudflare-tunnel"*"{.data.token}"*)
    # base64 of a fake token, not a real one.
    [ "${STUB_SECRET:-0}" != 1 ] || printf 'ZmFrZS1leGlzdGluZy10dW5uZWwtdG9rZW4tMDAwMDAwMDAwMDAwMDAwMDAwMDAwMA==' ;;
  *"get secret cloudflare-tunnel"*)
    [ "${STUB_SECRET_FAILS:-0}" != 1 ] || exit 1
    [ "${STUB_SECRET:-0}" != 1 ] || echo secret/cloudflare-tunnel ;;
  *"get deployment cloudflared"*) printf '%s' "${STUB_CLOUDFLARED_REPLICAS:-}" ;;
  *"scale deployment/cloudflared"*) [ "${STUB_SCALE_FAILS:-0}" != 1 ] || exit 1 ;;
  *"create secret generic"* | *"create configmap"*) echo "kind: Stub" ;;
  *"apply"*"-f -"*) sed 's/^/  stdin: /' >>"$STUB_LOG" ;;
esac
exit 0
STUB
chmod +x "$stub_dir/kubectl"
# A different kubectl first on PATH, as on a host where kubectl is another cluster's
# wrapper: nothing may call it. Checked at the end.
mkdir "$stub_dir/path"
printf '#!/usr/bin/env bash\necho "kubectl on PATH: $*" >>%q\nexit 1\n' "$path_log" >"$stub_dir/path/kubectl"
chmod +x "$stub_dir/path/kubectl"
export PATH="$stub_dir/path:$PATH" KUBECTL="$stub_dir/kubectl" STUB_LOG="$log"

# run_lib SCRIPT: runs SCRIPT in a subshell with the library; output to $out.
run_lib() {
  : >"$log"; rm -f "$log.ctx"
  (
    # Only the '.spec.replicas = N' form apply_cloudflared uses.
    yq() {
      python3 -c 'import re, sys, yaml
d = yaml.safe_load(open(sys.argv[2]))
d["spec"]["replicas"] = int(re.fullmatch(r"\.spec\.replicas = (\d+)", sys.argv[1])[1])
print(yaml.safe_dump(d))' "$1" "$2"
    }
    unset CLUSTER_ROLE_CONTEXT
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
PUBLIC_HOST=econgraph.com run_lib public_host_init ||
  fail "public_host_init failed: $(cat "$out")"
grep -q 'delete ingress econ-graph-ssl-ingress' "$log" || fail "public_host_init: econ-graph-ssl-ingress not deleted"

# A failed delete stops: the old Ingress would keep serving the public names.
if PUBLIC_HOST='' STUB_DELETE_FAILS=1 run_lib public_host_init; then
  fail "public_host_init went on after failing to delete econ-graph-ssl-ingress"
fi

# With a public host: real-ip settings only, use-forwarded-headers removed.
PUBLIC_HOST=econgraph.com run_lib 'public_host_init; apply_public_edge' ||
  fail "public host: deploy failed: $(cat "$out")"
grep -q 'patch configmap ingress-nginx-controller' "$log" || fail "public host: ConfigMap not patched"
grep -q '"enable-real-ip":"true","forwarded-for-header":"CF-Connecting-IP","proxy-real-ip-cidr":"10.244.0.0/24,10.244.1.0/24","use-forwarded-headers":null' "$log" ||
  fail "public host: wrong ingress-nginx settings: $(grep patch "$log")"
grep -q 'delete ingress econ-graph-ssl-ingress' "$log" || fail "public host: econ-graph-ssl-ingress not deleted"
grep -q 'delete ingress -l' "$log" && fail "public host: public Ingresses deleted"

# Without one: every setting removed, public Ingresses deleted.
PUBLIC_HOST='' run_lib 'public_host_init; apply_public_edge' ||
  fail "no public host: deploy failed: $(cat "$out")"
grep -q '"enable-real-ip":null,"forwarded-for-header":null,"proxy-real-ip-cidr":null,"use-forwarded-headers":null' "$log" ||
  fail "no public host: settings not removed: $(grep patch "$log")"
grep -q 'delete ingress -l econgraph.io/public-host=true' "$log" || fail "no public host: public Ingresses kept"
grep -q 'delete ingress econ-graph-ssl-ingress' "$log" || fail "no public host: econ-graph-ssl-ingress not deleted"

# cloudflared on a marked cluster: 0 replicas without the Secret or without a public
# host, 1 with both.
PUBLIC_HOST=econgraph.com STUB_SECRET=0 run_lib 'public_host_init; apply_cloudflared' ||
  fail "no Secret: apply_cloudflared failed: $(cat "$out")"
grep -q 'stdin: .*replicas: 0' "$log" || fail "no Secret: cloudflared not at 0 replicas"
PUBLIC_HOST='' STUB_SECRET=1 run_lib 'public_host_init; apply_cloudflared' ||
  fail "no public host: apply_cloudflared failed: $(cat "$out")"
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
for case_env in STUB_CLUSTER_FAILS=1 STUB_EMPTY_UID=1 STUB_MARKER=fails; do
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

# run_stubbed [VAR=value ...] COMMAND...: COMMAND with the stub kubectl, no terminal.
run_stubbed() {
  : >"$log"; rm -f "$log.ctx"
  env -u CLUSTER_ROLE_CONTEXT "$@" </dev/null >"$out" 2>&1
}
# run_in_terminal [VAR=value ...] COMMAND-STRING: the same under a terminal (script(1)),
# with stdin fed to it.
run_in_terminal() {
  local cmd="${*: -1}"
  : >"$log"; rm -f "$log.ctx"
  env -u CLUSTER_ROLE_CONTEXT "${@:1:$#-1}" script -qec "$cmd" /dev/null >"$out" 2>&1
}
have_script=false
command -v script >/dev/null 2>&1 && have_script=true

# --- create-secrets.sh: the tunnel token goes only into a marked cluster ---------
fake_token="fake-tunnel-token-for-tests-00000000000000000000000"
# Refused before anything is written, whichever way the marker check fails.
for case_env in STUB_MARKER=none STUB_MARKER=copied STUB_MARKER=fails STUB_CLUSTER_FAILS=1 STUB_EMPTY_UID=1; do
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
run_stubbed STUB_MARKER=copied STUB_SECRET=1 scripts/deploy/create-secrets.sh ||
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

# An inherited CLUSTER_ROLE_CONTEXT is followed only while it is the selected kubectl's
# current context (as from deploy.sh, which switches to it first); a stray one is refused
# before anything is written.
if run_stubbed CLUSTER_ROLE_CONTEXT=prod-elsewhere STUB_MARKER=production \
  CLOUDFLARE_TUNNEL_TOKEN="$fake_token" scripts/deploy/create-secrets.sh; then
  fail "create-secrets: followed a stray CLUSTER_ROLE_CONTEXT"
fi
grep -q 'create secret' "$log" && fail "create-secrets: wrote Secrets with a stray CLUSTER_ROLE_CONTEXT"
grep -q 'is not the current' "$out" || fail "create-secrets: the stray context was not reported: $(cat "$out")"
run_stubbed CLUSTER_ROLE_CONTEXT=kind-test STUB_MARKER=production scripts/deploy/create-secrets.sh ||
  fail "create-secrets: refused the context it is on: $(cat "$out")"
grep -q -- '--context kind-test -n econ-graph create secret generic' "$log" ||
  fail "create-secrets: did not write on the inherited context"

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
# --unmark warns on the live production cluster, also in a dry run.
run_stubbed STUB_MARKER=production STUB_CLOUDFLARED_REPLICAS=1 "$mark" --unmark --dry-run ||
  fail "mark --unmark --dry-run on production failed: $(cat "$out")"
grep -q 'WARNING: context kind-test is the cluster marked production' "$out" ||
  fail "mark --unmark --dry-run: no warning on the production cluster"
# A token without a marker or a running connector is still something to unmark.
run_stubbed STUB_MARKER=none STUB_SECRET=1 "$mark" --unmark --dry-run ||
  fail "mark --unmark --dry-run with a token failed: $(cat "$out")"
grep -q 'would offer to delete Secret econ-graph/cloudflare-tunnel' "$out" ||
  fail "mark --unmark --dry-run: a token on an unmarked cluster is not offered for deletion"
# --unmark works on econ-graph, whatever NAMESPACE says.
run_stubbed NAMESPACE=elsewhere STUB_MARKER=none STUB_CLOUDFLARED_REPLICAS=1 "$mark" --unmark --dry-run || true
grep -q -- '-n elsewhere' "$log" && fail "mark --unmark: followed NAMESPACE instead of econ-graph"
grep -q -- '-n econ-graph get deployment cloudflared' "$log" || fail "mark --unmark: did not look in econ-graph"
if run_stubbed STUB_CLUSTER_FAILS=1 "$mark" --dry-run; then
  fail "mark: went on without the cluster's kube-system UID"
fi
if run_stubbed STUB_EMPTY_UID=1 "$mark" --dry-run; then
  fail "mark: went on with an empty kube-system UID"
fi
if [ "$have_script" = true ]; then
  # The confirmation shows the cluster's identity and wants the kube-system UID's first 8
  # characters ("uid-this" here): a wrong answer, or the shared context name, changes
  # nothing.
  for answer in wrong kind-test; do
    if printf '%s\n' "$answer" | run_in_terminal STUB_MARKER=none "$mark"; then
      fail "mark: the confirmation '$answer' was accepted"
    fi
    grep -Eq "$writes" "$log" && fail "mark: the confirmation '$answer' changed the cluster"
  done
  for shown in 'this host' 'API server .*https://127.0.0.1:6443' 'nodes .*node-a node-b' 'kube-system UID uid-this-cluster'; do
    grep -Eq "$shown" "$out" || fail "mark: the confirmation does not show '$shown': $(cat "$out")"
  done
  # The right answer writes the marker (the stub cluster still reports no marker
  # afterwards, so the final check fails, as it must for a marker that does not check out).
  printf 'uid-this\n' | run_in_terminal STUB_MARKER=none "$mark" || true
  grep -q 'create configmap econgraph-cluster-role --from-literal=role=production --from-literal=kube-system-uid=uid-this-cluster' "$log" ||
    fail "mark: the confirmed marker was not written: $(cat "$out")"

  # --unmark scales cloudflared to 0 before it deletes the marker, and keeps the token
  # unless told to delete it...
  printf 'uid-this\n\n' | run_in_terminal STUB_MARKER=production STUB_CLOUDFLARED_REPLICAS=1 STUB_SECRET=1 "$mark --unmark" ||
    fail "mark --unmark failed: $(cat "$out")"
  grep -Eo 'scale deployment/cloudflared --replicas=0|delete configmap econgraph-cluster-role' "$log" | tr '\n' ' ' |
    grep -q '^scale deployment/cloudflared --replicas=0 delete configmap econgraph-cluster-role $' ||
    fail "mark --unmark: did not scale cloudflared, then delete the marker: $(grep -E 'scale|delete' "$log")"
  grep -q 'delete secret' "$log" && fail "mark --unmark: deleted the token without being told to"
  grep -q 'kept Secret econ-graph/cloudflare-tunnel' "$out" || fail "mark --unmark: did not say the token was kept"
  # On the live production cluster it warns, before the confirmation.
  warning_line="$(grep -n 'WARNING: context kind-test is the cluster marked production' "$out" | cut -d: -f1 || true)"
  prompt_line="$(grep -n 'Type the first 8 characters' "$out" | cut -d: -f1 || true)"
  [ -n "$warning_line" ] && [ -n "$prompt_line" ] && [ "$warning_line" -lt "$prompt_line" ] ||
    fail "mark --unmark: no warning before the confirmation on the production cluster: $(cat "$out")"
  # End of input at the token prompt keeps the token (and does not fail).
  printf 'uid-this\n' | run_in_terminal STUB_MARKER=none STUB_SECRET=1 "$mark --unmark" ||
    fail "mark --unmark: end of input at the token prompt failed: $(cat "$out")"
  grep -q 'delete secret' "$log" && fail "mark --unmark: end of input deleted the token"
  grep -q 'kept Secret econ-graph/cloudflare-tunnel' "$out" || fail "mark --unmark: end of input did not say the token was kept"
  grep -q 'WARNING' "$out" && fail "mark --unmark: warned about production on an unmarked cluster"
  # ...deletes it when told to...
  printf 'uid-this\ndelete\n' | run_in_terminal STUB_MARKER=production STUB_CLOUDFLARED_REPLICAS=0 STUB_SECRET=1 "$mark --unmark" ||
    fail "mark --unmark (delete the token) failed: $(cat "$out")"
  grep -q -- '-n econ-graph delete secret cloudflare-tunnel' "$log" || fail "mark --unmark: did not delete the token when told to"
  # ...and keeps the marker when scaling fails.
  if printf 'uid-this\n' | run_in_terminal STUB_MARKER=production STUB_CLOUDFLARED_REPLICAS=1 STUB_SCALE_FAILS=1 "$mark --unmark"; then
    fail "mark --unmark: went on after failing to scale cloudflared"
  fi
  grep -q 'delete configmap' "$log" && fail "mark --unmark: deleted the marker although cloudflared was not scaled to 0"
fi

# --- one kubectl, one context --------------------------------------------------------
# A full deploy pass: every call goes to $KUBECTL with the pinned context.
PUBLIC_HOST=econgraph.com STUB_SECRET=1 run_lib 'public_host_init; apply_public_edge; apply_cloudflared' ||
  fail "full deploy pass failed: $(cat "$out")"
grep '^kubectl ' "$log" | grep -v 'config current-context' | grep -v -- '--context kind-test' &&
  fail "a kubectl call did not use the pinned context"
grep -q '^kubectl config current-context' "$log" || fail "the context was not pinned"
# No current context: stop before anything else.
if PUBLIC_HOST='' STUB_NO_CONTEXT=1 run_lib public_host_init; then
  fail "public_host_init went on without a current context"
fi
# A CLUSTER_ROLE_CONTEXT inherited from the environment (another cluster's) does not
# redirect a deploy: cluster_role_use_context replaces it, as deploy.sh and
# restart-k8s-rollout.sh do, and their kubectl wrapper sends their own calls through kc.
PUBLIC_HOST='' STUB_SECRET=1 run_lib 'export CLUSTER_ROLE_CONTEXT=prod-elsewhere
  cluster_role_use_context kind-test; kubectl() { kc "$@"; }
  public_host_init; apply_public_edge; apply_cloudflared; kubectl get nodes' ||
  fail "deploy pass after cluster_role_use_context failed: $(cat "$out")"
grep -q 'prod-elsewhere' "$log" && fail "an inherited CLUSTER_ROLE_CONTEXT redirected the deploy: $(grep prod-elsewhere "$log")"
grep -q '^kubectl config use-context kind-test' "$log" || fail "cluster_role_use_context did not switch the context"
grep '^kubectl ' "$log" | grep -v 'config use-context' | grep -v -- '--context kind-test' &&
  fail "a call of the deploy pass did not use the pinned context"
grep -q '^kubectl --context kind-test get nodes' "$log" || fail "the kubectl wrapper did not go through kc"
# mark-production-cluster.sh ignores an inherited CLUSTER_ROLE_CONTEXT too.
run_stubbed CLUSTER_ROLE_CONTEXT=prod-elsewhere STUB_MARKER=production "$mark" ||
  fail "mark: failed with an inherited CLUSTER_ROLE_CONTEXT: $(cat "$out")"
grep -q 'prod-elsewhere' "$log" && fail "mark: followed an inherited CLUSTER_ROLE_CONTEXT"
# The kubectl first on PATH (another cluster's) was never called, by any test above.
[ ! -s "$path_log" ] || fail "calls went to the kubectl on PATH instead of \$KUBECTL: $(cat "$path_log")"
# With KUBECTL unset, the wrapper reaches the kubectl binary on PATH through kc, once,
# instead of calling itself.
timeout 20 env -u KUBECTL CLUSTER_ROLE_CONTEXT=ctx bash -c \
  'source scripts/deploy/lib/cluster-role.sh; kubectl() { kc "$@"; }; kubectl version' >/dev/null 2>&1 || true
[ "$(cat "$path_log")" = "kubectl on PATH: --context ctx version" ] ||
  fail "the kubectl wrapper without KUBECTL did not call the binary once: $(cat "$path_log")"
: >"$path_log"

# --- static: who applies cloudflared.yaml ----------------------------------------------
for script in scripts/deploy/deploy.sh scripts/deploy/restart-k8s-rollout.sh; do
  grep -q '^apply_cloudflared$' "$script" || fail "$script does not call apply_cloudflared"
done
grep -q 'cloudflared.yaml) continue' scripts/deploy/restart-k8s-rollout.sh ||
  fail "restart-k8s-rollout.sh does not skip cloudflared.yaml in its manifest loop"
# Both pin their context and send every kubectl call through kc before the first deploy step.
for script in scripts/deploy/deploy.sh scripts/deploy/restart-k8s-rollout.sh; do
  pin="$(grep -n '^if ! cluster_role_use_context ' "$script" | cut -d: -f1 || true)"
  wrap="$(grep -nF 'kubectl() { kc "$@"; }' "$script" | cut -d: -f1 || true)"
  init="$(grep -n '^public_host_init$' "$script" | cut -d: -f1 || true)"
  first="$(grep -nE '^[^#]*\bkubectl (apply|get|wait|delete|patch|rollout|scale|create)' "$script" | head -1 | cut -d: -f1 || true)"
  if ! { [ -n "$pin" ] && [ -n "$wrap" ] && [ "$pin" -lt "$wrap" ] && [ "$wrap" -lt "$init" ] &&
    { [ -z "$first" ] || [ "$wrap" -lt "$first" ]; }; }; then
    fail "$script does not pin its context and wrap kubectl before using it (pin $pin, wrap $wrap, init $init, first call $first)"
  fi
done
# deploy.sh pins the kind kubectl that ensure_kind_kubectl set up (lib/kind-env.sh), not
# whatever KUBECTL or PATH says: on a host where kubectl is another cluster's wrapper.
kubectl_line="$(grep -n '^export KUBECTL="${ECONGRAPH_KUBE_DIR:-${ECONGRAPH_CACHE}}/bin/kubectl"$' scripts/deploy/deploy.sh | cut -d: -f1 || true)"
ensure_line="$(grep -n '^ensure_kind_kubectl$' scripts/deploy/deploy.sh | cut -d: -f1 || true)"
pin_line="$(grep -n '^if ! cluster_role_use_context "kind-${KIND_CLUSTER}"' scripts/deploy/deploy.sh | cut -d: -f1 || true)"
if ! { [ -n "$kubectl_line" ] && [ -n "$ensure_line" ] && [ -n "$pin_line" ] &&
  [ "$ensure_line" -lt "$kubectl_line" ] && [ "$kubectl_line" -lt "$pin_line" ]; }; then
  fail "deploy.sh does not pin the kind kubectl before its context (ensure $ensure_line, KUBECTL $kubectl_line, pin $pin_line)"
fi
grep -q '^unset CLUSTER_ROLE_CONTEXT$' scripts/deploy/mark-production-cluster.sh ||
  fail "mark-production-cluster.sh does not drop an inherited CLUSTER_ROLE_CONTEXT"
# restart-k8s-rollout.sh binds plain kubectl to the kubeconfig it wrote from MicroK8s,
# not a KUBECTL or KUBECONFIG from the environment, before it pins microk8s.
bind_line="$(grep -n '^export KUBECTL=kubectl KUBECONFIG="$HOME/.kube/config"$' scripts/deploy/restart-k8s-rollout.sh | cut -d: -f1 || true)"
pin_line="$(grep -n '^if ! cluster_role_use_context microk8s' scripts/deploy/restart-k8s-rollout.sh | cut -d: -f1 || true)"
if ! { [ -n "$bind_line" ] && [ -n "$pin_line" ] && [ "$bind_line" -lt "$pin_line" ]; }; then
  fail "restart-k8s-rollout.sh does not bind its kubectl and kubeconfig before pinning microk8s"
fi
# Nothing else applies cloudflared.yaml, the whole k8s/manifests directory (which would
# stop the connector on production, since the manifest is committed with 0 replicas), or
# k8s/ recursively or through kustomize.
offenders="$(grep -rnE '\bapply\b' scripts --include='*.sh' |
  grep -E -e '[[:space:]](-k|--kustomize)([[:space:]=]|$)' -e '[[:space:]](-R|--recursive)([[:space:]=]|$)' \
    -e '(-f|--filename)[[:space:]=]"?[^[:space:]"]*(cloudflared\.yaml|k8s/?|manifests/?)("|[[:space:]]|$)' |
  grep -v -e '^scripts/deploy/lib/public-host.sh:' -e '^scripts/deploy/test-' || true)"
[ -z "$offenders" ] || fail "scripts apply cloudflared.yaml or k8s/manifests wholesale: $offenders"

# --- set-crawler-keys.sh --tunnel (kind cluster, lib/kind-env.sh) --------------------
# The kind kubectl ensure_kind_kubectl expects, already in place: ${XDG_CACHE_HOME}/econgraph
# holds bin/kubectl (first on PATH) and kubeconfig-econ-graph (KUBECONFIG), so it is used
# as it is. set-crawler-keys.sh runs create-secrets.sh with a scrubbed environment, so that
# kubectl reads the stub settings from $stub_dir/stub.env rather than the environment.
kind_dir="$stub_dir/cache/econgraph"
mkdir -p "$kind_dir/bin" "$stub_dir/yqbin"
echo "apiVersion: v1" >"$kind_dir/kubeconfig-econ-graph"
printf '#!/usr/bin/env bash\nset -a\n. %q\nset +a\nexec %q "$@"\n' "$stub_dir/stub.env" "$stub_dir/kubectl" >"$kind_dir/bin/kubectl"
printf '#!/usr/bin/env bash\necho "27.0 /var/lib/docker [name=seccomp]"\n' >"$kind_dir/bin/docker"
cat >"$stub_dir/yqbin/yq" <<'STUB'
#!/usr/bin/env python3
# Only the '.spec.replicas = N' form apply_cloudflared uses.
import re, sys, yaml
d = yaml.safe_load(open(sys.argv[2]))
d["spec"]["replicas"] = int(re.fullmatch(r"\.spec\.replicas = (\d+)", sys.argv[1])[1])
print(yaml.safe_dump(d))
STUB
chmod +x "$kind_dir/bin/kubectl" "$kind_dir/bin/docker" "$stub_dir/yqbin/yq"
keys=scripts/deploy/set-crawler-keys.sh
# run_keys STUB-SETTINGS ARGS [INPUT]: set-crawler-keys.sh under a terminal, with KUBECTL
# and CLUSTER_ROLE_CONTEXT in the environment pointing elsewhere (they must be replaced).
run_keys() {
  printf 'STUB_LOG=%q\n%s\n' "$log" "$1" >"$stub_dir/stub.env"
  : >"$log"; rm -f "$log.ctx"
  printf '%s' "${3:-}" | env XDG_CACHE_HOME="$stub_dir/cache" KUBECONFIG="$kind_dir/kubeconfig-econ-graph" \
    PATH="$kind_dir/bin:$stub_dir/yqbin:$PATH" KUBECTL="$stub_dir/path/kubectl" CLUSTER_ROLE_CONTEXT=prod-elsewhere \
    script -qec "$keys $2" /dev/null >"$out" 2>&1
}
if [ "$have_script" = true ]; then
  # Not production: refused before the prompt, nothing written or started.
  if run_keys 'STUB_MARKER=none STUB_SECRET=1 STUB_CLOUDFLARED_REPLICAS=0' --tunnel "$fake_token"$'\n'; then
    fail "set-crawler-keys --tunnel: accepted a cluster that is not production"
  fi
  grep -q 'Enter each key' "$out" && fail "set-crawler-keys --tunnel: prompted on a cluster that is not production"
  grep -q 'mark-production-cluster.sh' "$out" || fail "set-crawler-keys --tunnel: the refusal is not shown: $(cat "$out")"
  grep -Eq 'create secret|apply|scale|rollout' "$log" && fail "set-crawler-keys --tunnel: changed a cluster that is not production"
  run_keys 'STUB_MARKER=copied' '--tunnel --dry-run' || fail "set-crawler-keys --tunnel --dry-run failed: $(cat "$out")"
  grep -q 'would refuse the tunnel token' "$out" || fail "set-crawler-keys --tunnel --dry-run: no refusal on a copied marker"

  # Production with a public host: the token is written, then cloudflared is started by
  # apply_cloudflared (1 replica) and restarted; never scaled directly.
  run_keys 'STUB_MARKER=production STUB_SECRET=1 STUB_CLOUDFLARED_REPLICAS=0 STUB_PUBLIC_HOST=econgraph.com' \
    --tunnel "$fake_token"$'\n' || fail "set-crawler-keys --tunnel failed on production: $(cat "$out")"
  grep -q 'create secret generic cloudflare-tunnel --from-file=token=' "$log" ||
    fail "set-crawler-keys --tunnel: the token was not written: $(cat "$out")"
  grep -q 'stdin: .*replicas: 1' "$log" || fail "set-crawler-keys --tunnel: cloudflared not started through apply_cloudflared"
  grep -q 'rollout restart deployment/cloudflared' "$log" || fail "set-crawler-keys --tunnel: cloudflared not restarted"
  grep -q 'scale deployment/cloudflared' "$log" && fail "set-crawler-keys --tunnel: scaled cloudflared directly"
  grep '^kubectl ' "$log" | grep -v -e 'config use-context kind-econ-graph' -e 'config current-context' | grep -v -- '--context kind-econ-graph' &&
    fail "set-crawler-keys --tunnel: a call did not use the kind context"
  grep -q 'prod-elsewhere' "$log" && fail "set-crawler-keys --tunnel: followed an inherited CLUSTER_ROLE_CONTEXT"

  # Production without a public host yet: token written, cloudflared left at 0.
  run_keys 'STUB_MARKER=production STUB_SECRET=1 STUB_CLOUDFLARED_REPLICAS=0' --tunnel "$fake_token"$'\n' ||
    fail "set-crawler-keys --tunnel failed without a public host: $(cat "$out")"
  grep -q 'stays at 0 replicas' "$out" || fail "set-crawler-keys --tunnel: no public host was not reported"
  grep -Eq 'stdin: .*replicas: [1-9]|rollout restart deployment/cloudflared' "$log" &&
    fail "set-crawler-keys --tunnel: started cloudflared without a public host"
  # Every one of these went to the kind kubectl, none to the KUBECTL from the environment.
  [ ! -s "$path_log" ] || fail "set-crawler-keys: calls went to the KUBECTL from the environment: $(cat "$path_log")"
fi

# --- mark-production-cluster.sh run bare: the kind cluster, through its own kubectl --------
printf 'STUB_LOG=%q\nSTUB_MARKER=none\n' "$log" >"$stub_dir/stub.env"
: >"$log"; rm -f "$log.ctx"
env -u KUBECTL XDG_CACHE_HOME="$stub_dir/cache" KUBECONFIG="$kind_dir/kubeconfig-econ-graph" \
  PATH="$kind_dir/bin:$PATH" CLUSTER_ROLE_CONTEXT=prod-elsewhere \
  "$mark" --dry-run </dev/null >"$out" 2>&1 || fail "mark (bare) --dry-run failed: $(cat "$out")"
grep -q 'would write ConfigMap kube-system/econgraph-cluster-role' "$out" || fail "mark (bare) --dry-run: no plan shown: $(cat "$out")"
grep -q '^kubectl config use-context kind-econ-graph' "$log" || fail "mark (bare): did not switch to kind-econ-graph"
grep '^kubectl ' "$log" | grep -v 'config use-context' | grep -v -- '--context kind-econ-graph' &&
  fail "mark (bare): a call did not use the kind context"
[ ! -s "$path_log" ] || fail "mark (bare): called the kubectl on PATH instead of the kind one: $(cat "$path_log")"

# --- kind-env.sh restore_secrets: the saved tunnel token only into a marked cluster --------
backup="$stub_dir/backup"
mkdir -p "$backup/secrets"
for s in grafana-admin cloudflare-tunnel; do
  printf 'apiVersion: v1\nkind: Secret\nmetadata:\n  name: %s\n' "$s" >"$backup/secrets/$s.yaml"
done
for marker in none copied fails production; do
  printf 'STUB_LOG=%q\nSTUB_MARKER=%s\n' "$log" "$marker" >"$stub_dir/stub.env"
  : >"$log"; rm -f "$log.ctx"
  env XDG_CACHE_HOME="$stub_dir/cache" KUBECONFIG="$kind_dir/kubeconfig-econ-graph" \
    PATH="$kind_dir/bin:$PATH" KUBECTL="$stub_dir/path/kubectl" CLUSTER_ROLE_CONTEXT=prod-elsewhere \
    bash -c 'source scripts/deploy/lib/kind-env.sh; DRY_RUN=false; restore_secrets "$1"' _ "$backup" >"$out" 2>&1 ||
    fail "restore_secrets (marker $marker) failed: $(cat "$out")"
  grep -q "create -f $backup/secrets/grafana-admin.yaml" "$log" || fail "restore_secrets (marker $marker): grafana-admin not restored"
  if [ "$marker" = production ]; then
    grep -q "create -f $backup/secrets/cloudflare-tunnel.yaml" "$log" ||
      fail "restore_secrets: the token was not restored into the production cluster"
  else
    grep -q "create -f $backup/secrets/cloudflare-tunnel.yaml" "$log" &&
      fail "restore_secrets (marker $marker): restored the token into a cluster that is not production"
    grep -q 'secret cloudflare-tunnel skipped' "$out" || fail "restore_secrets (marker $marker): the skip was not reported"
  fi
  grep -E 'get (namespace kube-system|configmap econgraph-cluster-role)' "$log" | grep -v -- '--context kind-econ-graph' &&
    fail "restore_secrets (marker $marker): the production check did not use the kind context"
  [ ! -s "$path_log" ] || fail "restore_secrets: called another kubectl: $(cat "$path_log")"
done

# --- deploy.sh --dry-run reaches no cluster --------------------------------------------
# With the kind kubectl layout above (and the real yq, which deploy.sh needs for more than
# the replica count), a dry run writes nothing through $KUBECTL, the kind kubectl or the
# kubectl on PATH, with or without a public host, and reports the production check. It
# runs from a copy of scripts/deploy and k8s only, so deploy.sh skips the dashboard and
# monitoring linters (scripts/test-*.sh), which check other things with other tools.
if command -v yq >/dev/null 2>&1; then
  writes_re='(^| )(apply|delete|patch|scale|create|label|annotate|replace|edit|set|cordon|drain|taint)( |$)|rollout restart'
  deploy_root="$stub_dir/deploy-root"
  mkdir -p "$deploy_root/scripts"
  cp -r scripts/deploy "$deploy_root/scripts/"
  cp -r k8s "$deploy_root/"
  for host in '' econgraph.com; do
    printf 'STUB_LOG=%q\nSTUB_MARKER=none\n' "$log" >"$stub_dir/stub.env"
    : >"$log"; rm -f "$log.ctx"
    if ! env XDG_CACHE_HOME="$stub_dir/cache" KUBECONFIG="$kind_dir/kubeconfig-econ-graph" \
      PATH="$kind_dir/bin:$PATH" KUBECTL="$stub_dir/path/kubectl" CLUSTER_ROLE_CONTEXT=prod-elsewhere \
      PUBLIC_HOST="$host" "$deploy_root/scripts/deploy/deploy.sh" --dry-run </dev/null >"$out" 2>&1; then
      fail "deploy.sh --dry-run (PUBLIC_HOST=$host) failed: $(grep -E '❌|error' "$out" | head -5)"
    fi
    grep -E "$writes_re" "$log" && fail "deploy.sh --dry-run (PUBLIC_HOST=$host) wrote through the kind kubectl"
    [ ! -s "$path_log" ] || fail "deploy.sh --dry-run (PUBLIC_HOST=$host) called another kubectl: $(cat "$path_log")"
    grep -q '\[dry-run\] kubectl --context kind-econ-graph apply -f -' "$out" ||
      fail "deploy.sh --dry-run (PUBLIC_HOST=$host): the cloudflared apply was not printed on the pinned context"
    if [ -n "$host" ]; then
      grep -q '\[dry-run\] check that context kind-econ-graph is marked production' "$out" ||
        fail "deploy.sh --dry-run: the production check was not reported"
    fi
  done
fi

# --- which kubectl when run by hand: create-secrets.sh detection, KUBECONFIG, one file ---
# A second cache (not the kind-env layout above) with tagging stub kubectls: the cached
# kind kubectl and a working plain kubectl first on PATH, so the tests see which ran.
cache2="$stub_dir/cache2/econgraph"
mkdir -p "$cache2/bin" "$stub_dir/plain"
printf '#!/usr/bin/env bash\necho "[cache kubectl] KUBECONFIG=${KUBECONFIG:-}" >>"$STUB_LOG"\nexec %q "$@"\n' "$stub_dir/kubectl" >"$cache2/bin/kubectl"
printf '#!/usr/bin/env bash\necho "[plain kubectl] KUBECONFIG=${KUBECONFIG:-}" >>"$STUB_LOG"\nexec %q "$@"\n' "$stub_dir/kubectl" >"$stub_dir/plain/kubectl"
chmod +x "$cache2/bin/kubectl" "$stub_dir/plain/kubectl"
echo "apiVersion: v1" >"$cache2/kubeconfig-econ-graph"
echo "apiVersion: v1" >"$stub_dir/other-kubeconfig"
# run_bare [VAR=value ...] COMMAND...: no KUBECTL, KUBECONFIG or pinned context, the
# second cache, and the working plain kubectl first on PATH.
run_bare() {
  : >"$log"; rm -f "$log.ctx"
  env -u KUBECTL -u KUBECONFIG -u CLUSTER_ROLE_CONTEXT XDG_CACHE_HOME="$stub_dir/cache2" \
    PATH="$stub_dir/plain:$PATH" "$@" </dev/null >"$out" 2>&1
}
# used_only WHO KUBECONFIG LABEL: every call went to WHO ("cache" or "plain") with KUBECONFIG.
used_only() {
  local other=plain
  [ "$1" = plain ] && other=cache
  if ! grep -q "^\[$1 kubectl\] KUBECONFIG=$2\$" "$log"; then
    fail "$3: did not run the $1 kubectl with KUBECONFIG=$2: $(head -3 "$log")"
  fi
  if grep -v "^\[$1 kubectl\] KUBECONFIG=$2\$" "$log" | grep -q '^\['; then
    fail "$3: also ran another kubectl or kubeconfig: $(grep '^\[' "$log" | sort -u)"
  fi
  if grep -q "^\[$other kubectl\]" "$log"; then
    fail "$3: ran the $other kubectl"
  fi
}
detected="Using the kind cluster's kubectl $cache2/bin/kubectl and KUBECONFIG=$cache2/kubeconfig-econ-graph"
run_bare STUB_MARKER=none scripts/deploy/create-secrets.sh || fail "create-secrets (bare) failed: $(cat "$out")"
grep -qF "$detected" "$out" || fail "create-secrets (bare): did not say it uses the kind kubectl: $(cat "$out")"
used_only cache "$cache2/kubeconfig-econ-graph" "create-secrets (bare)"
# An exported KUBECONFIG skips the detection, and says so, for both scripts.
run_bare KUBECONFIG="$stub_dir/other-kubeconfig" STUB_MARKER=none scripts/deploy/create-secrets.sh ||
  fail "create-secrets with KUBECONFIG failed: $(cat "$out")"
grep -q "KUBECONFIG is set ($stub_dir/other-kubeconfig)" "$out" || fail "create-secrets with KUBECONFIG: skipping detection was not said"
used_only plain "$stub_dir/other-kubeconfig" "create-secrets with KUBECONFIG"
run_bare KUBECONFIG="$stub_dir/other-kubeconfig" STUB_MARKER=none "$mark" --dry-run ||
  fail "mark with KUBECONFIG failed: $(cat "$out")"
grep -q "KUBECONFIG is set ($stub_dir/other-kubeconfig)" "$out" || fail "mark with KUBECONFIG: skipping the kind kubectl was not said"
grep -qE "KUBECONFIG +$stub_dir/other-kubeconfig" "$out" || fail "mark with KUBECONFIG: the kubeconfig in use is not shown"
grep -qE "context +kind-test" "$out" || fail "mark with KUBECONFIG: the context in use is not shown"
used_only plain "$stub_dir/other-kubeconfig" "mark with KUBECONFIG"
# An explicit KUBECTL wins silently; a context pinned by deploy.sh keeps deploy.sh's kubectl.
run_bare KUBECTL="$stub_dir/plain/kubectl" STUB_MARKER=none scripts/deploy/create-secrets.sh ||
  fail "create-secrets with KUBECTL failed: $(cat "$out")"
grep -qE "Using the kind cluster|KUBECONFIG is set|warning: found" "$out" && fail "create-secrets with KUBECTL: detection spoke"
used_only plain "" "create-secrets with KUBECTL"
run_bare CLUSTER_ROLE_CONTEXT=kind-test STUB_MARKER=none scripts/deploy/create-secrets.sh ||
  fail "create-secrets with a pinned context failed: $(cat "$out")"
grep -qE "Using the kind cluster|KUBECONFIG is set|warning: found" "$out" && fail "create-secrets with a pinned context: detection spoke"
grep -q -- '--context kind-test -n econ-graph create secret generic' "$log" || fail "create-secrets with a pinned context: did not write on it"
used_only plain "" "create-secrets with a pinned context"
# Only one of the two cached files: plain kubectl, with a warning naming what is missing.
mv "$cache2/kubeconfig-econ-graph" "$cache2/kubeconfig-econ-graph.away"
run_bare STUB_MARKER=none scripts/deploy/create-secrets.sh || fail "create-secrets (kubectl only) failed: $(cat "$out")"
grep -qF "warning: found $cache2/bin/kubectl but not $cache2/kubeconfig-econ-graph: using plain kubectl" "$out" ||
  fail "create-secrets (kubectl only): no warning: $(cat "$out")"
used_only plain "" "create-secrets (kubectl only)"
mv "$cache2/kubeconfig-econ-graph.away" "$cache2/kubeconfig-econ-graph"
mv "$cache2/bin/kubectl" "$cache2/bin/kubectl.away"
run_bare STUB_MARKER=none scripts/deploy/create-secrets.sh || fail "create-secrets (kubeconfig only) failed: $(cat "$out")"
grep -qF "warning: found $cache2/kubeconfig-econ-graph but not $cache2/bin/kubectl: using plain kubectl" "$out" ||
  fail "create-secrets (kubeconfig only): no warning: $(cat "$out")"
used_only plain "" "create-secrets (kubeconfig only)"
mv "$cache2/bin/kubectl.away" "$cache2/bin/kubectl"
[ ! -s "$path_log" ] || fail "detection tests: called the failing kubectl on PATH: $(cat "$path_log")"

if [ "$failures" -gt 0 ]; then
  echo "$failures check(s) failed"
  exit 1
fi
echo "public-host.sh, create-secrets.sh, mark-production-cluster.sh and set-crawler-keys.sh: all checks passed"
