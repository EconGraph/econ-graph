#!/bin/bash
#
# Check that teardown.sh (and protect-postgres-pv.sh, which it runs) never call
# plain `kubectl`, and that every call they make goes to the kind cluster's own
# kubectl with --context kind-econ-graph:
#
#   - teardown.sh --dry-run, and with --include-data-plane --delete-cluster;
#   - the refusals that stop before anything is deleted (no terminal; a
#     current context other than kind-econ-graph).
#
# Runs offline against stubs, never a cluster: a plain `kubectl` first on PATH
# that records any call and fails; `docker` and `kind` stubs that pretend a kind
# cluster econ-graph exists and hand out a fake "kind kubectl"; and that fake,
# which records every call and answers the few reads teardown makes.
#
# Usage: scripts/deploy/test-teardown-kubectl.sh

set -uo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/bin" "$WORK/home" "$WORK/tmp"
PLAIN="$WORK/plain-kubectl.log"
KIND_CALLS="$WORK/kind-kubectl.log"
OTHER="$WORK/other.log"

# Plain kubectl: must never run.
cat >"$WORK/bin/kubectl" <<EOF
#!/bin/sh
echo "\$*" >>"$PLAIN"
echo "plain kubectl stub: refusing" >&2
exit 1
EOF

# The fake kind kubectl that docker cp hands out. FAKE_CURRENT_CONTEXT, when
# set, is what it reports as the current context after use-context;
# FAKE_PRODUCTION=true makes the cluster carry a production marker for its own
# kube-system UID; FAKE_PHASE is every pod's phase (default Running).
CLUSTER_UID="11111111-2222-3333-4444-555555555555"
cat >"$WORK/kind-kubectl" <<EOF
#!/bin/sh
echo "\$*" >>"$KIND_CALLS"
case " \$* " in
    *" version "*) exit 0 ;;
    *" config use-context "*) exit 0 ;;
    *" config current-context"*) echo "\${FAKE_CURRENT_CONTEXT:-kind-econ-graph}"; exit 0 ;;
    *" config view "*) echo "https://127.0.0.1:6443"; exit 0 ;;
    *" get namespace kube-system "*) echo "$CLUSTER_UID"; exit 0 ;;
    *" get configmap econgraph-cluster-role "*)
        [ "\${FAKE_PRODUCTION:-false}" = true ] && echo "production $CLUSTER_UID"
        exit 0 ;;
    *"{.status.phase}"*) echo "\${FAKE_PHASE:-Running}"; exit 0 ;;
    *" -o name"*) echo "persistentvolumeclaim/stub"; exit 0 ;;
    *"{.spec.volumeName}"*) echo pv-stub; exit 0 ;;
    *"persistentVolumeReclaimPolicy"*) echo Retain; exit 0 ;;
esac
exit 0
EOF

# docker exec on the control-plane node reports FAKE_NODE_UID (default: the
# same cluster's UID) for kube-system.
cat >"$WORK/bin/docker" <<EOF
#!/bin/sh
echo "docker \$*" >>"$OTHER"
case "\$1" in
    info) echo "28.0.0 /var/lib/docker [name=seccomp]" ;;
    cp) for last; do :; done; cp "$WORK/kind-kubectl" "\$last" ;;
    exec)
        case " \$* " in
            *" get namespace kube-system "*) echo "\${FAKE_NODE_UID:-$CLUSTER_UID}" ;;
        esac ;;
esac
exit 0
EOF

cat >"$WORK/bin/kind" <<EOF
#!/bin/sh
echo "kind \$*" >>"$OTHER"
case "\$1 \$2" in
    "get clusters") echo econ-graph ;;
    "get kubeconfig") echo "apiVersion: v1" ;;
    "version "*) echo "kind v0.33.0 go1.26.7 linux/amd64" ;;
esac
exit 0
EOF
# This suite only plans a backup; actual recipient parsing/encryption is tested
# with real age in scripts/backup/test-encryption.sh.
cat >"$WORK/bin/age" <<'EOF'
#!/bin/sh
cat >/dev/null
EOF
cat >"$WORK/bin/age-keygen" <<'EOF'
#!/bin/sh
exit 0
EOF
chmod +x "$WORK/bin/"* "$WORK/kind-kubectl"
# The scripts run kind from its pinned path (lib/kind-version.sh); the stub stands in there too.
mkdir -p "$WORK/home/.local/share/econgraph/bin"
cp "$WORK/bin/kind" "$WORK/home/.local/share/econgraph/bin/kind-v0.33.0"

