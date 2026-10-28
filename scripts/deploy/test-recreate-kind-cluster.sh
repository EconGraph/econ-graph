#!/bin/bash
#
# Check recreate-kind-cluster.sh:
#
#   - kind and docker always run on the rootful daemon, whatever DOCKER_HOST
#     (here a rootless socket) the shell has; never plain kind or kubectl;
#   - every `kind create` and `kind delete` passes --kubeconfig with the kind
#     cluster's kubeconfig in ~/.cache/econgraph; ~/.kube/config and $KUBECONFIG
#     are never written, and no current context is switched;
#   - --if-missing keeps an existing cluster and changes nothing;
#   - before deleting it shows the cluster; on a cluster with a production
#     marker (any role other than exactly `none`, whatever its case or UID) or
#     whose marker can't be read, only the kube-system UID prefix, typed or
#     given as --confirm-production, answers; a wrong prefix is refused on any
#     cluster; otherwise the cluster name typed at a terminal, or --yes;
#     without a terminal and without --yes it refuses;
#   - cluster_role_marker_guards_delete (lib/cluster-role.sh), the marker test
#     the script uses;
#   - an unidentifiable cluster (no kube-system UID) or a missing host mount
#     is refused; --dry-run changes nothing.
#
# Runs offline against stubs, never a cluster or a Docker daemon: a plain
# `kubectl` and `kind` first on PATH that record any call and fail; the kind
# binary kept in ${XDG_CACHE_HOME}/econgraph/bin (the one cluster_role_kind
# runs), `docker` and `mountpoint` stubs that record every call; `docker cp`
# writes a fake kind kubectl that records its calls. HOME, XDG_CACHE_HOME and
# TMPDIR all point into a scratch directory. The terminal cases run the script
# under a pseudo-terminal (python3's pty module).
#
# Usage: scripts/deploy/test-recreate-kind-cluster.sh

set -uo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
CACHE="$WORK/cache/econgraph"
mkdir -p "$WORK/bin" "$WORK/home/.kube" "$WORK/tmp" "$CACHE/bin"
PLAIN="$WORK/plain.log"
CALLS="$WORK/calls.log"
KUBE_CALLS="$WORK/kind-kubectl.log"
KIND_KUBECONFIG="$CACHE/kubeconfig-econ-graph"
HOME_KUBECONFIG="$WORK/home/.kube/config"
ROOTFUL="unix:///var/run/docker.sock"
ROOTLESS="unix:///run/user/1000/docker.sock"
echo "current-context: microk8s" >"$HOME_KUBECONFIG"
unset KUBECONFIG KUBECTL KIND CLUSTER_ROLE_CONTEXT DOCKER_HOST DOCKER_CONTEXT KIND_EXPERIMENTAL_PROVIDER \
    ECONGRAPH_DOCKER_HOST

# Plain kubectl and kind (on the production host, the MicroK8s wrapper and
# whatever kind is on PATH): must never run.
for tool in kubectl kind; do
    cat >"$WORK/bin/$tool" <<EOF
#!/bin/sh
echo "$tool \$*" >>"$PLAIN"
echo "plain $tool stub: refusing" >&2
exit 1
EOF
done

# The kind binary cluster_role_kind runs. FAKE_NO_CLUSTER set: no cluster
# econ-graph. Without --kubeconfig, create and delete write to $KUBECONFIG or
# ~/.kube/config, as kind does.
cat >"$CACHE/bin/kind" <<EOF
#!/bin/sh
echo "DOCKER_HOST=\${DOCKER_HOST:-} kind \$*" >>"$CALLS"
case "\$1 \$2" in
    "get clusters") [ -n "\${FAKE_NO_CLUSTER:-}" ] || echo econ-graph; exit 0 ;;
    "get nodes") printf '%s\n' econ-graph-control-plane econ-graph-worker; exit 0 ;;
    "get kubeconfig") echo "apiVersion: v1 # econ-graph"; exit 0 ;;
    "create cluster"|"delete cluster")
        case " \$* " in
            *" --kubeconfig "*) ;;
            *) echo "kind \$*" >>"\${KUBECONFIG:-\$HOME/.kube/config}" ;;
        esac
        exit 0 ;;
