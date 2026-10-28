#!/bin/bash
#
# Check teardown.sh (and protect-postgres-pv.sh, which it runs):
#
#   - it never calls plain `kubectl`; every kubectl call goes to the kind
#     cluster's own kubectl with --context kind-econ-graph, and none switches
#     the kubeconfig's current context;
#   - it refuses before any change when that context is missing, or reaches
#     another cluster than the kind cluster econ-graph on DOCKER_HOST (same node
#     names, different kube-system UID);
#   - it asks once before any change; on a cluster marked production only the
#     kube-system UID prefix (typed, or --confirm-production) answers that;
#   - without a terminal it refuses up front when a prompt would be needed;
#   - --dry-run deletes and patches nothing; --keep-cluster keeps the cluster.
#
# Runs offline against stubs, never a cluster: a plain `kubectl` first on PATH
# that records any call and fails; `kind` and `docker` stubs that pretend the
# kind cluster econ-graph exists and record every call; and the "kind kubectl"
# kept in ${XDG_CACHE_HOME}/econgraph, a fake that records every call and
# answers the reads teardown makes. HOME, XDG_CACHE_HOME, TMPDIR and the host
# and backup roots all point into a scratch directory. The terminal cases run
# teardown under a pseudo-terminal (python3's pty module).
#
# Usage: scripts/deploy/test-teardown-kubectl.sh

set -uo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
CACHE="$WORK/cache/econgraph"
mkdir -p "$WORK/bin" "$WORK/home" "$WORK/tmp" "$CACHE/bin"
PLAIN="$WORK/plain-kubectl.log"
CALLS="$WORK/kind-kubectl.log"
KIND="$WORK/kind.log"
KIND_KUBECONFIG="$CACHE/kubeconfig-econ-graph"
echo "apiVersion: v1" >"$KIND_KUBECONFIG"
# Nothing from the caller's environment chooses the cluster; a case that wants
# one of these sets it on its own command line.
unset KUBECONFIG KUBECTL CLUSTER_ROLE_CONTEXT DOCKER_HOST DOCKER_CONTEXT KIND_EXPERIMENTAL_PROVIDER \
    ECONGRAPH_DOCKER_HOST

# Plain kubectl (on the production host, the MicroK8s wrapper): must never run.
cat >"$WORK/bin/kubectl" <<EOF
#!/bin/sh
echo "\$*" >>"$PLAIN"
echo "plain kubectl stub: refusing" >&2
exit 1
EOF

# The kind cluster's kubectl. FAKE_NO_CONTEXT set: it has no kind-econ-graph
# context. FAKE_UID: the kube-system UID it reports. FAKE_MARKED set: the
# cluster carries a production marker made on it. FAKE_MARKER_FAILS set: the
# marker can't be read.
cat >"$CACHE/bin/kubectl" <<EOF
#!/bin/sh
echo "KUBECONFIG=\${KUBECONFIG:-} \$*" >>"$CALLS"
uid="\${FAKE_UID:-uid-kind-cluster}"
case " \$* " in
    *" config get-contexts "*) [ -z "\${FAKE_NO_CONTEXT:-}" ] || exit 1; echo kind-econ-graph ;;
    *" config use-context "*) exit 0 ;;
    *" config view "*) printf '%s' "https://127.0.0.1:6443" ;;
    *" get nodes "*) printf '%s' "econ-graph-control-plane econ-graph-worker" ;;
    *" get namespace kube-system "*) printf '%s' "\$uid" ;;
    *" get configmap econgraph-cluster-role "*)
        [ -z "\${FAKE_MARKER_FAILS:-}" ] || exit 1
        [ -z "\${FAKE_MARKED:-}" ] || printf '%s' "production \$uid" ;;
    *" -o name"*) echo "persistentvolumeclaim/stub" ;;
    *"{.spec.volumeName}"*) printf '%s' pv-stub ;;
    *"persistentVolumeReclaimPolicy"*) printf '%s' Delete ;;
esac
exit 0
EOF

