#!/usr/bin/env bash
# Resolve the ${VAR} and ${VAR:default} placeholders of a Keycloak realm export
# from the environment, the way Keycloak itself does with --import-realm, and
# print the result. keycloak-config-cli applies the realm through the admin API,
# which does not resolve them, so the k8s realm-import Job renders first.
#
# Usage: scripts/keycloak/render-realm.sh <realm.json>
# A placeholder with no default and no variable set fails the render. Values are
# spliced into JSON strings, so `\`, `"` and the newline/carriage-return/tab
# control characters in them are escaped; a value is never scanned for
# placeholders of its own.
set -euo pipefail

if [ $# -ne 1 ]; then
  echo "usage: $0 <realm.json>" >&2
  exit 2
fi

rest="$(cat -- "$1")"
rendered=""
pattern='\$\{([A-Za-z_][A-Za-z0-9_]*)(:([^}]*))?\}'
while [[ "$rest" =~ $pattern ]]; do
  placeholder="${BASH_REMATCH[0]}"
  name="${BASH_REMATCH[1]}"
  if [ -n "${!name+x}" ]; then
    value="${!name}"
  elif [ -n "${BASH_REMATCH[2]}" ]; then
    value="${BASH_REMATCH[3]}"
  else
    echo "render-realm: ${name} is not set and ${placeholder} has no default" >&2
    exit 1
  fi
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  value="${value//$'\n'/\\n}"
  value="${value//$'\r'/\\r}"
  value="${value//$'\t'/\\t}"
  # Everything before the first match is done; keep scanning after it.
  rendered+="${rest%%"$placeholder"*}${value}"
  rest="${rest#*"$placeholder"}"
done
printf '%s\n' "${rendered}${rest}"
