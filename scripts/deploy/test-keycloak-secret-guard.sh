#!/usr/bin/env bash
# The stub kubectl is called by the sourced library.
# shellcheck disable=SC2329
# Tests for scripts/deploy/lib/keycloak-secret.sh against a stub kubectl, no cluster:
#   - a Secret econ-graph-keycloak without anonymous-client-secret stops the deploy and
#     names scripts/deploy/create-secrets.sh; a failed read stops it too;
#   - a Secret with the key, or no Secret at all (create-secrets.sh writes it in full), passes;
#   - --dry-run contacts nothing;
#   - the guard only reads, and deploy.sh runs it before it applies anything.
#
# Usage: scripts/deploy/test-keycloak-secret-guard.sh (from anywhere)
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$repo_root"
failures=0
fail() {
  echo "FAIL $1"
  failures=$((failures + 1))
}

# The stub cluster. STUB_SECRET: missing, without-key, with-key or read-fails.
STUB_LOG=""
kubectl() {
  STUB_LOG+="kubectl $*"$'\n'
  case "$STUB_SECRET:$*" in
    missing:*) return 1 ;;
    read-fails:*jsonpath*) return 1 ;;
    with-key:*jsonpath*) printf 'c2VjcmV0' ;;
    *) return 0 ;;
  esac
}

# shellcheck source=scripts/deploy/lib/keycloak-secret.sh
source scripts/deploy/lib/keycloak-secret.sh

# check NAME SECRET DRY_RUN EXPECTED_STATUS [EXPECTED_MESSAGE]
check() {
  local name="$1" status=0 output
  STUB_SECRET="$2" DRY_RUN="$3" STUB_LOG=""
  output="$(require_anonymous_client_secret 2>&1)" || status=$?
  [ "$status" -eq "$4" ] || fail "$name: status $status, expected $4 ($output)"
  if [ -n "${5:-}" ] && [[ "$output" != *"$5"* ]]; then
    fail "$name: output lacks '$5': $output"
  fi
}

check "Secret without the key stops the deploy" without-key false 1 "scripts/deploy/create-secrets.sh"
check "a failed read stops the deploy" read-fails false 1 "nothing was applied"
check "Secret with the key passes" with-key false 0
check "no Secret passes" missing false 0
check "dry run passes" without-key true 0 "[dry-run]"

# The stub log of a dry run, and of every other case, holds no write.
STUB_SECRET=without-key DRY_RUN=true STUB_LOG=""
require_anonymous_client_secret >/dev/null
[ -z "$STUB_LOG" ] || fail "dry run contacted the cluster: $STUB_LOG"
for secret in missing without-key with-key read-fails; do
  STUB_SECRET="$secret" DRY_RUN=false STUB_LOG=""
  require_anonymous_client_secret >/dev/null 2>&1 || true
  if grep -vE '^kubectl -n econ-graph get secret econ-graph-keycloak' <<<"$STUB_LOG" | grep -q .; then
    fail "$secret: the guard did more than read: $STUB_LOG"
  fi
done

# deploy.sh runs the guard, and stops on it, before its first apply, helper script or
# public-host step.
guard_line="$(grep -n '^require_anonymous_client_secret || exit 1$' scripts/deploy/deploy.sh | cut -d: -f1 || true)"
first_change="$(grep -nE '^(kubectl apply|run_script |render_manifest |public_host_init)' scripts/deploy/deploy.sh \
  | head -1 | cut -d: -f1)"
if [ -z "$guard_line" ]; then
  fail "deploy.sh does not stop on require_anonymous_client_secret"
elif [ -z "$first_change" ] || [ "$guard_line" -ge "$first_change" ]; then
  fail "deploy.sh runs the guard at line $guard_line, after its first change at line $first_change"
fi

if [ "$failures" -gt 0 ]; then
  echo "❌ $failures failure(s)"
  exit 1
fi
echo "✅ Keycloak Secret guard tests passed"
