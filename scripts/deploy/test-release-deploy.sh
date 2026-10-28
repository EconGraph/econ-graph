#!/usr/bin/env bash
# The stubs are quoted heredocs and run on their own.
# `A && B || fail` below fails when either check fails, as meant.
# shellcheck disable=SC2016,SC2015
#
# Tests for scripts/deploy/release-deploy.sh against stubs, no cluster, no
# Docker daemon, no registry:
#   - the steps run in order (backup, load, keycloak, precheck, deploy, verify),
#     each with its arguments, the kind cluster's kubectl and kubeconfig, the
#     rootful Docker daemon and PUBLIC_HOST from the deploy config; plain
#     kubectl and kind are never called;
#   - a failing step stops the run: nothing after it runs, the exit status is
#     non-zero, and the output names the step, the state and the rollback
#     command (with --cluster for another cluster); --report records each
#     step, the backups and the result; a step that leaves a background process
#     holding its output still fails promptly, with its guidance;
#   - the guidance after a failure: Keycloak's and the crawler worker's
#     replicas (from this run, an earlier run's record, or the manifest), the
#     backups to restore from (the tag's first run's), and a backup Job still
#     running;
#   - before any step: the checkout must be the tag's commit and clean; the
#     PUBLIC_HOST guard (a production cluster without PUBLIC_HOST configured,
#     one that serves another host than the configured one, a cluster not
#     marked production with one, a marker that can't be read, an environment
#     PUBLIC_HOST that differs from the config, the last config line winning,
#     and uppercase lowercased); --yes alone is not enough on a production
#     cluster (--confirm-production);
#   - --dry-run passes --dry-run to every step it runs and does not run verify;
#   - without a terminal and without --yes it refuses before any step;
#   - a final tag deploys the release candidate tag on its commit.
#
# The step scripts are stubs in a copy of scripts/deploy (a scratch git
# repository, checked out at the release candidate's tag, so the checkout check
# and the final tag resolve); lib/ is the real one. The kind kubectl,
# kubeconfig, kind and docker are stubs in a scratch HOME.
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
UID_PREFIX="11111111"
# What a production run needs instead of a typed answer.
YES=(--yes --confirm-production "$UID_PREFIX")
mkdir -p "$WORK/bin" "$WORK/tmp" "$WORK/run" "$WORK/source" "$LOGS" "$CACHE/bin" "$(dirname "$CONFIG")"

for tool in kubectl kind; do
    cat >"$WORK/bin/$tool" <<'STUB'
#!/usr/bin/env bash
echo "$(basename "$0") $*" >>"$TEST_LOGS/plain.log"
echo "plain $(basename "$0") from PATH was called: $*" >&2
exit 1
STUB
done

# The kind cluster's kubectl: context kind-econ-graph, a kube-system UID, the
# production marker as STUB_MARKER says (production, none, fails), and the
# public host the cluster serves as STUB_SERVED_HOST says (default
# econgraph.com; "fail" fails).
cat >"$WORK/source/kubectl" <<'STUB'
#!/usr/bin/env bash
echo "$0 KUBECONFIG=${KUBECONFIG:-} $*" >>"$TEST_LOGS/kind-kubectl.log"
args=("$@")
if [ "${args[0]:-}" = --context ]; then
    case "${args[1]}" in kind-econ-graph | kind-kg-test) ;; *) echo "error: no context ${args[1]}" >&2; exit 1 ;; esac
    args=("${args[@]:2}")
fi
if [ "${args[0]:-}" = -n ]; then args=("${args[@]:2}"); fi
set -- "${args[@]}"
uid=11111111-2222-3333-4444-555555555555
case "$*" in
    "version "*) ;;
    "config get-contexts -o name"*) printf 'kind-econ-graph\nkind-kg-test\n' ;;
    "config view --minify"*) printf 'https://127.0.0.1:6443' ;;
    "get nodes"*) printf 'econ-graph-control-plane econ-graph-worker' ;;
    "get namespace kube-system"*) printf '%s' "$uid" ;;
    "get namespace econ-graph --ignore-not-found -o name") echo namespace/econ-graph ;;
    "get crd httproutes.gateway.networking.k8s.io"*) ;;
    "get httproute -l econgraph.io/public-host=true "*)
        [ "${STUB_SERVED_HOST-}" != fail ] || exit 1
        printf '%s\n' "${STUB_SERVED_HOST-econgraph.com}" ;;
    "get ingress "*) ;;
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
    "cp -q -L "*"-control-plane:/usr/bin/kubectl "*) cp "$TEST_SOURCE/kubectl" "${*: -1}" ;;
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
    "get clusters") printf 'econ-graph\nkg-test\n' ;;
    "get kubeconfig --name "*) cat "$TEST_SOURCE/kubeconfig" ;;
    *) echo "stub kind: unexpected call: $*" >&2; exit 1 ;;
