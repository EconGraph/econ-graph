#!/usr/bin/env bash
# Offline tests that scripts/deploy/deploy.sh keeps its temporary files to itself, so two
# deploys on one host (production's and a QA cluster's, say) never share one:
#   - deploy.sh and the libraries it sources name no fixed /tmp path; temporary files come
#     from mktemp under ${TMPDIR:-/tmp};
#   - the Grafana dashboards block (run on its own, with a stub kubectl and yq) builds its
#     ConfigMap in a file of its own under TMPDIR with both dashboards, applies it, and
#     removes it, also when the apply fails (the exit cleanup).
#
# Usage: scripts/deploy/test-deploy-temp-files.sh
# The pattern below matches the literal ${TMPDIR:-/tmp}; add_exit_cleanup is called by the
# evaluated block, and expands its command when set, as kind-env.sh's does.
# shellcheck disable=SC2016,SC2329,SC2064
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DEPLOY="$ROOT/scripts/deploy/deploy.sh"
W="$(mktemp -d)"
trap 'rm -rf "$W"' EXIT
failures=0
ok() { echo "ok   $1"; }
bad() { echo "FAIL $1"; failures=$((failures + 1)); }

# 1. No fixed /tmp path in deploy.sh or what it sources.
files=("$DEPLOY")
while read -r lib; do files+=("$ROOT/scripts/deploy/$lib"); done < <(grep -o 'lib/[a-z_-]*\.sh' "$DEPLOY" | sort -u)
fixed="$(grep -Hn '/tmp/' "${files[@]}" | grep -v '\${TMPDIR:-/tmp}' || true)"
if [ -z "$fixed" ]; then
    ok "deploy.sh and its libraries name no fixed /tmp path"
else
    bad "fixed /tmp paths: ${fixed}"
fi

# 2. The dashboards block, from its heading to the next step.
block="$(awk '/^# Configure Grafana dashboards$/,/^# Wait for monitoring stack to be ready$/' "$DEPLOY")"
[ -n "$block" ] || { echo "FAIL the dashboards block was not found in deploy.sh"; exit 1; }
mkdir -p "$W/bin" "$W/tmp"
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
# run_block: the block with errexit on, as in deploy.sh; its status in $status. Called
# outside any condition, where errexit would be ignored.
run_block() {
    (
        cd "$ROOT"
        export PATH="$W/bin:$PATH" TMPDIR="$W/tmp" STUB_LOG="$W/kubectl.log" STUB_COPY="$W/applied.yaml"
        # As kind-env.sh's: chain onto the EXIT trap.
        add_exit_cleanup() { trap "$1" EXIT; }
        set -euo pipefail
        eval "$block"
    ) >"$W/out" 2>&1
    status=$?
}
: >"$W/kubectl.log"
set +e; run_block; set -e
if [ "$status" = 0 ]; then
    applied="$(sed -n 's/^apply -f //p' "$W/kubectl.log")"
    case "$applied" in
        "$W/tmp/grafana-dashboards."*.yaml) ok "the ConfigMap is applied from a file of its own under TMPDIR" ;;
        *) bad "applied from '${applied}', not a mktemp file under TMPDIR" ;;
    esac
    if python3 -c 'import sys, yaml; d = yaml.safe_load(open(sys.argv[1])); assert set(d["data"]) == {"econgraph-overview.json", "logging-dashboard.json"}, d["data"].keys()' "$W/applied.yaml"; then
        ok "with both dashboards"
    else
        bad "the applied ConfigMap lacks a dashboard"
    fi
    if compgen -G "$W/tmp/grafana-dashboards.*" >/dev/null; then bad "the file is left in TMPDIR"; else ok "and removed after"; fi
else
    bad "the dashboards block failed: $(cat "$W/out")"
fi
: >"$W/kubectl.log"
export STUB_FAIL=1
set +e; run_block; set -e
unset STUB_FAIL
if [ "$status" = 0 ]; then
    bad "a failed apply did not fail the block"
elif compgen -G "$W/tmp/grafana-dashboards.*" >/dev/null; then
    bad "a failed apply left its file in TMPDIR"
else
    ok "a failed apply still removes its file (exit cleanup)"
fi

if [ "$failures" -eq 0 ]; then
    echo "deploy.sh temporary files: all passed"
else
    echo "${failures} failed"
    exit 1
fi
