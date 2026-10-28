#!/usr/bin/env bash
# The stubs are quoted heredocs and run on their own.
# `A && B || fail` below fails when either check fails, as meant.
# shellcheck disable=SC2016,SC2015
#
# Tests for scripts/deploy/release-deploy.sh against stubs, no cluster, no
# Docker daemon, no registry:
#   - the steps run in order (load, backup, keycloak, precheck, deploy, verify),
#     each with its arguments, the kind cluster's kubectl and kubeconfig, the
#     rootful Docker daemon and PUBLIC_HOST from the deploy config; plain
#     kubectl and kind are never called;
#   - a failing step stops the run: nothing after it runs, the exit status is
#     non-zero, and the output names the step, the state and the rollback
#     command; --report records each step and the result;
#   - the PUBLIC_HOST guard: a production cluster without PUBLIC_HOST
#     configured (no file, or an empty value), a cluster not marked production
#     with one, a marker that can't be read, and an environment PUBLIC_HOST that
#     differs from the config are refused before any step;
#   - --dry-run passes --dry-run to every step it runs and does not run verify;
#   - without a terminal and without --yes it refuses before any step;
#   - a final tag deploys the release candidate tag on its commit.
#
# The step scripts are stubs in a copy of scripts/deploy (a scratch git
# repository, so the final tag resolves); lib/ is the real one. The kind
# kubectl, kubeconfig, kind and docker are stubs in a scratch HOME.
#
# Usage: scripts/deploy/test-release-deploy.sh (from anywhere)

set -uo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
HOME_DIR="$WORK/home"
CACHE="$HOME_DIR/.cache/econgraph"
CONFIG="$HOME_DIR/.config/econgraph/deploy.env"
LOGS="$WORK/logs"
ROOTFUL="unix:///var/run/docker.sock"
TAG="v4.0.0-rc.4"
mkdir -p "$WORK/bin" "$WORK/tmp" "$WORK/source" "$LOGS" "$CACHE/bin" "$(dirname "$CONFIG")"

for tool in kubectl kind; do
    cat >"$WORK/bin/$tool" <<'STUB'
#!/usr/bin/env bash
echo "$(basename "$0") $*" >>"$TEST_LOGS/plain.log"
echo "plain $(basename "$0") from PATH was called: $*" >&2
exit 1
STUB
done

# The kind cluster's kubectl: context kind-econ-graph, a kube-system UID, and
# the production marker as STUB_MARKER says (production, none, fails).
cat >"$WORK/source/kubectl" <<'STUB'
#!/usr/bin/env bash
echo "$0 KUBECONFIG=${KUBECONFIG:-} $*" >>"$TEST_LOGS/kind-kubectl.log"
args=("$@")
if [ "${args[0]:-}" = --context ]; then
    [ "${args[1]}" = kind-econ-graph ] || { echo "error: no context ${args[1]}" >&2; exit 1; }
    args=("${args[@]:2}")
fi
if [ "${args[0]:-}" = -n ]; then args=("${args[@]:2}"); fi
set -- "${args[@]}"
uid=11111111-2222-3333-4444-555555555555
case "$*" in
    "version "*) ;;
    "config get-contexts -o name"*) echo kind-econ-graph ;;
    "config view --minify"*) printf 'https://127.0.0.1:6443' ;;
    "get nodes"*) printf 'econ-graph-control-plane econ-graph-worker' ;;
    "get namespace kube-system"*) printf '%s' "$uid" ;;
    "get configmap econgraph-cluster-role"*)
        case "${STUB_MARKER:-production}" in
            production) printf 'production %s' "$uid" ;;
            fails) exit 1 ;;
        esac ;;
    "get deployment keycloak "*) printf '%s' "${STUB_KEYCLOAK_REPLICAS-1}" ;;
    *) echo "stub kind kubectl: unexpected call: $*" >&2; exit 1 ;;
esac
STUB
echo "# stub kubeconfig" >"$WORK/source/kubeconfig"
cp "$WORK/source/kubectl" "$CACHE/bin/kubectl"
cp "$WORK/source/kubeconfig" "$CACHE/kubeconfig-econ-graph"

