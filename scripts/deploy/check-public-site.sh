#!/usr/bin/env bash
#
# Smoke-check the public site (ECO-457).
#
# Usage:
#   scripts/deploy/check-public-site.sh --host econgraph.com             # local gateway
#   scripts/deploy/check-public-site.sh --host econgraph.com --via-cloudflare
#   scripts/deploy/check-public-site.sh --host econgraph.com --main-host econgraph.com # apex rollback
#
# MAIN_HOST defaults to www.HOST; --main-host overrides the environment.
# The redirects to MAIN_HOST are permanent (301) when MAIN_HOST is HOST, or when
# MAIN_HOST_PERMANENT names MAIN_HOST (scripts/deploy/switch-main-host.sh
# --finalize); otherwise temporary (302). render-public-host.py uses the same rule.
# MAIN_HOST_PERMANENT comes from the environment, else from the host's deploy config
# when its PUBLIC_HOST is HOST or unset (as lib/public-host.sh's main_host_init reads it).
#
# What it checks:
#   - the frontend, and a /graphql search sent with the public Origin;
#   - the Keycloak discovery document, whose issuer must be
#     https://MAIN_HOST/idp/realms/econ-graph;
#   - the sign-in page for the web client: Keycloak accepts the public
#     redirect URI and does not answer "HTTPS required", and refuses a
#     http://localhost redirect URI (the live realm, not just the rendered one);
#   - CORS for both public origins, and no CORS for origins the backend must
#     refuse (the dev server's http://localhost:3000, an unrelated
#     https://evil.example): the backend's 403 with no Access-Control-Allow-Origin,
#     so a backend still on an older origin list fails;
#   - HTTP on both names and HTTPS on the alternate name redirect (301 or 302,
#     as above) to https://MAIN_HOST with the path and query kept and no port;
#   - spoofing: X-Forwarded-Proto/-Host/-Port sent by the client change
#     neither the issuer, the sign-in form, nor the redirect, and plain http
#     is redirected to https: by the gateway straight to https://MAIN_HOST locally,
#     and through Cloudflare by its Always Use HTTPS (a 301 to https on the same
#     name), whose https request is then checked;
#   - until the gateway cut-over (ECO-800), ingress-nginx's 308 with :443 for the
#     alternate name is reported as the known ECO-709 case;
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
MAIN_HOST="${MAIN_HOST:-}"
MAIN_HOST_PERMANENT_SET="${MAIN_HOST_PERMANENT+set}"
MAIN_HOST_PERMANENT="${MAIN_HOST_PERMANENT:-}"
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
            [ $# -ge 2 ] || { echo "--host needs a value" >&2; exit 2; }
            HOST="$2"
            shift 2
            ;;
        --host=*)
            HOST="${1#--host=}"
            shift
            ;;
        --main-host)
            [ $# -ge 2 ] || { echo "--main-host needs a value" >&2; exit 2; }
            MAIN_HOST="$2"
            shift 2
            ;;
        --main-host=*)
            MAIN_HOST="${1#--main-host=}"
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
    echo "usage: $0 --host HOST [--main-host HOST] [--via-cloudflare]" >&2
    exit 2
}
HOST="${HOST,,}"
MAIN_HOST="${MAIN_HOST:-www.${HOST}}"
MAIN_HOST="${MAIN_HOST,,}"
case "$MAIN_HOST" in
    "$HOST") ALTERNATE_HOST="www.${HOST}" ;;
    "www.${HOST}") ALTERNATE_HOST="$HOST" ;;
    *) echo "--main-host must be ${HOST} or www.${HOST}" >&2; exit 2 ;;
esac
for tool in curl python3; do
    command -v "$tool" >/dev/null || {
        echo "$tool not found" >&2
        exit 2
    }
done

if [ -z "${MAIN_HOST_PERMANENT_SET:-}" ]; then
    # shellcheck source=scripts/deploy/lib/deploy-config.sh
    source "$(dirname "${BASH_SOURCE[0]}")/lib/deploy-config.sh"
    configured_base="$(deploy_config_value PUBLIC_HOST "host name")" || exit 2
    if [ -z "$configured_base" ] || [ "${configured_base,,}" = "$HOST" ]; then
        MAIN_HOST_PERMANENT="$(deploy_config_value MAIN_HOST_PERMANENT "host name")" || exit 2
    fi
