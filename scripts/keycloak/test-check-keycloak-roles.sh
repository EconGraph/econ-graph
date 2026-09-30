#!/usr/bin/env bash
# Fixture tests for scripts/check-keycloak-roles: a matching realm passes, and a
# role missing from either side, a duplicate client role, a composite naming an
# undefined role, an admin composite missing a role and a user or default role
# granting a staff role each fail with a message naming the role.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
check="${repo_root}/scripts/check-keycloak-roles"
fixtures="${repo_root}/scripts/keycloak/testdata/check-roles"
catalog="${fixtures}/catalog.txt"
failures=0

# expect <exit code> <realm fixture> <text the output must contain>
expect() {
  local want="$1" realm="$2" text="$3" output status=0
  output="$("$check" --realm "${fixtures}/${realm}" --roles-file "$catalog" 2>&1)" || status=$?
  if [ "$status" -ne "$want" ] || ! grep -qF -- "$text" <<<"$output"; then
    echo "FAIL ${realm}: want exit ${want} and output containing '${text}', got exit ${status}:"
    echo "$output"
    failures=$((failures + 1))
  else
    echo "ok   ${realm}"
  fi
}

expect 0 realm-match.json "3 roles match"
expect 1 realm-missing-role.json "role 'api:mcp' is in the Role enum but not a client role"
expect 1 realm-extra-role.json "client role 'chart:delete' on econ-graph-api is not in the Role enum"
expect 1 realm-duplicate-role.json "client role 'api:mcp' is defined more than once"
expect 1 realm-unknown-composite.json "composite 'admin' includes econ-graph-api role 'admin.users:nuke'"
expect 1 realm-admin-incomplete.json "composite 'admin' does not grant 'admin.users:read'"
expect 1 realm-user-has-staff-role.json "composite 'user' grants staff role 'admin.users:read'"
expect 1 realm-default-has-staff-role.json "composite 'default-roles-econ-graph' grants staff role 'admin.users:read'"
expect 2 does-not-exist.json "error:"

if [ "$failures" -ne 0 ]; then
  echo "${failures} fixture test(s) failed"
  exit 1
fi
echo "all check-keycloak-roles fixture tests passed"
