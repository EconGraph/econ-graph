#!/usr/bin/env bash
# The functions are extracted from cutover-to-gateway.sh and called in subshells.
# shellcheck disable=SC2016,SC2034,SC2329
# Tests for cutover-to-gateway.sh --rollback's NetworkPolicy step
# (apply_rollback_policy), against a stub kubectl, no cluster:
#   - with another NetworkPolicy in econ-graph, econ-graph-ingress-nginx-rollback is
#     applied, and admits ingress-nginx on 3000, 8080 and 9876 only (not Postgres);
#   - with none (or only a left-over rollback policy), it is not applied, and a
#     left-over one is deleted: alone it would deny every other connection to the
#     namespace's pods, the backend's to Postgres among them;
#   - in a dry run nothing is changed, the saved policies count (a real run applies
#     them first), and the decision is printed;
#   - a failed lookup stops the rollback.
#
# Usage: scripts/deploy/test-cutover-rollback-policy.sh (from anywhere)
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
script="$repo_root/scripts/deploy/cutover-to-gateway.sh"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
log="$work/log"
out="$work/out"
failed=0
fail() { echo "FAIL $*"; failed=1; }

# The functions under test, as the script defines them.
extract() { sed -n "/^$1() {\$/,/^}\$/p" "$script"; }
funcs="$work/funcs.sh"
{ extract rollback_policy_yaml; extract apply_rollback_policy; } >"$funcs"
grep -q '^apply_rollback_policy() {$' "$funcs" || { echo "FAIL apply_rollback_policy not found in $script"; exit 1; }

saved="$work/networkpolicies.yaml"
cat >"$saved" <<'EOF'
apiVersion: v1
items:
- apiVersion: networking.k8s.io/v1
  kind: NetworkPolicy
  metadata:
    name: econ-graph-network-policy
    namespace: econ-graph
  spec:
    podSelector: {}
kind: List
EOF

# run_case DRY_RUN LIVE [SAVED]: apply_rollback_policy with a stub kc whose
# `get networkpolicy -o name` prints LIVE (STUB_GET_FAILS=1: fails). Every kc call,
# and what is piped to `apply -f -`, goes to $log; output to $out.
run_case() {
  : >"$log"
  (
    NAMESPACE=econ-graph
    ROLLBACK_POLICY=econ-graph-ingress-nginx-rollback
    KUBE_CONTEXT=kind-test
    DRY_RUN="$1"
    stub_live="$2"
    die() { echo "error: $*"; exit 1; }
    run() { echo "+ $*"; if [ "$DRY_RUN" != true ]; then "$@"; fi; }
    kc() {
      echo "kc $*" >>"$log"
      case "$*" in
        *"get networkpolicy -o name"*)
          [ "${STUB_GET_FAILS:-0}" != 1 ] || return 1
          [ -z "$stub_live" ] || printf "%s\n" "$stub_live" ;;
        *"apply -f -"*) sed 's/^/  stdin: /' >>"$log" ;;
      esac
    }
    # shellcheck source=/dev/null
    source "$funcs"
    apply_rollback_policy "${3:-$work/none.yaml}"
  ) >"$out" 2>&1
}

# Another policy in force: applied, ingress-nginx admitted on the app ports only.
run_case false 'networkpolicy.networking.k8s.io/econ-graph-network-policy' ||
  fail "with another policy: failed: $(cat "$out")"
grep -q '^kc apply -f -$' "$log" || fail "with another policy: the rollback policy was not applied"
grep -q 'stdin:   name: econ-graph-ingress-nginx-rollback' "$log" || fail "with another policy: wrong policy applied"
ports="$(sed -n 's/^  stdin:       port: //p' "$log" | tr '\n' ' ')"
[ "$ports" = "3000 8080 9876 " ] || fail "with another policy: ports are '$ports', not 3000 8080 9876"
grep -q 'app.kubernetes.io/name: ingress-nginx' "$log" || fail "with another policy: not limited to ingress-nginx pods"
grep -q 'delete networkpolicy' "$log" && fail "with another policy: a policy was deleted"

# No policy at all: not applied; a left-over one is deleted.
run_case false '' || fail "without policies: failed: $(cat "$out")"
grep -q 'apply' "$log" && fail "without policies: the rollback policy was applied (it would deny backend -> Postgres)"
grep -q '^kc -n econ-graph delete networkpolicy econ-graph-ingress-nginx-rollback --ignore-not-found$' "$log" ||
  fail "without policies: a left-over rollback policy is not deleted"
grep -q 'is not added' "$out" || fail "without policies: the decision was not printed"

# Only a left-over rollback policy: the same as none.
run_case false 'networkpolicy.networking.k8s.io/econ-graph-ingress-nginx-rollback' ||
  fail "only the rollback policy: failed: $(cat "$out")"
grep -q 'apply' "$log" && fail "only the rollback policy: it was re-applied on its own"
grep -q 'delete networkpolicy econ-graph-ingress-nginx-rollback' "$log" || fail "only the rollback policy: not deleted"

# Dry run, nothing live but a saved policy (which a real run applies first): printed, not applied.
run_case true '' "$saved" || fail "dry run with a saved policy: failed: $(cat "$out")"
grep -q 'apply' "$log" && fail "dry run: applied"
grep -q 'delete' "$log" && fail "dry run: deleted"
grep -q 'econ-graph-network-policy): adding econ-graph-ingress-nginx-rollback' "$out" ||
  fail "dry run with a saved policy: the decision was not printed: $(cat "$out")"
grep -q '    kind: NetworkPolicy' "$out" || fail "dry run with a saved policy: the policy was not printed"

# Dry run, no policy anywhere: the decision printed, nothing changed.
run_case true '' || fail "dry run without policies: failed: $(cat "$out")"
grep -q 'apply\|delete' "$log" && fail "dry run without policies: changed something"
grep -q 'is not added' "$out" || fail "dry run without policies: the decision was not printed"

# The lookup fails: stop, change nothing.
if STUB_GET_FAILS=1 run_case false 'networkpolicy.networking.k8s.io/econ-graph-network-policy'; then
  fail "a failed lookup did not stop the rollback"
fi
grep -q 'apply\|delete' "$log" && fail "a failed lookup changed something"

# The rollback's post-check also asks the backend for data, not only the frontend root.
grep -q '^    wait_backend_db || die ' "$script" || fail "the rollback does not check the backend reaches its database"

if [ "$failed" != 0 ]; then
  echo "cutover rollback policy checks failed"
  exit 1
fi
echo "cutover rollback policy: all checks passed"