fi
if [ "$MAIN_HOST" = "$HOST" ] || [ "${MAIN_HOST_PERMANENT,,}" = "$MAIN_HOST" ]; then
    REDIRECT_STATUS=301
else
    REDIRECT_STATUS=302
fi
PUBLIC_URL="https://${MAIN_HOST}"
ISSUER="${PUBLIC_URL}/idp/realms/${REALM}"
BASE="$PUBLIC_URL"
ALTERNATE_BASE="https://${ALTERNATE_HOST}"
CURL=(curl -sS --max-time 20)
if [ "$VIA_CLOUDFLARE" = true ]; then
    echo "==> Checking ${BASE} through Cloudflare"
else
    CURL+=(--insecure
        --connect-to "${HOST}:443:${LOCAL_INGRESS_IP}:${LOCAL_HTTPS_PORT}"
        --connect-to "www.${HOST}:443:${LOCAL_INGRESS_IP}:${LOCAL_HTTPS_PORT}"
        --connect-to "${HOST}:80:${LOCAL_INGRESS_IP}:${LOCAL_HTTP_PORT}"
        --connect-to "www.${HOST}:80:${LOCAL_INGRESS_IP}:${LOCAL_HTTP_PORT}")
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
if [ "$status" = 200 ] && [ "$issuer" = "$ISSUER" ]; then
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

# 5. Both public names remain allowed as CORS origins during the host switch.
# The request URL is canonical, independent of the Origin being checked.
for origin in "https://${HOST}" "https://www.${HOST}"; do
    status="$(fetch cors -X OPTIONS -H "Origin: ${origin}" \
        -H 'Access-Control-Request-Method: POST' \
        -H 'Access-Control-Request-Headers: content-type, authorization' "${BASE}/graphql")"
    allowed="$(header cors Access-Control-Allow-Origin)"
    if [[ "$status" == 2* ]] && [ "$allowed" = "$origin" ]; then
        pass "CORS preflight from ${origin} allowed"
    else
        fail "CORS preflight returned ${status}, Access-Control-Allow-Origin '${allowed}', expected ${origin}"
    fi
done
# 5b. Origins the backend must not list get no Access-Control-Allow-Origin.
for refused_origin in http://localhost:3000 https://evil.example; do
    status="$(fetch cors-refused -X OPTIONS -H "Origin: ${refused_origin}" \
        -H 'Access-Control-Request-Method: POST' \
        -H 'Access-Control-Request-Headers: content-type, authorization' "${BASE}/graphql")"
    allowed="$(header cors-refused Access-Control-Allow-Origin)"
    # The backend answers a preflight from an origin it doesn't list with a 403; any other
    # status (none, a 404 or 5xx) means the backend didn't answer, which isn't a refusal.
    if [ -n "$allowed" ]; then
        fail "CORS preflight from ${refused_origin} returned ${status} with Access-Control-Allow-Origin '${allowed}'; the backend allows an origin it must not"
    elif [ "$status" = 403 ]; then
        pass "CORS preflight from ${refused_origin} refused (403)"
    else
        fail "CORS preflight from ${refused_origin} returned ${status}, expected the backend's 403"
    fi
done

# 6. The alternate HTTPS host redirects to the canonical origin exactly once, with
# render-public-host.py's status, keeping the path and query and spelling out no port.
# The frontend check above verifies that canonical HTTPS serves, rather than redirects.
# Any other answer means something else answered, such as an old ingress or a
# Cloudflare redirect rule.
REDIRECT_PATH='/series/smoke-check?from=alternate'
REDIRECT_TARGET="${PUBLIC_URL}${REDIRECT_PATH}"
# Until production is cut over to the gateway, and only until then (ECO-800 removes
# this): ingress-nginx still answers the alternate name with a 308 to the main host that
# spells out :443 (ECO-709). That exact response, with nginx's own redirect page as its
# body (which a Cloudflare redirect rule would not send), is reported as known; any
# other one fails.
INGRESS_NGINX_TARGET="https://${MAIN_HOST}:443${REDIRECT_PATH}"
# ingress_nginx_redirect NAME STATUS LOCATION: the response saved as NAME is ingress-nginx's.
ingress_nginx_redirect() {
    [ "$2" = 308 ] && [ "$3" = "$INGRESS_NGINX_TARGET" ] &&
        grep -qF '<center>nginx</center>' "$tmp/$1"
}
status="$(fetch alternate "${ALTERNATE_BASE}${REDIRECT_PATH}")"
location="$(header alternate Location)"
if [ "$status" = "$REDIRECT_STATUS" ] && [ "$location" = "$REDIRECT_TARGET" ]; then
    pass "${ALTERNATE_HOST} redirects (${status}) to ${location}"
