#!/bin/bash
#
# Check that `scripts/test-monitoring.sh --lint-only` contacts no cluster and
# no network. kubectl, kind, docker, microk8s, curl, wget and nc are replaced
# by stubs that record the call and fail; the test fails if the lint fails or
# if any stub was called. KUBECONFIG and HOME point at an empty temporary
# directory. Never touches a real cluster. Runs in CI.
#
# Usage: scripts/test-monitoring-lint-only.sh

set -uo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/bin" "$WORK/home"
CALLS="$WORK/calls.log"
: >"$CALLS"

for tool in kubectl kind docker microk8s curl wget nc; do
    cat >"$WORK/bin/$tool" <<EOF
#!/bin/sh
echo "$tool \$*" >>"$CALLS"
echo "stub $tool: refusing (called by --lint-only)" >&2
exit 1
EOF
    chmod +x "$WORK/bin/$tool"
done

# run_isolated CMD...: CMD with the stubs first on PATH and no kube config.
run_isolated() {
    env -u KUBECONFIG -u DOCKER_HOST -u KIND_CLUSTER HOME="$WORK/home" \
        PATH="$WORK/bin:/usr/local/bin:/usr/bin:/bin" "$@"
}

failed=0

# The harness itself: a command that calls kubectl must be caught.
run_isolated sh -c 'kubectl get pods' >/dev/null 2>&1
if grep -q '^kubectl get pods' "$CALLS"; then
    echo "ok   the stubs record calls"
else
    echo "FAIL the kubectl stub did not record a call"
    failed=1
fi
: >"$CALLS"

out="$(run_isolated "$PROJECT_ROOT/scripts/test-monitoring.sh" --lint-only 2>&1)"
rc=$?
if [ "$rc" -eq 0 ]; then
    echo "ok   test-monitoring.sh --lint-only exits 0"
else
    echo "FAIL test-monitoring.sh --lint-only exited $rc:"
    printf '     %s\n' "$out"
    failed=1
fi
if [ -s "$CALLS" ]; then
    echo "FAIL test-monitoring.sh --lint-only called:"
    sed 's/^/     /' "$CALLS"
    failed=1
else
    echo "ok   test-monitoring.sh --lint-only called no kubectl, kind, docker, microk8s, curl, wget or nc"
fi

# expect_lint_failure NAME FILE CONTENT MESSAGE: a copy of the monitoring tree
# with FILE (relative) set to CONTENT must fail the lint with MESSAGE.
expect_lint_failure() {
    local name="$1" file="$2" content="$3" message="$4" tree="$WORK/tree-$1"
    mkdir -p "$tree/k8s" "$tree/grafana-dashboards"
    cp -r "$PROJECT_ROOT/k8s/monitoring" "$tree/k8s/"
    cp "$PROJECT_ROOT"/grafana-dashboards/*.json "$tree/grafana-dashboards/"
    printf '%s\n' "$content" >"$tree/$file"
    out="$(MONITORING_LINT_ROOT="$tree" run_isolated "$PROJECT_ROOT/scripts/test-monitoring.sh" --lint-only 2>&1)"
    rc=$?
    if [ "$rc" -ne 0 ] && grep -qF -- "$message" <<<"$out"; then
        echo "ok   --lint-only rejects $name"
    else
        echo "FAIL --lint-only did not reject $name (exit $rc):"
        printf '     %s\n' "$out"
        failed=1
    fi
}
expect_lint_failure "a duplicate YAML key" k8s/monitoring/dup.yaml \
    $'apiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: dup\ndata:\n  a: "1"\ndata:\n  b: "2"' \
    "duplicate key 'data'"
expect_lint_failure "a duplicate JSON key" grafana-dashboards/dup.json \
    '{"title": "a", "title": "b"}' "duplicate key(s) 'title'"
expect_lint_failure "invalid YAML" k8s/monitoring/bad.yaml \
    $'kind: ConfigMap\nmetadata: [unclosed' "FAIL k8s/monitoring/bad.yaml"
if [ -s "$CALLS" ]; then
    echo "FAIL the negative lint runs called:"
    sed 's/^/     /' "$CALLS"
    failed=1
fi

if [ "$failed" -ne 0 ]; then
    echo "monitoring lint isolation checks failed"
    exit 1
fi
echo "monitoring lint isolation checks passed"
