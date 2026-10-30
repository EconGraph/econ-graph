#!/bin/bash
#
# Check the confirmation recreate-kind-cluster.sh asks before it deletes the
# kind cluster:
#
#   - the cluster is identified (kube-system UID, production marker) with
#     `docker exec` into its control plane, on the rootful daemon whatever
#     DOCKER_HOST (here a rootless socket) the shell has; never plain kind or
#     kubectl;
#   - a cluster with a production marker (any role other than `none`, valid or
#     not), or whose marker can't be read, is deleted only with the first 8
#     characters of its kube-system UID, typed at a terminal or given as
#     --confirm-production; --yes alone is refused, and so is a prefix that
#     doesn't match, on any cluster;
#   - any other cluster takes a typed `yes`, or --yes; without a terminal and
#     without the confirmation on the command line it refuses;
#   - --delete-unreachable gets the same checks: an unreachable cluster whose
#     kube-system UID can't be read is not deleted, and one whose marker can't
#     be read takes the UID prefix;
#   - every check runs before the backup, the delete and the create; the backup
#     still runs before the delete unless --no-backup;
#   - --dry-run deletes and creates nothing.
#
# Runs offline against stubs, never a cluster or a Docker daemon: a plain
# `kubectl` and `kind` first on PATH that record any call and fail; the pinned
# kind (in ${XDG_DATA_HOME}/econgraph/bin), `docker`, and the kubectl that
# `docker cp` copies out of the control plane, all of which record their
# calls. The stub `kind create cluster` records the call and then stops the
# run, so nothing after it (the gateway install) runs. HOME, XDG_CACHE_HOME,
# XDG_DATA_HOME, TMPDIR and ECONGRAPH_BACKUP_ROOT all point into a scratch
# directory, and the kind config used has no host mounts or ports. The
# terminal cases run the script under a pseudo-terminal (python3's pty module).
#
# Usage: scripts/deploy/test-recreate-kind-cluster.sh

set -uo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
CACHE="$WORK/cache/econgraph"
DATA_BIN="$WORK/data/econgraph/bin"
mkdir -p "$WORK/bin" "$WORK/home/.kube" "$WORK/tmp" "$CACHE" "$DATA_BIN" "$WORK/backups"
PLAIN="$WORK/plain.log"
CALLS="$WORK/calls.log"
KUBE_CALLS="$WORK/kind-kubectl.log"
KIND_KUBECONFIG="$CACHE/kubeconfig-econ-graph"
ROOTFUL="unix:///var/run/docker.sock"
ROOTLESS="unix:///run/user/1000/docker.sock"
CONFIG="$WORK/kind-config.yaml"
printf '%s\n' 'kind: Cluster' 'apiVersion: kind.x-k8s.io/v1alpha4' 'nodes:' '- role: control-plane' >"$CONFIG"

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

# The pinned kind (scripts/deploy/lib/kind-version.sh). FAKE_NO_CLUSTER set: no
# cluster econ-graph. `create cluster` stops the run once it is reached.
cat >"$DATA_BIN/kind-v0.33.0" <<EOF
#!/bin/sh
echo "DOCKER_HOST=\${DOCKER_HOST:-} kind \$*" >>"$CALLS"
case "\$1 \$2" in
    "version ") echo "kind v0.33.0 go1.26 linux/amd64"; exit 0 ;;
    "get clusters") [ -n "\${FAKE_NO_CLUSTER:-}" ] || echo econ-graph; exit 0 ;;
    "get nodes") printf '%s\n' econ-graph-control-plane econ-graph-worker; exit 0 ;;
    "get kubeconfig") echo "apiVersion: v1 # econ-graph"; exit 0 ;;
    "delete cluster") exit 0 ;;
    "create cluster") echo "kind stub: create reached; the stub stops here" >&2; exit 3 ;;
esac
echo "kind stub: unexpected call \$*" >&2
exit 1
EOF

