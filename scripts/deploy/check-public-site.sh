#!/usr/bin/env bash
#
# Smoke-check the public site (ECO-457).
#
# Usage:
#   scripts/deploy/check-public-site.sh --host econgraph.com             # local gateway
#   scripts/deploy/check-public-site.sh --host econgraph.com --via-cloudflare
#
# What it checks:
#   - the frontend, and a /graphql search sent with the public Origin;
#   - the Keycloak discovery document, whose issuer must be
#     https://HOST/idp/realms/econ-graph;
#   - the sign-in page for the web client: Keycloak accepts the public
#     redirect URI and does not answer "HTTPS required", and refuses a
#     http://localhost redirect URI (the live realm, not just the rendered one);
#   - CORS for the public origin, and the www.HOST redirect: a 301 to
#     https://HOST with the path and query kept and no port;
#   - spoofing: X-Forwarded-Proto/-Host/-Port sent by the client change
#     neither the issuer, the sign-in form, nor the redirect, and plain http
#     is redirected to https;
#   - not served on the public host: Keycloak's admin console and master
#     realm, metrics, the GraphQL playground and the admin API, including
#     encoded-traversal variants.
#
# Default (local) mode needs no DNS and no tunnel: curl --connect-to sends
# each request to the gateway on this host (LOCAL_INGRESS_IP, default
# 127.0.0.1) with the public Host header, over https to port 443
# (LOCAL_HTTPS_PORT), as cloudflared does (the gateway's certificate is not
# verified, like the tunnel's No TLS Verify). Plain http goes to port 80
# (LOCAL_HTTP_PORT).
# --via-cloudflare goes the way a visitor does: real DNS, through Cloudflare
# and the cloudflared tunnel.
#
# Read-only: GET, HEAD and OPTIONS requests plus one GraphQL query. Exits
# non-zero if any check fails.
set -euo pipefail

HOST=""
VIA_CLOUDFLARE=false
LOCAL_INGRESS_IP="${LOCAL_INGRESS_IP:-127.0.0.1}"
LOCAL_HTTPS_PORT="${LOCAL_HTTPS_PORT:-443}"
LOCAL_HTTP_PORT="${LOCAL_HTTP_PORT:-80}"
REALM="econ-graph"
CLIENT_ID="econ-graph-web"
SPOOF_HOST="spoofed.invalid"

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
            awk 'NR > 1 && !/^#/ { exit } NR > 1' "$0" | sed 's/^# \{0,1\}//'
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
for tool in curl python3; do
    command -v "$tool" >/dev/null || {
        echo "$tool not found" >&2
        exit 2
    }
done

PUBLIC_URL="https://${HOST}"
ISSUER="${PUBLIC_URL}/idp/realms/${REALM}"
BASE="https://${HOST}"
WWW_BASE="https://www.${HOST}"
HTTP_BASE="http://${HOST}"
CURL=(curl -sS --max-time 20)
if [ "$VIA_CLOUDFLARE" = true ]; then
    echo "==> Checking ${BASE} through Cloudflare"
else
    CURL+=(--insecure
        --connect-to "${HOST}:443:${LOCAL_INGRESS_IP}:${LOCAL_HTTPS_PORT}"
        --connect-to "www.${HOST}:443:${LOCAL_INGRESS_IP}:${LOCAL_HTTPS_PORT}"
        --connect-to "${HOST}:80:${LOCAL_INGRESS_IP}:${LOCAL_HTTP_PORT}")
    echo "==> Checking ${HOST} on the local gateway (${LOCAL_INGRESS_IP}), as the tunnel would send it"
fi
SPOOF=(-H "X-Forwarded-Proto: http" -H "X-Forwarded-Host: ${SPOOF_HOST}" -H "X-Forwarded-Port: 8443")

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
        # curl still prints the status when the transfer fails after the
        # response line arrived (a timeout or reset mid-body); 000 means none did.
        [ -n "$code" ] || code=000
    fi
    printf '%s' "$code"
}

# header NAME HEADER: the value of HEADER in $tmp/NAME.h (last response), or nothing.
header() {
    tr -d '\r' <"$tmp/$1.h" | awk -v h="$(printf '%s' "$2" | tr '[:upper:]' '[:lower:]')" \
        'BEGIN { FS = ": " } tolower($1) == h { v = substr($0, length($1) + 3) } END { print v }'
}

# json NAME KEY: a top-level string from the JSON body $tmp/NAME, or nothing.
json() {
    python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get(sys.argv[2],""))' \
        "$tmp/$1" "$2" 2>/dev/null || true
}

# 1. The frontend.
status="$(fetch home "${BASE}/")"
if [ "$status" = 200 ] && grep -q 'id="root"' "$tmp/home"; then
    pass "GET / serves the app (200)"
else
    fail "GET / returned ${status}, expected 200 with the app's HTML"
fi

