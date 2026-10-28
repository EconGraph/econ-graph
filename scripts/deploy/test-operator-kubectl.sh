#!/bin/bash
#
# Check that the operator scripts backup-postgres-now.sh,
# restore-postgres-backup.sh, setup-letsencrypt-cloudflare.sh and
# test-letsencrypt-cloudflare.sh never call plain `kubectl`, and that every call
# they make goes to the kind cluster's own kubectl with an explicit --context:
# in --dry-run, and in the refusal paths (an empty or unknown --context, an
# unconfirmed restore with no terminal), also with another CLUSTER_ROLE_CONTEXT
# inherited from the environment.
#
# Runs offline against stubs, never a cluster: a plain `kubectl` first on PATH
# that records any call and fails, and a fake kind kubectl and kubeconfig in a
# scratch ~/.cache/econgraph, which records every call and knows two contexts,
# kind-econ-graph and kind-other.
#
# Usage: scripts/deploy/test-operator-kubectl.sh

set -uo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
CACHE="$WORK/cache/econgraph"
mkdir -p "$WORK/bin" "$WORK/home" "$CACHE/bin"
PLAIN="$WORK/plain-kubectl.log"
CALLS="$WORK/kind-kubectl.log"

cat >"$WORK/bin/kubectl" <<EOF
#!/bin/sh
echo "\$*" >>"$PLAIN"
echo "plain kubectl stub: refusing" >&2
exit 1
EOF

# The kind kubectl: contexts kind-econ-graph and kind-other exist.
cat >"$CACHE/bin/kubectl" <<EOF
#!/bin/sh
echo "\$*" >>"$CALLS"
case " \$* " in
    *" --context kind-econ-graph "*|*" --context kind-other "*) ;;
    *) echo "error: context not found" >&2; exit 1 ;;
esac
case " \$* " in
    *" config view "*) echo "https://127.0.0.1:6443" ;;
    *" get nodes "*)
        [ -z "\${FAKE_NODES_FAIL:-}" ] || { echo "error: the server is unavailable" >&2; exit 1; }
        echo "econ-graph-control-plane econ-graph-worker" ;;
    *" get namespace kube-system "*) echo "11111111-2222-3333-4444-555555555555" ;;
    # backup-postgres-now.sh: a wait that times out, and the Job's active pods.
    *" wait "*) [ -z "\${FAKE_WAIT_FAIL:-}" ] || { echo "error: timed out waiting for the condition" >&2; exit 1; } ;;
    *" get job/"*) echo "\${FAKE_JOB_ACTIVE:-}" ;;
esac
exit 0
EOF
echo "apiVersion: v1" >"$CACHE/kubeconfig-econ-graph"
chmod +x "$WORK/bin/kubectl" "$CACHE/bin/kubectl"

# run SCRIPT ARGS...: the script with the stubs, no kube config, stdin not a
# terminal. Prints its output; returns its exit status.
run() {
    local script="$1"
    shift
    env -u KUBECONFIG -u KUBECTL -u ECONGRAPH_KUBE_DIR \
        HOME="$WORK/home" XDG_CACHE_HOME="$WORK/cache" TMPDIR="$WORK" \
        PATH="$WORK/bin:/usr/local/bin:/usr/bin:/bin" \
        "$PROJECT_ROOT/scripts/deploy/$script" "$@" </dev/null 2>&1
}

failed=0
fail() { echo "FAIL $*"; failed=1; }
reset() { : >"$PLAIN"; : >"$CALLS"; }

