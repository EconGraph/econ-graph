#!/usr/bin/env bash
# F10: dev- and QA-only realm data must never reach the cluster.
#
# Fails if any k8s manifest under k8s/manifests/ references the dev seeded users
# file, the QA-only users file, or turns on the password grant (KC_WEB_DIRECT_GRANTS).
# Those are for docker-compose only; QA users are opted into with an explicit
# docker-compose override (docker-compose.qa-users.yml), never in k8s.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

fail=0

check() {
  local pattern="$1" description="$2"
  local matches
  matches="$(grep -rn --include='*.yaml' --include='*.yml' -E "$pattern" k8s/manifests/ || true)"
  if [ -n "$matches" ]; then
    echo "check-no-dev-users-in-k8s: $description:" >&2
    echo "$matches" >&2
    fail=1
  fi
}

check 'econ-graph-users-0\.json' "a k8s manifest references the dev seeded users file"
check 'econ-graph-users-1\.json' "a k8s manifest references the QA-only users file"
check 'KC_WEB_DIRECT_GRANTS' "a k8s manifest sets KC_WEB_DIRECT_GRANTS (the password grant is dev-only)"

if [ "$fail" -ne 0 ]; then
  exit 1
fi
echo "check-no-dev-users-in-k8s: ok"
