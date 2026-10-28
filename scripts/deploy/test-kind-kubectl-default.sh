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
# run and a real one; deploy.sh up to its first apply; cutover-to-gateway.sh
# --dry-run; install-gateway.sh; apply-gateway-routes.sh --dry-run; and
# setup-port-forwarding.sh, which stops when the kind cluster is missing
# instead of falling back to plain kubectl.
#
# Runs offline against stubs, never a cluster or a Docker daemon: a plain
# `kubectl` and `kind` first on PATH that record any call and fail; a fake kind
# kubectl and kubeconfig in a scratch HOME's ~/.cache/econgraph; the pinned
# kind (scripts/deploy/lib/kind-version.sh) and `docker` as stubs that record
# the DOCKER_HOST they were run with. HOME, TMPDIR and the backup roots are in
# a scratch directory.
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
mkdir -p "$WORK/bin" "$WORK/tmp" "$WORK/source" "$LOGS" "$CACHE/bin" "$HOME_DIR/.local/share/econgraph/bin" || exit 1

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
cat >"$WORK/source/kubectl" <<'STUB'
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
    "config get-contexts -o name" | "config current-context") echo kind-econ-graph ;;
    "config use-context kind-econ-graph") echo 'Switched to context "kind-econ-graph".' ;;
    "config use-context "*) echo "error: no context exists with the name: ${*: -1}" >&2; exit 1 ;;
    "config view --minify"*) printf 'https://127.0.0.1:6443' ;;
    "get namespace kube-system"*) printf '11111111-2222-3333-4444-555555555555' ;;
    "get pod keycloak-postgres-0"*) printf true ;;
esac
exit 0
STUB
echo "# fake kubeconfig of kind cluster econ-graph" >"$WORK/source/kubeconfig"
cp "$WORK/source/kubectl" "$CACHE/bin/kubectl" || exit 1
cp "$WORK/source/kubeconfig" "$CACHE/kubeconfig-econ-graph" || exit 1

# docker: `info` names a rootful daemon, `cp` of the node's kubectl copies the
# fake; anything else fails.
cat >"$WORK/bin/docker" <<'STUB'
#!/usr/bin/env bash
echo "docker DOCKER_HOST=${DOCKER_HOST:-} $*" >>"$TEST_LOGS/docker.log"
case "$*" in
    "info --format "*) echo "29.0.0 /var/lib/docker [name=seccomp]" ;;
    "cp -q -L econ-graph-control-plane:/usr/bin/kubectl "*) cp "$TEST_SOURCE/kubectl" "${*: -1}" ;;
    *) echo "stub docker: unexpected call: $*" >&2; exit 1 ;;
esac
STUB

# The pinned kind the scripts run (kind_bin in lib/kind-env.sh). FAKE_CLUSTERS
# is what `kind get clusters` lists (default econ-graph).
# shellcheck source=scripts/deploy/lib/kind-version.sh
KIND_PINNED="$(HOME="$HOME_DIR" XDG_DATA_HOME="" bash -c \
    "source '$PROJECT_ROOT/scripts/deploy/lib/kind-version.sh'; echo \"\$KIND_PINNED_BIN\"")"
