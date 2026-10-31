#!/usr/bin/env bash
# `A && B || fail` below fails when either check fails, as meant; each step runs the library
# in its own subshell, so its PATH and STATE stay there.
# shellcheck disable=SC2015,SC2030,SC2031
# Tests for scripts/deploy/lib/app-config.sh, offline: a stub kubectl first on PATH keeps
# the ConfigMap's data and each Deployment's annotation in files, no cluster.
#   - a Deployment without the annotation, or with another hash, is patched with the
#     ConfigMap's hash (which rolls it); one with the same hash is left alone;
#   - changing the ConfigMap's data changes the hash, so a same-tag redeploy rolls the
#     app; re-running with nothing changed patches nothing;
#   - a repeat deploy (apply the manifests, which don't name the annotation, then the step)
#     with unchanged data rolls nothing: apply leaves the patched annotation alone;
#   - a Deployment that doesn't exist fails the step before anything is patched;
#   - a ConfigMap that can't be read fails the step and patches nothing;
#   - under DRY_RUN nothing is read or patched;
#   - deploy.sh runs the step after it applies the Deployments.
#
# Usage: scripts/deploy/test-app-config.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
W="$(mktemp -d)"
trap 'rm -rf "$W"' EXIT
mkdir -p "$W/bin" "$W/state"

cat >"$W/bin/kubectl" <<'STUB'
#!/usr/bin/env bash
# Stub kubectl: get configmap prints $STATE/configmap (fails when it is missing); get
# deployment/D prints $STATE/D.hash, or fails like kubectl when $STATE/D.absent exists;
# patch deployment/D records the patched hash in $STATE/D.hash and the call in
# $STATE/patches; apply -f records the call in $STATE/applies and, as client-side apply
# does for a template annotation it never set, leaves $STATE/D.hash alone.
args=" $* "
case "$args" in
    *" get configmap econ-graph-config "*)
        [ -f "$STATE/configmap" ] || { echo "not found" >&2; exit 1; }
        cat "$STATE/configmap" ;;
    *" get deployment/"*)
        d="${args#* get deployment/}"; d="${d%% *}"
        if [ -f "$STATE/$d.absent" ]; then
            echo "Error from server (NotFound): deployments.apps \"$d\" not found" >&2
            exit 1
        fi
        cat "$STATE/$d.hash" 2>/dev/null || true ;;
    *" apply -f "*)
        echo "$*" >>"$STATE/applies" ;;
    *" patch deployment/"*)
        d="${args#* patch deployment/}"; d="${d%% *}"
        hash="$(printf '%s' "$args" | sed -n 's/.*app-config-hash\\*":\\*"\([0-9a-f]*\).*/\1/p')"
        printf '%s' "$hash" >"$STATE/$d.hash"
        echo "$d" >>"$STATE/patches" ;;
    *) echo "unexpected kubectl $*" >&2; exit 1 ;;
esac
STUB
chmod +x "$W/bin/kubectl"

failures=0
ok() { echo "ok   $1"; }
bad() {
    echo "FAIL $1"
    failures=$((failures + 1))
}

# step: run roll_app_on_config_change against the stub; its status, and the patched
# Deployments in $W/state/patches (emptied first).
step() {
    : >"$W/state/patches"
    (
        export STATE="$W/state" PATH="$W/bin:$PATH"
        # shellcheck source=scripts/deploy/lib/app-config.sh
        source "$ROOT/scripts/deploy/lib/app-config.sh"
        roll_app_on_config_change
    ) >"$W/out" 2>&1
}
patched() { tr '\n' ' ' <"$W/state/patches" | sed 's/ $//'; }

printf '%s' '{"CORS_ORIGIN":"http://localhost,http://localhost:3000"}' >"$W/state/configmap"
step && [ "$(patched)" = "econ-graph-backend crawler-worker econ-graph-frontend" ] &&
    ok "Deployments without the annotation are given the hash" ||
    bad "first run patched '$(patched)'"
first="$(cat "$W/state/econ-graph-backend.hash")"
[ "${#first}" = 16 ] && ok "the hash is 16 hex digits" || bad "hash '${first}'"

