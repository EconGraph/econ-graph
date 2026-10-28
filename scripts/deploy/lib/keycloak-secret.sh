#!/usr/bin/env bash
# Sourced by scripts/deploy/deploy.sh. Uses whatever `kubectl` is in effect there (the
# pinned kind context, or the --dry-run printing stub).

# The backend, Keycloak and the realm-import Job all read the anonymous catalog reader's
# credential from Secret econ-graph-keycloak, key anonymous-client-secret. A Secret written
# by an earlier version lacks that key, and those pods would then fail to start. Stop the
# deploy before anything is applied, naming the fix. A cluster without the Secret passes:
# create-secrets.sh writes it in full during the deploy.
# Returns 0 to continue; prints the reason and returns 1 to stop.
require_anonymous_client_secret() {
  local value
  if [ "${DRY_RUN:-false}" = true ]; then
    echo "[dry-run] check that Secret econ-graph-keycloak has the key anonymous-client-secret"
    return 0
  fi
  if ! kubectl -n econ-graph get secret econ-graph-keycloak >/dev/null 2>&1; then
    return 0
  fi
  if ! value="$(kubectl -n econ-graph get secret econ-graph-keycloak \
      -o 'jsonpath={.data.anonymous-client-secret}')"; then
    echo "❌ Couldn't read Secret econ-graph-keycloak; nothing was applied." >&2
    return 1
  fi
  if [ -z "$value" ]; then
    echo "❌ Secret econ-graph-keycloak has no anonymous-client-secret key; nothing was applied." >&2
    echo "   Run scripts/deploy/create-secrets.sh (it keeps every existing value and adds" >&2
    echo "   the missing key), then deploy again." >&2
    return 1
  fi
}