case "$KIND_PINNED" in
    "$HOME_DIR"/*) ;;
    *) echo "FAIL the pinned kind would be $KIND_PINNED, outside the scratch HOME"; exit 1 ;;
esac
mkdir -p "$(dirname "$KIND_PINNED")" || exit 1
cat >"$KIND_PINNED" <<'STUB'
#!/usr/bin/env bash
echo "kind DOCKER_HOST=${DOCKER_HOST:-} $*" >>"$TEST_LOGS/pinned-kind.log"
case "$*" in
    "get clusters") [ -z "${FAKE_CLUSTERS-econ-graph}" ] || echo "${FAKE_CLUSTERS-econ-graph}" ;;
    "get kubeconfig --name econ-graph") cat "$TEST_SOURCE/kubeconfig" ;;
    version) echo "kind v0.33.0 go1.25.0 linux/amd64" ;;
    *) echo "stub kind: unexpected call: $*" >&2; exit 1 ;;
esac
STUB
chmod +x "$WORK/bin/"* "$WORK/source/kubectl" "$CACHE/bin/kubectl" "$KIND_PINNED" ||
    { echo "FAIL could not make the test stubs executable" >&2; exit 1; }

# deploy.sh runs from a copy of scripts/deploy only, so that it skips the
# Grafana and monitoring linters (absent) and stops at its first apply.
TREE="$WORK/tree"
mkdir -p "$TREE/scripts" || exit 1
cp -r "$PROJECT_ROOT/scripts/deploy" "$TREE/scripts/deploy" || exit 1

# run SCRIPT ARGS...: the script with none of KUBECTL, KUBECONFIG, DOCKER_HOST
# (and the settings that would choose another cluster) set, the scratch HOME,
# the stubs first on PATH, stdin not a terminal. Output in $out, status in $rc.
run() {
    : >"$LOGS/plain.log"
    : >"$LOGS/kind-kubectl.log"
    : >"$LOGS/docker.log"
    : >"$LOGS/pinned-kind.log"
    out="$(env -u KUBECTL -u KUBECONFIG -u DOCKER_HOST -u DOCKER_CONTEXT -u ECONGRAPH_DOCKER_HOST \
        -u ECONGRAPH_KUBE_DIR -u CLUSTER_ROLE_CONTEXT -u KIND_CLUSTER -u NAMESPACE \
        -u ECONGRAPH_SCRATCH_STORAGE -u XDG_CACHE_HOME -u XDG_DATA_HOME -u XDG_STATE_HOME \
        -u KIND_FALLBACK_WARNED -u DRY_RUN \
        HOME="$HOME_DIR" TMPDIR="$WORK/tmp" PATH="$WORK/bin:/usr/local/bin:/usr/bin:/bin" \
        ECONGRAPH_BACKUP_ROOT="$WORK/backups" KEYCLOAK_BACKUP_ROOT="$WORK/backups/keycloak" \
        TEST_LOGS="$LOGS" TEST_SOURCE="$WORK/source" \
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
    bad="$(cat "$LOGS/docker.log" "$LOGS/pinned-kind.log" | grep -vF "DOCKER_HOST=$ROOTFUL " || true)"
    if [ -n "$bad" ]; then
        fail "$name: kind or docker ran on another Docker daemon: $bad"
        return
    fi
    echo "ok   $name"
}

# expect_rootful NAME: the last run asked kind for the cluster and named the
# rootful daemon it chose.
expect_rootful() {
    if ! grep -qxF "kind DOCKER_HOST=$ROOTFUL get clusters" "$LOGS/pinned-kind.log"; then
        fail "$1: did not list the kind clusters on $ROOTFUL: $(cat "$LOGS/pinned-kind.log")"
    elif ! grep -qF "docker: $ROOTFUL (rootful" <<<"$out"; then
        fail "$1: did not say which Docker daemon it uses"
    else
        echo "ok   $1 (kind on $ROOTFUL)"
    fi
}

DEPLOY="$PROJECT_ROOT/scripts/deploy"

# --- backup-keycloak-db.sh (lib/keycloak-db.sh kdb_use_context) ------------------
run "$DEPLOY/backup-keycloak-db.sh" --dry-run
expect "backup-keycloak-db.sh --dry-run" 0 "dry run on context kind-econ-graph" \
    "--context kind-econ-graph get namespace kube-system"
expect_rootful "backup-keycloak-db.sh --dry-run"
grep -qF " config get-contexts -o name" "$LOGS/kind-kubectl.log" ||
    fail "backup-keycloak-db.sh --dry-run did not look up its context with the kind kubectl"

# A real run copies the node's kubectl into the cache, then stops at the dump.
run env FAKE_FAIL=pg_dump "$DEPLOY/backup-keycloak-db.sh"
expect "backup-keycloak-db.sh" nonzero "failed; what was written is in" \
    "--context kind-econ-graph -n econ-graph exec -i keycloak-postgres-0 -c postgres -- pg_dump"
expect_rootful "backup-keycloak-db.sh"
grep -qF "cp -q -L econ-graph-control-plane:/usr/bin/kubectl $CACHE/bin/kubectl.new" "$LOGS/docker.log" ||
    fail "backup-keycloak-db.sh did not refresh the kind kubectl in $CACHE"

# --- deploy.sh, up to its first apply ----------------------------------------------
run env PUBLIC_HOST= "$TREE/scripts/deploy/deploy.sh"
expect "deploy.sh" nonzero "fake kind kubectl: refusing to apply" \
    "--context kind-econ-graph apply -f k8s/manifests/namespace.yaml"
expect_rootful "deploy.sh"
grep -qF " config use-context kind-econ-graph" "$LOGS/kind-kubectl.log" ||
    fail "deploy.sh did not switch the kind kubectl to kind-econ-graph"

# The operator's shell exports DOCKER_HOST for the rootless daemon: kind and
# docker still run on the rootful one.
run env PUBLIC_HOST= DOCKER_HOST=unix:///run/user/1000/docker.sock "$TREE/scripts/deploy/deploy.sh"
expect "deploy.sh with a rootless DOCKER_HOST" nonzero "fake kind kubectl: refusing to apply" \
    "--context kind-econ-graph apply -f k8s/manifests/namespace.yaml"
expect_rootful "deploy.sh with a rootless DOCKER_HOST"
run env DOCKER_HOST=unix:///run/user/1000/docker.sock "$DEPLOY/backup-keycloak-db.sh" --dry-run
expect "backup-keycloak-db.sh --dry-run with a rootless DOCKER_HOST" 0 "dry run on context kind-econ-graph" \
    "--context kind-econ-graph get namespace kube-system"
expect_rootful "backup-keycloak-db.sh --dry-run with a rootless DOCKER_HOST"

# --- cutover-to-gateway.sh -----------------------------------------------------------
run env FAKE_FAIL="get namespace econ-graph" "$DEPLOY/cutover-to-gateway.sh" --dry-run
expect "cutover-to-gateway.sh --dry-run" nonzero "namespace econ-graph not found" \
    "--context kind-econ-graph version --request-timeout=15s"

# --- install-gateway.sh: stops when the server version can't be read ----------------
run "$DEPLOY/install-gateway.sh" --dry-run
expect "install-gateway.sh --dry-run" nonzero "cannot reach the Kubernetes API server" "version -o json"
grep -qF "Using the kind cluster's kubectl" <<<"$out" ||
    fail "install-gateway.sh did not say which kubectl it uses"

# --- apply-gateway-routes.sh -----------------------------------------------------------
run env PUBLIC_HOST=none "$DEPLOY/apply-gateway-routes.sh" --dry-run
expect "apply-gateway-routes.sh --dry-run" 0 "No public host: localhost routes only" \
    "get crd httproutes.gateway.networking.k8s.io"

# --- setup-port-forwarding.sh: no kind cluster, no fallback to plain kubectl ---------
run env FAKE_CLUSTERS= "$DEPLOY/setup-port-forwarding.sh"
if [ "$rc" -eq 0 ]; then
    fail "setup-port-forwarding.sh without the kind cluster exits 0"
elif [ -s "$LOGS/plain.log" ]; then
    fail "setup-port-forwarding.sh without the kind cluster called plain kubectl: $(cat "$LOGS/plain.log")"
elif ! grep -qF "cannot reach the API server of kind cluster 'econ-graph' on $ROOTFUL" <<<"$out"; then
    fail "setup-port-forwarding.sh without the kind cluster: no reason given"
else
    echo "ok   setup-port-forwarding.sh stops without the kind cluster"
fi
expect_rootful "setup-port-forwarding.sh"
# An explicit KUBECTL is used as it is (here the same fake), without kind or docker.
run env KUBECTL="$CACHE/bin/kubectl" KUBECONFIG="$CACHE/kubeconfig-econ-graph" FAKE_FAIL=cluster-info \
    "$DEPLOY/setup-port-forwarding.sh"
expect "setup-port-forwarding.sh with KUBECTL" nonzero "Cannot connect to Kubernetes cluster" "cluster-info"
if [ -s "$LOGS/pinned-kind.log" ] || [ -s "$LOGS/docker.log" ]; then
    fail "setup-port-forwarding.sh with KUBECTL: still ran kind or docker"
fi
# Past the cluster check, an explicit KUBECTL keeps its current context: no switch to
# kind-econ-graph. Stopped at the service check, before any port forward (or pkill).
run env KUBECTL="$CACHE/bin/kubectl" KUBECONFIG="$CACHE/kubeconfig-econ-graph" \
    FAKE_FAIL="get service grafana-service" "$DEPLOY/setup-port-forwarding.sh"
expect "setup-port-forwarding.sh with KUBECTL, past the cluster check" nonzero \
    "Grafana service not found" "get service grafana-service"
if grep -qF " config use-context" "$LOGS/kind-kubectl.log"; then
    fail "setup-port-forwarding.sh with KUBECTL: switched the context of its kubeconfig"
fi

# --- the release-image scripts' pin (ri_pin_context, lib/release-images.sh) ------------
# test-release-images.sh names its stub kubectl with KUBECTL; this is the path without
# it, as verify-release-images.sh takes it (cluster-role.sh, then release-images.sh): the
# kind kubectl and kubeconfig, on kind-<cluster>.
run bash -c 'RI_SCRIPT=test; source "$1/lib/cluster-role.sh"; source "$1/lib/release-images.sh"; ri_pin_context econ-graph' \
    _ "$DEPLOY"
if [ "$rc" -ne 0 ] || [ -s "$LOGS/plain.log" ] || [ ! -s "$LOGS/kind-kubectl.log" ] ||
    grep -vqF "$CACHE/bin/kubectl KUBECONFIG=$CACHE/kubeconfig-econ-graph --context kind-econ-graph " "$LOGS/kind-kubectl.log"; then
    fail "ri_pin_context without KUBECTL: exit $rc, $(cat "$LOGS/plain.log" "$LOGS/kind-kubectl.log")"
else
    echo "ok   ri_pin_context without KUBECTL uses the kind kubectl on kind-econ-graph"
fi
run env HOME="$WORK/empty-home" bash -c \
    'RI_SCRIPT=test; source "$1/lib/cluster-role.sh"; source "$1/lib/release-images.sh"; ri_pin_context econ-graph' _ "$DEPLOY"
if [ "$rc" -eq 0 ] || [ -s "$LOGS/plain.log" ] || ! grep -qF "or set KUBECTL (and KUBECONFIG)" <<<"$out"; then
    fail "ri_pin_context without KUBECTL or the kind kubectl: exit $rc, $(cat "$LOGS/plain.log")"
else
    echo "ok   ri_pin_context without KUBECTL or the kind kubectl stops"
fi

# --- cluster_role_default_kubectl: never plain kubectl ---------------------------------
# No kind copies (a HOME without the cache): the gateway and Postgres backup scripts stop
# before any kubectl.
for script in install-gateway.sh apply-gateway-routes.sh cutover-to-gateway.sh backup-postgres-now.sh; do
    run env HOME="$WORK/empty-home" "$DEPLOY/$script" --dry-run
    if [ "$rc" -eq 0 ] || [ -s "$LOGS/plain.log" ] || [ -s "$LOGS/kind-kubectl.log" ] ||
        ! grep -qF "or set KUBECTL (and KUBECONFIG)" <<<"$out"; then
        fail "$script without the kind kubectl: exit $rc, plain $(cat "$LOGS/plain.log")"
    else
        echo "ok   $script without the kind kubectl stops"
    fi
done
# A CLUSTER_ROLE_CONTEXT left in the environment, without KUBECTL, doesn't choose the
# kubectl: the kind one is still used (here, past it, the gateway scripts reach the
# fake), and without the kind copies they still stop.
for script in install-gateway.sh apply-gateway-routes.sh; do
    run env CLUSTER_ROLE_CONTEXT=x PUBLIC_HOST=none "$DEPLOY/$script" --dry-run
    if [ -s "$LOGS/plain.log" ] || [ ! -s "$LOGS/kind-kubectl.log" ] ||
        grep -vqF "$CACHE/bin/kubectl KUBECONFIG=$CACHE/kubeconfig-econ-graph " "$LOGS/kind-kubectl.log"; then
        fail "$script with only CLUSTER_ROLE_CONTEXT: not the kind kubectl: $(cat "$LOGS/plain.log" "$LOGS/kind-kubectl.log")"
    else
        echo "ok   $script with only CLUSTER_ROLE_CONTEXT uses the kind kubectl"
    fi
    run env HOME="$WORK/empty-home" CLUSTER_ROLE_CONTEXT=x "$DEPLOY/$script" --dry-run
    if [ "$rc" -eq 0 ] || [ -s "$LOGS/plain.log" ] || ! grep -qF "or set KUBECTL (and KUBECONFIG)" <<<"$out"; then
        fail "$script with only CLUSTER_ROLE_CONTEXT and no kind kubectl: exit $rc, plain $(cat "$LOGS/plain.log")"
    else
        echo "ok   $script with only CLUSTER_ROLE_CONTEXT and no kind kubectl stops"
    fi
done
# KUBECONFIG alone: the kind kubectl binary with it (plain kubectl may ignore it).
other="$WORK/other-kubeconfig"
echo "# another kubeconfig" >"$other"
run env KUBECONFIG="$other" PUBLIC_HOST=none "$DEPLOY/apply-gateway-routes.sh" --dry-run
if [ "$rc" -ne 0 ] || [ -s "$LOGS/plain.log" ] ||
    grep -vqF "$CACHE/bin/kubectl KUBECONFIG=$other " "$LOGS/kind-kubectl.log" ||
    ! grep -qF "Using the kind cluster's kubectl $CACHE/bin/kubectl with KUBECONFIG=$other" <<<"$out"; then
    fail "apply-gateway-routes.sh with only KUBECONFIG: exit $rc, $(cat "$LOGS/kind-kubectl.log" "$LOGS/plain.log")"
else
    echo "ok   apply-gateway-routes.sh with only KUBECONFIG uses the kind kubectl with it"
fi
run env HOME="$WORK/empty-home" KUBECONFIG="$other" "$DEPLOY/install-gateway.sh" --dry-run
if [ "$rc" -eq 0 ] || [ -s "$LOGS/plain.log" ] || ! grep -qF "Set KUBECTL too" <<<"$out"; then
    fail "install-gateway.sh with only KUBECONFIG and no kind kubectl: exit $rc"
else
    echo "ok   install-gateway.sh with only KUBECONFIG and no kind kubectl stops"
fi

if [ "$failed" -ne 0 ]; then
    exit 1
fi
echo "The deploy scripts reach only the kind cluster's kubectl, and kind on $ROOTFUL."
