#!/usr/bin/env bash
# Offline tests that scripts/deploy/deploy.sh keeps its temporary files to itself, so two
# runs on one host (production's deploy and a QA cluster's, say) never share one:
#   - neither the script nor any library it sources names a fixed /tmp path; temporary files
#     come from mktemp under ${TMPDIR:-/tmp};
#   - the script's Grafana dashboards block (run on its own, with a stub kubectl and yq)
#     builds its ConfigMap in a file of its own under TMPDIR with both dashboards, applies
#     it, and removes it, also when the apply fails (the exit cleanup). deploy.sh's runs with
#     kind-env.sh's own add_exit_cleanup and must keep an earlier exit cleanup that holds a
#     quote;
#   - add_exit_cleanup chains cleanups that hold quotes, in order.
#
# Usage: scripts/deploy/test-deploy-temp-files.sh
# The pattern below matches the literal ${TMPDIR:-/tmp}; add_exit_cleanup is called by the
# evaluated block, and expands its command when set, as kind-env.sh's does.
# shellcheck disable=SC2016,SC2329,SC2064,SC2015
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DEPLOY="$ROOT/scripts/deploy/deploy.sh"
W="$(mktemp -d)"
trap 'rm -rf "$W"' EXIT
# Everything this test writes is under its own scratch directory, TMPDIR included.
# Quotes and shell metacharacters in TMPDIR must remain literal in deferred cleanup.
temp_root="$W/tmp/quote' space; literal \$HOME"
mkdir -p "$temp_root"
export TMPDIR="$temp_root"
failures=0
ok() { echo "ok   $1"; }
bad() { echo "FAIL $1"; failures=$((failures + 1)); }

# 1. No fixed /tmp path in the script or what it sources.
files=("$DEPLOY")
while read -r lib; do files+=("$ROOT/scripts/deploy/$lib"); done < <(grep -o 'lib/[a-z_-]*\.sh' "$DEPLOY" | sort -u)
fixed="$(grep -Hn '/tmp/' "${files[@]}" | grep -v '\${TMPDIR:-/tmp}' || true)"
if [ -z "$fixed" ]; then
    ok "deploy.sh and its libraries name no fixed /tmp path"
else
    bad "fixed /tmp paths: ${fixed}"
fi

# 2. add_exit_cleanup, as kind-env.sh has it, chains cleanups that hold quotes, in order.
cleanup_function="$(awk '/^add_exit_cleanup\(\) \{$/,/^\}$/' "$ROOT/scripts/deploy/lib/kind-env.sh")"
[ -n "$cleanup_function" ] || { echo "FAIL add_exit_cleanup was not found in kind-env.sh"; exit 1; }
(
    trap - EXIT  # not the test's own cleanup
    eval "$cleanup_function"
    add_exit_cleanup "echo 'first one' >>'$W/order'"
    add_exit_cleanup "echo \"second 'quoted'\" >>'$W/order'"
    add_exit_cleanup "echo third >>'$W/order'"
) 2>"$W/chain.err" || true
if [ "$(cat "$W/order" 2>/dev/null)" = "first one
second 'quoted'
third" ] && [ ! -s "$W/chain.err" ]; then
    ok "add_exit_cleanup chains cleanups that hold quotes, in order"
else
    bad "add_exit_cleanup lost or broke a cleanup: $(cat "$W/order" "$W/chain.err" 2>/dev/null)"
fi

# Exercise the real dry-run kubectl staging path, with every external caller stubbed.
# Its deferred cleanup must also quote the path produced beneath TMPDIR.
(
    trap - EXIT
    eval "$cleanup_function"
    eval "$(awk '/^try_use_kind_kubectl\(\) \{$/,/^\}$/' "$ROOT/scripts/deploy/lib/kind-env.sh")"
    eval "$(awk '/^point_kubectl_at\(\) \{$/,/^\}$/' "$ROOT/scripts/deploy/lib/kind-env.sh")"
    kind_cluster_exists() { return 0; }
    docker() {
        [ "$1" = cp ] || return 1
        printf '#!/usr/bin/env bash\nexit 0\n' >"${@: -1}"
    }
    kind() { echo 'stub kubeconfig'; }
    export ECONGRAPH_CACHE="$W/absent-cache" DRY_RUN=true KIND_CLUSTER=test KIND_CONTROL_PLANE=test
    export ECONGRAPH_AGENT_SESSION=0
    try_use_kind_kubectl || exit 1
    [ -x "$ECONGRAPH_KUBE_DIR/bin/kubectl" ] || exit 1
    case "$ECONGRAPH_KUBE_DIR" in "$temp_root"/econgraph-kube.*) ;; *) exit 1 ;; esac
    printf '%s' "$ECONGRAPH_KUBE_DIR" >"$W/kubectl-dir"
) >"$W/kubectl-cleanup.out" 2>&1 || bad "dry-run kubectl staging failed: $(cat "$W/kubectl-cleanup.out")"
if [ -s "$W/kubectl-dir" ] && [ ! -e "$(cat "$W/kubectl-dir")" ]; then
    ok "dry-run kubectl scratch directory under quoted TMPDIR is removed on exit"
