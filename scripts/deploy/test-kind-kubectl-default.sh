#!/usr/bin/env bash
# The stubs are quoted heredocs and run on their own.
# shellcheck disable=SC2016
#
# Check that the deploy scripts run with none of KUBECTL, KUBECONFIG and
# DOCKER_HOST set reach the kind cluster through its own kubectl and kubeconfig
# in ~/.cache/econgraph, and kind and docker through the rootful Docker daemon
# (unix:///var/run/docker.sock), never a plain `kubectl` or `kind` from PATH. On
# the production host plain `kubectl` is the MicroK8s snap wrapper and the
# operator's DOCKER_HOST is a rootless daemon, so either would reach another
# cluster, or none.
#
# Covered: backup-keycloak-db.sh (lib/keycloak-db.sh kdb_use_context), in a dry
# run and a real one; deploy.sh up to its first apply; teardown.sh --dry-run;
# apply-host-storage.sh up to its host checks; build-images.sh;
# setup-port-forwarding.sh and setup-letsencrypt-cloudflare.sh up to their
# cluster check. Also that DOCKER_HOST set to another daemon is replaced, and
# said so.
#
# Runs offline against stubs, never a cluster or a Docker daemon: a plain
# `kubectl` and `kind` first on PATH that record any call and fail; a fake kind
# kubectl, kubeconfig and kind in a scratch HOME's ~/.cache/econgraph; and
# `docker` as a stub. kind and docker record the DOCKER_HOST they were run with.
# HOME, TMPDIR, the backup roots and the host-storage root are in a scratch
# directory.
#
# Usage: scripts/deploy/test-kind-kubectl-default.sh

set -uo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
HOME_DIR="$WORK/home"
CACHE="$HOME_DIR/.cache/econgraph"
LOGS="$WORK/logs"
ROOTFUL="unix:///var/run/docker.sock"
UID_KIND="11111111-2222-3333-4444-555555555555"
mkdir -p "$WORK/bin" "$WORK/tmp" "$WORK/host" "$LOGS" "$CACHE/bin"

# Plain kubectl and kind: any call is a failure of the test.
for tool in kubectl kind; do
    cat >"$WORK/bin/$tool" <<'STUB'
#!/usr/bin/env bash
echo "$(basename "$0") $*" >>"$TEST_LOGS/plain.log"
echo "plain $(basename "$0") from PATH was called: $*" >&2
exit 1
STUB
done

# The kind cluster's kubectl. It knows context kind-econ-graph only, refuses to
# apply anything (so deploy.sh stops at its first apply), and fails any call
# that contains FAKE_FAIL. Every call is recorded with its path and KUBECONFIG.
cat >"$CACHE/bin/kubectl" <<'STUB'
#!/usr/bin/env bash
echo "$0 KUBECONFIG=${KUBECONFIG:-} $*" >>"$TEST_LOGS/kind-kubectl.log"
args=("$@")
ctx=""
if [ "${args[0]:-}" = --context ]; then ctx="${args[1]}"; args=("${args[@]:2}"); fi
if [ "${args[0]:-}" = -n ]; then args=("${args[@]:2}"); fi
set -- "${args[@]}"
if [ -n "$ctx" ] && [ "$ctx" != kind-econ-graph ]; then
    echo "error: context \"$ctx\" does not exist" >&2
    exit 1
fi
if [ -n "${FAKE_FAIL:-}" ] && [[ "$*" == *"$FAKE_FAIL"* ]]; then
    echo "fake kind kubectl: failing as asked: $*" >&2
    exit 1
fi
case " $* " in
    *" apply "*) echo "fake kind kubectl: refusing to apply" >&2; exit 1 ;;
esac
case "$*" in
    "config get-contexts -o name"* | "config current-context") echo kind-econ-graph ;;
    "config use-context kind-econ-graph") echo 'Switched to context "kind-econ-graph".' ;;
    "config use-context "*) echo "error: no context exists with the name: ${*: -1}" >&2; exit 1 ;;
    "config view --minify"*) printf 'https://127.0.0.1:6443' ;;
    "get nodes"*) printf 'econ-graph-control-plane econ-graph-worker' ;;
    "get namespace kube-system"*) printf '%s' "$TEST_UID" ;;
    "get pod keycloak-postgres-0"*) printf true ;;
