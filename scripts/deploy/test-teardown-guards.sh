#!/bin/bash
#
# Check that the destructive deploy scripts refuse, before they reach Docker
# or a cluster, when a confirmation they need can't be typed:
#
#   - teardown.sh without a terminal and without --yes;
#   - teardown.sh --include-data-plane without a terminal, even with --yes;
#   - teardown.sh --delete-cluster without --include-data-plane;
#   - released-pvs.sh --delete without a terminal.
#
# Runs offline: ECONGRAPH_DOCKER_HOST points at a socket that doesn't exist,
# so a script that got past its guard would fail on Docker instead, with a
# different message. Usage: scripts/deploy/test-teardown-guards.sh

set -uo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
export ECONGRAPH_DOCKER_HOST="unix://${WORK}/no-docker.sock"
export XDG_CACHE_HOME="${WORK}/cache"
export ECONGRAPH_BACKUP_ROOT="${WORK}/backups"

failed=0
# expect_refusal NAME EXPECTED_MESSAGE COMMAND...: COMMAND (stdin not a
# terminal) must exit non-zero and print EXPECTED_MESSAGE.
expect_refusal() {
    local name="$1" expected="$2" out rc
    shift 2
    out="$("$@" </dev/null 2>&1)"
    rc=$?
    if [ "$rc" -ne 0 ] && grep -qF -- "$expected" <<<"$out"; then
        echo "ok   $name"
    else
        echo "FAIL $name (exit $rc)"
        printf '     %s\n' "$out"
        failed=1
    fi
}

expect_refusal "teardown.sh without a terminal or --yes" \
    "no terminal to confirm on" "${PROJECT_ROOT}/scripts/deploy/teardown.sh"
expect_refusal "teardown.sh --include-data-plane --yes without a terminal" \
    "--yes doesn't answer it" "${PROJECT_ROOT}/scripts/deploy/teardown.sh" --include-data-plane --yes
expect_refusal "teardown.sh --delete-cluster without --include-data-plane" \
    "it needs --include-data-plane" "${PROJECT_ROOT}/scripts/deploy/teardown.sh" --delete-cluster --yes
expect_refusal "released-pvs.sh --delete without a terminal" \
    "needs the PV name typed on a terminal" "${PROJECT_ROOT}/scripts/deploy/released-pvs.sh" --delete some-pv
[ -e "${ECONGRAPH_BACKUP_ROOT}" ] && { echo "FAIL a backup directory was created"; failed=1; }

if [ "$failed" -ne 0 ]; then
    echo "teardown guard checks failed"
    exit 1
fi
echo "teardown guard checks passed"
