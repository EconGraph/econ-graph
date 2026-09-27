#!/usr/bin/env bash
# Tests for scripts/keycloak/render-realm.sh: the committed realm renders to valid
# JSON with no placeholder left, set variables (even empty ones) win over defaults,
# values are JSON-escaped and not re-expanded, and a placeholder with neither a
# value nor a default fails.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
render="${repo_root}/scripts/keycloak/render-realm.sh"
realm="${repo_root}/config/keycloak/econ-graph-realm.json"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
failures=0

fail() {
  echo "FAIL $1"
  failures=$((failures + 1))
}

# Render with a clean environment so a KC_* variable on the machine cannot leak in.
clean_render() {
  env -i PATH="$PATH" "$@" "$render" "$realm"
}

rendered="$(clean_render KC_WEB_BASE_URL=https://app.example.test KC_WEB_E2E_BASE_URL=https://e2e.example.test KC_GOOGLE_CLIENT_ID=gid)"
python3 -c 'import json, sys; json.loads(sys.stdin.read())' <<<"$rendered" || fail "rendered realm is not valid JSON"
grep -qF -- '${' <<<"$rendered" && fail "rendered realm still has a placeholder"
grep -qF -- 'https://app.example.test/auth/callback' <<<"$rendered" || fail "KC_WEB_BASE_URL was not substituted"
grep -qF -- '"sslRequired": "external"' <<<"$rendered" || fail "default for KC_REALM_SSL_REQUIRED was not used"
grep -qF -- '"clientId": "gid"' <<<"$rendered" || fail "KC_GOOGLE_CLIENT_ID was not substituted"
grep -qF -- '"clientSecret": "unset"' <<<"$rendered" || fail "default for KC_GOOGLE_CLIENT_SECRET was not used"

rendered="$(clean_render KC_WEB_BASE_URL=http://localhost KC_WEB_E2E_BASE_URL=http://localhost KC_GOOGLE_CLIENT_SECRET=)"
grep -qF -- '"clientSecret": ""' <<<"$rendered" || fail "an empty but set variable should win over the default"

if clean_render >"$scratch/out" 2>"$scratch/err"; then
  fail "a placeholder with no value and no default should fail the render"
fi
grep -qF -- 'KC_WEB_BASE_URL is not set' "$scratch/err" || fail "the missing-variable error should name the variable"

# Values are escaped for JSON and never re-expanded; a default may not contain `}`.
printf '{"a": "${A}", "b": "${B:x}", "c": "${C:plain default}"}\n' >"$scratch/realm.json"
rendered="$(env -i PATH="$PATH" A='p\q"r/&' B='${A}' "$render" "$scratch/realm.json")"
python3 - "$rendered" <<'EOF' || fail "escaped values should render to the expected JSON"
import json, sys
got = json.loads(sys.argv[1])
assert got == {"a": 'p\\q"r/&', "b": "${A}", "c": "plain default"}, got
EOF

# A value containing a newline (e.g. a multi-line secret) must not break the JSON.
printf '{"secret": "${SECRET}"}\n' >"$scratch/newline-realm.json"
rendered="$(env -i PATH="$PATH" SECRET=$'line one\nline two' "$render" "$scratch/newline-realm.json")"
python3 - "$rendered" <<'EOF' || fail "a newline in a value should be escaped, not break the JSON"
import json, sys
got = json.loads(sys.argv[1])
assert got == {"secret": "line one\nline two"}, got
EOF

# Every JSON control character (0x00-0x1F), not only newline/CR/tab, must be escaped.
printf '{"secret": "${SECRET}"}\n' >"$scratch/control-char-realm.json"
rendered="$(env -i PATH="$PATH" SECRET=$'back\bspace\x1bescape' "$render" "$scratch/control-char-realm.json")"
python3 - "$rendered" <<'EOF' || fail "a backspace/escape control character in a value should be escaped, not break the JSON"
import json, sys
got = json.loads(sys.argv[1])
assert got == {"secret": "back\bspace\x1bescape"}, got
EOF

if [ "$failures" -ne 0 ]; then
  echo "${failures} render-realm test(s) failed"
  exit 1
fi
echo "all render-realm tests passed"