esac
exit 0
STUB
echo "# fake kubeconfig of kind cluster econ-graph" >"$CACHE/kubeconfig-econ-graph"

# The kind kept next to it (cluster_role_kind in lib/cluster-role.sh). FAKE_CLUSTERS
# is what `kind get clusters` lists (default econ-graph).
cat >"$CACHE/bin/kind" <<'STUB'
#!/usr/bin/env bash
echo "kind DOCKER_HOST=${DOCKER_HOST:-} $*" >>"$TEST_LOGS/cached-kind.log"
case "$*" in
    "get clusters") [ -z "${FAKE_CLUSTERS-econ-graph}" ] || echo "${FAKE_CLUSTERS-econ-graph}" ;;
    "get nodes --name econ-graph") printf '%s\n' econ-graph-control-plane econ-graph-worker ;;
    "load docker-image "*) ;;
    *) echo "stub kind: unexpected call: $*" >&2; exit 1 ;;
esac
STUB

# docker: builds (doing nothing) and the kube-system UID read inside the
# control-plane container (teardown.sh); anything else fails.
cat >"$WORK/bin/docker" <<'STUB'
#!/usr/bin/env bash
echo "docker DOCKER_HOST=${DOCKER_HOST:-} $*" >>"$TEST_LOGS/docker.log"
case "$*" in
    "build "*) ;;
    "exec econ-graph-control-plane kubectl --kubeconfig /etc/kubernetes/admin.conf get namespace kube-system "*)
        printf '%s' "$TEST_UID" ;;
    *) echo "stub docker: unexpected call: $*" >&2; exit 1 ;;
esac
STUB
chmod +x "$WORK/bin/"* "$CACHE/bin/kubectl" "$CACHE/bin/kind"

# deploy.sh runs from a copy of scripts/deploy only, so that it skips the
# Grafana and monitoring linters (absent) and stops at its first apply.
TREE="$WORK/tree"
mkdir -p "$TREE/scripts"
cp -r "$PROJECT_ROOT/scripts/deploy" "$TREE/scripts/deploy"

# run SCRIPT ARGS...: the script with none of KUBECTL, KUBECONFIG, DOCKER_HOST
# (and the settings that would choose another cluster) set, the scratch HOME,
# the stubs first on PATH, stdin not a terminal. Output in $out, status in $rc.
run() {
    : >"$LOGS/plain.log"
    : >"$LOGS/kind-kubectl.log"
    : >"$LOGS/docker.log"
    : >"$LOGS/cached-kind.log"
    out="$(env -u KUBECTL -u KUBECONFIG -u DOCKER_HOST -u DOCKER_CONTEXT -u ECONGRAPH_DOCKER_HOST \
        -u CLUSTER_ROLE_CONTEXT -u KIND -u KIND_CLUSTER_NAME -u KIND_EXPERIMENTAL_PROVIDER \
        -u NAMESPACE -u XDG_CACHE_HOME -u DRY_RUN -u VERSION \
        HOME="$HOME_DIR" TMPDIR="$WORK/tmp" PATH="$WORK/bin:/usr/local/bin:/usr/bin:/bin" \
        ECONGRAPH_BACKUP_ROOT="$WORK/backups" KEYCLOAK_BACKUP_ROOT="$WORK/backups/keycloak" \
        ECONGRAPH_HOST_ROOT="$WORK/host" TEST_LOGS="$LOGS" TEST_UID="$UID_KIND" \
        "$@" </dev/null 2>&1)"
    rc=$?
}

failed=0
fail() {
    echo "FAIL $*"
    printf '     %s\n' "$out"
    failed=1
}