esac
STUB
chmod +x "$WORK/bin/"* "$WORK/source/kubectl" "$CACHE/bin/kubectl" "$KIND_PINNED"

# The tree: release-deploy.sh, render-public-host.py, lib/ and the two
# manifests it reads replicas from as they are, the steps as stubs that record
# their name, arguments and environment, print what the real ones print that
# release-deploy.sh reads, and fail when STUB_FAIL names them.
TREE="$WORK/tree"
mkdir -p "$TREE/scripts/deploy" "$TREE/k8s/manifests/keycloak"
cp "$PROJECT_ROOT/scripts/deploy/release-deploy.sh" "$PROJECT_ROOT/scripts/deploy/render-public-host.py" "$TREE/scripts/deploy/"
cp -r "$PROJECT_ROOT/scripts/deploy/lib" "$TREE/scripts/deploy/lib"
cp "$PROJECT_ROOT/k8s/manifests/crawler-worker.yaml" "$TREE/k8s/manifests/"
cp "$PROJECT_ROOT/k8s/manifests/keycloak/deployment.yaml" "$TREE/k8s/manifests/keycloak/"
for step in load-release-images backup-postgres-now backup-keycloak-db precheck-db-migration deploy verify-release-images; do
    cat >"$TREE/scripts/deploy/$step.sh" <<'STUB'
#!/usr/bin/env bash
name="$(basename "$0" .sh)"
echo "$name $* | PUBLIC_HOST=${PUBLIC_HOST-<unset>} KIND_CLUSTER=${KIND_CLUSTER:-} KUBECTL=${KUBECTL:-} KUBECONFIG=${KUBECONFIG:-} DOCKER_HOST=${DOCKER_HOST:-} PWD=$PWD" >>"$TEST_LOGS/steps.log"
stamp="${STUB_STAMP:-20261028T080000Z}"
case "$name" in
    backup-postgres-now)
        if [ -n "${STUB_BACKUP_RUNNING:-}" ]; then
            echo "backup-postgres-now: still running: job/postgres-backup-manual-1"
            exit 1
        fi
        # A step that makes the next steps' tee fail to write the step log.
        if [ -n "${STUB_BREAK_LOG:-}" ]; then chmod 000 "$XDG_RUNTIME_DIR"/release-deploy.*; fi
        [ "${STUB_FAIL:-}" = "$name" ] ||
            for db in econ_graph econ_graph_app; do
                echo "[pod/postgres-backup-manual-1-x/pg-dump] [${db}] Backup OK: 3 tables, 1.0M in /backups/${db}-${stamp}.dump"
            done ;;
    backup-keycloak-db)
        [ "${STUB_FAIL:-}" = "$name" ] ||
            echo "backup-keycloak-db: verified dump in /home/op/econgraph-backups/keycloak/${stamp}" ;;
    precheck-db-migration)
        [ -z "${STUB_CRAWLER_FROM-3}" ] ||
            echo "precheck-db-migration: scaling deployment/crawler-worker from ${STUB_CRAWLER_FROM-3} to 0" ;;
    deploy)
        # deploy.sh's pod monitor, left running: it keeps the step's output open.
        if [ -n "${STUB_LINGER:-}" ]; then
            sleep 300 &
            echo $! >"$TEST_LOGS/linger.pid"
            echo "stub deploy: failing with a background process left running" >&2
            exit 1
        fi ;;
esac
if [ "${STUB_FAIL:-}" = "$name" ]; then
    echo "stub $name: failing as asked" >&2
    exit 1
fi
# verify-release-images.sh's summary of what it could not check.
if [ "$name" = verify-release-images ] && [ -n "${STUB_UNCHECKED:-}" ]; then
    echo "Not checked:"
    echo "  unchecked job/crawler-migrate: no Job (deploy.sh skips it without a data database, and Kubernetes deletes it a day after it finishes)"
fi
STUB
    chmod +x "$TREE/scripts/deploy/$step.sh"
done
# A git repository with the tree committed: v4.0.0 is on the commit of rc.3 and
# rc.4, which is checked out; rc.5 is on a later commit.
(
    cd "$TREE" &&
        git init -q &&
        git add -A &&
        git -c user.email=t@example.org -c user.name=test commit -q -m one &&
        git tag v4.0.0-rc.3 && git tag v4.0.0-rc.4 && git tag v4.0.0 &&
        git -c user.email=t@example.org -c user.name=test commit -q --allow-empty -m two &&
        git tag v4.0.0-rc.5 &&
        git checkout -q v4.0.0-rc.4
) || { echo "FAIL could not set up the scratch git repository"; exit 1; }

RELEASE_DEPLOY="$TREE/scripts/deploy/release-deploy.sh"