step && [ -z "$(patched)" ] && ok "re-running with the same data patches nothing" ||
    bad "same data patched '$(patched)'"

printf '%s' '{"CORS_ORIGIN":"https://econgraph.com,https://www.econgraph.com,http://localhost"}' >"$W/state/configmap"
step && [ "$(patched)" = "econ-graph-backend crawler-worker econ-graph-frontend" ] &&
    ok "changed data rolls every Deployment that reads it" ||
    bad "changed data patched '$(patched)'"
[ "$(cat "$W/state/crawler-worker.hash")" != "$first" ] && ok "with a new hash" || bad "hash unchanged"
grep -q "deployment/econ-graph-backend: econ-graph-config [0-9a-f]* (was ${first}); rolling it" "$W/out" &&
    ok "and says which hash it replaced" || { bad "no rolling line"; cat "$W/out"; }

printf 'stale' >"$W/state/econ-graph-frontend.hash"
step && [ "$(patched)" = "econ-graph-frontend" ] &&
    ok "only a Deployment on another hash is rolled" || bad "patched '$(patched)'"

# A repeat deploy: deploy.sh applies the Deployment manifests, then runs the step.
repeat_deploy() {
    (
        export STATE="$W/state" PATH="$W/bin:$PATH"
        for m in backend-deployment crawler-worker frontend-deployment; do
            kubectl apply -f "k8s/manifests/$m.yaml"
        done
    )
    step
}
repeat_deploy && [ -z "$(patched)" ] && [ "$(wc -l <"$W/state/applies")" = 3 ] &&
    ok "apply, then the step, with unchanged data rolls nothing" || bad "repeat deploy patched '$(patched)'"
repeat_deploy && [ -z "$(patched)" ] && ok "and again" || bad "second repeat patched '$(patched)'"

touch "$W/state/crawler-worker.absent"
if step; then bad "a missing Deployment passed"; else ok "a missing Deployment fails the step"; fi
grep -q "crawler-worker" "$W/state/patches" && bad "the missing Deployment was patched" ||
    ok "and it is never patched"
rm "$W/state/crawler-worker.absent"

rm "$W/state/configmap"
if step; then bad "an unreadable ConfigMap passed"; else ok "an unreadable ConfigMap fails the step"; fi
[ -z "$(patched)" ] && ok "and patches nothing" || bad "patched '$(patched)'"

: >"$W/state/patches"
(
    export STATE="$W/state" PATH="$W/bin:$PATH" DRY_RUN=true
    # shellcheck source=scripts/deploy/lib/app-config.sh
    source "$ROOT/scripts/deploy/lib/app-config.sh"
    roll_app_on_config_change
) >"$W/out" 2>&1 && [ -z "$(patched)" ] && grep -q '^\[dry-run\]' "$W/out" &&
    ok "DRY_RUN reads and patches nothing" || bad "DRY_RUN: $(cat "$W/out")"

deploy="$ROOT/scripts/deploy/deploy.sh"
step_line="$(grep -n '^roll_app_on_config_change || exit 1' "$deploy" | cut -d: -f1)"
last_apply="$(grep -n 'kubectl apply -f k8s/manifests/frontend-deployment.yaml' "$deploy" | cut -d: -f1)"
backend_apply="$(grep -n 'render_manifest k8s/manifests/backend-deployment.yaml' "$deploy" | cut -d: -f1)"
worker_apply="$(grep -n 'kubectl apply -f k8s/manifests/crawler-worker.yaml' "$deploy" | cut -d: -f1)"
if [ -n "$step_line" ] && [ "$step_line" -gt "$last_apply" ] && [ "$step_line" -gt "$backend_apply" ] &&
    [ "$step_line" -gt "$worker_apply" ]; then
    ok "deploy.sh runs the step after it applies the three Deployments"
else
    bad "deploy.sh: the step (line ${step_line:-none}) is not after the Deployments"
fi

if [ "$failures" -eq 0 ]; then
    echo "all passed"
else
    echo "${failures} failed"
    exit 1
fi