else
    bad "dry-run kubectl scratch cleanup failed: $(cat "$W/kubectl-cleanup.out")"
fi

# 3. Each dashboards block, from its heading to the next step.
mkdir -p "$W/bin"
cat >"$W/bin/kubectl" <<'STUB'
#!/usr/bin/env bash
# Records the applied file's path and a copy of it; fails when STUB_FAIL is set.
echo "$*" >>"$STUB_LOG"
[ "$1 $2" = "apply -f" ] && cp "$3" "$STUB_COPY"
[ -z "${STUB_FAIL:-}" ]
STUB
cat >"$W/bin/yq" <<'STUB'
#!/usr/bin/env bash
echo '{"title": "logging"}'
STUB
chmod +x "$W/bin/kubectl" "$W/bin/yq"

# run_block: $block with errexit on, as in the script; its status in $status. Called outside
# any condition, where errexit would be ignored. With $earlier, an earlier quoted exit cleanup
# is registered first through add_exit_cleanup, as deploy.sh's pod monitor is.
run_block() {
    (
        cd "$ROOT"
        export PATH="$W/bin:$PATH" TMPDIR="$temp_root" STUB_LOG="$W/kubectl.log" STUB_COPY="$W/applied.yaml"
        trap - EXIT  # not the test's own cleanup
        eval "$cleanup_function"
        [ -z "$earlier" ] || add_exit_cleanup "touch '$W/earlier ran'"
        set -euo pipefail
        eval "$block"
    ) >"$W/out" 2>&1
    status=$?
}

# check_block NAME SCRIPT START END EARLIER: the dashboards block of SCRIPT between the lines
# START and END; EARLIER (true or false) whether it must keep an earlier exit cleanup.
check_block() {
    local name="$1" script="$2" start="$3" end="$4"
    earlier="$5"
    block="$(awk -v s="$start" -v e="$end" '$0 == s { on = 1 } on { print } on && $0 == e { exit }' "$script")"
    if [ -z "$block" ]; then
        bad "${name}: the dashboards block was not found"
        return
    fi
    # A block that names a fixed /tmp path would write the host's shared file whatever
    # TMPDIR says: never run it.
    if grep -v '\${TMPDIR:-/tmp}' <<<"$block" | grep -q '/tmp/'; then
        bad "${name}: the dashboards block names a fixed /tmp path; not running it"
        return
    fi
    [ "$earlier" = true ] || earlier=""
    rm -f "$W/earlier ran" "$W/applied.yaml"
    : >"$W/kubectl.log"
    set +e; run_block; set -e
    if [ "$status" = 0 ]; then
        applied="$(sed -n 's/^apply -f //p' "$W/kubectl.log")"
        case "$applied" in
            "$temp_root/grafana-dashboards."*.yaml) ok "${name}: the ConfigMap is applied from a file of its own under TMPDIR" ;;
            *) bad "${name}: applied from '${applied}', not a mktemp file under TMPDIR" ;;
        esac
        if python3 -c 'import sys, yaml; d = yaml.safe_load(open(sys.argv[1])); import json; assert json.loads(d["data"]["econgraph-overview.json"]) == json.load(open("grafana-dashboards/econgraph-overview.json")); assert json.loads(d["data"]["logging-dashboard.json"]) == {"title": "logging"}' "$W/applied.yaml"; then
            ok "${name}: with both dashboards"
        else
            bad "${name}: the applied ConfigMap lacks a dashboard"
        fi
        if compgen -G "$temp_root/grafana-dashboards.*" >/dev/null; then
            bad "${name}: the file is left in TMPDIR"
        else
            ok "${name}: and removed after"
        fi
        if [ -n "$earlier" ]; then
            [ -e "$W/earlier ran" ] && ok "${name}: the earlier exit cleanup still runs" ||
                bad "${name}: the earlier exit cleanup was lost"
        fi
    else
        bad "${name}: the dashboards block failed: $(cat "$W/out")"
    fi
    rm -f "$W/earlier ran"
    : >"$W/kubectl.log"
    export STUB_FAIL=1
    set +e; run_block; set -e
    unset STUB_FAIL
    if [ "$status" != 1 ] || [ ! -s "$W/kubectl.log" ]; then
        bad "${name}: failed apply was not reached or returned the wrong status ($status): $(cat "$W/out")"
    elif compgen -G "$temp_root/grafana-dashboards.*" >/dev/null; then
        bad "${name}: a failed apply left its file in TMPDIR"
    else
        ok "${name}: a failed apply still removes its file (exit cleanup)"
    fi
    if [ -n "$earlier" ]; then
        [ -e "$W/earlier ran" ] && ok "${name}: and the earlier exit cleanup still runs" ||
            bad "${name}: a failed apply lost the earlier exit cleanup"
    fi
}

check_block deploy.sh "$DEPLOY" '# Configure Grafana dashboards' \
    '# Wait for monitoring stack to be ready' true

if [ "$failures" -eq 0 ]; then
    echo "deploy temporary files: all passed"
else
    echo "${failures} failed"
    exit 1
fi