esac
echo "kind stub: unexpected call" >&2
exit 1
EOF

# docker: the reads inside the control plane, and the kubectl copy.
# FAKE_UID: the kube-system UID. FAKE_MARKED set: a production marker made on
# this cluster; FAKE_MARKER: what the marker reads as ("ROLE UID");
# FAKE_MARKER_FAILS set: the marker can't be read. FAKE_EXEC_FAILS set: nothing
# can be read (the API server doesn't answer).
cat >"$WORK/bin/docker" <<EOF
#!/bin/sh
echo "DOCKER_HOST=\${DOCKER_HOST:-} docker \$*" >>"$CALLS"
uid="\${FAKE_UID:-uid-kind-cluster}"
case "\$*" in
    "exec econ-graph-control-plane kubectl --kubeconfig /etc/kubernetes/admin.conf get namespace kube-system -o jsonpath={.metadata.uid}")
        [ -z "\${FAKE_EXEC_FAILS:-}" ] || exit 1
        printf '%s' "\$uid"; exit 0 ;;
    "exec econ-graph-control-plane kubectl --kubeconfig /etc/kubernetes/admin.conf -n kube-system get configmap econgraph-cluster-role --ignore-not-found -o jsonpath={.data.role}{\" \"}{.data.kube-system-uid}")
        [ -z "\${FAKE_EXEC_FAILS:-}" ] || exit 1
        [ -z "\${FAKE_MARKER_FAILS:-}" ] || exit 1
        [ -z "\${FAKE_MARKED:-}" ] || printf '%s' "production \$uid"
        [ -z "\${FAKE_MARKER:-}" ] || printf '%s' "\$FAKE_MARKER"
        exit 0 ;;
    "cp -q -L econ-graph-control-plane:/usr/bin/kubectl "*)
        printf '%s\n' '#!/bin/sh' 'echo "KUBECONFIG=\${KUBECONFIG:-} \$*" >>"$KUBE_CALLS"' >"\$5"
        exit 0 ;;
esac
echo "docker stub: unexpected call" >&2
exit 1
EOF

# mountpoint: FAKE_UNMOUNTED set: the host subvolumes are not mounted.
cat >"$WORK/bin/mountpoint" <<EOF
#!/bin/sh
echo "mountpoint \$*" >>"$CALLS"
[ -z "\${FAKE_UNMOUNTED:-}" ]
EOF
chmod +x "$WORK/bin/"* "$CACHE/bin/kind"

RECREATE_ENV=(
    HOME="$WORK/home" XDG_CACHE_HOME="$WORK/cache" TMPDIR="$WORK/tmp"
    DOCKER_HOST="$ROOTLESS"
    PATH="$WORK/bin:/usr/local/bin:/usr/bin:/bin"
)

# recreate ARGS...: the script with the stubs, stdin not a terminal. Variables
# set on the call (`FAKE_X=y recreate ...`) reach it.
recreate() {
    env "${RECREATE_ENV[@]}" "$PROJECT_ROOT/scripts/deploy/recreate-kind-cluster.sh" "$@" </dev/null 2>&1
}

# recreate_tty INPUT ARGS...: the same under a pseudo-terminal, with INPUT typed
# ahead; killed after a minute if it waits for more input.
recreate_tty() {
    local input="$1"
    shift
    timeout 60 env "${RECREATE_ENV[@]}" python3 -c '
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
' "$input" "$PROJECT_ROOT/scripts/deploy/recreate-kind-cluster.sh" "$@"
}

failed=0
fail() { echo "FAIL $*"; failed=1; }
reset() {
    : >"$PLAIN"; : >"$CALLS"; : >"$KUBE_CALLS"
    rm -f "$KIND_KUBECONFIG" "$CACHE/bin/kubectl"
    echo "current-context: microk8s" >"$HOME_KUBECONFIG"
}