# expect NAME RC MESSAGE CALL: the last run exited RC (0 or "nonzero") and
# printed MESSAGE; plain kubectl and kind were never called; the kind kubectl
# was called, always as $CACHE/bin/kubectl with KUBECONFIG
# $CACHE/kubeconfig-econ-graph, and among others with CALL; and kind and docker,
# if called, ran on the rootful daemon.
expect() {
    local name="$1" want="$2" message="$3" call="$4" bad
    if { [ "$want" = 0 ] && [ "$rc" -ne 0 ]; } || { [ "$want" = nonzero ] && [ "$rc" -eq 0 ]; }; then
        fail "$name: exit status $rc, expected $want"
        return
    fi
    if ! grep -qF -- "$message" <<<"$out"; then
        fail "$name: no '$message' in the output"
        return
    fi
    if [ -s "$LOGS/plain.log" ]; then
        fail "$name: called plain kubectl or kind from PATH: $(cat "$LOGS/plain.log")"
        return
    fi
    if [ ! -s "$LOGS/kind-kubectl.log" ]; then
        fail "$name: never called the kind cluster's kubectl"
        return
    fi
    bad="$(grep -vF "$CACHE/bin/kubectl KUBECONFIG=$CACHE/kubeconfig-econ-graph " "$LOGS/kind-kubectl.log" || true)"
    if [ -n "$bad" ]; then
        fail "$name: kubectl ran from another place or with another KUBECONFIG: $bad"
        return
    fi
    if ! grep -qF -- " $call" "$LOGS/kind-kubectl.log"; then
        fail "$name: the kind cluster's kubectl never got '$call': $(cat "$LOGS/kind-kubectl.log")"
        return
    fi
    expect_docker "$name"
}

# expect_docker NAME: kind and docker, if called, ran on the rootful daemon.
expect_docker() {
    local bad
    bad="$(cat "$LOGS/docker.log" "$LOGS/cached-kind.log" | grep -vF "DOCKER_HOST=$ROOTFUL " || true)"
    if [ -n "$bad" ]; then
        fail "$1: kind or docker ran on another Docker daemon: $bad"
        return
    fi
    echo "ok   $1"
}

# expect_rootful NAME CALL: the last run ran CALL (kind ... or docker ...) on
# the rootful daemon and said which daemon it chose.
expect_rootful() {
    local call="$2"
    if ! cat "$LOGS/cached-kind.log" "$LOGS/docker.log" |
        grep -qF "${call%% *} DOCKER_HOST=$ROOTFUL ${call#* }"; then
        fail "$1: did not run '$call' on $ROOTFUL: $(cat "$LOGS/cached-kind.log" "$LOGS/docker.log")"
    elif ! grep -qF "Docker daemon of the kind cluster: $ROOTFUL" <<<"$out"; then
        fail "$1: did not say which Docker daemon it uses"
    else
        echo "ok   $1 ($call on $ROOTFUL)"
    fi
}

DEPLOY="$PROJECT_ROOT/scripts/deploy"

# --- backup-keycloak-db.sh (lib/keycloak-db.sh kdb_use_context) ------------------
run "$DEPLOY/backup-keycloak-db.sh" --dry-run
expect "backup-keycloak-db.sh --dry-run" 0 "dry run on context kind-econ-graph" \
    "config get-contexts -o name"
grep -qF -- " --context kind-econ-graph get namespace kube-system" "$LOGS/kind-kubectl.log" ||
    fail "backup-keycloak-db.sh --dry-run did not read the cluster on context kind-econ-graph"

# A CLUSTER_ROLE_CONTEXT left in the environment doesn't keep the kind kubectl
# from being chosen.
run env CLUSTER_ROLE_CONTEXT=microk8s "$DEPLOY/backup-keycloak-db.sh" --dry-run
expect "backup-keycloak-db.sh --dry-run with CLUSTER_ROLE_CONTEXT set" 0 \
    "dry run on context kind-econ-graph" "config get-contexts -o name"

run env FAKE_FAIL=pg_dump "$DEPLOY/backup-keycloak-db.sh"
expect "backup-keycloak-db.sh" nonzero "failed; what was written is in" \
    "--context kind-econ-graph -n econ-graph exec -i keycloak-postgres-0 -- pg_dump"

# --- deploy.sh, up to its first apply ----------------------------------------------
run env PUBLIC_HOST= "$TREE/scripts/deploy/deploy.sh"
expect "deploy.sh" nonzero "fake kind kubectl: refusing to apply" \
    "--context kind-econ-graph apply -f k8s/manifests/namespace.yaml"
expect_rootful "deploy.sh" "kind get clusters"
grep -qF " config use-context kind-econ-graph" "$LOGS/kind-kubectl.log" ||
    fail "deploy.sh did not switch the kind kubectl to kind-econ-graph"