cat >"$WORK/bin/kind" <<EOF
#!/bin/sh
echo "kind \$*" >>"$KIND"
case "\$1 \$2" in
    "get clusters") echo econ-graph ;;
    "get nodes") printf '%s\n' econ-graph-control-plane econ-graph-worker ;;
esac
exit 0
EOF

# docker: only the kube-system UID read inside the control-plane container.
# FAKE_DOCKER_UID: the UID of the kind cluster on this DOCKER_HOST.
# FAKE_DOCKER_FAILS set: the exec fails (no such container on this daemon).
cat >"$WORK/bin/docker" <<EOF
#!/bin/sh
echo "docker \$*" >>"$KIND"
case "\$*" in
    "exec econ-graph-control-plane kubectl --kubeconfig /etc/kubernetes/admin.conf get namespace kube-system -o jsonpath={.metadata.uid}")
        [ -z "\${FAKE_DOCKER_FAILS:-}" ] || { echo "Error: No such container" >&2; exit 1; }
        printf '%s' "\${FAKE_DOCKER_UID:-uid-kind-cluster}"; exit 0 ;;
esac
echo "docker stub: unexpected call" >&2
exit 1
EOF
chmod +x "$WORK/bin/"* "$CACHE/bin/kubectl"

TEARDOWN_ENV=(
    HOME="$WORK/home" XDG_CACHE_HOME="$WORK/cache" TMPDIR="$WORK/tmp"
    ECONGRAPH_BACKUP_ROOT="$WORK/backups" KDB_BACKUP_ROOT="$WORK/backups"
    KEYCLOAK_BACKUP_ROOT="$WORK/backups" ECONGRAPH_HOST_ROOT="$WORK/host"
    HOST_ROOT="$WORK/host" HOST_STORAGE="$WORK/host"
    PATH="$WORK/bin:/usr/local/bin:/usr/bin:/bin"
)

# teardown ARGS...: teardown.sh with the stubs, stdin not a terminal. Prints
# its output; returns its exit status. Variables set on the call
# (`FAKE_X=y teardown ...`) reach it.
teardown() {
    env -u KIND "${TEARDOWN_ENV[@]}" "$PROJECT_ROOT/scripts/deploy/teardown.sh" "$@" </dev/null 2>&1
}

# teardown_tty INPUT ARGS...: the same under a pseudo-terminal, with INPUT typed
# ahead; killed after a minute if it waits for more input.
teardown_tty() {
    local input="$1"
    shift
    timeout 60 env -u KIND "${TEARDOWN_ENV[@]}" python3 -c '
import os, pty, sys
pid, fd = pty.fork()
if pid == 0:
    os.execv(sys.argv[2], sys.argv[2:])
os.write(fd, sys.argv[1].encode())
out = b""
while True:
    try:
        data = os.read(fd, 4096)
    except OSError:
        break
    if not data:
        break
    out += data
_, status = os.waitpid(pid, 0)
sys.stdout.write(out.decode(errors="replace"))
sys.exit(os.waitstatus_to_exitcode(status))
' "$input" "$PROJECT_ROOT/scripts/deploy/teardown.sh" "$@"
}

failed=0
fail() { echo "FAIL $*"; failed=1; }
reset() { : >"$PLAIN"; : >"$CALLS"; : >"$KIND"; }