# teardown CMD-ARGS...: teardown.sh with the stubs first on PATH and no kube
# config, stdin not a terminal. Prints its output; returns its exit status.
teardown() {
    env -u KUBECONFIG -u KUBECTL -u CLUSTER_ROLE_CONTEXT -u ECONGRAPH_KUBE_DIR -u KIND_CLUSTER \
        HOME="$WORK/home" XDG_DATA_HOME="$WORK/home/.local/share" XDG_CACHE_HOME="$WORK/cache" TMPDIR="$WORK/tmp" \
        BACKUP_AGE_RECIPIENT=age1synthetic BACKUP_PLAINTEXT_ROOT=/dev/shm \
        ECONGRAPH_BACKUP_ROOT="$WORK/backups" PATH="$WORK/bin:/usr/local/bin:/usr/bin:/bin" \
        "$PROJECT_ROOT/scripts/deploy/teardown.sh" "$@" </dev/null 2>&1
}

failed=0
fail() { echo "FAIL $*"; failed=1; }

# check NAME: no plain kubectl, and every fake kind kubectl call (other than
# kind-env.sh's reachability check, `version`) carries --context kind-econ-graph.
check() {
    local name="$1" bad
    if [ -s "$PLAIN" ]; then
        fail "$name called plain kubectl:"
        sed 's/^/     /' "$PLAIN"
    fi
    bad="$(grep -v -- '--context kind-econ-graph' "$KIND_CALLS" | grep -vE '^(version|config use-context kind-econ-graph)( |$)' || true)"
    if [ -n "$bad" ]; then
        fail "$name made kind kubectl calls without --context kind-econ-graph:"
        printf '     %s\n' "$bad"
    fi
}
reset() { : >"$PLAIN"; : >"$KIND_CALLS"; : >"$OTHER"; }

reset
out="$(teardown --dry-run)"; rc=$?
if [ "$rc" -eq 0 ] && grep -q "Dry run complete" <<<"$out" &&
    grep -q "Cluster: context kind-econ-graph, API server https://127.0.0.1:6443" <<<"$out"; then
    echo "ok   teardown.sh --dry-run completes and names the cluster"
else
    fail "teardown.sh --dry-run (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh --dry-run"
grep -q "kube-system UID $CLUSTER_UID" <<<"$out" || fail "teardown.sh --dry-run did not print the kube-system UID"
if grep -q "use-context" "$KIND_CALLS"; then
    fail "teardown.sh --dry-run ran kubectl config use-context"
else
    echo "ok   teardown.sh --dry-run never runs config use-context"
fi
grep -q -- '--context kind-econ-graph get pvc postgresql-data-postgresql-0' "$KIND_CALLS" ||
    fail "protect-postgres-pv.sh --dry-run did not read the PVC through the kind kubectl"
[ -s "$PLAIN" ] || echo "ok   no plain kubectl in teardown.sh --dry-run (with protect-postgres-pv.sh)"

reset
out="$(teardown --dry-run --include-data-plane --delete-cluster)"; rc=$?
if [ "$rc" -eq 0 ] && grep -q "+ kind delete cluster" <<<"$out"; then
    echo "ok   teardown.sh --dry-run --include-data-plane --delete-cluster completes"
else
    fail "teardown.sh --dry-run --include-data-plane --delete-cluster (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh --dry-run --include-data-plane --delete-cluster"
grep -q "^kind delete" "$OTHER" && fail "a dry run ran kind delete"

reset
out="$(FAKE_CURRENT_CONTEXT=microk8s teardown --yes)"; rc=$?
if [ "$rc" -ne 0 ] && grep -q "context is microk8s, not kind-econ-graph; nothing was changed" <<<"$out"; then
    echo "ok   teardown.sh refuses when the current context is not kind-econ-graph"
else
    fail "teardown.sh with context microk8s (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh with the wrong context"
grep -qE -- ' (delete|patch) ' "$KIND_CALLS" && fail "teardown.sh deleted or patched before refusing the context"

reset
out="$(teardown)"; rc=$?
if [ "$rc" -ne 0 ] && grep -q "no terminal to confirm on" <<<"$out"; then
    echo "ok   teardown.sh refuses without a terminal or --yes"
else
    fail "teardown.sh without a terminal (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh without a terminal"
[ -s "$KIND_CALLS" ] && fail "teardown.sh without a terminal called kubectl before refusing"

reset
out="$(teardown --include-data-plane --yes)"; rc=$?
if [ "$rc" -ne 0 ] && grep -q -- "--yes doesn't answer it" <<<"$out"; then
    echo "ok   teardown.sh --include-data-plane --yes refuses without a terminal"
else
    fail "teardown.sh --include-data-plane --yes (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh --include-data-plane --yes"