# docker. FAKE_UID: the kube-system UID. FAKE_MARKER: what the marker reads as
# ("ROLE UID"; unset: no marker). FAKE_MARKER_FAILS set: the marker can't be
# read. FAKE_EXEC_FAILS set: nothing can be read inside the control plane.
# FAKE_UNREACHABLE set: the copied kubectl can't reach the API server.
cat >"$WORK/bin/docker" <<EOF
#!/bin/sh
echo "DOCKER_HOST=\${DOCKER_HOST:-} docker \$*" >>"$CALLS"
uid="\${FAKE_UID:-uid-kind-cluster}"
case "\$*" in
    "info --format "*) echo "29.0.0 /var/lib/docker [name=seccomp]"; exit 0 ;;
    "exec econ-graph-control-plane kubectl --kubeconfig /etc/kubernetes/admin.conf --request-timeout=30s get namespace kube-system -o jsonpath={.metadata.uid}")
        [ -z "\${FAKE_EXEC_FAILS:-}" ] || exit 1
        printf '%s' "\$uid"; exit 0 ;;
    "exec econ-graph-control-plane kubectl --kubeconfig /etc/kubernetes/admin.conf --request-timeout=30s -n kube-system get configmap econgraph-cluster-role --ignore-not-found -o jsonpath={.data.role}{\" \"}{.data.kube-system-uid}")
        [ -z "\${FAKE_EXEC_FAILS:-}" ] || exit 1
        [ -z "\${FAKE_MARKER_FAILS:-}" ] || exit 1
        [ -z "\${FAKE_MARKER+x}" ] || printf '%s' "\$FAKE_MARKER"
        exit 0 ;;
    "cp -q -L econ-graph-control-plane:/usr/bin/kubectl "*)
        cat >"\$5" <<'KUBECTL'
#!/bin/sh
echo "KUBECONFIG=\${KUBECONFIG:-} \$*" >>"$KUBE_CALLS"
case "\$*" in
    *" version "*) [ -z "\${FAKE_UNREACHABLE:-}" ] ;;
    *" get pod "*) exit 0 ;;
    *) echo "kubectl stub: unexpected call \$*" >&2; exit 1 ;;
esac
KUBECTL
        exit 0 ;;
    "ps -a "*) echo "  econ-graph-control-plane  Up 2 days  created 2026-10-01"; exit 0 ;;
esac
echo "docker stub: unexpected call \$*" >&2
exit 1
EOF
chmod +x "$WORK/bin/"* "$DATA_BIN/kind-v0.33.0"

RECREATE_ENV=(
    -u KUBECONFIG -u KUBECTL -u KIND -u KIND_CLUSTER -u CLUSTER_ROLE_CONTEXT -u DOCKER_CONTEXT
    -u KIND_EXPERIMENTAL_PROVIDER -u ECONGRAPH_DOCKER_HOST -u ECONGRAPH_AGENT_SESSION
    -u ECONGRAPH_KUBE_DIR -u ECONGRAPH_SCRATCH_STORAGE -u DRY_RUN -u ASSUME_YES
    HOME="$WORK/home" XDG_CACHE_HOME="$WORK/cache" XDG_DATA_HOME="$WORK/data" TMPDIR="$WORK/tmp"
    ECONGRAPH_BACKUP_ROOT="$WORK/backups" DOCKER_HOST="$ROOTLESS"
    PATH="$WORK/bin:/usr/local/bin:/usr/bin:/bin"
)

# recreate ARGS...: the script with the stubs and the scratch kind config,
# stdin not a terminal. Variables set on the call (`FAKE_X=y recreate ...`)
# reach it.
recreate() {
    env "${RECREATE_ENV[@]}" "$PROJECT_ROOT/scripts/deploy/recreate-kind-cluster.sh" \
        --config "$CONFIG" "$@" </dev/null 2>&1
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
' "$input" "$PROJECT_ROOT/scripts/deploy/recreate-kind-cluster.sh" --config "$CONFIG" "$@"
}

failed=0
fail() { echo "FAIL $*"; failed=1; }
reset() {
    : >"$PLAIN"; : >"$CALLS"; : >"$KUBE_CALLS"
    rm -rf "$KIND_KUBECONFIG" "${CACHE:?}/bin" "${WORK:?}/backups"
    mkdir -p "$WORK/backups"
}