# check NAME: no plain kubectl; every kind kubectl call used the kind
# kubeconfig and carried --context kind-econ-graph; no call switched the
# current context.
check() {
    local name="$1" bad
    if [ -s "$PLAIN" ]; then
        fail "$name called plain kubectl:"
        sed 's/^/     /' "$PLAIN"
    fi
    bad="$(grep -v "^KUBECONFIG=$KIND_KUBECONFIG " "$CALLS" || true)"
    [ -z "$bad" ] || { fail "$name ran the kind kubectl with another kubeconfig:"; printf '     %s\n' "$bad"; }
    bad="$(grep -v -- ' --context kind-econ-graph ' "$CALLS" || true)"
    [ -z "$bad" ] || { fail "$name made kind kubectl calls without --context kind-econ-graph:"; printf '     %s\n' "$bad"; }
    if grep -q ' use-context ' "$CALLS"; then fail "$name switched the kubeconfig's current context"; fi
}
# changed: true when anything was deleted or patched, through kubectl or kind.
changed() { grep -qE ' (delete|patch) ' "$CALLS" || grep -q '^kind delete' "$KIND"; }
# expect_refusal NAME PATTERN: the last run ($rc, $out) failed with PATTERN and
# changed nothing.
expect_refusal() {
    if [ "$rc" -ne 0 ] && grep -q -- "$2" <<<"$out" && grep -q "nothing was changed" <<<"$out"; then
        echo "ok   $1 refuses"
    else
        fail "$1 (exit $rc), expected a refusal matching '$2':"; printf '     %s\n' "$out"
    fi
    check "$1"
    if changed; then fail "$1 deleted or patched something before refusing"; fi
}
# expect_teardown NAME: the last run succeeded, protected the PVs before
# deleting anything, deleted the namespace, and deleted no PV.
expect_teardown() {
    if [ "$rc" -eq 0 ] &&
        grep -q -- ' --context kind-econ-graph patch pv pv-stub ' "$CALLS" &&
        grep -q -- ' --context kind-econ-graph delete -f k8s/manifests/namespace.yaml ' "$CALLS"; then
        echo "ok   $1 protects and deletes on kind-econ-graph"
    else
        fail "$1 (exit $rc):"; printf '     %s\n' "$out"
    fi
    check "$1"
    local first_patch first_delete
    first_patch="$(grep -n ' patch pv ' "$CALLS" | head -1 | cut -d: -f1)"
    first_delete="$(grep -n ' delete ' "$CALLS" | head -1 | cut -d: -f1)"
    if [ -z "$first_patch" ] || [ -z "$first_delete" ] || [ "$first_patch" -gt "$first_delete" ]; then
        fail "$1 deleted before protecting the PVs"
    fi
    if grep -E ' delete ' "$CALLS" | grep -qE ' (pv|persistentvolume)s? '; then
        fail "$1 deleted a PV"
    fi
}

# 1. --dry-run: shows the cluster, deletes and patches nothing.
reset
out="$(ECONGRAPH_DOCKER_HOST=unix:///stub/docker.sock teardown --dry-run)"; rc=$?
if [ "$rc" -eq 0 ] && grep -q "Dry run complete: nothing was changed" <<<"$out" &&
    grep -qE "DOCKER_HOST +unix:///stub/docker.sock" <<<"$out" &&
    grep -qE "context +kind-econ-graph" <<<"$out" &&
    grep -qE "API server +https://127.0.0.1:6443" <<<"$out" &&
    grep -qE "nodes +econ-graph-control-plane econ-graph-worker" <<<"$out" &&
    grep -qE "kube-system UID uid-kind-cluster \(the same inside econ-graph-control-plane\)" <<<"$out" &&
    grep -qE "production +context kind-econ-graph is not marked production" <<<"$out" &&
    grep -q "+ kubectl delete -f k8s/manifests/backend-deployment.yaml" <<<"$out"; then
    echo "ok   teardown.sh --dry-run completes and shows the cluster and the deletes"
else
    fail "teardown.sh --dry-run (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh --dry-run"
if changed; then fail "teardown.sh --dry-run deleted or patched something"; else
    echo "ok   teardown.sh --dry-run deletes and patches nothing"; fi

# 2. --dry-run with every deletion asked for, on production, still changes nothing.
reset
out="$(FAKE_MARKED=1 teardown --dry-run --yes --delete-cluster)"; rc=$?
if [ "$rc" -eq 0 ] && grep -q "+ kind delete cluster --name econ-graph" <<<"$out" &&
    grep -q "+ kubectl delete -f k8s/manifests/namespace.yaml" <<<"$out" &&
    grep -qE "production +marked production" <<<"$out" &&
    grep -q "would require the first 8 characters of the kube-system UID (uid-kind)" <<<"$out"; then
    echo "ok   teardown.sh --dry-run --yes --delete-cluster on production shows the deletes and the confirmation"