reset
out="$(FAKE_NODE_UID=99999999-0000-0000-0000-000000000000 teardown --yes)"; rc=$?
if [ "$rc" -ne 0 ] && grep -q "kubectl and kind would act on different clusters; nothing was changed" <<<"$out"; then
    echo "ok   teardown.sh refuses when kubectl and the kind node report different kube-system UIDs"
else
    fail "teardown.sh with a UID mismatch (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh with a UID mismatch"
grep -qE -- ' (delete|patch|exec) ' "$KIND_CALLS" && fail "teardown.sh deleted, patched or dumped before refusing the UID mismatch"

reset
out="$(FAKE_PRODUCTION=true teardown --dry-run)"; rc=$?
if [ "$rc" -eq 0 ] && grep -q "This cluster is marked production" <<<"$out" &&
    grep -q "would ask you to type '${CLUSTER_UID:0:8}'" <<<"$out"; then
    echo "ok   teardown.sh --dry-run on a production-marked cluster would ask for the UID prefix"
else
    fail "teardown.sh --dry-run on a production cluster (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh --dry-run on a production cluster"

# On a terminal (script(1) gives it one): the namespace typed right, then a
# wrong UID prefix, must abort with nothing deleted. Pods report Pending, so
# no backup is attempted (--no-backup).
reset
out="$(printf 'econ-graph\nwrong\n' | env -u KUBECONFIG -u KUBECTL -u CLUSTER_ROLE_CONTEXT -u ECONGRAPH_KUBE_DIR -u KIND_CLUSTER \
    FAKE_PRODUCTION=true FAKE_PHASE=Pending HOME="$WORK/home" XDG_CACHE_HOME="$WORK/cache" TMPDIR="$WORK/tmp" \
    ECONGRAPH_BACKUP_ROOT="$WORK/backups" PATH="$WORK/bin:/usr/local/bin:/usr/bin:/bin" \
    script -qec "$PROJECT_ROOT/scripts/deploy/teardown.sh --no-backup" /dev/null 2>&1)"; rc=$?
if [ "$rc" -ne 0 ] && grep -q "Type '${CLUSTER_UID:0:8}' to continue" <<<"$out" &&
    grep -q "aborted; nothing was deleted" <<<"$out"; then
    echo "ok   teardown.sh on a production-marked cluster asks for the UID prefix and aborts on a wrong one"
else
    fail "teardown.sh on a production cluster with a wrong UID prefix (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh on a production cluster"
grep -qE -- ' delete ' "$KIND_CALLS" && fail "teardown.sh deleted something after a wrong UID prefix"

# --confirm-production: a matching prefix lets a production teardown go ahead
# with --yes and no terminal; a wrong one is refused on any cluster; --yes alone
# is refused on production. Pods report Pending, so no backup is attempted.
reset
out="$(FAKE_PRODUCTION=true FAKE_PHASE=Pending teardown --yes --no-backup --confirm-production "${CLUSTER_UID:0:8}")"; rc=$?
if [ "$rc" -eq 0 ] && grep -q "confirm-production ${CLUSTER_UID:0:8} matches this cluster" <<<"$out" &&
    grep -q "App plane removed" <<<"$out"; then
    echo "ok   teardown.sh --yes --confirm-production <right prefix> tears down a production cluster"
else
    fail "teardown.sh --yes with the right --confirm-production (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh --yes --confirm-production <right prefix>"

reset
out="$(FAKE_PHASE=Pending teardown --yes --no-backup --confirm-production deadbeef)"; rc=$?
if [ "$rc" -ne 0 ] &&
    grep -q "confirm-production deadbeef does not match this cluster's kube-system UID; nothing was changed" <<<"$out"; then
    echo "ok   teardown.sh refuses a --confirm-production that doesn't match, on a cluster not marked production"
else
    fail "teardown.sh with a wrong --confirm-production (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh with a wrong --confirm-production"
grep -qE -- ' (delete|patch|exec) ' "$KIND_CALLS" && fail "teardown.sh changed something before refusing a wrong --confirm-production"

reset
out="$(FAKE_PRODUCTION=true teardown --yes)"; rc=$?
if [ "$rc" -ne 0 ] &&
    grep -q "tearing down the production cluster needs --confirm-production with the first 8 characters" <<<"$out"; then
    echo "ok   teardown.sh --yes without --confirm-production refuses on a production cluster"
else
    fail "teardown.sh --yes on a production cluster (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh --yes on a production cluster"
grep -qE -- ' (delete|patch|exec) ' "$KIND_CALLS" && fail "teardown.sh changed or dumped something before refusing --yes on production"

if [ "$failed" -ne 0 ]; then
    echo "teardown kubectl checks failed"
    exit 1
fi
echo "teardown kubectl checks passed"
