#!/bin/bash
#
# Check that teardown.sh (and protect-postgres-pv.sh, which it runs) never call
# plain `kubectl`, that every kubectl call they make goes to the kind cluster's
# own kubectl with --context kind-econ-graph, that teardown refuses a context
# that isn't the kind cluster before changing anything, that it refuses up
# front when a confirmation would be needed and stdin is not a terminal, and
# that --dry-run deletes and patches nothing.
#
# Runs offline against stubs, never a cluster: a plain `kubectl` first on PATH
# that records any call and fails; a `kind` stub that pretends the kind cluster
# econ-graph exists and records any delete; and the "kind kubectl" kept in
# ${XDG_CACHE_HOME}/econgraph, a fake that records every call and answers the
# reads teardown makes. HOME, XDG_CACHE_HOME, TMPDIR and the host and backup
# roots all point into a scratch directory.
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

# Plain kubectl (on the production host, the MicroK8s wrapper): must never run.
cat >"$WORK/bin/kubectl" <<EOF
#!/bin/sh
echo "\$*" >>"$PLAIN"
echo "plain kubectl stub: refusing" >&2
exit 1
EOF

# The kind cluster's kubectl. FAKE_CURRENT_CONTEXT is the context it reports
# as current after use-context; FAKE_NODES the nodes it reports; FAKE_MARKED
# set makes the cluster carry a production marker.
cat >"$CACHE/bin/kubectl" <<EOF
#!/bin/sh
echo "KUBECONFIG=\${KUBECONFIG:-} \$*" >>"$CALLS"
case " \$* " in
    *" config use-context "*) exit 0 ;;
    *" config current-context"*) echo "\${FAKE_CURRENT_CONTEXT:-kind-econ-graph}" ;;
    *" config view "*) printf '%s' "https://127.0.0.1:6443" ;;
    *" get nodes "*) printf '%s' "\${FAKE_NODES:-econ-graph-control-plane econ-graph-worker}" ;;
    *" get namespace kube-system "*) printf '%s' "uid-kind-cluster" ;;
    *" get configmap econgraph-cluster-role "*)
        [ -z "\${FAKE_MARKED:-}" ] || printf '%s' "production uid-kind-cluster" ;;
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
chmod +x "$WORK/bin/"* "$CACHE/bin/kubectl"

# teardown ARGS...: teardown.sh with the stubs first on PATH, no KUBECTL,
# KUBECONFIG or pinned context of its own, everything writable in $WORK, stdin
# not a terminal. Prints its output; returns its exit status. Extra
# environment goes before it: `FAKE_X=y teardown ...`.
teardown() {
    env -u KUBECONFIG -u KUBECTL -u CLUSTER_ROLE_CONTEXT \
        HOME="$WORK/home" XDG_CACHE_HOME="$WORK/cache" TMPDIR="$WORK/tmp" \
        ECONGRAPH_BACKUP_ROOT="$WORK/backups" KDB_BACKUP_ROOT="$WORK/backups" \
        KEYCLOAK_BACKUP_ROOT="$WORK/backups" ECONGRAPH_HOST_ROOT="$WORK/host" \
        HOST_ROOT="$WORK/host" HOST_STORAGE="$WORK/host" \
        PATH="$WORK/bin:/usr/local/bin:/usr/bin:/bin" \
        "$PROJECT_ROOT/scripts/deploy/teardown.sh" "$@" </dev/null 2>&1
}

failed=0
fail() { echo "FAIL $*"; failed=1; }
reset() { : >"$PLAIN"; : >"$CALLS"; : >"$KIND"; }

# check NAME: no plain kubectl; every kind kubectl call used the kind
# kubeconfig and, apart from selecting the context itself, carried
# --context kind-econ-graph.
check() {
    local name="$1" bad
    if [ -s "$PLAIN" ]; then
        fail "$name called plain kubectl:"
        sed 's/^/     /' "$PLAIN"
    fi
    bad="$(grep -v "^KUBECONFIG=$KIND_KUBECONFIG " "$CALLS" || true)"
    [ -z "$bad" ] || { fail "$name ran the kind kubectl with another kubeconfig:"; printf '     %s\n' "$bad"; }
    bad="$(grep -v -- ' --context kind-econ-graph ' "$CALLS" |
        grep -v "^KUBECONFIG=[^ ]* config use-context kind-econ-graph\$" || true)"
    [ -z "$bad" ] || { fail "$name made kind kubectl calls without --context kind-econ-graph:"; printf '     %s\n' "$bad"; }
}
# changed: true when anything was deleted or patched, through kubectl or kind.
changed() { grep -qE ' (delete|patch) ' "$CALLS" || grep -q '^kind delete' "$KIND"; }

# 1. --dry-run: shows the cluster, deletes and patches nothing.
reset
out="$(teardown --dry-run)"; rc=$?
if [ "$rc" -eq 0 ] && grep -q "Dry run complete: nothing was changed" <<<"$out" &&
    grep -qE "context +kind-econ-graph" <<<"$out" &&
    grep -qE "API server +https://127.0.0.1:6443" <<<"$out" &&
    grep -qE "nodes +econ-graph-control-plane econ-graph-worker" <<<"$out" &&
    grep -qE "kube-system UID uid-kind-cluster" <<<"$out" &&
    grep -qE "production +context kind-econ-graph is not marked production" <<<"$out"; then
    echo "ok   teardown.sh --dry-run completes and shows the cluster"
