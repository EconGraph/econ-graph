#!/usr/bin/env bash
# Get an access token for a seeded dev-realm user by password grant and print
# its iss, aud and sub claims (or the raw token with --raw).
#
# Dev realm only: the password grant is enabled on econ-graph-web through
# KC_WEB_DIRECT_GRANTS=true in docker-compose.yml and is off everywhere else.
#
# Usage: scripts/keycloak/dev-token.sh [alice|bob|staff-admin] [--raw]  (any order)
# Env:   KEYCLOAK_URL (default http://localhost:8081)
set -euo pipefail

user="alice"
raw=false
for arg in "$@"; do
  case "$arg" in
    --raw) raw=true ;;
    *) user="$arg" ;;
  esac
done
keycloak_url="${KEYCLOAK_URL:-http://localhost:8081}"

case "$user" in
  alice | bob | staff-admin) password="${user}-dev-password" ;;
  *)
    echo "unknown dev user: $user (expected alice, bob or staff-admin)" >&2
    exit 2
    ;;
esac

response="$(curl -sS --fail-with-body \
  -d grant_type=password \
  -d client_id=econ-graph-web \
  -d scope=openid \
  --data-urlencode "username=${user}" \
  --data-urlencode "password=${password}" \
  "${keycloak_url}/realms/econ-graph/protocol/openid-connect/token")" || {
  printf 'token request failed: %s\n' "${response:-no response from ${keycloak_url}}" >&2
  exit 1
}

token="$(printf '%s' "$response" | python3 -c 'import json, sys; print(json.load(sys.stdin)["access_token"])')"

if [ "$raw" = true ]; then
  printf '%s\n' "$token"
  exit 0
fi

printf '%s' "$token" | python3 -c '
import base64, json, sys
payload = sys.stdin.read().split(".")[1]
claims = json.loads(base64.urlsafe_b64decode(payload + "=" * (-len(payload) % 4)))
for name in ("iss", "aud", "sub", "preferred_username", "azp"):
    print(f"{name}: {json.dumps(claims.get(name))}")
'
