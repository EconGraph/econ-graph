#!/usr/bin/env bash
# The stub kubectls are written as quoted heredocs and run on their own.
# shellcheck disable=SC2016
#
# Tests which cluster scripts/deploy/restore-postgres-backup.sh and
# backup-postgres-now.sh reach, against stub kubectls, no cluster:
#   - with none of KUBECTL, KUBECONFIG and CLUSTER_ROLE_CONTEXT set, they use
#     the kind cluster's kubectl and kubeconfig kept in ~/.cache/econgraph,
#     and the plain kubectl on PATH (a stand-in for the microk8s snap wrapper)
#     is never called;
#   - every kubectl call carries --context kind-econ-graph (or --context's
#     value), also when another CLUSTER_ROLE_CONTEXT is inherited;
#   - the cluster's identity (context, API server, nodes, kube-system UID,
#     production marker) is shown before the restore's confirmation, and an
#     unconfirmed restore, or one on a context that doesn't exist, applies
#     nothing; an empty --context is refused before any call;
#   - the restore Job applied names the dump and RESTORE_OWNER=econgraph.
#
# Usage: scripts/deploy/test-postgres-backup-cluster.sh (from anywhere)
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
restore="$repo_root/scripts/deploy/restore-postgres-backup.sh"
backup="$repo_root/scripts/deploy/backup-postgres-now.sh"

failures=0
fail() {
    echo "FAIL $1"
    failures=$((failures + 1))
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
export STUB_LOG="$tmp/log" STUB_APPLIED="$tmp/applied.yaml"

# The plain kubectl on PATH: logs the call and fails.
mkdir -p "$tmp/path"
cat >"$tmp/path/kubectl" <<'STUB'
#!/usr/bin/env bash
echo "PLAIN kubectl $*" >>"$STUB_LOG"
exit 1
STUB
chmod +x "$tmp/path/kubectl"

# The kind cluster's kubectl, as kept in ~/.cache/econgraph. Every call is
# appended to $STUB_LOG.
#   STUB_MARKER=production  the cluster carries a production marker for its UID
#   STUB_WAIT_FAILS=1       kubectl wait fails
cache="$tmp/cache/econgraph"
mkdir -p "$cache/bin"
echo "apiVersion: v1" >"$cache/kubeconfig-econ-graph"
cat >"$cache/bin/kubectl" <<'STUB'
#!/usr/bin/env bash
echo "kubectl $*" >>"$STUB_LOG"
context=""
if [ "${1:-}" = --context ]; then context="$2"; shift 2; fi
case "$context" in
  kind-econ-graph | kind-other) ;;
  *) echo "error: context \"$context\" does not exist" >&2; exit 1 ;;
esac
case "$*" in
  "config view --minify -o jsonpath={.clusters[0].cluster.server}") printf 'https://127.0.0.1:41234' ;;
  "get nodes "*) printf 'econ-graph-control-plane' ;;
  "get namespace kube-system "*) printf 'uid-this-cluster' ;;
  "-n kube-system get configmap econgraph-cluster-role "*)
    [ "${STUB_MARKER:-none}" != production ] || printf 'production uid-this-cluster' ;;
  "run postgres-backup-list-"*) echo "econ_graph-20260101-080000.dump" ;;
  "apply -f -") cat >"$STUB_APPLIED"; echo "job.batch/postgres-restore created" ;;
  "wait "*) [ "${STUB_WAIT_FAILS:-0}" != 1 ] || exit 1 ;;
  "logs "*) echo "Restore complete." ;;
  "delete job "*) ;;
  "create job "*) ;;
  *) echo "stub: unexpected call: $*" >&2; exit 1 ;;
esac
STUB
chmod +x "$cache/bin/kubectl"