# 2. A /graphql search, with the Origin a browser on the public site sends.
status="$(fetch search -X POST -H 'Content-Type: application/json' \
    -H "Origin: ${PUBLIC_URL}" --data '{"query":"{ searchSeries(query: \"gdp\", first: 1) { totalCount } }"}' \
    "${BASE}/graphql")"
if [ "$status" = 200 ] && python3 -c '
import json, sys
body = json.load(open(sys.argv[1]))
sys.exit(0 if not body.get("errors") and isinstance(body["data"]["searchSeries"]["totalCount"], int) else 1)
' "$tmp/search" 2>/dev/null; then
    pass "POST /graphql searchSeries answers"
else
    fail "POST /graphql searchSeries returned ${status}: $(head -c 300 "$tmp/search" 2>/dev/null)"
fi

# 3. Keycloak discovery: the issuer the backend checks tokens against.
status="$(fetch discovery "${BASE}/idp/realms/${REALM}/.well-known/openid-configuration")"
issuer="$(json discovery issuer)"
auth_endpoint="$(json discovery authorization_endpoint)"
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
# accept the redirect URI (realm import) and not refuse with "HTTPS required".
# The PKCE challenge is a fixed dummy; nothing is signed in.
login_path="/idp/realms/${REALM}/protocol/openid-connect/auth?client_id=${CLIENT_ID}"
login_path+="&redirect_uri=$(python3 -c 'import sys, urllib.parse; print(urllib.parse.quote(sys.argv[1], safe=""))' "${PUBLIC_URL}/auth/callback")"
login_path+="&response_type=code&scope=openid&state=smoke&code_challenge_method=S256"
login_path+="&code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
status="$(fetch login "${BASE}${login_path}")"
if [ "$status" = 200 ] && grep -q 'id="kc-' "$tmp/login" &&
    ! grep -qi 'HTTPS required\|Invalid parameter: redirect_uri' "$tmp/login"; then
    pass "sign-in page accepts ${PUBLIC_URL}/auth/callback"
    if grep -q "action=\"${PUBLIC_URL}/idp/" "$tmp/login"; then
        pass "sign-in form posts to ${PUBLIC_URL}"
    else
        fail "sign-in form does not post to ${PUBLIC_URL}/idp/..."
    fi
else
    reason="$(grep -o 'HTTPS required\|Invalid parameter: redirect_uri\|Client not found' "$tmp/login" 2>/dev/null | head -1 || true)"
    fail "sign-in page returned ${status}${reason:+ (${reason})}"
fi

# 4b. A localhost redirect URI is refused: the production realm accepts the public host
# only (ECO-753). This checks the live client, whatever an earlier import left on it.
local_login_path="${login_path%%&redirect_uri=*}&redirect_uri=$(python3 -c 'import sys, urllib.parse; print(urllib.parse.quote(sys.argv[1], safe=""))' "http://localhost/auth/callback")&${login_path#*&redirect_uri=*&}"
status="$(fetch local-login "${BASE}${local_login_path}")"
if grep -q 'Invalid parameter: redirect_uri' "$tmp/local-login"; then
    pass "sign-in page refuses http://localhost/auth/callback"
else
    fail "sign-in page returned ${status} for http://localhost/auth/callback; the realm still accepts localhost"
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

# 6. www redirects to the apex on https, keeping the path and query, with no port
# spelled out: econ-graph-www-redirect's 301 (scripts/deploy/render-public-host.py).
# Any other status means something else answered, such as an old ingress or a
# Cloudflare redirect rule.
WWW_TARGET="${PUBLIC_URL}/series/smoke-check?from=www"
# Until production is cut over to the gateway, and only until then (ECO-800 removes
# this): ingress-nginx still answers www with a 308 that spells out :443 (ECO-709).
# That exact response, with nginx's own redirect page as its body (which a Cloudflare
# redirect rule would not send), is reported as known; any other one fails.
INGRESS_NGINX_WWW_TARGET="https://${HOST}:443/series/smoke-check?from=www"
# ingress_nginx_www NAME STATUS LOCATION: the response saved as NAME is ingress-nginx's.
ingress_nginx_www() {
    [ "$2" = 308 ] && [ "$3" = "$INGRESS_NGINX_WWW_TARGET" ] &&
        grep -qF '<center>nginx</center>' "$tmp/$1"
}
status="$(fetch www "${WWW_BASE}/series/smoke-check?from=www")"
location="$(header www Location)"
if [ "$status" = 301 ] && [ "$location" = "$WWW_TARGET" ]; then
    pass "www.${HOST} redirects (301) to ${location}"
elif ingress_nginx_www www "$status" "$location"; then
    pass "www.${HOST} redirects (308) through ingress-nginx, with :443 (ECO-709: production not yet cut over to the gateway)"
else
    fail "www.${HOST} redirects (${status}) to '${location}', expected a 301 to ${WWW_TARGET}"
fi

# 7. Spoofing. A client's X-Forwarded-Proto/-Host/-Port must not reach anything
# that builds URLs from them: the gateway drops them and sets the scheme from the
# connection (k8s/manifests/gateway/client-traffic-policy.yaml), and Keycloak
# uses its fixed KC_HOSTNAME.
status="$(fetch spoof-discovery "${SPOOF[@]}" "${BASE}/idp/realms/${REALM}/.well-known/openid-configuration")"
if [ "$(json spoof-discovery issuer)" = "$ISSUER" ] && ! grep -q "$SPOOF_HOST\|:8443" "$tmp/spoof-discovery"; then
    pass "spoofed X-Forwarded-* headers leave the discovery document alone"
else
    fail "with spoofed X-Forwarded-* headers discovery returned ${status}, issuer '$(json spoof-discovery issuer)'"
fi
status="$(fetch spoof-login "${SPOOF[@]}" "${BASE}${login_path}")"
if [ "$status" = 200 ] && grep -q "action=\"${PUBLIC_URL}/idp/" "$tmp/spoof-login" &&
    ! grep -q "$SPOOF_HOST\|:8443" "$tmp/spoof-login"; then
    pass "spoofed X-Forwarded-* headers leave the sign-in form alone"
else
    fail "with spoofed X-Forwarded-* headers the sign-in page returned ${status} or changed its form target"
fi
status="$(fetch spoof-www "${SPOOF[@]}" "${WWW_BASE}/series/smoke-check?from=www")"
location="$(header spoof-www Location)"
if [ "$status" = 301 ] && [ "$location" = "$WWW_TARGET" ]; then
    pass "spoofed X-Forwarded-* headers leave the www redirect on ${PUBLIC_URL}"
elif ingress_nginx_www spoof-www "$status" "$location"; then
    pass "spoofed X-Forwarded-* headers leave ingress-nginx's www redirect on ${HOST}:443 (ECO-709: production not yet cut over to the gateway)"
else
    fail "with spoofed X-Forwarded-* headers www redirects (${status}) to '${location}'"
fi
# Plain http, claiming to be https: still redirected to https, never served.
status="$(fetch plain -H "X-Forwarded-Proto: https" "${HTTP_BASE}/idp/realms/${REALM}/.well-known/openid-configuration")"
location="$(header plain Location)"
case "$status" in
    301 | 302 | 307 | 308)
        case "$location" in
            "${PUBLIC_URL}/"*) pass "plain http (even claiming https) is redirected to ${PUBLIC_URL}" ;;
            *) fail "plain http redirects (${status}) to '${location}', not to ${PUBLIC_URL}" ;;
        esac
        ;;
    *)
        if [ "$VIA_CLOUDFLARE" = true ]; then
            fail "plain http returned ${status}: turn on Cloudflare's Always Use HTTPS"
        else
            fail "plain http with X-Forwarded-Proto: https returned ${status}, expected a redirect to https"
        fi
        ;;