# The environment of every run: the stubs and none of KUBECTL, KUBECONFIG,
# DOCKER_HOST and PUBLIC_HOST set (a case sets one with `env X=y` first).
ENV=(env -u KUBECTL -u KUBECONFIG -u DOCKER_HOST -u DOCKER_CONTEXT -u ECONGRAPH_DOCKER_HOST
    -u ECONGRAPH_KUBE_DIR -u CLUSTER_ROLE_CONTEXT -u KIND_CLUSTER -u NAMESPACE -u PUBLIC_HOST
    -u ECONGRAPH_DEPLOY_ENV -u ECONGRAPH_SCRATCH_STORAGE -u XDG_CACHE_HOME -u XDG_DATA_HOME
    -u XDG_CONFIG_HOME -u XDG_STATE_HOME -u DRY_RUN -u ASSUME_YES -u KIND_FALLBACK_WARNED
    HOME="$HOME_DIR" TMPDIR="$WORK/tmp" XDG_RUNTIME_DIR="$WORK/run" PATH="$WORK/bin:/usr/local/bin:/usr/bin:/bin"
    ECONGRAPH_BACKUP_ROOT="$WORK/backups" ECONGRAPH_RELEASES_DIR="$WORK/releases"
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
    TEST_LOGS="$LOGS" TEST_SOURCE="$WORK/source")