# check NAME: no plain kind or kubectl; every kind and docker call on the
# rootful daemon; every kind create/delete with the cached kubeconfig;
# ~/.kube/config untouched; every kind kubectl call on the cached kubeconfig and
# context kind-econ-graph; no current context switched.
check() {
    local name="$1" bad
    if [ -s "$PLAIN" ]; then fail "$name called plain kind or kubectl:"; sed 's/^/     /' "$PLAIN"; fi
    bad="$(grep -v "^DOCKER_HOST=$ROOTFUL " "$CALLS" | grep -v '^mountpoint ' || true)"
    [ -z "$bad" ] || { fail "$name ran kind or docker on another daemon:"; printf '     %s\n' "$bad"; }
    bad="$(grep -E ' kind (create|delete) ' "$CALLS" | grep -vF -- "--kubeconfig $KIND_KUBECONFIG" || true)"
    [ -z "$bad" ] || { fail "$name ran kind create/delete without --kubeconfig $KIND_KUBECONFIG:"; printf '     %s\n' "$bad"; }
    [ "$(cat "$HOME_KUBECONFIG")" = "current-context: microk8s" ] || fail "$name wrote ~/.kube/config"
    bad="$(grep -v "^KUBECONFIG=$KIND_KUBECONFIG --context kind-econ-graph " "$KUBE_CALLS" || true)"
    [ -z "$bad" ] || { fail "$name ran the kind kubectl off its kubeconfig or context:"; printf '     %s\n' "$bad"; }
    if grep -qE 'use-context|set-context' "$KUBE_CALLS"; then fail "$name switched a current context"; fi
}
deleted() { grep -q ' kind delete ' "$CALLS"; }
created() { grep -q ' kind create ' "$CALLS"; }
# changed: anything deleted, created, copied or written to the cache.
changed() { deleted || created || grep -q ' docker cp ' "$CALLS" || [ -e "$KIND_KUBECONFIG" ]; }

# expect_refusal NAME PATTERN: the last run ($rc, $out) failed with PATTERN and
# changed nothing.
expect_refusal() {
    if [ "$rc" -ne 0 ] && grep -q -- "$2" <<<"$out" && grep -q "nothing was changed" <<<"$out"; then
        echo "ok   $1 refuses"
    else
        fail "$1 (exit $rc), expected a refusal matching '$2':"; printf '     %s\n' "$out"
    fi
    check "$1"
    if changed; then fail "$1 changed something before refusing"; fi
}
# expect_recreate NAME: the last run succeeded, deleted the cluster before
# creating it, and waited for the nodes with the new cluster's kubectl.
expect_recreate() {
    local first_delete first_create
    first_delete="$(grep -n ' kind delete ' "$CALLS" | head -1 | cut -d: -f1)"
    first_create="$(grep -n ' kind create ' "$CALLS" | head -1 | cut -d: -f1)"
    if [ "$rc" -eq 0 ] && [ -n "$first_delete" ] && [ -n "$first_create" ] &&
        [ "$first_delete" -lt "$first_create" ] &&
        grep -q -- ' wait --for=condition=Ready nodes ' "$KUBE_CALLS" &&
        [ "$(cat "$KIND_KUBECONFIG")" = "apiVersion: v1 # econ-graph" ]; then
        echo "ok   $1 deletes, then creates the cluster"
    else
        fail "$1 (exit $rc):"; printf '     %s\n' "$out"
    fi
    check "$1"
}

# 1. No cluster: created, with --kubeconfig, on the rootful daemon; nothing asked.
reset
out="$(FAKE_NO_CLUSTER=1 recreate)"; rc=$?
if [ "$rc" -eq 0 ] && created && ! deleted &&
    grep -q -- ' wait --for=condition=Ready nodes ' "$KUBE_CALLS" &&
    grep -qF "Docker daemon of the kind cluster: $ROOTFUL, not DOCKER_HOST=$ROOTLESS" <<<"$out"; then
    echo "ok   without a cluster: creates it on the rootful daemon, deletes nothing"
else
    fail "without a cluster (exit $rc):"; printf '     %s\n' "$out"
fi
check "without a cluster"

# 2. --if-missing with the cluster present: exit 0, nothing deleted or created.
reset
out="$(recreate --if-missing)"; rc=$?
if [ "$rc" -eq 0 ] && grep -q "kept (--if-missing)" <<<"$out" && ! changed; then
    echo "ok   --if-missing keeps an existing cluster and changes nothing"
else
    fail "--if-missing with a cluster (exit $rc):"; printf '     %s\n' "$out"