# check NAME: no plain kind or kubectl; every kind and docker call on the
# rootful daemon.
check() {
    local bad
    if [ -s "$PLAIN" ]; then fail "$1 called plain kind or kubectl:"; sed 's/^/     /' "$PLAIN"; fi
    bad="$(grep -v "^DOCKER_HOST=$ROOTFUL " "$CALLS" || true)"
    [ -z "$bad" ] || { fail "$1 ran kind or docker on another daemon:"; printf '     %s\n' "$bad"; }
}
deleted() { grep -q ' kind delete ' "$CALLS"; }
created() { grep -q ' kind create ' "$CALLS"; }
backed_up() { grep -q '^==> Backing up to ' <<<"$out" || [ -n "$(ls -A "$WORK/backups")" ]; }

# expect_refusal NAME PATTERN: the last run ($rc, $out) failed with PATTERN
# before any backup, delete or create.
expect_refusal() {
    if [ "$rc" -ne 0 ] && grep -q -- "$2" <<<"$out" && grep -qi "nothing was deleted" <<<"$out"; then
        echo "ok   $1 refuses"
    else
        fail "$1 (exit $rc), expected a refusal matching '$2':"; printf '     %s\n' "$out"
    fi
    check "$1"
    if deleted || created || backed_up; then fail "$1 backed up, deleted or created before refusing"; fi
}
# expect_recreate NAME: the last run deleted the cluster, then reached the
# create (where the stub stops it); no backup (the runs pass --no-backup or are
# unreachable).
expect_recreate() {
    local first_delete first_create
    first_delete="$(grep -n ' kind delete ' "$CALLS" | head -1 | cut -d: -f1)"
    first_create="$(grep -n ' kind create ' "$CALLS" | head -1 | cut -d: -f1)"
    if [ -n "$first_delete" ] && [ -n "$first_create" ] && [ "$first_delete" -lt "$first_create" ] &&
        grep -q "kind stub: create reached" <<<"$out"; then
        echo "ok   $1 deletes, then creates the cluster"
    else
        fail "$1 (exit $rc), expected a delete then a create:"; printf '     %s\n' "$out"
    fi
    check "$1"
}
# expect_backup_first NAME: the last run confirmed, then started the backup
# (the stub's postgresql-0 is not running, so it stops there), deleting nothing.
expect_backup_first() {
    if [ "$rc" -ne 0 ] && grep -q '^==> Backing up to ' <<<"$out" &&
        grep -q "postgresql-0 is not running; cannot take a backup" <<<"$out" && ! deleted && ! created; then
        echo "ok   $1 confirms, then backs up before deleting"
    else
        fail "$1 (exit $rc), expected the backup after the confirmation:"; printf '     %s\n' "$out"
    fi
    check "$1"
}

# 1. No cluster: created on the rootful daemon; nothing asked or deleted.
reset
out="$(FAKE_NO_CLUSTER=1 recreate)"; rc=$?
if created && ! deleted && ! backed_up; then
    echo "ok   without a cluster: creates it, deletes nothing"
else
    fail "without a cluster (exit $rc):"; printf '     %s\n' "$out"
fi
check "without a cluster"

# 2. An unmarked cluster: without a terminal and without --yes it refuses;
# with --yes it is shown, then deleted; with the backup, the backup runs after
# the confirmation and before the delete.
reset
out="$(recreate)"; rc=$?
expect_refusal "an unmarked cluster without a terminal and without --yes" "no terminal to confirm on"
reset
out="$(recreate --yes --no-backup)"; rc=$?
expect_recreate "an unmarked cluster with --yes"
if grep -qE "nodes +econ-graph-control-plane econ-graph-worker$" <<<"$out" &&
    grep -qE "kube-system UID uid-kind-cluster \(read inside econ-graph-control-plane\)" <<<"$out" &&
    grep -qE "production +not production \(no ConfigMap" <<<"$out"; then
    echo "ok   the cluster's nodes, kube-system UID and marker are shown before it is deleted"
else
    fail "the identity is not shown:"; printf '     %s\n' "$out"