else
    fail "teardown.sh --dry-run (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh --dry-run"
if changed; then fail "teardown.sh --dry-run deleted or patched something"; else
    echo "ok   teardown.sh --dry-run deletes and patches nothing"; fi
grep -q "+ kubectl delete -f k8s/manifests/backend-deployment.yaml" <<<"$out" ||
    fail "teardown.sh --dry-run does not show the deletes"

# 2. --dry-run with every deletion asked for still changes nothing.
reset
out="$(FAKE_MARKED=1 teardown --dry-run --yes --delete-cluster)"; rc=$?
if [ "$rc" -eq 0 ] && grep -q "+ kind delete cluster --name econ-graph" <<<"$out" &&
    grep -q "+ kubectl delete -f k8s/manifests/namespace.yaml" <<<"$out" &&
    grep -qE "production +marked production" <<<"$out"; then
    echo "ok   teardown.sh --dry-run --yes --delete-cluster shows the deletes and the production marker"
else
    fail "teardown.sh --dry-run --yes --delete-cluster (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh --dry-run --yes --delete-cluster"
changed && fail "teardown.sh --dry-run --yes --delete-cluster deleted or patched something"

# 3. A real run (all stubs): every call, protect-postgres-pv.sh's included, on the context.
reset
out="$(teardown --yes --delete-cluster)"; rc=$?
if [ "$rc" -eq 0 ] &&
    grep -q -- ' --context kind-econ-graph patch pv pv-stub ' "$CALLS" &&
    grep -q -- ' --context kind-econ-graph delete -f k8s/manifests/namespace.yaml ' "$CALLS" &&
    grep -q '^kind delete cluster --name econ-graph$' "$KIND"; then
    echo "ok   teardown.sh --yes --delete-cluster protects and deletes on kind-econ-graph"
else
    fail "teardown.sh --yes --delete-cluster (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh --yes --delete-cluster"
[ "$(grep -n ' patch pv ' "$CALLS" | head -1 | cut -d: -f1)" -lt \
    "$(grep -n ' delete ' "$CALLS" | head -1 | cut -d: -f1)" ] ||
    fail "teardown.sh deleted before protecting the PVs"
if grep -E ' delete ' "$CALLS" | grep -qE ' (pv|persistentvolume)s? '; then
    fail "teardown.sh deleted a PV"
fi

# 4. A context that isn't kind-econ-graph: refused before anything changes.
reset
out="$(FAKE_CURRENT_CONTEXT=microk8s teardown --yes --delete-cluster)"; rc=$?
if [ "$rc" -ne 0 ] && grep -q "context is microk8s, not kind-econ-graph; nothing was changed" <<<"$out"; then
    echo "ok   teardown.sh refuses when the current context is not kind-econ-graph"
else
    fail "teardown.sh with context microk8s (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh with the wrong context"
changed && fail "teardown.sh deleted or patched before refusing the context"

# 5. A context named kind-econ-graph that reaches another cluster's nodes: refused.
reset
out="$(FAKE_NODES=threadripper teardown --yes --delete-cluster)"; rc=$?
if [ "$rc" -ne 0 ] && grep -q "does not reach kind cluster econ-graph" <<<"$out"; then
    echo "ok   teardown.sh refuses when kind-econ-graph does not reach the kind cluster's nodes"
else
    fail "teardown.sh with another cluster's nodes (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh with another cluster's nodes"
changed && fail "teardown.sh deleted or patched before refusing another cluster's nodes"

# 6. An inherited CLUSTER_ROLE_CONTEXT is replaced, not followed.
reset
out="$(CLUSTER_ROLE_CONTEXT=microk8s teardown --dry-run)"; rc=$?
[ "$rc" -eq 0 ] || { fail "teardown.sh --dry-run with CLUSTER_ROLE_CONTEXT=microk8s (exit $rc):"; printf '     %s\n' "$out"; }
check "teardown.sh with CLUSTER_ROLE_CONTEXT=microk8s"
grep -q -- '--context microk8s' "$CALLS" && fail "teardown.sh followed an inherited CLUSTER_ROLE_CONTEXT"

# 7. A prompt with no terminal to answer it: refused before anything runs.
for args in "" "--yes" "--delete-cluster"; do
    reset
    # shellcheck disable=SC2086 # $args is zero or one word.
    out="$(teardown $args)"; rc=$?
    if [ "$rc" -ne 0 ] && grep -q "no terminal to confirm on" <<<"$out" && grep -q "nothing was changed" <<<"$out" &&
        [ ! -s "$CALLS" ] && [ ! -s "$PLAIN" ] && ! grep -q '^kind delete' "$KIND"; then
        echo "ok   teardown.sh ${args:-(no arguments)} without a terminal refuses before any change"
    else
        fail "teardown.sh ${args:-(no arguments)} without a terminal (exit $rc):"; printf '     %s\n' "$out"
    fi
done

# 8. An unknown argument (a mistyped --dry-run) stops before anything runs.
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