fi
check "--if-missing with a cluster"

# 3. --if-missing without a cluster: created.
reset
out="$(FAKE_NO_CLUSTER=1 recreate --if-missing)"; rc=$?
if [ "$rc" -eq 0 ] && created && ! deleted; then
    echo "ok   --if-missing without a cluster creates one"
else
    fail "--if-missing without a cluster (exit $rc):"; printf '     %s\n' "$out"
fi
check "--if-missing without a cluster"

# 4. An existing unmarked cluster without a terminal: --yes is needed.
reset
out="$(recreate)"; rc=$?
expect_refusal "an existing cluster without a terminal and without --yes" "no terminal to confirm on"

# 5. ... with --yes: shown, deleted and created.
reset
out="$(recreate --yes)"; rc=$?
expect_recreate "an existing unmarked cluster with --yes"
if grep -qE "nodes +econ-graph-control-plane econ-graph-worker$" <<<"$out" &&
    grep -qE "kube-system UID uid-kind-cluster" <<<"$out" &&
    grep -qE "production +not production \(no ConfigMap" <<<"$out"; then
    echo "ok   the cluster's nodes, kube-system UID and marker are shown before it is deleted"
else
    fail "the identity is not shown:"; printf '     %s\n' "$out"
fi

# 6. A cluster marked production: --yes alone is refused; a wrong prefix is
# refused; the right one deletes.
reset
out="$(FAKE_MARKED=1 recreate --yes)"; rc=$?
expect_refusal "a production cluster with --yes" "needs --confirm-production"
reset
out="$(FAKE_MARKED=1 recreate --yes --confirm-production uid-othe)"; rc=$?
expect_refusal "a production cluster with the wrong --confirm-production" "does not match"
reset
out="$(FAKE_MARKED=1 recreate --confirm-production uid-kind)"; rc=$?
expect_recreate "a production cluster with --confirm-production uid-kind"

# 7. A marker that can't be read, or any role other than exactly `none` (made
# on another cluster, without a UID, another case, another role), takes the
# prefix; --yes alone is refused.
for marker in FAILS "production uid-other-cluster" "production" "Production uid-kind-cluster" \
    "None uid-kind-cluster" "NONE" "staging uid-kind-cluster"; do
    reset
    if [ "$marker" = FAILS ]; then
        out="$(FAKE_MARKER_FAILS=1 recreate --yes)"; rc=$?
        name="an unreadable marker"
    else
        out="$(FAKE_MARKER="$marker" recreate --yes)"; rc=$?
        name="the marker '$marker'"
    fi
    expect_refusal "$name with --yes" "needs --confirm-production"
    grep -q "treated as production" <<<"$out" || fail "$name is not shown as treated as production"
done
reset
out="$(FAKE_MARKER="Production uid-kind-cluster" recreate --confirm-production uid-kind)"; rc=$?
expect_recreate "the marker 'Production' with --confirm-production uid-kind"
# role=none (with or without a UID) is not production: --yes deletes.
for marker in "none" "none uid-kind-cluster"; do
    reset
    out="$(FAKE_MARKER="$marker" recreate --yes)"; rc=$?
    expect_recreate "the marker '$marker' with --yes"
    grep -qE "production +not production \(role=none\)" <<<"$out" ||
        fail "the marker '$marker' is not shown as not production"
done
# A wrong prefix is refused on an unmarked cluster too.
reset
out="$(recreate --yes --confirm-production uid-othe)"; rc=$?
expect_refusal "an unmarked cluster with the wrong --confirm-production" "does not match"

# 8. A cluster that can't be identified, or missing host mounts: refused.
reset
out="$(FAKE_EXEC_FAILS=1 recreate --yes)"; rc=$?
expect_refusal "a cluster whose kube-system UID can't be read" "could not read the kube-system UID"
reset
out="$(FAKE_UNMOUNTED=1 recreate --yes)"; rc=$?
expect_refusal "unmounted host subvolumes" "is not mounted"