fi
reset
out="$(recreate --yes)"; rc=$?
expect_backup_first "an unmarked cluster with --yes and the backup"
reset
out="$(FAKE_MARKER="none" recreate --yes --no-backup)"; rc=$?
expect_recreate "a cluster marked role=none, with --yes"

# 3. A cluster marked production: --yes alone is refused; a wrong prefix is
# refused; the right one deletes; with the backup, it runs after the check.
reset
out="$(FAKE_MARKER="production uid-kind-cluster" recreate --yes)"; rc=$?
expect_refusal "a production cluster with --yes" "needs --confirm-production"
grep -q "marked production: this is the cluster serving econgraph.com" <<<"$out" ||
    fail "a production cluster is not shown as marked production"
reset
out="$(FAKE_MARKER="production uid-kind-cluster" recreate --yes --no-backup --confirm-production uid-othe)"; rc=$?
expect_refusal "a production cluster with the wrong --confirm-production" "does not match"
reset
out="$(FAKE_MARKER="production uid-kind-cluster" recreate --no-backup --confirm-production uid-kind)"; rc=$?
expect_recreate "a production cluster with --confirm-production uid-kind"
reset
out="$(FAKE_MARKER="production uid-kind-cluster" recreate --confirm-production=uid-kind)"; rc=$?
expect_backup_first "a production cluster with --confirm-production=uid-kind and the backup"

# 4. A marker that can't be read, or any other role than none (another
# cluster's UID, none at all, another case, another role), takes the prefix.
for marker in FAILS "production uid-other-cluster" "production" "Production uid-kind-cluster" "staging uid-kind-cluster"; do
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
out="$(FAKE_MARKER="Production uid-kind-cluster" recreate --no-backup --confirm-production uid-kind)"; rc=$?
expect_recreate "the marker 'Production' with --confirm-production uid-kind"

# 5. A wrong prefix is refused on an unmarked cluster too.
reset
out="$(recreate --yes --no-backup --confirm-production uid-othe)"; rc=$?
expect_refusal "an unmarked cluster with the wrong --confirm-production" "does not match"

# 6. An unreachable cluster: refused without --delete-unreachable; with it, the
# same checks: not deleted when its UID can't be read, the prefix when its
# marker can't be read or it is marked, --yes otherwise.
reset
out="$(FAKE_UNREACHABLE=1 recreate --yes)"; rc=$?
expect_refusal "an unreachable cluster without --delete-unreachable" "re-run with --delete-unreachable"
reset
out="$(FAKE_UNREACHABLE=1 FAKE_EXEC_FAILS=1 recreate --yes --delete-unreachable)"; rc=$?
expect_refusal "an unreachable cluster whose UID can't be read" "could not read the kube-system UID"
for hint in "docker start econ-graph-control-plane" "kubeadm certs renew all" \
    "DOCKER_HOST=$ROOTFUL $WORK/data/econgraph/bin/kind-v0.33.0 delete cluster --name econ-graph --kubeconfig $KIND_KUBECONFIG"; do
    grep -qF -- "$hint" <<<"$out" || fail "the unidentifiable-cluster refusal does not point to: $hint"
done
reset
out="$(FAKE_UNREACHABLE=1 FAKE_MARKER_FAILS=1 recreate --yes --delete-unreachable)"; rc=$?
expect_refusal "an unreachable cluster whose marker can't be read, with --yes" "needs --confirm-production"
reset
out="$(FAKE_UNREACHABLE=1 FAKE_MARKER="production uid-kind-cluster" recreate --yes --delete-unreachable)"; rc=$?
expect_refusal "an unreachable production cluster with --yes" "needs --confirm-production"
reset
out="$(FAKE_UNREACHABLE=1 FAKE_MARKER_FAILS=1 recreate --delete-unreachable --confirm-production uid-kind)"; rc=$?
expect_recreate "an unreachable cluster whose marker can't be read, with --confirm-production"
reset
out="$(FAKE_UNREACHABLE=1 recreate --yes --delete-unreachable)"; rc=$?
expect_recreate "an unreachable unmarked cluster with --yes"
if backed_up; then fail "an unreachable cluster was backed up"; fi