elif ingress_nginx_redirect alternate "$status" "$location"; then
    pass "${ALTERNATE_HOST} redirects (308) through ingress-nginx, with :443 (ECO-709: production not yet cut over to the gateway)"
else
    fail "${ALTERNATE_HOST} redirects (${status}) to '${location}', expected a ${REDIRECT_STATUS} to ${REDIRECT_TARGET}"
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
status="$(fetch spoof-alternate "${SPOOF[@]}" "${ALTERNATE_BASE}${REDIRECT_PATH}")"
location="$(header spoof-alternate Location)"
if [ "$status" = "$REDIRECT_STATUS" ] && [ "$location" = "$REDIRECT_TARGET" ]; then
    pass "spoofed X-Forwarded-* headers leave the alternate redirect on ${PUBLIC_URL}"
elif ingress_nginx_redirect spoof-alternate "$status" "$location"; then
    pass "spoofed X-Forwarded-* headers leave ingress-nginx's alternate redirect on ${MAIN_HOST}:443 (ECO-709: production not yet cut over to the gateway)"
else
    fail "with spoofed X-Forwarded-* headers ${ALTERNATE_HOST} redirects (${status}) to '${location}'"
fi
# Plain http on both names, even claiming to be https, is redirected to canonical
# https, never served. On the local gateway it is the gateway's own redirect, straight
# to https://MAIN_HOST. Through Cloudflare, plain http never reaches the gateway (the
# tunnel's origin is the https listener): Cloudflare's Always Use HTTPS answers with a
# 301 to https on the same name, and that https request must then be the main host's
# own page or the alternate name's redirect above.
for name in "$HOST" "www.${HOST}"; do
    status="$(fetch plain -H "X-Forwarded-Proto: https" "http://${name}${REDIRECT_PATH}")"
    location="$(header plain Location)"
    if [ "$status" = "$REDIRECT_STATUS" ] && [ "$location" = "$REDIRECT_TARGET" ]; then
        pass "plain http://${name} (even claiming https) redirects (${status}) to ${location}"
    elif [ "$VIA_CLOUDFLARE" = true ] && [ "$status" = 301 ] && [ "$location" = "https://${name}${REDIRECT_PATH}" ]; then
        status="$(fetch plain-https "https://${name}${REDIRECT_PATH}")"
        location="$(header plain-https Location)"
        if [ "$name" = "$MAIN_HOST" ] && [ "$status" = 200 ] && grep -q 'id="root"' "$tmp/plain-https"; then
            pass "plain http://${name} (even claiming https) redirects (301, Cloudflare) to https://${name}, which serves the app"
        elif [ "$name" != "$MAIN_HOST" ] && [ "$status" = "$REDIRECT_STATUS" ] && [ "$location" = "$REDIRECT_TARGET" ]; then
            pass "plain http://${name} (even claiming https) redirects (301, Cloudflare) to https://${name}, then (${status}) to ${location}"
        elif [ "$name" != "$MAIN_HOST" ] && ingress_nginx_redirect plain-https "$status" "$location"; then
            pass "plain http://${name} redirects (301, Cloudflare) to https://${name}, then (308) through ingress-nginx (ECO-709: production not yet cut over to the gateway)"
        else
            fail "plain http://${name} redirects (301, Cloudflare) to https://${name}, which answers ${status}${location:+ to ${location}}; expected $([ "$name" = "$MAIN_HOST" ] && echo 'the app' || echo "a ${REDIRECT_STATUS} to ${REDIRECT_TARGET}")"
        fi
    elif [ "$VIA_CLOUDFLARE" = true ] && [[ "$status" != 3* ]]; then
        fail "plain http://${name} returned ${status}: turn on Cloudflare's Always Use HTTPS"
    else
        fail "plain http://${name} redirects (${status}) to '${location}', expected a ${REDIRECT_STATUS} to ${REDIRECT_TARGET}$([ "$VIA_CLOUDFLARE" = true ] && echo ", or Cloudflare's 301 to https://${name}${REDIRECT_PATH}")"
    fi
done

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
echo "==> All checks passed for ${MAIN_HOST}"