# 9. --dry-run, on production with every confirmation given: shows the commands,
# changes nothing.
for marked in "" 1; do
    reset
    out="$(FAKE_MARKED=$marked recreate --dry-run --yes)"; rc=$?
    if [ "$rc" -eq 0 ] && grep -q "Dry run complete: nothing was changed" <<<"$out" &&
        grep -qF "+ kind delete cluster --name econ-graph --kubeconfig $KIND_KUBECONFIG" <<<"$out" &&
        grep -qF "+ kind create cluster --name econ-graph --config $PROJECT_ROOT/terraform/k8s/kind-config.yaml --kubeconfig $KIND_KUBECONFIG" <<<"$out" &&
        ! changed && [ ! -s "$KUBE_CALLS" ]; then
        echo "ok   --dry-run${marked:+ on production} shows the delete and create and changes nothing"
    else
        fail "--dry-run${marked:+ on production} (exit $rc):"; printf '     %s\n' "$out"
    fi
    check "--dry-run${marked:+ on production}"
    if [ -n "$marked" ] && ! grep -q "would require the first 8 characters of the kube-system UID (uid-kind)" <<<"$out"; then
        fail "--dry-run on production does not say it needs the UID prefix"
    fi
done

# 10. $KUBECONFIG set to another cluster's file: still never written.
reset
out="$(KUBECONFIG="$HOME_KUBECONFIG" recreate --yes)"; rc=$?
expect_recreate "an existing cluster with KUBECONFIG set to ~/.kube/config"

# 11. ECONGRAPH_DOCKER_HOST chooses the daemon.
reset
out="$(FAKE_NO_CLUSTER=1 ECONGRAPH_DOCKER_HOST=unix:///stub/docker.sock recreate)"; rc=$?
bad="$(grep -v '^DOCKER_HOST=unix:///stub/docker.sock ' "$CALLS" | grep -v '^mountpoint ' || true)"
if [ "$rc" -eq 0 ] && [ -z "$bad" ]; then
    echo "ok   ECONGRAPH_DOCKER_HOST chooses the daemon"
else
    fail "ECONGRAPH_DOCKER_HOST (exit $rc): $bad"; printf '     %s\n' "$out"
fi

# 12. At a terminal: the cluster name, or on production the UID prefix.
reset
out="$(recreate_tty $'econ-graph\n')"; rc=$?
grep -q "Type 'econ-graph' to delete it" <<<"$out" || fail "at a terminal: no prompt for the cluster name"
expect_recreate "an unmarked cluster at a terminal, typed econ-graph"
reset
out="$(recreate_tty $'y\n')"; rc=$?
expect_refusal "an unmarked cluster at a terminal, answered y" "confirmation did not match"
reset
out="$(FAKE_MARKED=1 recreate_tty $'y\n' --yes)"; rc=$?
expect_refusal "a production cluster at a terminal with --yes, answered y" "confirmation did not match"
reset
out="$(FAKE_MARKED=1 recreate_tty $'uid-kind\n')"; rc=$?
expect_recreate "a production cluster at a terminal, typed the UID prefix"

# 13. An unknown argument (a mistyped --dry-run) stops before anything runs.
reset
out="$(recreate --dryrun)"; rc=$?
if [ "$rc" -eq 2 ] && [ ! -s "$CALLS" ]; then
    echo "ok   an unknown argument stops before anything runs"
else
    fail "--dryrun (exit $rc) did not stop at the argument: $out"
fi

# 14. The marker test the script uses (lib/cluster-role.sh).
# shellcheck source=scripts/deploy/lib/cluster-role.sh
source "$PROJECT_ROOT/scripts/deploy/lib/cluster-role.sh"
for marker in "production uid-a" "production" "Production uid-a" "staging uid-a" " uid-a" "None uid-a" "NONE"; do
    cluster_role_marker_guards_delete "$marker" || fail "cluster_role_marker_guards_delete '$marker' is false"
done
for marker in "" " " "none" "none uid-a"; do
    if cluster_role_marker_guards_delete "$marker"; then fail "cluster_role_marker_guards_delete '$marker' is true"; fi
done
echo "ok   cluster_role_marker_guards_delete: any role but exactly none guards the delete"

if [ "$failed" -ne 0 ]; then
    echo "recreate-kind-cluster checks failed"
    exit 1
fi
echo "recreate-kind-cluster checks passed"