# 7. --dry-run on production: says what it would need, deletes and creates nothing.
reset
out="$(FAKE_MARKER="production uid-kind-cluster" recreate --dry-run --yes --no-backup)"; rc=$?
if [ "$rc" -eq 0 ] && grep -q "Dry run complete; nothing was changed" <<<"$out" &&
    grep -q "would require the first 8 characters of the kube-system UID, uid-kind" <<<"$out" &&
    grep -qF "+ kind delete cluster --name econ-graph --kubeconfig $KIND_KUBECONFIG" <<<"$out" &&
    ! deleted && ! created; then
    echo "ok   --dry-run on production shows the delete and the UID prefix it needs, changes nothing"
else
    fail "--dry-run on production (exit $rc):"; printf '     %s\n' "$out"
fi
check "--dry-run on production"
reset
out="$(FAKE_MARKER="production uid-kind-cluster" recreate --dry-run --no-backup --confirm-production uid-othe)"; rc=$?
expect_refusal "--dry-run with the wrong --confirm-production" "does not match"

# 8. At a terminal: `yes`, or on production the UID prefix, which --yes does
# not replace.
reset
out="$(recreate_tty $'yes\n' --no-backup)"; rc=$?
expect_recreate "an unmarked cluster at a terminal, typed yes"
reset
out="$(FAKE_MARKER="production uid-kind-cluster" recreate_tty $'yes\n' --yes --no-backup)"; rc=$?
grep -q "Type the first 8 characters of the kube-system UID above to delete it:" <<<"$out" ||
    fail "at a terminal: no prompt for the UID prefix"
expect_refusal "a production cluster at a terminal with --yes, typed yes" "confirmation did not match"
reset
out="$(FAKE_MARKER="production uid-kind-cluster" recreate_tty $'uid-kind\n' --yes --no-backup)"; rc=$?
expect_recreate "a production cluster at a terminal, typed the UID prefix"
reset
out="$(FAKE_UNREACHABLE=1 FAKE_MARKER_FAILS=1 recreate_tty $'uid-kind\n' --delete-unreachable)"; rc=$?
expect_recreate "an unreachable cluster with an unreadable marker at a terminal, typed the UID prefix"

# 9. Another kind provider, or --confirm-production without a value, stops
# before anything runs.
reset
out="$(env "${RECREATE_ENV[@]}" KIND_EXPERIMENTAL_PROVIDER=podman \
    "$PROJECT_ROOT/scripts/deploy/recreate-kind-cluster.sh" --config "$CONFIG" --yes --no-backup </dev/null 2>&1)"; rc=$?
if [ "$rc" -ne 0 ] && grep -q "KIND_EXPERIMENTAL_PROVIDER=podman is not supported" <<<"$out" && [ ! -s "$CALLS" ]; then
    echo "ok   another kind provider is refused before anything runs"
else
    fail "KIND_EXPERIMENTAL_PROVIDER=podman (exit $rc): $out"
fi
reset
out="$(recreate --yes --confirm-production)"; rc=$?
if [ "$rc" -eq 2 ] && [ ! -s "$CALLS" ]; then
    echo "ok   --confirm-production without a prefix stops at the argument"
else
    fail "--confirm-production without a prefix (exit $rc): $out"
fi

# 10. The marker test the script uses (lib/cluster-role.sh).
# shellcheck source=scripts/deploy/lib/cluster-role.sh
source "$PROJECT_ROOT/scripts/deploy/lib/cluster-role.sh"
for marker in "production uid-a" "production" "Production uid-a" "staging uid-a" " uid-a" "None uid-a" "NONE"; do
    cluster_role_marker_guards_delete "$marker" || fail "cluster_role_marker_guards_delete '$marker' is false"
done
for marker in "" " " "none" "none uid-a"; do
    if cluster_role_marker_guards_delete "$marker"; then fail "cluster_role_marker_guards_delete '$marker' is true"; fi
done

if [ "$failed" -ne 0 ]; then
    echo "recreate-kind-cluster checks failed"
    exit 1
fi
echo "recreate-kind-cluster checks passed"
