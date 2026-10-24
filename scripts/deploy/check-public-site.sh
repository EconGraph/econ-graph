#!/usr/bin/env bash
#
# Smoke-check the public site (ECO-457): the frontend, a /graphql search, the
# Keycloak discovery document (its issuer must be https://HOST/idp/realms/econ-graph),
# the sign-in page for the web client (Keycloak accepts the public redirect URI and
# does not refuse with "HTTPS required"), CORS for the public origin, and the
# www.HOST redirect.
#
# Usage:
#   scripts/deploy/check-public-site.sh --host econgraph.com             # local ingress
#   scripts/deploy/check-public-site.sh --host econgraph.com --via-cloudflare
#
# Default (local) mode needs no DNS and no tunnel: curl --resolve sends each request to
# the kind ingress on this host (LOCAL_INGRESS_IP, default 127.0.0.1, port 80) with the
# public Host header, and adds the headers Cloudflare's edge adds (X-Forwarded-Proto:
# https, CF-Connecting-IP), so it checks the cluster's public-host config before the
# tunnel is up. --via-cloudflare goes the way a visitor does: real DNS, https, through
# Cloudflare and the cloudflared tunnel.
#
# Read-only: GET, HEAD and OPTIONS requests plus one GraphQL query. Exits non-zero if
# any check fails.
set -euo pipefail

HOST=""
VIA_CLOUDFLARE=false
LOCAL_INGRESS_IP="${LOCAL_INGRESS_IP:-127.0.0.1}"
REALM="econ-graph"
CLIENT_ID="econ-graph-web"

while [ $# -gt 0 ]; do
    case "$1" in
        --host)
            HOST="${2:-}"
            shift 2
            ;;
        --host=*)
            HOST="${1#--host=}"
            shift
            ;;
        --via-cloudflare)
            VIA_CLOUDFLARE=true
            shift
            ;;
        -h | --help)
            sed -n '2,22p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "unknown argument: $1 (see --help)" >&2
            exit 2
            ;;
    esac
done
[ -n "$HOST" ] || {
    echo "usage: $0 --host HOST [--via-cloudflare]" >&2
    exit 2
}
command -v curl >/dev/null || {
    echo "curl not found" >&2
    exit 2
}
command -v python3 >/dev/null || {
    echo "python3 not found" >&2
    exit 2
}

PUBLIC_URL="https://${HOST}"
ISSUER="${PUBLIC_URL}/idp/realms/${REALM}"
CURL=(curl -sS --max-time 20)
if [ "$VIA_CLOUDFLARE" = true ]; then
    BASE="https://${HOST}"
    WWW_BASE="https://www.${HOST}"
    echo "==> Checking ${BASE} through Cloudflare"
else
    BASE="http://${HOST}"
    WWW_BASE="http://www.${HOST}"
    CURL+=(--resolve "${HOST}:80:${LOCAL_INGRESS_IP}" --resolve "www.${HOST}:80:${LOCAL_INGRESS_IP}"
        -H "X-Forwarded-Proto: https" -H "CF-Connecting-IP: 192.0.2.10")
    echo "==> Checking ${HOST} on the local ingress (${LOCAL_INGRESS_IP}:80), as the tunnel would send it"
fi