# run SCRIPT ARGS...: as an operator on the production host would, with only
# the plain kubectl on PATH. Output in $tmp/out, status in $status.
run() {
    : >"$STUB_LOG"
    rm -f "$STUB_APPLIED"
    status=0
    env -u KUBECTL -u KUBECONFIG -u CLUSTER_ROLE_CONTEXT \
        HOME="$tmp/home" XDG_CACHE_HOME="$tmp/cache" PATH="$tmp/path:/usr/bin:/bin" \
        "$@" >"$tmp/out" 2>&1 </dev/null || status=$?
}
plain() { grep '^PLAIN' "$STUB_LOG" || true; }
# Calls without the expected context ($1, default kind-econ-graph).
unpinned() { grep -v -e "^kubectl --context ${1:-kind-econ-graph} " "$STUB_LOG" || true; }
calls() { grep -c . "$STUB_LOG" || true; }
check_pinned() { # check_pinned LABEL [CONTEXT]
    [ -z "$(plain)" ] || fail "$1: called the plain kubectl: $(plain)"
    [ -z "$(unpinned "${2:-}")" ] || fail "$1: kubectl calls without the context: $(unpinned "${2:-}")"
    [ "$(calls)" -gt 0 ] || fail "$1: made no kubectl call"
}
check_identity() { # check_identity LABEL
    grep -q "kubectl         $cache/bin/kubectl" "$tmp/out" || fail "$1: does not name the kind cluster's kubectl"
    grep -q "KUBECONFIG      $cache/kubeconfig-econ-graph" "$tmp/out" || fail "$1: does not name the kubeconfig"
    grep -q "API server      https://127.0.0.1:41234" "$tmp/out" || fail "$1: does not show the API server"
    grep -q "nodes           econ-graph-control-plane" "$tmp/out" || fail "$1: does not show the nodes"
    grep -q "kube-system UID uid-this-cluster" "$tmp/out" || fail "$1: does not show the kube-system UID"
}

# restore --list
run "$restore" --list
[ "$status" = 0 ] || fail "restore --list exits $status: $(cat "$tmp/out")"
check_pinned "restore --list"
check_identity "restore --list"
grep -q "econ_graph-20260101-080000.dump" "$tmp/out" || fail "restore --list does not list the dumps"

# An unconfirmed restore (no terminal, no --yes): identity first, nothing applied.
run "$restore"
[ "$status" != 0 ] || fail "restore without a terminal or --yes succeeded"
check_pinned "unconfirmed restore"
check_identity "unconfirmed restore"
[ ! -e "$STUB_APPLIED" ] || fail "an unconfirmed restore applied a Job"
uid_line="$(grep -n 'kube-system UID' "$tmp/out" | head -n 1 | cut -d: -f1)"
refusal_line="$(grep -n 'terminal only' "$tmp/out" | head -n 1 | cut -d: -f1)"
[ -n "$uid_line" ] && [ -n "$refusal_line" ] && [ "$uid_line" -lt "$refusal_line" ] ||
    fail "the cluster identity is not shown before the confirmation: $(cat "$tmp/out")"
grep -q "cluster role    not production" "$tmp/out" || fail "restore does not say the cluster is not production"

# A context that doesn't exist: nothing but the check.
run "$restore" --context kind-missing --yes
[ "$status" != 0 ] || fail "restore on a missing context succeeded"
[ "$(calls)" = 1 ] || fail "restore on a missing context made more than the check: $(cat "$STUB_LOG")"
[ -z "$(plain)" ] || fail "restore on a missing context called the plain kubectl"
[ ! -e "$STUB_APPLIED" ] || fail "restore on a missing context applied a Job"

# An empty context name (which kubectl would read as "the current context")
# is refused before any call, by both scripts and by the lib function.
for script in "$restore" "$backup"; do
    run "$script" --context "" --yes
    [ "$status" = 2 ] || fail "$(basename "$script") --context \"\" exits $status: $(cat "$tmp/out")"
    [ "$(calls)" = 0 ] || fail "$(basename "$script") --context \"\" made calls: $(cat "$STUB_LOG")"
    [ ! -e "$STUB_APPLIED" ] || fail "$(basename "$script") --context \"\" applied a Job"
done
run bash -c 'source "$1"; KUBECTL="$2"; cluster_role_pin_named_context ""' _ \
    "$repo_root/scripts/deploy/lib/cluster-role.sh" "$cache/bin/kubectl"
[ "$status" != 0 ] || fail "cluster_role_pin_named_context \"\" succeeded"
[ "$(calls)" = 0 ] || fail "cluster_role_pin_named_context \"\" made calls: $(cat "$STUB_LOG")"