else
    fail "teardown.sh --dry-run --yes --delete-cluster on production (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh --dry-run --yes --delete-cluster"
if changed; then fail "teardown.sh --dry-run --yes --delete-cluster deleted or patched something"; fi

# 3. Without a terminal, --yes --delete-cluster: every call on the context.
reset
out="$(teardown --yes --delete-cluster)"; rc=$?
expect_teardown "teardown.sh --yes --delete-cluster"
grep -q '^kind delete cluster --name econ-graph$' "$KIND" || fail "teardown.sh --delete-cluster did not delete the cluster"

# 4. --keep-cluster: deletes the namespace without a prompt and keeps the cluster.
reset
out="$(teardown --yes --keep-cluster)"; rc=$?
expect_teardown "teardown.sh --yes --keep-cluster"
if grep -q '^kind delete' "$KIND"; then fail "teardown.sh --keep-cluster deleted the cluster"; fi
reset
out="$(teardown --yes --keep-cluster --delete-cluster)"; rc=$?
if [ "$rc" -eq 2 ] && [ ! -s "$CALLS" ] && [ ! -s "$KIND" ]; then
    echo "ok   teardown.sh rejects --keep-cluster with --delete-cluster"
else
    fail "teardown.sh --keep-cluster --delete-cluster (exit $rc): $out"
fi

# 5. The context is missing: refused.
reset
out="$(FAKE_NO_CONTEXT=1 teardown --yes --delete-cluster)"; rc=$?
expect_refusal "teardown.sh without a kind-econ-graph context" "has no context kind-econ-graph"

# 6. kubectl and kind reach different clusters with the same node names (say,
# econ-graph on the rootful and on the rootless Docker daemon): refused.
reset
out="$(FAKE_UID=uid-other-cluster teardown --yes --delete-cluster)"; rc=$?
expect_refusal "teardown.sh when the context is another econ-graph cluster than kind's" \
    "is not the kind cluster econ-graph on DOCKER_HOST"

# 6b. The control-plane container can't be asked (another daemon), or kind
# would use another provider than docker: refused.
reset
out="$(FAKE_DOCKER_FAILS=1 teardown --yes --delete-cluster)"; rc=$?
expect_refusal "teardown.sh when docker exec into the control plane fails" \
    "could not read the kube-system UID inside econ-graph-control-plane"
reset
out="$(KIND_EXPERIMENTAL_PROVIDER=podman teardown --yes --delete-cluster)"; rc=$?
expect_refusal "teardown.sh with KIND_EXPERIMENTAL_PROVIDER=podman" \
    "KIND_EXPERIMENTAL_PROVIDER=podman is not supported"

# 7. An inherited CLUSTER_ROLE_CONTEXT is dropped, not followed.
reset
out="$(CLUSTER_ROLE_CONTEXT=microk8s teardown --dry-run)"; rc=$?
if [ "$rc" -eq 0 ]; then echo "ok   teardown.sh --dry-run with CLUSTER_ROLE_CONTEXT=microk8s completes"; else
    fail "teardown.sh --dry-run with CLUSTER_ROLE_CONTEXT=microk8s (exit $rc):"; printf '     %s\n' "$out"; fi
check "teardown.sh with CLUSTER_ROLE_CONTEXT=microk8s"

# 8. A prompt with no terminal to answer it: refused before anything runs.
for args in "" "--yes" "--delete-cluster" "--keep-cluster"; do
    reset
    # shellcheck disable=SC2086 # $args is zero or one word.
    out="$(teardown $args)"; rc=$?
    if [ "$rc" -ne 0 ] && grep -q "no terminal" <<<"$out" && grep -q "nothing was changed" <<<"$out" &&
        [ ! -s "$CALLS" ] && [ ! -s "$PLAIN" ] && ! grep -q '^kind delete' "$KIND"; then
        echo "ok   teardown.sh ${args:-(no arguments)} without a terminal refuses before any change"
    else
        fail "teardown.sh ${args:-(no arguments)} without a terminal (exit $rc):"; printf '     %s\n' "$out"
    fi
