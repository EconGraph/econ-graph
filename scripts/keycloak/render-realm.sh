#!/usr/bin/env bash
# Resolve the ${VAR} and ${VAR:default} placeholders of a Keycloak realm export
# from the environment, the way Keycloak itself does with --import-realm, and
# print the result. keycloak-config-cli applies the realm through the admin API,
# which does not resolve them, so the k8s realm-import Job renders first.
#
# Usage: scripts/keycloak/render-realm.sh <realm.json>
# A placeholder with no default and no variable set fails the render. Values are
# spliced into JSON strings, so `\`, `"` and every ASCII control character
# (0x00-0x1F: newline, tab, etc.) in them are escaped, per RFC 8259; a value is
# never scanned for placeholders of its own.
set -euo pipefail

if [ $# -ne 1 ]; then
  echo "usage: $0 <realm.json>" >&2
  exit 2
fi

# Escapes `\`, `"` and every ASCII control character in $1 for a JSON string.
json_escape() {
  local s="$1" out="" c code
  local len=${#s}
  for (( i = 0; i < len; i++ )); do
    c="${s:i:1}"
    case "$c" in
      '\') out+='\\' ;;
      '"') out+='\"' ;;
      *)
        printf -v code '%d' "'$c"
        if (( code < 32 )); then
          printf -v c '\\u%04x' "$code"
        fi
        out+="$c"
        ;;
    esac
  done
  printf '%s' "$out"
}

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
  value="$(json_escape "$value")"
  # Everything before the first match is done; keep scanning after it.
  rendered+="${rest%%"$placeholder"*}${value}"
  rest="${rest#*"$placeholder"}"
done
printf '%s\n' "${rendered}${rest}"
