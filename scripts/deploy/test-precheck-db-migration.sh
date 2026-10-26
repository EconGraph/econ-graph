#!/usr/bin/env bash
# Tests for scripts/deploy/precheck-db-migration.sh against a stub kubectl, no cluster:
#   - a running postgres-backup Job blocks (exit 1) before anything else, a finished one doesn't;
#   - --stop-crawler scales crawler-worker to 0 and waits for its pods; with --dry-run it only
#     says so; a worker already at 0 with no pods is left alone;
#   - long or idle-in-transaction sessions on the data database are listed and exit 1; the
#     script never cancels or terminates a session; a quiet database exits 0;
#   - every kubectl call goes to $KUBECTL with --context (default kind-econ-graph), never the
#     kubectl on PATH, and the current context is not changed; run with no KUBECTL or
#     KUBECONFIG it uses the kind cluster's kubectl and kubeconfig in ~/.cache/econgraph.
#
# Usage: scripts/deploy/test-precheck-db-migration.sh (from anywhere)
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
script="$repo_root/scripts/deploy/precheck-db-migration.sh"
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
#   STUB_BACKUP_ACTIVE=1  a postgres-backup Job is running (another, finished, always exists)
#   STUB_REPLICAS         crawler-worker's replicas (default 1; "none": no Deployment)
#   STUB_PODS=0           no crawler-worker pods
#   STUB_SESSIONS=1       pg_stat_activity reports a blocking session
cat >"$work/kubectl" <<'STUB'
#!/usr/bin/env bash
args="$*"
echo "kubectl ${args//$'\n'/ }" >>"$STUB_LOG"
case "$*" in
  *"config get-contexts -o name"*) printf 'kind-econ-graph\nother\n' ;;
  *"config current-context"*) echo other ;;
  *"get namespace kube-system"*) printf 'uid-this-cluster' ;;
  *"get configmap econgraph-cluster-role"*) printf 'production uid-this-cluster' ;;
  *"get jobs"*)
    echo "postgres-backup-29123456 "
    echo "keycloak-realm-import 1"
    [ "${STUB_BACKUP_ACTIVE:-0}" != 1 ] || echo "postgres-backup-manual-1761400000 1" ;;
  *"get deployment crawler-worker"*)
    r="${STUB_REPLICAS:-1}"; [ "$r" = none ] || printf '%s' "$r" ;;
  *"get pods -l app=crawler-worker"*) [ "${STUB_PODS:-1}" = 0 ] || echo pod/crawler-worker-abc ;;
  *"exec -i postgresql-0 -- psql"*)
    [ "${STUB_SESSIONS:-0}" != 1 ] ||
      printf '4242\tpostgres\tpsql\tlocal\tidle in transaction\t37\tClient\tSELECT * FROM data_points\n' ;;
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

# Quiet cluster: passes, reads only.
run --
[ "$status" = 0 ] || fail "a quiet cluster did not pass: $(cat "$out")"
called "exec -i postgresql-0 -- psql -U postgres -d econ_graph" || fail "did not query econ_graph"
if called "scale"; then fail "scaled without --stop-crawler"; fi

# A running backup blocks, before the sessions or the crawler are touched.
run STUB_BACKUP_ACTIVE=1 -- --stop-crawler
[ "$status" = 1 ] || fail "a running backup did not block (status $status)"
grep -q "postgres-backup-manual-1761400000" "$out" || fail "the running backup is not named"
if called "scale" || called "psql"; then fail "went on past a running backup"; fi

# --stop-crawler scales to 0 and waits for the pods.
run -- --stop-crawler
[ "$status" = 0 ] || fail "--stop-crawler failed: $(cat "$out")"
called "scale deployment/crawler-worker --replicas=0" || fail "did not scale crawler-worker to 0"
called "wait --for=delete pod -l app=crawler-worker" || fail "did not wait for the crawler pods"

# --dry-run --stop-crawler only says what it would do.
run -- --dry-run --stop-crawler
[ "$status" = 0 ] || fail "dry run failed: $(cat "$out")"
grep -q "would scale deployment/crawler-worker in econ-graph from 1 to 0" "$out" ||
  fail "dry run did not show the plan: $(cat "$out")"
if called "scale" || called "wait --for"; then fail "dry run changed the cluster"; fi

# A worker already stopped is left alone; so is a cluster without one.
run STUB_REPLICAS=0 STUB_PODS=0 -- --stop-crawler
[ "$status" = 0 ] || fail "a stopped worker failed"
if called "scale" || called "wait --for"; then fail "touched a stopped worker"; fi
run STUB_REPLICAS=none STUB_PODS=0 -- --stop-crawler
[ "$status" = 0 ] || fail "a cluster without the worker failed"
if called "scale"; then fail "scaled a missing worker"; fi

# Blocking sessions are listed, exit 1, and are never ended.
run STUB_SESSIONS=1 -- --dry-run
[ "$status" = 1 ] || fail "a blocking session did not fail the check (status $status)"
if ! grep -q "4242" "$out" || ! grep -q "idle in transaction" "$out"; then
  fail "the session is not listed: $(cat "$out")"
fi
if grep -qiE "pg_terminate_backend|pg_cancel_backend" "$log"; then fail "ended a session"; fi

# Context: --context is passed on every cluster call, never a use-context; unknown contexts stop.
run -- --context other
[ "$status" = 0 ] || fail "--context other failed"
if grep -v "config " "$log" | grep -qv -- "--context other"; then fail "a call without --context other"; fi
if called "use-context"; then fail "changed the current context"; fi
run -- --context missing
[ "$status" = 1 ] || fail "an unknown context did not stop"

# With no KUBECTL or KUBECONFIG, the kind cluster's kubectl kept in ~/.cache/econgraph is used.
cache="$work/cache/econgraph"
mkdir -p "$cache/bin"
cp "$work/kubectl" "$cache/bin/kubectl"
echo "apiVersion: v1" >"$cache/kubeconfig-econ-graph"
: >"$log"
status=0
env -u KUBECTL -u KUBECONFIG XDG_CACHE_HOME="$work/cache" "$script" >"$out" 2>&1 || status=$?
[ "$status" = 0 ] || fail "the cached kind kubectl was not used: $(cat "$out")"
grep -q "Using the kind cluster's kubectl $cache/bin/kubectl" "$out" || fail "did not say which kubectl it used"

# Arguments.
status=0; "$script" --bogus >/dev/null 2>&1 || status=$?
[ "$status" = 2 ] || fail "an unknown argument did not exit 2"

if [ -s "$path_log" ]; then fail "the kubectl on PATH was called: $(cat "$path_log")"; fi
if [ "$failures" -gt 0 ]; then
  echo "$failures failure(s)"
  exit 1
fi
echo "precheck-db-migration: all checks passed"