cat >"$WORK/bin/docker" <<'STUB'
#!/usr/bin/env bash
echo "docker DOCKER_HOST=${DOCKER_HOST:-} $*" >>"$TEST_LOGS/docker.log"
case "$*" in
    "info --format "*) echo "29.0.0 /var/lib/docker [name=seccomp]" ;;
    "cp -q -L econ-graph-control-plane:/usr/bin/kubectl "*) cp "$TEST_SOURCE/kubectl" "${*: -1}" ;;
    *) echo "stub docker: unexpected call: $*" >&2; exit 1 ;;
esac
STUB
KIND_PINNED="$(HOME="$HOME_DIR" XDG_DATA_HOME="" bash -c \
    "source '$PROJECT_ROOT/scripts/deploy/lib/kind-version.sh'; echo \"\$KIND_PINNED_BIN\"")"
case "$KIND_PINNED" in "$HOME_DIR"/*) ;; *) echo "FAIL pinned kind outside the scratch HOME"; exit 1 ;; esac
mkdir -p "$(dirname "$KIND_PINNED")"
cat >"$KIND_PINNED" <<'STUB'
#!/usr/bin/env bash
echo "kind DOCKER_HOST=${DOCKER_HOST:-} $*" >>"$TEST_LOGS/docker.log"
case "$*" in
    "get clusters") echo econ-graph ;;
    "get kubeconfig --name econ-graph") cat "$TEST_SOURCE/kubeconfig" ;;
    *) echo "stub kind: unexpected call: $*" >&2; exit 1 ;;
esac
STUB
chmod +x "$WORK/bin/"* "$WORK/source/kubectl" "$CACHE/bin/kubectl" "$KIND_PINNED"

# The tree: release-deploy.sh and lib/ as they are, the steps as stubs that
# record their name, arguments and environment, and fail when STUB_FAIL names them.
TREE="$WORK/tree"
mkdir -p "$TREE/scripts/deploy"
cp "$PROJECT_ROOT/scripts/deploy/release-deploy.sh" "$TREE/scripts/deploy/"
cp -r "$PROJECT_ROOT/scripts/deploy/lib" "$TREE/scripts/deploy/lib"
for step in load-release-images backup-postgres-now backup-keycloak-db precheck-db-migration deploy verify-release-images; do
    cat >"$TREE/scripts/deploy/$step.sh" <<'STUB'
#!/usr/bin/env bash
name="$(basename "$0" .sh)"
echo "$name $* | PUBLIC_HOST=${PUBLIC_HOST-<unset>} KUBECTL=${KUBECTL:-} KUBECONFIG=${KUBECONFIG:-} DOCKER_HOST=${DOCKER_HOST:-} PWD=$PWD" >>"$TEST_LOGS/steps.log"
if [ "${STUB_FAIL:-}" = "$name" ]; then
    echo "stub $name: failing as asked" >&2
    exit 1
fi
STUB
    chmod +x "$TREE/scripts/deploy/$step.sh"
done
# A git repository for the final-tag case: v4.0.0 is on the commit of rc.3 and
# rc.4; rc.5 is on a later commit.
(
    cd "$TREE" &&
        git init -q &&
        git -c user.email=t@example.org -c user.name=test commit -q --allow-empty -m one &&
        git tag v4.0.0-rc.3 && git tag v4.0.0-rc.4 && git tag v4.0.0 &&
        git -c user.email=t@example.org -c user.name=test commit -q --allow-empty -m two &&
        git tag v4.0.0-rc.5
) || { echo "FAIL could not set up the scratch git repository"; exit 1; }

RELEASE_DEPLOY="$TREE/scripts/deploy/release-deploy.sh"

# run ARGS...: release-deploy.sh with the stubs and none of KUBECTL, KUBECONFIG,
# DOCKER_HOST and PUBLIC_HOST set (a case sets one with `env X=y` first).
# Output in $out, status in $rc.
run() {
    : >"$LOGS/plain.log"; : >"$LOGS/kind-kubectl.log"; : >"$LOGS/docker.log"; : >"$LOGS/steps.log"
    rm -f "$WORK/report"
    out="$(env -u KUBECTL -u KUBECONFIG -u DOCKER_HOST -u DOCKER_CONTEXT -u ECONGRAPH_DOCKER_HOST \
        -u ECONGRAPH_KUBE_DIR -u CLUSTER_ROLE_CONTEXT -u KIND_CLUSTER -u NAMESPACE -u PUBLIC_HOST \
        -u ECONGRAPH_DEPLOY_ENV -u ECONGRAPH_SCRATCH_STORAGE -u XDG_CACHE_HOME -u XDG_DATA_HOME \
        -u XDG_CONFIG_HOME -u DRY_RUN -u ASSUME_YES -u KIND_FALLBACK_WARNED \
        HOME="$HOME_DIR" TMPDIR="$WORK/tmp" PATH="$WORK/bin:/usr/local/bin:/usr/bin:/bin" \
        ECONGRAPH_BACKUP_ROOT="$WORK/backups" ECONGRAPH_RELEASES_DIR="$WORK/releases" \
        GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 \
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
steps() { cut -d' ' -f1 "$LOGS/steps.log" | tr '\n' ' ' | sed 's/ $//'; }
no_plain() { [ ! -s "$LOGS/plain.log" ] || fail "$1: called plain kubectl or kind: $(cat "$LOGS/plain.log")"; }

ALL="load-release-images backup-postgres-now backup-keycloak-db precheck-db-migration deploy verify-release-images"

# --- a full run on production ------------------------------------------------------------
echo "PUBLIC_HOST=econgraph.com" >"$CONFIG"
run "$RELEASE_DEPLOY" --tag "$TAG" --yes --report "$WORK/report"
if [ "$rc" -ne 0 ]; then
    fail "full run exits $rc"
elif [ "$(steps)" != "$ALL" ]; then
    fail "full run: steps ran as '$(steps)', expected '$ALL'"
else
    echo "ok   full run: the steps in order"
fi
no_plain "full run"
expect_line() { grep -qF -- "$2" "$LOGS/steps.log" || fail "full run: $1: no '$2' in $(cat "$LOGS/steps.log")"; }
expect_line "load arguments" "load-release-images --tag $TAG |"
expect_line "backup arguments" "backup-postgres-now --context kind-econ-graph |"
expect_line "keycloak arguments" "backup-keycloak-db --context kind-econ-graph |"
expect_line "precheck arguments" "precheck-db-migration --stop-crawler |"
expect_line "deploy PUBLIC_HOST" "deploy  | PUBLIC_HOST=econgraph.com "
expect_line "verify arguments" "verify-release-images --tag $TAG |"
bad="$(grep -vF "KUBECTL=$CACHE/bin/kubectl KUBECONFIG=$CACHE/kubeconfig-econ-graph DOCKER_HOST=$ROOTFUL PWD=$TREE" "$LOGS/steps.log" || true)"
[ -z "$bad" ] || fail "full run: a step ran without the kind kubectl, the rootful daemon or the repository root: $bad"
bad="$(grep -vF "DOCKER_HOST=$ROOTFUL " "$LOGS/docker.log" || true)"
[ -z "$bad" ] || fail "full run: kind or docker on another daemon: $bad"
grep -qxF "result=ok" "$WORK/report" && [ "$(grep -c '^step=.* status=ok$' "$WORK/report")" = 6 ] ||
    fail "full run: report is $(cat "$WORK/report" 2>/dev/null)"
grep -qF "is deployed on kind-econ-graph and verified" <<<"$out" || fail "full run: no final message"

# --- stop at the first failure ----------------------------------------------------------------
# The step names release-deploy.sh prints and reports, by script.
declare -A NAME=([load-release-images]=load [backup-postgres-now]=backup [backup-keycloak-db]=keycloak
    [precheck-db-migration]=precheck [deploy]=deploy [verify-release-images]=verify)
# state_of NAME: the state the last run's failure summary lists for step NAME.
state_of() {
    sed -n '/step .* failed; nothing after it ran/,$p' <<<"$out" |
        awk -v s="$1" '$1 == s { $1 = ""; sub(/^ +/, ""); print; exit }'
}
prev=""
for step in $ALL; do
    run env STUB_FAIL="$step" "$RELEASE_DEPLOY" --tag "$TAG" --yes --report "$WORK/report"
    want="${prev:+$prev }$step"
    name="${NAME[$step]}"
    if [ "$rc" -eq 0 ]; then
        fail "failing $step: exit 0"
    elif [ "$(steps)" != "$want" ]; then
        fail "failing $step: steps ran as '$(steps)', expected '$want'"
    elif ! grep -qF "rollback-release-images.sh --to pre-$TAG" <<<"$out"; then
        fail "failing $step: no rollback command"
    elif [ "$(state_of "$name")" != FAILED ]; then
        fail "failing $step: the summary lists $name as '$(state_of "$name")', not FAILED"
    elif [ -n "$prev" ] && [ "$(state_of "${NAME[${prev##* }]}")" != "done" ]; then
        fail "failing $step: the step before it is not listed done"
    elif ! grep -qxF "result=failed step=$name" "$WORK/report"; then
        fail "failing $step: report is $(cat "$WORK/report" 2>/dev/null)"
    else
        echo "ok   failing $step stops there and prints the rollback"
    fi
    prev="$want"
done
run env STUB_FAIL=precheck-db-migration "$RELEASE_DEPLOY" --tag "$TAG" --yes
grep -qF "scale deployment/crawler-worker --replicas=1" <<<"$out" ||
    fail "failing precheck: no command to start the crawler worker again"
run env STUB_FAIL=deploy "$RELEASE_DEPLOY" --tag "$TAG" --yes
grep -qF "restore-keycloak-db.sh" <<<"$out" || fail "failing deploy: no database restore pointer"
# Keycloak: stopped by the keycloak step, applied again by deploy.sh.
for step in backup-keycloak-db precheck-db-migration deploy; do
    run env STUB_FAIL="$step" STUB_KEYCLOAK_REPLICAS=2 "$RELEASE_DEPLOY" --tag "$TAG" --yes
    grep -qF "scale deployment/keycloak --replicas=2" <<<"$out" ||
        fail "failing $step: no command to start Keycloak again at its 2 replicas"
done
for step in backup-postgres-now verify-release-images; do
    run env STUB_FAIL="$step" "$RELEASE_DEPLOY" --tag "$TAG" --yes
    if grep -qF "scale deployment/keycloak" <<<"$out"; then
        fail "failing $step: says to start Keycloak, which that step leaves running"
    fi
done
run env STUB_FAIL=backup-keycloak-db STUB_KEYCLOAK_REPLICAS= "$RELEASE_DEPLOY" --tag "$TAG" --yes
if grep -qF "scale deployment/keycloak" <<<"$out"; then
    fail "failing keycloak without a Keycloak Deployment: says to start it"
fi
echo "ok   the Keycloak restart command after the steps that leave it stopped"
no_plain "failures"

# --- the report -------------------------------------------------------------------------------
mkdir -p "$WORK/readonly" && chmod 0555 "$WORK/readonly"
run "$RELEASE_DEPLOY" --tag "$TAG" --yes --report "$WORK/readonly/report"
if [ "$rc" -eq 0 ] || [ -s "$LOGS/steps.log" ] || ! grep -qF "cannot write the report" <<<"$out"; then
    fail "an unwritable report: exit $rc, steps '$(steps)'"
else
    echo "ok   an unwritable report stops the run before any step"
fi
chmod 0755 "$WORK/readonly"

# --- the PUBLIC_HOST guard --------------------------------------------------------------------
refused() { # NAME MESSAGE: the last run refused before any step
    if [ "$rc" -eq 0 ]; then
        fail "$1: exit 0"
    elif [ -s "$LOGS/steps.log" ]; then
        fail "$1: steps ran: $(steps)"
    elif ! grep -qF -- "$2" <<<"$out"; then
        fail "$1: no '$2' in the output"
    else
        echo "ok   $1 is refused"
    fi
}
rm -f "$CONFIG"
run "$RELEASE_DEPLOY" --tag "$TAG" --yes
refused "production without a deploy config" "no PUBLIC_HOST is configured"
echo "PUBLIC_HOST=" >"$CONFIG"
run "$RELEASE_DEPLOY" --tag "$TAG" --yes
refused "production with an empty PUBLIC_HOST" "no PUBLIC_HOST is configured"
echo "PUBLIC_HOST=econgraph.com" >"$CONFIG"
run env STUB_MARKER=none "$RELEASE_DEPLOY" --tag "$TAG" --yes
refused "a cluster not marked production with PUBLIC_HOST configured" "only the production cluster serves the public host"
run env STUB_MARKER=fails "$RELEASE_DEPLOY" --tag "$TAG" --yes
refused "a marker that can't be read" "treated as not production"
run env PUBLIC_HOST= "$RELEASE_DEPLOY" --tag "$TAG" --yes
refused "an environment PUBLIC_HOST that differs from the config" "the config decides"
run env PUBLIC_HOST=econgraph.com "$RELEASE_DEPLOY" --tag "$TAG" --yes
[ "$rc" -eq 0 ] && [ "$(steps)" = "$ALL" ] || fail "an environment PUBLIC_HOST equal to the config: exit $rc, steps '$(steps)'"
rm -f "$CONFIG"
run env STUB_MARKER=none "$RELEASE_DEPLOY" --tag "$TAG" --yes
if [ "$rc" -eq 0 ] && grep -qF "deploy  | PUBLIC_HOST= " "$LOGS/steps.log"; then
    echo "ok   a test cluster without a deploy config deploys with PUBLIC_HOST empty"
else
    fail "a test cluster without a deploy config: exit $rc, $(cat "$LOGS/steps.log")"
fi
echo "PUBLIC_HOST=econgraph.com" >"$CONFIG"

# --- confirmation -------------------------------------------------------------------------------
run "$RELEASE_DEPLOY" --tag "$TAG"
refused "a run without a terminal or --yes" "needs confirmation"

# --- dry run --------------------------------------------------------------------------------------
run "$RELEASE_DEPLOY" --tag "$TAG" --dry-run --report "$WORK/report"
want="load-release-images backup-postgres-now backup-keycloak-db precheck-db-migration deploy"
if [ "$rc" -ne 0 ]; then
    fail "dry run exits $rc"
elif [ "$(steps)" != "$want" ]; then
    fail "dry run: steps ran as '$(steps)', expected '$want'"
elif grep -v -- "--dry-run |" "$LOGS/steps.log"; then
    fail "dry run: a step ran without --dry-run"
elif ! grep -qF "it would run:" <<<"$out" || ! grep -qxF "step=verify status=dry-run" "$WORK/report"; then
    fail "dry run: verify is not explained or reported"
elif grep -qF "cp -q -L" "$LOGS/docker.log" || [ -n "$(ls -A "$WORK/tmp")" ]; then
    fail "dry run: refreshed the kubectl cache or left files in TMPDIR"
else
    echo "ok   dry run: every step dry-runs, verify is described"
fi
run env STUB_FAIL=backup-keycloak-db "$RELEASE_DEPLOY" --tag "$TAG" --dry-run
[ "$rc" -ne 0 ] && grep -qF "the dry run of step keycloak failed; nothing was changed" <<<"$out" ||
    fail "a failing dry-run step: exit $rc"
no_plain "dry run"

# --- tags ---------------------------------------------------------------------------------------------
run "$RELEASE_DEPLOY" --tag v4.0.0 --yes
if [ "$rc" -eq 0 ] && grep -qF "load-release-images --tag v4.0.0-rc.4 |" "$LOGS/steps.log" &&
    grep -qF "v4.0.0 is v4.0.0-rc.4" <<<"$out"; then
    echo "ok   a final tag deploys the highest release candidate on its commit"
else
    fail "final tag: exit $rc, steps $(cat "$LOGS/steps.log")"
fi
run "$RELEASE_DEPLOY" --tag v4.0 --yes
[ "$rc" -eq 2 ] && [ ! -s "$LOGS/steps.log" ] || fail "a malformed tag: exit $rc"
run "$RELEASE_DEPLOY" --yes
[ "$rc" -eq 2 ] || fail "no --tag: exit $rc"

if [ "$failed" -ne 0 ]; then
    exit 1
fi
echo "release-deploy.sh: all checks passed"
