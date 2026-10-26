#!/usr/bin/env bash
# Tests for scripts/deploy/refetch-full-history.sh against a stub kubectl, no cluster:
#   - every kubectl call goes to $KUBECTL with --context kind-econ-graph, never the kubectl on
#     PATH, and the current context is not changed; without that context it stops before
#     running anything; run with no KUBECTL or KUBECONFIG it uses the kind cluster's kubectl
#     and kubeconfig in ~/.cache/econgraph;
#   - the cluster's context, kube-system UID and production marker are printed before the
#     crawler runs;
#   - --dry-run passes --dry-run to the crawler and prints no progress or resume hint;
#   - the Resume hint repeats --include-inactive and --priority N, and the source selection;
#   - --status runs refetch-status.
#
# Usage: scripts/deploy/test-refetch-full-history.sh (from anywhere)
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
script="$repo_root/scripts/deploy/refetch-full-history.sh"
failures=0
fail() {
  echo "FAIL $1"
  failures=$((failures + 1))
}

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
log="$work/kubectl.log"
out="$work/out"
path_log="$work/path.log"

# The stub cluster. Every call is appended to $STUB_LOG; nothing is changed.
#   STUB_NO_KIND=1  there is no kind-econ-graph context
cat >"$work/kubectl" <<'STUB'
#!/usr/bin/env bash
echo "kubectl $*" >>"$STUB_LOG"
case "$*" in
  *"config get-contexts -o name"*)
    [ "${STUB_NO_KIND:-0}" = 1 ] || echo kind-econ-graph
    echo other ;;
  *"config current-context"*) echo other ;;
  *"get namespace kube-system"*) printf 'uid-this-cluster' ;;
  *"get configmap econgraph-cluster-role"*) printf 'production uid-this-cluster' ;;
  *"exec deploy/crawler-worker -- /app/crawler"*) echo "crawler ran" ;;
esac
exit 0
STUB
chmod +x "$work/kubectl"
mkdir "$work/path"
printf '#!/usr/bin/env bash\necho "kubectl on PATH: $*" >>%q\nexit 1\n' "$path_log" >"$work/path/kubectl"
chmod +x "$work/path/kubectl"
export PATH="$work/path:$PATH" STUB_LOG="$log"

# run [ENV=VALUE...] -- ARGS...: runs the script with KUBECTL=the stub; status in $status.
run() {
  : >"$log"
  local envs=()
  while [ "$1" != -- ]; do envs+=("$1"); shift; done
  shift
  status=0
  env KUBECTL="$work/kubectl" "${envs[@]}" "$script" "$@" >"$out" 2>&1 || status=$?
}
called() { grep -qF -- "$1" "$log"; }
# Every call but listing the contexts is on --context kind-econ-graph, and none changes it.
all_pinned() {
  local line
  while IFS= read -r line; do
    case "$line" in
      "kubectl config get-contexts -o name") ;;
      "kubectl --context kind-econ-graph "*) ;;
      *) return 1 ;;
    esac
  done <"$log"
  ! grep -q "use-context" "$log"
}

# Enqueue with the options a resume must keep.
run -- --source FRED,BLS --include-inactive --priority 7
[ "$status" = 0 ] || fail "enqueue failed: $(cat "$out")"
called "kubectl --context kind-econ-graph -n econ-graph exec deploy/crawler-worker -- /app/crawler refetch --source FRED,BLS --include-inactive --priority 7" ||
  fail "the crawler was not run on kind-econ-graph with the options: $(cat "$log")"
all_pinned || fail "a kubectl call was not pinned to kind-econ-graph: $(cat "$log")"
grep -q "^Cluster: context kind-econ-graph, kube-system UID uid-this-cluster, marked production" "$out" ||
  fail "the cluster identity is not shown: $(cat "$out")"
