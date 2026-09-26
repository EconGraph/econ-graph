#!/usr/bin/env bash
# Create or update the Secret `keycloak-secrets` from environment variables.
# Nothing here is committed: values come only from the caller's environment.
#
# Required:
#   KEYCLOAK_ADMIN_PASSWORD   bootstrap admin password for the Keycloak console
#   KEYCLOAK_DB_PASSWORD      password for Keycloak's own Postgres
# Optional:
#   KEYCLOAK_ADMIN_USERNAME   defaults to "admin"
#   KC_GOOGLE_CLIENT_ID and KC_GOOGLE_CLIENT_SECRET  set both to enable Google sign-in
#
# Refuses empty values, known placeholders and passwords shorter than 16 characters.
# Usage: scripts/deploy/create-keycloak-secret.sh [--dry-run]
set -euo pipefail

namespace="econ-graph"
dry_run=""
if [ "${1:-}" = "--dry-run" ]; then
  dry_run="--dry-run=client"
fi

fail() {
  echo "create-keycloak-secret: $*" >&2
  exit 1
}

check_value() {
  local name="$1" value="$2" min_length="$3"
  [ -n "$value" ] || fail "$name is not set"
  case "$(printf '%s' "$value" | tr '[:upper:]' '[:lower:]')" in
    admin | admin123 | password | changeme | change-me | keycloak | keycloak123 | secret | unset | your-* | *placeholder* | *example*)
      fail "$name is a placeholder value; set a real one"
      ;;
  esac
  [ "${#value}" -ge "$min_length" ] || fail "$name must be at least $min_length characters"
}

admin_username="${KEYCLOAK_ADMIN_USERNAME:-admin}"
admin_password="${KEYCLOAK_ADMIN_PASSWORD:-}"
db_password="${KEYCLOAK_DB_PASSWORD:-}"
google_client_id="${KC_GOOGLE_CLIENT_ID:-}"
google_client_secret="${KC_GOOGLE_CLIENT_SECRET:-}"

check_value KEYCLOAK_ADMIN_PASSWORD "$admin_password" 16
check_value KEYCLOAK_DB_PASSWORD "$db_password" 16

args=(
  --from-literal=admin-username="$admin_username"
  --from-literal=admin-password="$admin_password"
  --from-literal=db-password="$db_password"
)

if [ -n "$google_client_id" ] || [ -n "$google_client_secret" ]; then
  check_value KC_GOOGLE_CLIENT_ID "$google_client_id" 1
  check_value KC_GOOGLE_CLIENT_SECRET "$google_client_secret" 1
  args+=(
    --from-literal=google-enabled=true
    --from-literal=google-client-id="$google_client_id"
    --from-literal=google-client-secret="$google_client_secret"
  )
else
  echo "create-keycloak-secret: KC_GOOGLE_CLIENT_ID/SECRET not set, Google sign-in stays disabled" >&2
fi

# Render client-side, then apply, so re-running updates the Secret in place.
kubectl -n "$namespace" create secret generic keycloak-secrets "${args[@]}" \
  --dry-run=client -o yaml | kubectl apply $dry_run -f - >/dev/null
if [ -n "$dry_run" ]; then
  echo "create-keycloak-secret: keycloak-secrets validated (dry run)"
else
  echo "create-keycloak-secret: keycloak-secrets applied"
fi