# expect NAME RC MESSAGE CHANGES: the last run (rc in $rc, output in $out)
# exited RC (0 or "nonzero") and printed MESSAGE; plain kubectl was never
# called; every kind kubectl call carried --context kind-econ-graph unless it
# was refused; and with CHANGES=none, no call created, applied, ran or deleted.
expect() {
    local name="$1" want="$2" message="$3" changes="$4" bad
    if { [ "$want" = 0 ] && [ "$rc" -eq 0 ]; } || { [ "$want" = nonzero ] && [ "$rc" -ne 0 ]; }; then
        if grep -qF -- "$message" <<<"$out"; then
            echo "ok   $name"
        else
            fail "$name: no '$message' in:"; printf '     %s\n' "$out"
        fi
    else
        fail "$name exited $rc:"; printf '     %s\n' "$out"
    fi
    if [ -s "$PLAIN" ]; then
        fail "$name called plain kubectl:"; sed 's/^/     /' "$PLAIN"
    fi
    bad="$(grep -v -e '--context kind-econ-graph ' -e '--context kind-other ' -e '--context missing ' "$CALLS" || true)"
    [ -z "$bad" ] || { fail "$name made kind kubectl calls without an explicit --context:"; printf '     %s\n' "$bad"; }
    if [ "$changes" = none ] && grep -qE '(^| )(create|apply|run|delete|patch|wait) ' "$CALLS"; then
        fail "$name changed something:"; sed 's/^/     /' "$CALLS"
    fi
}

# only_context NAME CONTEXT: the last run called the kind kubectl, and only
# with --context CONTEXT.
only_context() {
    local name="$1" context="$2" bad
    grep -q -- "--context ${context} " "$CALLS" || fail "$name did not use context ${context}"
    bad="$(grep -v -- "--context ${context} " "$CALLS" || true)"
    [ -z "$bad" ] || { fail "$name used another context:"; printf '     %s\n' "$bad"; }
}

# An identity that can't be read stops every script before it changes anything.
reset; out="$(FAKE_NODES_FAIL=1 run backup-postgres-now.sh)"; rc=$?
expect "backup-postgres-now.sh with an unreadable identity" nonzero "could not read the identity" none
reset; out="$(FAKE_NODES_FAIL=1 run restore-postgres-backup.sh --yes)"; rc=$?
expect "restore-postgres-backup.sh with an unreadable identity" nonzero "could not read the identity" none
reset; out="$(FAKE_NODES_FAIL=1 CLOUDFLARE_API_TOKEN=stub-token run setup-letsencrypt-cloudflare.sh -d example.invalid)"; rc=$?
expect "setup-letsencrypt-cloudflare.sh with an unreadable identity" nonzero "could not read the identity" none
reset; out="$(FAKE_NODES_FAIL=1 run test-letsencrypt-cloudflare.sh -d example.invalid)"; rc=$?
expect "test-letsencrypt-cloudflare.sh with an unreadable identity" nonzero "could not read the identity" none

# A non-default --context reaches every call, in each script.
for script in backup-postgres-now.sh restore-postgres-backup.sh; do
    reset; out="$(run "$script" --dry-run --context kind-other)"; rc=$?
    expect "$script --dry-run --context kind-other" 0 "context         kind-other" none
    only_context "$script --context kind-other" kind-other
done
reset; out="$(CLOUDFLARE_API_TOKEN=stub-token run setup-letsencrypt-cloudflare.sh -d example.invalid --dry-run --context kind-other)"; rc=$?
expect "setup-letsencrypt-cloudflare.sh --dry-run --context kind-other" 0 "Dry run: on context kind-other" none
only_context "setup-letsencrypt-cloudflare.sh --context kind-other" kind-other
reset; out="$(run test-letsencrypt-cloudflare.sh -d example.invalid --dry-run --context kind-other)"; rc=$?
expect "test-letsencrypt-cloudflare.sh --dry-run --context kind-other" 0 "Dry run: on context kind-other" none
only_context "test-letsencrypt-cloudflare.sh --context kind-other" kind-other

reset; out="$(run backup-postgres-now.sh --dry-run)"; rc=$?
expect "backup-postgres-now.sh --dry-run" 0 "kube-system UID 11111111-2222-3333-4444-555555555555" none
reset; out="$(CLUSTER_ROLE_CONTEXT=prod-elsewhere run backup-postgres-now.sh --dry-run)"; rc=$?
expect "backup-postgres-now.sh --dry-run with an inherited CLUSTER_ROLE_CONTEXT" 0 "context         kind-econ-graph" none
reset; out="$(run backup-postgres-now.sh --context '')"; rc=$?
expect "backup-postgres-now.sh --context ''" nonzero "--context needs a non-empty value" none
[ -s "$CALLS" ] && fail "backup-postgres-now.sh --context '' called kubectl"
reset; out="$(run backup-postgres-now.sh --context missing)"; rc=$?
expect "backup-postgres-now.sh on an unknown context" nonzero "has no context missing" none
# A backup Job still running when the wait times out is said, with how to wait
# for it, and not called failed; a Job that is not running is.
reset; out="$(FAKE_WAIT_FAIL=1 FAKE_JOB_ACTIVE=1 run backup-postgres-now.sh)"; rc=$?
expect "backup-postgres-now.sh with the Job still running at the timeout" nonzero "backup-postgres-now: still running: job/postgres-backup-manual-" changes
grep -qF "did not complete successfully" <<<"$out" &&
    fail "backup-postgres-now.sh with the Job still running: calls it failed"