done

# 9. Production without a terminal: --yes alone doesn't answer; only the right
# --confirm-production does.
reset
out="$(FAKE_MARKED=1 teardown --yes --delete-cluster)"; rc=$?
expect_refusal "teardown.sh --yes --delete-cluster on production" "needs --confirm-production"
reset
out="$(FAKE_MARKED=1 teardown --yes --delete-cluster --confirm-production uid-othe)"; rc=$?
expect_refusal "teardown.sh --confirm-production with the wrong prefix" "does not match"
reset
out="$(FAKE_MARKED=1 teardown --yes --keep-cluster --confirm-production uid-kind)"; rc=$?
expect_teardown "teardown.sh --yes --keep-cluster --confirm-production on production"

# 9b. A marker that can't be read is treated as production; a
# --confirm-production that doesn't match is refused on any cluster.
reset
out="$(FAKE_MARKER_FAILS=1 teardown --yes --delete-cluster)"; rc=$?
expect_refusal "teardown.sh --yes --delete-cluster with an unreadable marker" "needs --confirm-production"
grep -q "treated as production for the confirmation" <<<"$out" ||
    fail "teardown.sh with an unreadable marker does not say it is treated as production"
reset
out="$(FAKE_MARKER_FAILS=1 teardown --yes --keep-cluster --confirm-production uid-kind)"; rc=$?
expect_teardown "teardown.sh --confirm-production with an unreadable marker"
reset
out="$(teardown --yes --delete-cluster --confirm-production uid-othe)"; rc=$?
expect_refusal "teardown.sh --confirm-production with the wrong prefix on an unmarked cluster" "does not match"

# 10. At a terminal: one confirmation before any change.
reset
out="$(teardown_tty $'n\n' --yes --keep-cluster)"; rc=$?
if grep -q "Tear down EconGraph" <<<"$out"; then fail "teardown.sh --yes at a terminal asked to go ahead"; fi
expect_teardown "teardown.sh --yes --keep-cluster at a terminal"
reset
out="$(teardown_tty $'n\n')"; rc=$?
if [ "$rc" -eq 0 ] && grep -q "Tear down EconGraph on this cluster" <<<"$out" &&
    grep -q "Nothing was changed" <<<"$out"; then
    echo "ok   teardown.sh at a terminal asks before any change, and 'n' changes nothing"
else
    fail "teardown.sh at a terminal answered n (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh at a terminal answered n"
if changed; then fail "teardown.sh at a terminal changed something before the confirmation"; fi
reset
out="$(teardown_tty $'y\ny' --delete-cluster)"; rc=$?
expect_teardown "teardown.sh --delete-cluster at a terminal answered y, then y for the namespace"
reset
out="$(FAKE_MARKED=1 teardown_tty $'y\n' --yes --delete-cluster)"; rc=$?
expect_refusal "teardown.sh --yes --delete-cluster on production at a terminal, answered y" \
    "confirmation did not match"
reset
out="$(FAKE_MARKED=1 teardown_tty $'uid-kind\n' --yes --delete-cluster)"; rc=$?
expect_teardown "teardown.sh on production at a terminal, typed the UID prefix"

# 11. An unknown argument (a mistyped --dry-run) stops before anything runs.
reset
out="$(teardown --dryrun)"; rc=$?
if [ "$rc" -eq 2 ] && [ ! -s "$CALLS" ] && [ ! -s "$KIND" ]; then
    echo "ok   teardown.sh rejects an unknown argument before anything runs"
else
    fail "teardown.sh --dryrun (exit $rc) did not stop at the argument: $out"
fi

if [ "$failed" -ne 0 ]; then
    echo "teardown kubectl checks failed"
    exit 1
fi
echo "teardown kubectl checks passed"