# run ARGS...: release-deploy.sh with the stubs. Output in $out, status in $rc.
# Each run starts without the records of earlier runs unless KEEP_RECORDS=1.
run() {
    : >"$LOGS/plain.log"; : >"$LOGS/kind-kubectl.log"; : >"$LOGS/docker.log"; : >"$LOGS/steps.log"
    rm -f "$WORK/report"
    [ "${KEEP_RECORDS:-}" = 1 ] || rm -rf "$WORK/releases"
    out="$("${ENV[@]}" "$@" </dev/null 2>&1)"
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
has() { grep -qF -- "$1" <<<"$out"; }

ALL="backup-postgres-now load-release-images backup-keycloak-db precheck-db-migration deploy verify-release-images"

# --- a full run on production ------------------------------------------------------------
echo "PUBLIC_HOST=econgraph.com" >"$CONFIG"
run "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}" --report "$WORK/report"
if [ "$rc" -ne 0 ]; then
    fail "full run exits $rc"
elif [ "$(steps)" != "$ALL" ]; then
    fail "full run: steps ran as '$(steps)', expected '$ALL'"
else
    echo "ok   full run: the steps in order"
fi
no_plain "full run"
expect_line() { grep -qF -- "$2" "$LOGS/steps.log" || fail "full run: $1: no '$2' in $(cat "$LOGS/steps.log")"; }
expect_line "load arguments" "load-release-images --tag $TAG --cluster econ-graph |"
expect_line "backup arguments" "backup-postgres-now --context kind-econ-graph |"
expect_line "keycloak arguments" "backup-keycloak-db --context kind-econ-graph |"
expect_line "precheck arguments" "precheck-db-migration --stop-crawler |"
expect_line "deploy PUBLIC_HOST" "deploy  | PUBLIC_HOST=econgraph.com "
expect_line "verify arguments" "verify-release-images --tag $TAG --cluster econ-graph |"
expect_line "deploy KIND_CLUSTER" "deploy  | PUBLIC_HOST=econgraph.com KIND_CLUSTER=econ-graph "
bad="$(grep -vF "KIND_CLUSTER=econ-graph KUBECTL=$CACHE/bin/kubectl KUBECONFIG=$CACHE/kubeconfig-econ-graph DOCKER_HOST=$ROOTFUL PWD=$TREE" "$LOGS/steps.log" || true)"
[ -z "$bad" ] || fail "full run: a step ran without the kind kubectl, the rootful daemon or the repository root: $bad"
bad="$(grep -vF "DOCKER_HOST=$ROOTFUL " "$LOGS/docker.log" || true)"
[ -z "$bad" ] || fail "full run: kind or docker on another daemon: $bad"
grep -qxF "result=ok" "$WORK/report" && [ "$(grep -c '^step=.* status=ok$' "$WORK/report")" = 6 ] ||
    fail "full run: report is $(cat "$WORK/report" 2>/dev/null)"
grep -qxF "backup=/backups/econ_graph-20261028T080000Z.dump" "$WORK/report" &&
    grep -qxF "keycloak_backup=/home/op/econgraph-backups/keycloak/20261028T080000Z" "$WORK/report" ||
    fail "full run: the report does not record the backups: $(cat "$WORK/report")"
grep -qF "is deployed on kind-econ-graph and verified" <<<"$out" || fail "full run: no final message"
[ -z "$(ls -A "$WORK/run")" ] || fail "full run: left a step log in XDG_RUNTIME_DIR: $(ls -A "$WORK/run")"

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
    run env STUB_FAIL="$step" "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}" --report "$WORK/report"
    want="${prev:+$prev }$step"
    name="${NAME[$step]}"
    if [ "$rc" -eq 0 ]; then
        fail "failing $step: exit 0"
    elif [ "$(steps)" != "$want" ]; then
        fail "failing $step: steps ran as '$(steps)', expected '$want'"
    elif ! has "rollback-release-images.sh --to pre-$TAG --dry-run"; then
        fail "failing $step: no rollback command (without --cluster on the default cluster)"
    elif [ "$(state_of "$name")" != FAILED ]; then
        fail "failing $step: the summary lists $name as '$(state_of "$name")', not FAILED"
    elif [ -n "$prev" ] && [ "$(state_of "${NAME[${prev##* }]}")" != "done" ]; then
        fail "failing $step: the step before it is not listed done"
    elif [ "$(tail -1 "$WORK/report")" != "result=failed step=$name" ]; then
        fail "failing $step: report is $(cat "$WORK/report" 2>/dev/null)"
    else
        echo "ok   failing $step stops there and prints the rollback"
    fi
    prev="$want"
done

# A step that leaves a background process holding its output (deploy.sh's pod
# monitor) still fails promptly, with its guidance and report. The output goes
# to a file: the background process keeps any pipe open.
rm -rf "$WORK/releases"; : >"$LOGS/steps.log"; rm -f "$WORK/report" "$LOGS/linger.pid"
start=$SECONDS
timeout 60 "${ENV[@]}" STUB_LINGER=1 "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}" --report "$WORK/report" \
    </dev/null >"$WORK/linger.out" 2>&1
rc=$?
elapsed=$((SECONDS - start))
out="$(cat "$WORK/linger.out")"
if [ "$rc" -ne 1 ]; then
    fail "a deploy that leaves a background process: exit $rc after ${elapsed}s"
elif [ "$elapsed" -ge 15 ]; then
    fail "a deploy that leaves a background process: took ${elapsed}s"
elif ! has "step deploy failed; nothing after it ran" || ! has "rollback-release-images.sh --to pre-$TAG"; then
    fail "a deploy that leaves a background process: no failure guidance"
elif [ "$(tail -1 "$WORK/report")" != "result=failed step=deploy" ]; then
    fail "a deploy that leaves a background process: report is $(cat "$WORK/report")"
else
    echo "ok   a deploy that leaves a background process fails within ${elapsed}s, with its guidance"
fi
[ ! -s "$LOGS/linger.pid" ] || kill "$(cat "$LOGS/linger.pid")" 2>/dev/null

# A step log that can't be written (tee fails) is not a failed step.
run env STUB_BREAK_LOG=1 "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}" --report "$WORK/report"
if [ "$rc" -eq 0 ] && [ "$(steps)" = "$ALL" ]; then
    echo "ok   a step log that can't be written does not fail the steps"
else
    fail "a step log that can't be written: exit $rc, steps '$(steps)'"
fi
chmod -R u+rwX "$WORK/run" && rm -rf "${WORK:?}/run"/*

# --- the guidance after a failure ---------------------------------------------------------------
# The crawler worker: at the replicas the precheck scaled it down from, after a
# failed precheck or deploy, and only after the rollback.
for step in precheck-db-migration deploy; do
    run env STUB_FAIL="$step" "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
    if ! has "scale deployment/crawler-worker --replicas=3"; then
        fail "failing $step: no command to start the crawler worker again at the 3 replicas the precheck scaled it from"
    elif ! has "Any failure before that leaves it at 0" || ! has "start it only after"; then
        fail "failing $step: the crawler guidance does not say when it stays stopped or when to start it"
    else
        echo "ok   failing $step: the crawler worker's restart at the precheck's replicas"
    fi
done
# The precheck of a re-run finds it stopped: the first run's record, else the manifest.
run env STUB_FAIL=deploy STUB_CRAWLER_FROM=5 "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
KEEP_RECORDS=1 run env STUB_FAIL=deploy STUB_CRAWLER_FROM= "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
has "scale deployment/crawler-worker --replicas=5" ||
    fail "a re-run after the crawler was stopped: not the replicas the first run recorded"
run env STUB_FAIL=deploy STUB_CRAWLER_FROM= "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
has "scale deployment/crawler-worker --replicas=1" && has "crawler-worker.yaml's replicas" ||
    fail "a run whose precheck did not scale the crawler, with no record: not the manifest's replicas"
echo "ok   the crawler worker's replicas from an earlier run's record, else the manifest"
for step in backup-postgres-now load-release-images backup-keycloak-db verify-release-images; do
    run env STUB_FAIL="$step" "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
    if has "scale deployment/crawler-worker"; then
        fail "failing $step: says to start the crawler worker, which that step leaves running"
    fi
done

# Keycloak: stopped by the keycloak step, applied again by deploy.sh.
for step in backup-keycloak-db precheck-db-migration deploy; do
    run env STUB_FAIL="$step" STUB_KEYCLOAK_REPLICAS=2 "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
    has "scale deployment/keycloak --replicas=2" ||
        fail "failing $step: no command to start Keycloak again at its 2 replicas"
done
for step in backup-postgres-now load-release-images verify-release-images; do
    run env STUB_FAIL="$step" "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
    if has "scale deployment/keycloak"; then
        fail "failing $step: says to start Keycloak, which that step leaves running"
    fi
done
run env STUB_FAIL=backup-keycloak-db STUB_KEYCLOAK_REPLICAS= "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
if has "scale deployment/keycloak"; then
    fail "failing keycloak without a Keycloak Deployment: says to start it"
fi
# A re-run finds Keycloak at 0 (the first run stopped it): the first run's
# replicas, else the manifest's.
run env STUB_FAIL=precheck-db-migration STUB_KEYCLOAK_REPLICAS=2 "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
KEEP_RECORDS=1 run env STUB_FAIL=deploy STUB_KEYCLOAK_REPLICAS=0 "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
has "scale deployment/keycloak --replicas=2" ||
    fail "a re-run with Keycloak at 0: not the replicas the first run recorded"
run env STUB_FAIL=deploy STUB_KEYCLOAK_REPLICAS=0 "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
has "scale deployment/keycloak --replicas=1" && has "keycloak/deployment.yaml's replicas" ||
    fail "a run with Keycloak at 0 and no record: not the manifest's replicas"
echo "ok   the Keycloak restart command after the steps that leave it stopped, across re-runs"
no_plain "failures"

# The restores: this run's backups, by path; on a re-run, the first run's,
# which hold the state before the tag's migrations.
run env STUB_FAIL=deploy STUB_STAMP=20261028T010000Z "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}" --report "$WORK/report"
if ! has "restore-postgres-backup.sh --context kind-econ-graph --file econ_graph-20261028T010000Z.dump" ||
    ! has "/backups/econ_graph_app-20261028T010000Z.dump" ||
    ! has "restore-keycloak-db.sh /home/op/econgraph-backups/keycloak/20261028T010000Z --context kind-econ-graph"; then
    fail "failing deploy: the restore commands do not name this run's backups"
elif ! grep -qxF "restore_backup=/backups/econ_graph-20261028T010000Z.dump" "$WORK/report" ||
    ! grep -qxF "restore_keycloak_backup=/home/op/econgraph-backups/keycloak/20261028T010000Z" "$WORK/report"; then
    fail "failing deploy: the report does not record the backups to restore from: $(cat "$WORK/report")"
else
    echo "ok   failing deploy: the restores name this run's backups"
fi
KEEP_RECORDS=1 run env STUB_FAIL=deploy STUB_STAMP=20261028T020000Z "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}" --report "$WORK/report"
restore="$(sed -n '/restore them from the backups/,/use the ones above, not these/p' <<<"$out")"
if ! grep -qF "restore-postgres-backup.sh --context kind-econ-graph --file econ_graph-20261028T010000Z.dump" <<<"$restore" ||
    ! grep -qF "restore-keycloak-db.sh /home/op/econgraph-backups/keycloak/20261028T010000Z" <<<"$restore"; then
    fail "a re-run that fails at deploy: the restores do not name the first run's backups"
elif grep -qF "20261028T020000Z" <<<"$restore"; then
    fail "a re-run that fails at deploy: offers this run's backups as the ones to restore"
elif ! sed -n '/use the ones above, not these/,$p' <<<"$out" | grep -qF "/backups/econ_graph-20261028T020000Z.dump"; then
    fail "a re-run that fails at deploy: does not list this run's backups as not to use"
elif ! grep -qxF "restore_backup=/backups/econ_graph-20261028T010000Z.dump" "$WORK/report"; then
    fail "a re-run that fails at deploy: report $(cat "$WORK/report")"
else
    echo "ok   a re-run that fails at deploy: restore from the first run's backups, not this run's"
fi

# A backup Job still running at the timeout: wait for it, no second backup.
run env STUB_BACKUP_RUNNING=1 "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
if ! has "wait --for=condition=complete job/postgres-backup-manual-1" || ! has "only then run this command again"; then
    fail "a backup still running: does not say to wait for it"
elif has "Fix the backup and run this command again"; then
    fail "a backup still running: suggests running again now (a second backup)"
else
    echo "ok   a backup still running: wait for it, then run again"
fi

# --- the report -------------------------------------------------------------------------------
mkdir -p "$WORK/readonly" && chmod 0555 "$WORK/readonly"
run "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}" --report "$WORK/readonly/report"
if [ "$rc" -eq 0 ] || [ -s "$LOGS/steps.log" ] || ! has "cannot write the report"; then
    fail "an unwritable report: exit $rc, steps '$(steps)'"
else
    echo "ok   an unwritable report stops the run before any step"
fi
# A step log that can't be created stops the run before any step.
run env XDG_RUNTIME_DIR="$WORK/readonly" "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
if [ "$rc" -eq 0 ] || [ -s "$LOGS/steps.log" ] || ! has "cannot create a step log"; then
    fail "an unwritable step log directory: exit $rc, steps '$(steps)'"
else
    echo "ok   a step log that can't be created stops the run before any step"
fi
chmod 0755 "$WORK/readonly"

# --- the checkout -------------------------------------------------------------------------------
refused() { # NAME MESSAGE: the last run refused before any step
    if [ "$rc" -eq 0 ]; then
        fail "$1: exit 0"
    elif [ -s "$LOGS/steps.log" ]; then
        fail "$1: steps ran: $(steps)"
    elif ! has "$2"; then
        fail "$1: no '$2' in the output"
    else
        echo "ok   $1 is refused"
    fi
}
git -C "$TREE" checkout -q v4.0.0-rc.5
run "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
refused "a checkout at another commit than the tag" "not ${TAG}"
git -C "$TREE" checkout -q "$TAG"
echo "# local edit" >>"$TREE/scripts/deploy/lib/kind-env.sh"
run "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
refused "a checkout with a changed file" "has changes or untracked files"
git -C "$TREE" checkout -q -- scripts/deploy/lib/kind-env.sh
echo "kind: ConfigMap" >"$TREE/k8s/manifests/extra.yaml"
run "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
refused "a checkout with an untracked file" "k8s/manifests/extra.yaml"
rm -f "$TREE/k8s/manifests/extra.yaml"

# --- the PUBLIC_HOST guard --------------------------------------------------------------------
rm -f "$CONFIG"
run "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
refused "production without a deploy config" "no PUBLIC_HOST is configured"
echo "PUBLIC_HOST=" >"$CONFIG"
run "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
refused "production with an empty PUBLIC_HOST" "no PUBLIC_HOST is configured"
echo "PUBLIC_HOST=econgraph.com" >"$CONFIG"
run env STUB_MARKER=none "$RELEASE_DEPLOY" --tag "$TAG" --yes
refused "a cluster not marked production with PUBLIC_HOST configured" "only the production cluster serves the public host"
run env STUB_MARKER=fails "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
refused "a marker that can't be read" "treated as not production"
run env PUBLIC_HOST= "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
refused "an environment PUBLIC_HOST that differs from the config" "the config decides"
printf 'PUBLIC_HOST=econgraph.com\r\n' >"$CONFIG"
run "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
refused "a CRLF deploy config" "carriage return"
echo "PUBLIC_HOST=econgraph.com # the public site" >"$CONFIG"
run "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
refused "a PUBLIC_HOST with a trailing comment" "with blanks in it"
echo "PUBLIC_HOST=localhost" >"$CONFIG"
run "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
refused "a PUBLIC_HOST that is not a public DNS name" "is not a public DNS name"
echo 'PUBLIC_HOST=" econgraph.com "' >"$CONFIG"
run "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
refused "a quoted PUBLIC_HOST with blanks inside" "with blanks in it"
echo "PUBLIC_HOST=\"econgraph.com\"" >"$CONFIG"
run "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
if [ "$rc" -eq 0 ] && grep -qF "deploy  | PUBLIC_HOST=econgraph.com " "$LOGS/steps.log"; then
    echo "ok   a quoted PUBLIC_HOST is unquoted"
else
    fail "a quoted PUBLIC_HOST: exit $rc, $(cat "$LOGS/steps.log")"
fi
# A host other than the one the production cluster serves: refused, both named,
# unless --change-public-host.
echo "PUBLIC_HOST=econ-graph.com" >"$CONFIG"
run "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
refused "a PUBLIC_HOST other than the one the cluster serves" "serves the public host 'econgraph.com' now, but"
has "PUBLIC_HOST=econ-graph.com" || fail "a PUBLIC_HOST other than the one served: the configured one is not named"
run "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}" --change-public-host
if [ "$rc" -eq 0 ] && grep -qF "deploy  | PUBLIC_HOST=econ-graph.com " "$LOGS/steps.log" &&
    has "changes from econgraph.com (served now) to econ-graph.com"; then
    echo "ok   --change-public-host deploys another PUBLIC_HOST, naming both"
else
    fail "--change-public-host: exit $rc, $(cat "$LOGS/steps.log")"
fi
echo "PUBLIC_HOST=econgraph.com" >"$CONFIG"
run env STUB_SERVED_HOST= "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
refused "a production cluster that serves no public host yet" "serves the public host '(none)' now"
run env STUB_SERVED_HOST=fail "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
refused "a production cluster whose served host can't be read" "could not read which public host"
run env STUB_SERVED_HOST=EconGraph.com "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
[ "$rc" -eq 0 ] && [ "$(steps)" = "$ALL" ] ||
    fail "a cluster serving EconGraph.com, PUBLIC_HOST=econgraph.com: exit $rc (the same host)"
# Uppercase is the same host name: lowercased, and compared lowercased.
echo "PUBLIC_HOST=ECONGRAPH.COM" >"$CONFIG"
run "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
if [ "$rc" -eq 0 ] && grep -qF "deploy  | PUBLIC_HOST=econgraph.com " "$LOGS/steps.log"; then
    echo "ok   PUBLIC_HOST=ECONGRAPH.COM deploys econgraph.com"
else
    fail "PUBLIC_HOST=ECONGRAPH.COM: exit $rc, $(cat "$LOGS/steps.log")"
fi
# Several PUBLIC_HOST lines: the last one wins, as if the file were sourced.
printf 'PUBLIC_HOST=econ-graph.com\nPUBLIC_HOST=econgraph.com\n' >"$CONFIG"
run "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
[ "$rc" -eq 0 ] && grep -qF "deploy  | PUBLIC_HOST=econgraph.com " "$LOGS/steps.log" ||
    fail "two PUBLIC_HOST lines, the last right: exit $rc, $(cat "$LOGS/steps.log")"
printf 'PUBLIC_HOST=econgraph.com\nPUBLIC_HOST=econ-graph.com\n' >"$CONFIG"
run "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
refused "two PUBLIC_HOST lines, the last wrong" "PUBLIC_HOST=econ-graph.com"
echo "PUBLIC_HOST=econgraph.com" >"$CONFIG"
run env PUBLIC_HOST=econgraph.com "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
[ "$rc" -eq 0 ] && [ "$(steps)" = "$ALL" ] || fail "an environment PUBLIC_HOST equal to the config: exit $rc, steps '$(steps)'"
rm -f "$CONFIG"
run env STUB_MARKER=none "$RELEASE_DEPLOY" --tag "$TAG" --yes
if [ "$rc" -eq 0 ] && grep -qF "deploy  | PUBLIC_HOST= " "$LOGS/steps.log"; then
    echo "ok   a test cluster without a deploy config deploys with PUBLIC_HOST empty, on --yes"
else
    fail "a test cluster without a deploy config: exit $rc, $(cat "$LOGS/steps.log")"
fi
echo "PUBLIC_HOST=econgraph.com" >"$CONFIG"

# --- confirmation -------------------------------------------------------------------------------
run "$RELEASE_DEPLOY" --tag "$TAG"
refused "a run without a terminal or --yes" "needs --confirm-production"
run "$RELEASE_DEPLOY" --tag "$TAG" --yes
refused "--yes alone on the production cluster" "--yes alone does not answer it"
run "$RELEASE_DEPLOY" --tag "$TAG" --yes --confirm-production deadbeef
refused "a --confirm-production that doesn't match" "--confirm-production deadbeef does not match"
# On a terminal (script(1) gives it one), --yes still asks for the UID prefix on
# the production cluster: "yes" is refused, the prefix goes ahead.
on_terminal() { # ANSWER ARGS...: release-deploy.sh on a terminal, ANSWER typed
    local answer="$1" cmd
    shift
    : >"$LOGS/steps.log"; rm -rf "$WORK/releases"
    cmd="$(printf '%q ' "${ENV[@]}" "$RELEASE_DEPLOY" "$@")"
    out="$(printf '%s\n' "$answer" | timeout 60 script -qec "$cmd" /dev/null 2>&1)"
    rc=$?
}
on_terminal yes --tag "$TAG" --yes
if [ -s "$LOGS/steps.log" ] || ! has "type the first 8 characters of its kube-system UID"; then
    fail "--yes on a terminal, production: did not ask for the UID prefix (steps '$(steps)')"
else
    echo "ok   --yes on a terminal still asks for the production cluster's UID prefix"
fi
on_terminal "$UID_PREFIX" --tag "$TAG" --yes
[ "$(steps)" = "$ALL" ] || fail "the UID prefix typed on a terminal: steps '$(steps)'"
rm -f "$CONFIG"
run env STUB_MARKER=none "$RELEASE_DEPLOY" --tag "$TAG"
refused "a test cluster without a terminal or --yes" "needs confirmation"
run env STUB_MARKER=none "$RELEASE_DEPLOY" --tag "$TAG" --yes --confirm-production deadbeef
refused "a --confirm-production that doesn't match a test cluster" "--confirm-production deadbeef does not match"
echo "PUBLIC_HOST=econgraph.com" >"$CONFIG"

# --- dry run --------------------------------------------------------------------------------------
run "$RELEASE_DEPLOY" --tag "$TAG" --dry-run --report "$WORK/report"
want="backup-postgres-now load-release-images backup-keycloak-db precheck-db-migration deploy"
if [ "$rc" -ne 0 ]; then
    fail "dry run exits $rc"
elif [ "$(steps)" != "$want" ]; then
    fail "dry run: steps ran as '$(steps)', expected '$want'"
elif grep -v -- "--dry-run |" "$LOGS/steps.log"; then
    fail "dry run: a step ran without --dry-run"
elif ! has "it would run:" || ! grep -qxF "step=verify status=dry-run" "$WORK/report"; then
    fail "dry run: verify is not explained or reported"
elif grep -qF "cp -q -L" "$LOGS/docker.log" || [ -n "$(ls -A "$WORK/tmp")" ] || [ -n "$(ls -A "$WORK/run")" ] || [ -e "$WORK/releases" ]; then
    fail "dry run: refreshed the kubectl cache or left files behind"
else
    echo "ok   dry run: every step dry-runs, verify is described"
fi
run env STUB_FAIL=backup-keycloak-db "$RELEASE_DEPLOY" --tag "$TAG" --dry-run
[ "$rc" -ne 0 ] && has "the dry run of step keycloak failed; nothing was changed" ||
    fail "a failing dry-run step: exit $rc"
no_plain "dry run"

# --- tags ---------------------------------------------------------------------------------------------
run "$RELEASE_DEPLOY" --tag v4.0.0 "${YES[@]}"
if [ "$rc" -eq 0 ] && grep -qF "load-release-images --tag v4.0.0-rc.4 --cluster econ-graph |" "$LOGS/steps.log" &&
    has "v4.0.0 is v4.0.0-rc.4"; then
    echo "ok   a final tag deploys the highest release candidate on its commit"
else
    fail "final tag: exit $rc, steps $(cat "$LOGS/steps.log")"
fi
run "$RELEASE_DEPLOY" --tag v4.0 "${YES[@]}"
[ "$rc" -eq 2 ] && [ ! -s "$LOGS/steps.log" ] || fail "a malformed tag: exit $rc"
run "$RELEASE_DEPLOY" --yes
[ "$rc" -eq 2 ] || fail "no --tag: exit $rc"

# A load that refused changed nothing (load-release-images.sh refuses before any change).
run env STUB_FAIL=load-release-images "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}"
has "If the load refused (its last line says so), nothing was changed" ||
    fail "failing load: does not say a refusal changed nothing"

# --- verify's unchecked items ---------------------------------------------------------------
run env STUB_UNCHECKED=1 "$RELEASE_DEPLOY" --tag "$TAG" "${YES[@]}" --report "$WORK/report"
if [ "$rc" -ne 0 ]; then
    fail "verify with unchecked items: exit $rc"
elif ! sed -n '/is deployed on kind-econ-graph and verified/,$p' <<<"$out" | grep -qF "unchecked job/crawler-migrate: no Job"; then
    fail "verify with unchecked items: the final summary does not list them"
elif ! grep -qxF "step=verify status=ok unchecked=1" "$WORK/report" || ! grep -qxF "result=ok unchecked=1" "$WORK/report"; then
    fail "verify with unchecked items: report is $(cat "$WORK/report")"
else
    echo "ok   verify's unchecked items are listed at the end and reported"
fi

# --- another kind cluster ------------------------------------------------------------------
run env ECONGRAPH_SCRATCH_STORAGE="$WORK/storage" "$RELEASE_DEPLOY" --tag "$TAG" --cluster kg-test "${YES[@]}"
if [ "$rc" -ne 0 ]; then
    fail "--cluster kg-test: exit $rc"
elif ! grep -qF "load-release-images --tag $TAG --cluster kg-test |" "$LOGS/steps.log" ||
    ! grep -qF "verify-release-images --tag $TAG --cluster kg-test |" "$LOGS/steps.log" ||
    ! grep -qF "backup-postgres-now --context kind-kg-test |" "$LOGS/steps.log" ||
    ! grep -qF "backup-keycloak-db --context kind-kg-test |" "$LOGS/steps.log" ||
    [ "$(grep -c "KIND_CLUSTER=kg-test KUBECTL=$CACHE/bin/kubectl KUBECONFIG=$CACHE/kubeconfig-kg-test " "$LOGS/steps.log")" != 6 ]; then
    fail "--cluster kg-test: the steps did not all get it: $(cat "$LOGS/steps.log")"
elif [ -e "$WORK/storage" ]; then
    fail "--cluster kg-test: created the scratch storage"
else
    echo "ok   --cluster reaches every step, set before kind-env.sh"
fi
run env ECONGRAPH_SCRATCH_STORAGE="$WORK/storage" STUB_FAIL=deploy "$RELEASE_DEPLOY" --tag "$TAG" --cluster kg-test "${YES[@]}"
has "rollback-release-images.sh --to pre-$TAG --cluster kg-test --dry-run" ||
    fail "--cluster kg-test, failing deploy: the rollback command lacks --cluster kg-test"
# kind-env.sh's rule: a throwaway cluster needs ECONGRAPH_SCRATCH_STORAGE, checked first.
run "$RELEASE_DEPLOY" --tag "$TAG" --cluster kg-test "${YES[@]}"
if [ "$rc" -eq 0 ] || [ -s "$LOGS/steps.log" ] || ! has "needs ECONGRAPH_SCRATCH_STORAGE"; then
    fail "--cluster kg-test without scratch storage: exit $rc, steps '$(steps)'"
else
    echo "ok   --cluster kg-test without scratch storage is refused before any step"
fi

if [ "$failed" -ne 0 ]; then
    exit 1
fi
echo "release-deploy.sh: all checks passed"