grep -qF "don't start another backup" <<<"$out" ||
    fail "backup-postgres-now.sh with the Job still running: does not warn against a second backup"
reset; out="$(FAKE_WAIT_FAIL=1 run backup-postgres-now.sh)"; rc=$?
expect "backup-postgres-now.sh with a failed Job" nonzero "Backup job did not complete successfully" changes
grep -qF "still running" <<<"$out" && fail "backup-postgres-now.sh with a failed Job: says it is still running"

reset; out="$(run restore-postgres-backup.sh --dry-run)"; rc=$?
expect "restore-postgres-backup.sh --dry-run" 0 "Dry run: would DROP and recreate" none
reset; out="$(run restore-postgres-backup.sh --list --dry-run)"; rc=$?
expect "restore-postgres-backup.sh --list --dry-run" 0 "Dry run: would list postgres-backup-data" none
reset; out="$(CLUSTER_ROLE_CONTEXT=prod-elsewhere run restore-postgres-backup.sh --dry-run --yes)"; rc=$?
expect "restore-postgres-backup.sh --dry-run with an inherited CLUSTER_ROLE_CONTEXT" 0 "context         kind-econ-graph" none
reset; out="$(run restore-postgres-backup.sh)"; rc=$?
expect "restore-postgres-backup.sh unconfirmed, no terminal" nonzero "The confirmation is read from a terminal only" none
if ! awk '/kube-system UID/ { id = NR } /This will DROP/ { warn = NR } END { exit !(id && warn && id < warn) }' <<<"$out"; then
    fail "restore-postgres-backup.sh did not show the cluster before the confirmation"
fi
reset; out="$(run restore-postgres-backup.sh --context '' --yes)"; rc=$?
expect "restore-postgres-backup.sh --context ''" nonzero "--context needs a non-empty value" none
[ -s "$CALLS" ] && fail "restore-postgres-backup.sh --context '' called kubectl"
reset; out="$(run restore-postgres-backup.sh --context missing --yes)"; rc=$?
expect "restore-postgres-backup.sh on an unknown context" nonzero "has no context missing" none

reset; out="$(CLOUDFLARE_API_TOKEN=stub-token run setup-letsencrypt-cloudflare.sh -d example.invalid --dry-run)"; rc=$?
expect "setup-letsencrypt-cloudflare.sh --dry-run" 0 "Dry run: on context kind-econ-graph" none
reset; out="$(CLOUDFLARE_API_TOKEN=stub-token run setup-letsencrypt-cloudflare.sh -d example.invalid --context missing)"; rc=$?
expect "setup-letsencrypt-cloudflare.sh on an unknown context" nonzero "has no context missing" none
reset; out="$(CLOUDFLARE_API_TOKEN=stub-token run setup-letsencrypt-cloudflare.sh -d example.invalid --context '')"; rc=$?
expect "setup-letsencrypt-cloudflare.sh --context ''" nonzero "--context needs a non-empty value" none

reset; out="$(run test-letsencrypt-cloudflare.sh -d example.invalid --dry-run)"; rc=$?
expect "test-letsencrypt-cloudflare.sh --dry-run" 0 "Dry run: on context kind-econ-graph" none
reset; out="$(run test-letsencrypt-cloudflare.sh -d example.invalid --context missing)"; rc=$?
expect "test-letsencrypt-cloudflare.sh on an unknown context" nonzero "has no context missing" none

if [ "$failed" -ne 0 ]; then
    echo "operator kubectl checks failed"
    exit 1
fi
echo "operator kubectl checks passed"