# A confirmed restore.
run "$restore" --file econ_graph-20260101-080000.dump --yes
[ "$status" = 0 ] || fail "restore --yes exits $status: $(cat "$tmp/out")"
check_pinned "restore --yes"
check_identity "restore --yes"
if [ ! -s "$STUB_APPLIED" ]; then
    fail "restore --yes applied no Job"
else
    grep -A1 -- '- name: RESTORE_OWNER' "$STUB_APPLIED" | grep -q 'value: "econgraph"' ||
        fail "the restore Job does not set RESTORE_OWNER=econgraph"
    grep -A1 -- '- name: BACKUP_FILE' "$STUB_APPLIED" | grep -q 'value: "/backups/econ_graph-20260101-080000.dump"' ||
        fail "the restore Job does not name the dump"
fi
order="$(grep -oE '^kubectl --context kind-econ-graph (apply|wait|logs|delete)' "$STUB_LOG" | awk '{ print $4 }' | tr '\n' ,)"
[ "$order" = "apply,wait,logs,delete," ] || fail "restore steps in the wrong order: $order"

# An inherited CLUSTER_ROLE_CONTEXT is replaced, not followed.
: >"$STUB_LOG"
status=0
env -u KUBECTL -u KUBECONFIG CLUSTER_ROLE_CONTEXT=kind-other \
    HOME="$tmp/home" XDG_CACHE_HOME="$tmp/cache" PATH="$tmp/path:/usr/bin:/bin" \
    "$restore" --list >"$tmp/out" 2>&1 </dev/null || status=$?
[ "$status" = 0 ] || fail "restore --list with an inherited context exits $status: $(cat "$tmp/out")"
check_pinned "restore --list with CLUSTER_ROLE_CONTEXT=kind-other inherited"

# --context picks another context, and every call carries it.
run "$restore" --list --context kind-other
[ "$status" = 0 ] || fail "restore --list --context kind-other exits $status: $(cat "$tmp/out")"
check_pinned "restore --list --context kind-other" kind-other

# A cluster marked production says so before the confirmation.
STUB_MARKER=production run "$restore"
grep -q "cluster role    PRODUCTION" "$tmp/out" || fail "restore on the production cluster does not say so: $(cat "$tmp/out")"

# An explicit KUBECTL wins over the cache, and is pinned too.
: >"$STUB_LOG"
status=0
env -u KUBECONFIG -u CLUSTER_ROLE_CONTEXT KUBECTL="$cache/bin/kubectl" \
    HOME="$tmp/home" XDG_CACHE_HOME="$tmp/nonexistent" PATH="$tmp/path:/usr/bin:/bin" \
    "$restore" --list >"$tmp/out" 2>&1 </dev/null || status=$?
[ "$status" = 0 ] || fail "restore --list with KUBECTL set exits $status: $(cat "$tmp/out")"
check_pinned "restore --list with KUBECTL set"

# backup-postgres-now.sh
run "$backup"
[ "$status" = 0 ] || fail "backup-postgres-now exits $status: $(cat "$tmp/out")"
check_pinned "backup-postgres-now"
check_identity "backup-postgres-now"
grep -q '^kubectl --context kind-econ-graph create job postgres-backup-manual-[0-9]* --from=cronjob/postgres-backup -n econ-graph$' "$STUB_LOG" ||
    fail "backup-postgres-now did not create the Job from the CronJob: $(cat "$STUB_LOG")"
STUB_WAIT_FAILS=1 run "$backup"
[ "$status" = 1 ] || fail "backup-postgres-now with a failed Job exits $status"
check_pinned "backup-postgres-now with a failed Job"
grep -q '^kubectl --context kind-econ-graph logs job/postgres-backup-manual-' "$STUB_LOG" ||
    fail "backup-postgres-now with a failed Job does not show its logs"
run "$backup" --context kind-missing
[ "$status" != 0 ] || fail "backup-postgres-now on a missing context succeeded"
[ "$(calls)" = 1 ] || fail "backup-postgres-now on a missing context made more than the check: $(cat "$STUB_LOG")"
[ -z "$(plain)" ] || fail "backup-postgres-now on a missing context called the plain kubectl"

if [ "$failures" -ne 0 ]; then
    echo "$failures check(s) failed"
    exit 1
fi
echo "All checks passed."