# The shell's DOCKER_HOST (here a rootless socket) is replaced, and said so.
run env PUBLIC_HOST= DOCKER_HOST=unix:///run/user/1000/docker.sock "$TREE/scripts/deploy/deploy.sh"
if grep -qF "Docker daemon of the kind cluster: $ROOTFUL, not DOCKER_HOST=unix:///run/user/1000/docker.sock" <<<"$out"; then
    expect_docker "deploy.sh with a rootless DOCKER_HOST"
else
    fail "deploy.sh with a rootless DOCKER_HOST did not say it replaced it"
fi

# --- teardown.sh -------------------------------------------------------------------
run "$DEPLOY/teardown.sh" --dry-run
expect "teardown.sh --dry-run" 0 "Dry run complete: nothing was changed" \
    "--context kind-econ-graph get namespace kube-system"
expect_rootful "teardown.sh --dry-run" "docker exec econ-graph-control-plane"

# --- apply-host-storage.sh: stops at the (scratch) host checks ---------------------
run "$DEPLOY/apply-host-storage.sh" --dry-run
expect "apply-host-storage.sh --dry-run" nonzero "context kind-econ-graph" "config current-context"
grep -qF "Docker daemon of the kind cluster: $ROOTFUL" <<<"$out" ||
    fail "apply-host-storage.sh did not say which Docker daemon it uses"

# --- build-images.sh ------------------------------------------------------------------
: >"$LOGS/kind-kubectl.log"
run "$DEPLOY/build-images.sh"
if [ "$rc" -ne 0 ]; then
    fail "build-images.sh exits $rc"
elif [ -s "$LOGS/plain.log" ]; then
    fail "build-images.sh called plain kubectl or kind from PATH: $(cat "$LOGS/plain.log")"
elif ! grep -qF "kind DOCKER_HOST=$ROOTFUL load docker-image econ-graph-backend:" "$LOGS/cached-kind.log"; then
    fail "build-images.sh did not load the images with the cached kind on $ROOTFUL"
else
    expect_rootful "build-images.sh" "docker build"
    expect_docker "build-images.sh"
fi

# --- setup-port-forwarding.sh and setup-letsencrypt-cloudflare.sh: stop at the cluster check
run env FAKE_FAIL=cluster-info "$DEPLOY/setup-port-forwarding.sh"
expect "setup-port-forwarding.sh" nonzero "Cannot connect to Kubernetes cluster" "cluster-info"

run env FAKE_FAIL=cluster-info "$DEPLOY/setup-letsencrypt-cloudflare.sh" -d example.org -t stub-token --staging
expect "setup-letsencrypt-cloudflare.sh" nonzero "Cannot connect to Kubernetes cluster" "cluster-info"

# An explicit KUBECTL is used as it is (here the same fake).
run env KUBECTL="$CACHE/bin/kubectl" KUBECONFIG="$CACHE/kubeconfig-econ-graph" FAKE_FAIL=cluster-info \
    "$DEPLOY/setup-port-forwarding.sh"
expect "setup-port-forwarding.sh with KUBECTL" nonzero "Cannot connect to Kubernetes cluster" "cluster-info"
# Past the cluster check, an explicit KUBECTL keeps its current context: no switch to
# kind-econ-graph. Stopped at the service check, before any port forward (or pkill).
run env KUBECTL="$CACHE/bin/kubectl" KUBECONFIG="$CACHE/kubeconfig-econ-graph" \
    FAKE_FAIL="get service grafana-service" "$DEPLOY/setup-port-forwarding.sh"
expect "setup-port-forwarding.sh with KUBECTL, past the cluster check" nonzero \
    "Grafana service not found" "get service grafana-service"
if grep -qF " config use-context" "$LOGS/kind-kubectl.log"; then
    fail "setup-port-forwarding.sh with KUBECTL: switched the context of its kubeconfig"
fi
# Without either, the kind kubeconfig is switched to kind-econ-graph.
run env FAKE_FAIL="get service grafana-service" "$DEPLOY/setup-port-forwarding.sh"
expect "setup-port-forwarding.sh, past the cluster check" nonzero \
    "Grafana service not found" "config use-context kind-econ-graph"