identity_line="$(grep -n "^Cluster:" "$out" | cut -d: -f1)"
crawler_line="$(grep -n "^crawler ran" "$out" | cut -d: -f1)"
[ "${identity_line:-999}" -lt "${crawler_line:-0}" ] ||
  fail "the cluster identity is not shown before the crawler runs: $(cat "$out")"
grep -qE "^Resume: +$script --source FRED,BLS --include-inactive --priority 7 --completed-since [0-9TZ:-]+$" "$out" ||
  fail "the Resume hint drops options: $(cat "$out")"
grep -qE "^Progress: +$script --status --source FRED,BLS --since [0-9TZ:-]+ --failures 50$" "$out" ||
  fail "the Progress hint is wrong: $(cat "$out")"

# A resumed run keeps the original start and the options.
run -- --all --priority 3 --completed-since 2026-10-26T00:00:00Z
[ "$status" = 0 ] || fail "resume failed: $(cat "$out")"
grep -qxE "Resume: +$script --all --priority 3 --completed-since 2026-10-26T00:00:00Z" "$out" ||
  fail "the resumed Resume hint is wrong: $(cat "$out")"

# Without extra options the hint has none.
run -- --all
grep -qE "^Resume: +$script --all --completed-since [0-9TZ:-]+$" "$out" ||
  fail "the plain Resume hint is wrong: $(cat "$out")"

# --dry-run: the crawler's own dry run, no hints.
run -- --all --dry-run --include-inactive
[ "$status" = 0 ] || fail "dry run failed: $(cat "$out")"
called "/app/crawler refetch --all --dry-run --include-inactive" || fail "--dry-run was not passed on: $(cat "$log")"
if grep -qE "^(Resume|Progress):" "$out"; then fail "dry run printed hints: $(cat "$out")"; fi
all_pinned || fail "a dry-run kubectl call was not pinned: $(cat "$log")"

# --status runs refetch-status on the pinned context.
run -- --status --all --since 2026-10-26T00:00:00Z --failures 5
[ "$status" = 0 ] || fail "--status failed: $(cat "$out")"
called "kubectl --context kind-econ-graph -n econ-graph exec deploy/crawler-worker -- /app/crawler refetch-status --all --since 2026-10-26T00:00:00Z --failures 5" ||
  fail "refetch-status was not run: $(cat "$log")"
all_pinned || fail "a --status kubectl call was not pinned: $(cat "$log")"

# Without the kind context it stops before running anything.
run STUB_NO_KIND=1 -- --all
[ "$status" = 1 ] || fail "a missing kind-econ-graph context did not stop it (status $status)"
grep -q "has no context kind-econ-graph" "$out" || fail "the missing context is not named: $(cat "$out")"
if called "exec"; then fail "ran the crawler without the kind context"; fi

# With no KUBECTL or KUBECONFIG, the kind cluster's kubectl kept in ~/.cache/econgraph is
# used, still on the pinned context.
cache="$work/cache/econgraph"
mkdir -p "$cache/bin"
cp "$work/kubectl" "$cache/bin/kubectl"
echo "apiVersion: v1" >"$cache/kubeconfig-econ-graph"
: >"$log"
status=0
env -u KUBECTL -u KUBECONFIG XDG_CACHE_HOME="$work/cache" "$script" --all >"$out" 2>&1 || status=$?
[ "$status" = 0 ] || fail "the cached kind kubectl was not used: $(cat "$out")"
grep -q "Using the kind cluster's kubectl $cache/bin/kubectl" "$out" || fail "did not say which kubectl it used"
all_pinned || fail "a kubectl call with the cached kubectl was not pinned: $(cat "$log")"

# The kubectl on PATH (the microk8s wrapper on production) is never called.
if [ -s "$path_log" ]; then fail "the kubectl on PATH was called: $(cat "$path_log")"; fi

if [ "$failures" -ne 0 ]; then
  echo "$failures failure(s)"
  exit 1
fi
echo "refetch-full-history: all checks passed"