failures=0
pass() { echo "  ok    $1"; }
fail() {
    echo "  FAIL  $1"
    failures=$((failures + 1))
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# fetch NAME [curl args...]: body to $tmp/NAME, headers to $tmp/NAME.h; prints the status.
fetch() {
    local name="$1" code
    shift
    : >"$tmp/$name"
    : >"$tmp/$name.h"
    if ! code="$("${CURL[@]}" -o "$tmp/$name" -D "$tmp/$name.h" -w '%{http_code}' "$@" 2>"$tmp/$name.err")"; then
        sed 's/^/        /' "$tmp/$name.err" >&2
        code=000
    fi
    printf '%s' "$code"
}

# header NAME HEADER: the value of HEADER in $tmp/NAME.h (last response), or nothing.
header() {
    tr -d '\r' <"$tmp/$1.h" | awk -v h="$(printf '%s' "$2" | tr '[:upper:]' '[:lower:]')" \
        'BEGIN { FS = ": " } tolower($1) == h { v = substr($0, length($1) + 3) } END { print v }'
}

# 1. The frontend.
status="$(fetch home "${BASE}/")"
if [ "$status" = 200 ] && grep -q 'id="root"' "$tmp/home"; then
    pass "GET / serves the app (200)"
else
    fail "GET / returned ${status}, expected 200 with the app's HTML"
fi

# 2. A /graphql search.
status="$(fetch search -X POST -H 'Content-Type: application/json' \
    -H "Origin: ${PUBLIC_URL}" --data '{"query":"{ searchSeries(query: \"gdp\", first: 1) { totalCount } }"}' \
    "${BASE}/graphql")"
if [ "$status" = 200 ] && python3 -c '
import json, sys
body = json.load(open(sys.argv[1]))
sys.exit(0 if not body.get("errors") and isinstance(body["data"]["searchSeries"]["totalCount"], int) else 1)
' "$tmp/search" 2>/dev/null; then
    pass "POST /graphql searchSeries answers ($(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["data"]["searchSeries"]["totalCount"])' "$tmp/search") results for gdp)"
else
    fail "POST /graphql searchSeries returned ${status}: $(head -c 300 "$tmp/search" 2>/dev/null)"
fi

# 3. Keycloak discovery: the issuer the backend checks tokens against.
status="$(fetch discovery "${BASE}/idp/realms/${REALM}/.well-known/openid-configuration")"
issuer=""
auth_endpoint=""
if [ "$status" = 200 ]; then
    issuer="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("issuer",""))' "$tmp/discovery" 2>/dev/null || true)"
    auth_endpoint="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("authorization_endpoint",""))' "$tmp/discovery" 2>/dev/null || true)"
fi
if [ "$issuer" = "$ISSUER" ]; then
    pass "discovery issuer is ${ISSUER}"
else
    fail "discovery returned ${status} with issuer '${issuer}', expected ${ISSUER}"
fi
case "$auth_endpoint" in
    "${ISSUER}/"*) pass "authorization endpoint is on ${PUBLIC_URL}" ;;
    *) fail "authorization endpoint '${auth_endpoint}' is not under ${ISSUER}" ;;
esac

# 4. The sign-in page for the web client with the public redirect URI: Keycloak must
# accept the redirect URI (realm import) and see https (no "HTTPS required").
# The PKCE challenge is a fixed dummy; nothing is signed in.
login_path="/idp/realms/${REALM}/protocol/openid-connect/auth?client_id=${CLIENT_ID}"
login_path+="&redirect_uri=$(python3 -c 'import sys, urllib.parse; print(urllib.parse.quote(sys.argv[1], safe=""))' "${PUBLIC_URL}/auth/callback")"
login_path+="&response_type=code&scope=openid&state=smoke&code_challenge_method=S256"
login_path+="&code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
status="$(fetch login "${BASE}${login_path}")"
if [ "$status" = 200 ] && grep -q 'kc-form-login\|id="kc-' "$tmp/login" &&
    ! grep -qi 'HTTPS required\|Invalid parameter: redirect_uri' "$tmp/login"; then
    pass "sign-in page accepts ${PUBLIC_URL}/auth/callback over https"
    if grep -q "action=\"${PUBLIC_URL}/idp/" "$tmp/login"; then
        pass "sign-in form posts to ${PUBLIC_URL}"
    else
        fail "sign-in form does not post to ${PUBLIC_URL}/idp/..."
    fi
else
    reason="$(grep -o 'HTTPS required\|Invalid parameter: redirect_uri\|Client not found' "$tmp/login" 2>/dev/null | head -1 || true)"
    fail "sign-in page returned ${status}${reason:+ (${reason})}"
fi

# 5. CORS: the API answers a preflight from the public origin.
status="$(fetch cors -X OPTIONS -H "Origin: ${PUBLIC_URL}" \
    -H 'Access-Control-Request-Method: POST' \
    -H 'Access-Control-Request-Headers: content-type, authorization' "${BASE}/graphql")"
allowed="$(header cors Access-Control-Allow-Origin)"
if [ "$allowed" = "$PUBLIC_URL" ]; then
    pass "CORS preflight from ${PUBLIC_URL} allowed"
else
    fail "CORS preflight returned ${status}, Access-Control-Allow-Origin '${allowed}', expected ${PUBLIC_URL}"
fi

# 6. www redirects to the apex, keeping the path, on https.
status="$(fetch www "${WWW_BASE}/series/smoke-check")"
location="$(header www Location)"
case "$status" in
    301 | 308)
        if [ "$location" = "${PUBLIC_URL}/series/smoke-check" ]; then
            pass "www.${HOST} redirects (${status}) to ${location}"
        else
            fail "www.${HOST} redirects (${status}) to '${location}', expected ${PUBLIC_URL}/series/smoke-check"
        fi
        ;;
    *) fail "www.${HOST} returned ${status}, expected a 301/308 redirect to ${PUBLIC_URL}" ;;
esac

if [ "$failures" -gt 0 ]; then
    echo "==> ${failures} check(s) failed"
    exit 1
fi
echo "==> All checks passed for ${HOST}"
