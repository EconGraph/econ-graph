#!/usr/bin/env bash
# F10: dev- and QA-only realm data must never reach the cluster.
#
# Fails if anything that builds or deploys the cluster (k8s/, scripts/deploy/,
# terraform/) references the dev or QA users (config/keycloak/dev or qa, or any
# econ-graph-users-<n>.json) or turns on the password grant (KC_WEB_DIRECT_GRANTS).
# Those are for docker-compose only; QA users are opted into with an explicit
# docker-compose override (docker-compose.qa-users.yml), never in k8s.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

fail=0

check() {
  local pattern="$1" description="$2"
  local matches
  matches="$(grep -rnE "$pattern" k8s/ scripts/deploy/ terraform/ || true)"
  if [ -n "$matches" ]; then
    echo "check-no-dev-users-in-k8s: $description:" >&2
    echo "$matches" >&2
    fail=1
  fi
}

check 'config/keycloak/(dev|qa)|econ-graph-users-[0-9]+\.json' "deploy config references the dev or QA users"
check 'KC_WEB_DIRECT_GRANTS' "deploy config sets KC_WEB_DIRECT_GRANTS (the password grant is dev-only)"

if [ "$fail" -ne 0 ]; then
  exit 1
fi
echo "check-no-dev-users-in-k8s: ok"