esac

# 8. Not served on the public host. An unrouted path falls through to the app,
# which answers with its own HTML (id="root"); anything else that answers 2xx,
# or any body that looks like Keycloak's admin or master realm, metrics or a
# GraphQL playground, fails. So does no response at all (000: timeout, reset):
# that shows nothing about whether the path is blocked.
for path in \
    /idp/admin/ \
    /idp/admin/master/console/ \
    /idp/realms/master/.well-known/openid-configuration \
    /idp/realms/master/protocol/openid-connect/auth \
    /idp/realms/econ-graph/../master/.well-known/openid-configuration \
    /idp/realms/econ-graph/%2e%2e/master/.well-known/openid-configuration \
    /idp/realms/econ-graph/..%2fmaster/.well-known/openid-configuration \
    /idp/realms/econ-graph/%2e%2e%2fmaster/.well-known/openid-configuration \
    /idp/resources/..%2f..%2fadmin/master/console/ \
    /idp/resources/%2e%2e/%2e%2e/admin/ \
    /metrics \
    /api/metrics \
    /playground \
    /api/admin \
    /api/admin/users \
    /admin \
    /%2e%2e/playground; do
    status="$(fetch neg --path-as-is "${BASE}${path}")"
    if [ "$status" = 000 ]; then
        fail "${path} got no HTTP response, so whether it is blocked is unknown"
    elif grep -qi 'realms/master\|"realm" *: *"master"\|^# HELP\|^# TYPE\|kc-form\|keycloak\|graphiql\|graphql-playground\|playground' "$tmp/neg"; then
        fail "${path} is served on the public host (${status})"
    elif [[ "$status" == 2* ]] && ! grep -q 'id="root"' "$tmp/neg"; then
        fail "${path} answered ${status} with something other than the app"
    else
        pass "${path} not served (${status})"
    fi
done

if [ "$failures" -gt 0 ]; then
    echo "==> ${failures} check(s) failed"
    exit 1
fi
echo "==> All checks passed for ${HOST}"