# --- cluster_role_default_kubectl: never plain kubectl ---------------------------------
# No kind kubectl copies (a HOME whose cache holds only the kind binary): each script
# stops before any kubectl.
mkdir -p "$WORK/kind-only-home/.cache/econgraph/bin"
cp "$CACHE/bin/kind" "$WORK/kind-only-home/.cache/econgraph/bin/kind"
nocache() { # NAME ARGS...: the script, run without the kind kubectl copies, stopped before any kubectl
    local name="$1"
    shift
    run env HOME="$WORK/kind-only-home" "$@"
    if [ "$rc" -eq 0 ] || [ -s "$LOGS/plain.log" ] || [ -s "$LOGS/kind-kubectl.log" ] ||
        ! grep -qF "or set KUBECTL (and KUBECONFIG)" <<<"$out"; then
        fail "$name without the kind kubectl: exit $rc, plain $(cat "$LOGS/plain.log")"
    else
        echo "ok   $name without the kind kubectl stops"
    fi
}
nocache "deploy.sh" env PUBLIC_HOST= "$TREE/scripts/deploy/deploy.sh"
nocache "backup-keycloak-db.sh --dry-run" "$DEPLOY/backup-keycloak-db.sh" --dry-run
nocache "setup-port-forwarding.sh" "$DEPLOY/setup-port-forwarding.sh"
nocache "setup-letsencrypt-cloudflare.sh" "$DEPLOY/setup-letsencrypt-cloudflare.sh" -d example.org -t stub-token --staging
nocache "backup-postgres-now.sh" "$DEPLOY/backup-postgres-now.sh"
nocache "protect-postgres-pv.sh with only CLUSTER_ROLE_CONTEXT" env CLUSTER_ROLE_CONTEXT=kind-econ-graph \
    "$DEPLOY/protect-postgres-pv.sh"
# A CLUSTER_ROLE_CONTEXT left in the environment, without KUBECTL, doesn't choose the
# kubectl: the kind one, on that context.
run env CLUSTER_ROLE_CONTEXT=kind-econ-graph "$DEPLOY/protect-postgres-pv.sh"
if [ -s "$LOGS/plain.log" ] || [ ! -s "$LOGS/kind-kubectl.log" ] ||
    grep -vqF "$CACHE/bin/kubectl KUBECONFIG=$CACHE/kubeconfig-econ-graph --context kind-econ-graph " "$LOGS/kind-kubectl.log"; then
    fail "protect-postgres-pv.sh with only CLUSTER_ROLE_CONTEXT: $(cat "$LOGS/plain.log" "$LOGS/kind-kubectl.log")"
else
    echo "ok   protect-postgres-pv.sh with only CLUSTER_ROLE_CONTEXT uses the kind kubectl on it"
fi
# KUBECONFIG alone: the kind kubectl binary with it (plain kubectl may ignore it).
other="$WORK/other-kubeconfig"
echo "# another kubeconfig" >"$other"
run env KUBECONFIG="$other" "$DEPLOY/backup-keycloak-db.sh" --dry-run
if [ "$rc" -ne 0 ] || [ -s "$LOGS/plain.log" ] ||
    grep -vqF "$CACHE/bin/kubectl KUBECONFIG=$other " "$LOGS/kind-kubectl.log" ||
    ! grep -qF "Using the kind cluster's kubectl $CACHE/bin/kubectl with KUBECONFIG=$other" <<<"$out"; then
    fail "backup-keycloak-db.sh with only KUBECONFIG: exit $rc, $(cat "$LOGS/kind-kubectl.log" "$LOGS/plain.log")"
else
    echo "ok   backup-keycloak-db.sh with only KUBECONFIG uses the kind kubectl with it"
fi
run env HOME="$WORK/empty-home" KUBECONFIG="$other" "$DEPLOY/backup-keycloak-db.sh" --dry-run
if [ "$rc" -eq 0 ] || [ -s "$LOGS/plain.log" ] || ! grep -qF "Set KUBECTL too" <<<"$out"; then
    fail "backup-keycloak-db.sh with only KUBECONFIG and no kind kubectl: exit $rc"
else
    echo "ok   backup-keycloak-db.sh with only KUBECONFIG and no kind kubectl stops"
fi

if [ "$failed" -ne 0 ]; then
    exit 1
fi
echo "The deploy scripts reach only the kind cluster's kubectl, and kind on $ROOTFUL."
