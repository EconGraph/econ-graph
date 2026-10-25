#!/bin/bash
#
# Check every route of the gateway (k8s/manifests/gateway/routes/) from the
# outside: each path reaches the right Service, the routes that must not reach
# Keycloak don't, /data/graphql drops credentials, and the frontend's security
# headers are set. Read-only apart from the requests themselves.
#
# Usage: scripts/deploy/test-gateway-routes.sh [--rate-limit] [BASE_URL]
#
#   BASE_URL      Where the gateway answers (default http://localhost, the kind
#                 host ports). For the proxy Service before it holds the host
#                 ports, port-forward it first, e.g.
#                   kubectl -n envoy-gateway-system port-forward svc/econ-graph-gateway-proxy 18080:80
#                 and pass http://127.0.0.1:18080.
#   --rate-limit  Also send bursts to /graphql and /data/graphql: the first
#                 100 requests from one client pass, a burst beyond that gets
#                 429s. This uses up this client's tokens for a few seconds.
#
# Environment:
#   ROUTE_HOST       Host header sent (default localhost: the routes' host name).
#   EXPECTED_ISSUER  Keycloak issuer expected in the discovery document
#                    (default http://localhost/idp/realms/econ-graph).
#   SKIP_KEYCLOAK    true: skip the /idp checks (Keycloak not deployed).
#
# Exit status 0 when every check passes.

set -uo pipefail

RATE_LIMIT=false
BASE_URL=""
for arg in "$@"; do
    case "$arg" in
        --rate-limit) RATE_LIMIT=true ;;
        -h | --help)
            sed -n '2,/^$/p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        -*)
            echo "error: unknown option: $arg" >&2
            exit 2
            ;;
        *) BASE_URL="$arg" ;;
    esac
done
BASE_URL="${BASE_URL:-http://localhost}"
BASE_URL="${BASE_URL%/}"
ROUTE_HOST="${ROUTE_HOST:-localhost}"
EXPECTED_ISSUER="${EXPECTED_ISSUER:-http://localhost/idp/realms/econ-graph}"
SKIP_KEYCLOAK="${SKIP_KEYCLOAK:-false}"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

passed=0
failed=0
pass() {
    echo "  ok    $*"
    passed=$((passed + 1))
}
fail() {
    echo "  FAIL  $*"
    failed=$((failed + 1))
}

# req NAME METHOD PATH [curl args...]: response body in $WORK/NAME.body,
# headers in $WORK/NAME.headers; prints the status code.
req() {
    local name="$1" method="$2" path="$3"
    shift 3
    : >"$WORK/$name.body"
    : >"$WORK/$name.headers"
    # On a connection failure curl still prints 000 as the code.
    curl -sS -o "$WORK/$name.body" -D "$WORK/$name.headers" -w '%{http_code}' \
        --max-time 20 -X "$method" -H "Host: ${ROUTE_HOST}" "$@" "${BASE_URL}${path}" 2>"$WORK/$name.err" || true
}
header() { grep -i "^$2:" "$WORK/$1.headers" | tail -1 | cut -d: -f2- | sed 's/^ *//; s/\r$//'; }
is_html() { grep -qi '^content-type: *text/html' "$WORK/$1.headers"; }
# NAME CODE: the response is the frontend's single-page app, exactly as served
# for / (fetched first): 200, HTML, no redirect, same body.
is_frontend_page() {
    [ "$2" = 200 ] && is_html "$1" && [ -z "$(header "$1" Location)" ] &&
        cmp -s "$WORK/$1.body" "$WORK/root.body"
}

GQL=(-H 'Content-Type: application/json' --data '{"query":"{ __typename }"}')

echo "Gateway routes at ${BASE_URL} (Host: ${ROUTE_HOST})"

# econ-graph-frontend
code="$(req root GET /)"
if [ "$code" = 200 ] && is_html root; then pass "/ -> frontend (200, HTML)"; else fail "/ -> frontend: got $code"; fi
if [ "$(header root X-Frame-Options)" = DENY ] &&
    header root Content-Security-Policy | grep -q "frame-ancestors 'none'" &&
    [ -n "$(header root Strict-Transport-Security)" ]; then
    pass "/ has the security headers (X-Frame-Options DENY, CSP, HSTS)"
else
    fail "/ security headers: X-Frame-Options='$(header root X-Frame-Options)' CSP='$(header root Content-Security-Policy)'"
fi
code="$(req callback GET /auth/callback)"
if [ "$code" = 200 ] && is_html callback; then pass "/auth/callback -> frontend (200, HTML)"; else fail "/auth/callback -> frontend: got $code"; fi
code="$(req silent GET /silent-callback.html)"
if [ "$code" = 200 ] && [ "$(header silent X-Frame-Options)" = SAMEORIGIN ] &&
    header silent Content-Security-Policy | grep -q "frame-ancestors 'self'"; then
    pass "/silent-callback.html -> frontend, frameable by this origin only"
else
    fail "/silent-callback.html: got $code, X-Frame-Options='$(header silent X-Frame-Options)'"
fi

# econ-graph-backend
code="$(req health GET /health)"
if [ "$code" = 200 ] && ! is_html health && grep -qi 'status' "$WORK/health.body"; then
    pass "/health -> backend (200)"
    if [ "$(header health X-Content-Type-Options)" = nosniff ] && [ "$(header health X-Frame-Options)" = DENY ]; then
        pass "/health has X-Content-Type-Options nosniff and X-Frame-Options DENY"
    else
        fail "/health headers: X-Content-Type-Options='$(header health X-Content-Type-Options)' X-Frame-Options='$(header health X-Frame-Options)'"
    fi
else
    fail "/health -> backend: got $code"
fi
code="$(req api GET /api/no-such-endpoint)"
if [ "$code" = 404 ] && ! is_html api; then pass "/api/... -> backend (404 from the backend, not the frontend's page)"; else fail "/api/... -> backend: got $code"; fi
code="$(req auth GET /auth/login)"
if [ "$code" = 404 ] && ! is_html auth; then pass "/auth/... -> backend (404 from the backend)"; else fail "/auth/... -> backend: got $code"; fi
code="$(req playground GET /playground)"
if is_frontend_page playground "$code"; then pass "/playground -> the frontend's page, not the backend"; else fail "/playground: expected the frontend's page, got $code"; fi

# econ-graph-graphql
code="$(req gql POST /graphql "${GQL[@]}")"
if [ "$code" = 200 ] && grep -q '"__typename"' "$WORK/gql.body"; then pass "/graphql -> backend GraphQL (200)"; else fail "/graphql: got $code $(head -c 200 "$WORK/gql.body")"; fi
code="$(req gqlbogus POST /graphql -H 'Authorization: Bearer not-a-real-token' "${GQL[@]}")"
if [ "$code" = 401 ]; then pass "/graphql with a bogus bearer token -> 401 (the token reaches the backend)"; else fail "/graphql with a bogus token: expected 401, got $code"; fi

# econ-graph-data-graphql
code="$(req data POST /data/graphql "${GQL[@]}")"
if [ "$code" = 200 ] && grep -q '"__typename"' "$WORK/data.body"; then pass "/data/graphql -> backend /graphql (200)"; else fail "/data/graphql: got $code $(head -c 200 "$WORK/data.body")"; fi
code="$(req databogus POST /data/graphql -H 'Authorization: Bearer not-a-real-token' -H 'Cookie: session=x' "${GQL[@]}")"
if [ "$code" = 200 ] && grep -q '"__typename"' "$WORK/databogus.body"; then
    pass "/data/graphql with a bogus bearer token -> anonymous answer (Authorization removed)"
else
    fail "/data/graphql with a bogus token: expected 200, got $code"
fi

# econ-graph-idp
if [ "$SKIP_KEYCLOAK" = true ]; then
    echo "  skip  /idp checks (SKIP_KEYCLOAK=true)"
else
    code="$(req oidc GET /idp/realms/econ-graph/.well-known/openid-configuration)"
    issuer="$(grep -o '"issuer":"[^"]*"' "$WORK/oidc.body" | cut -d'"' -f4)"
    if [ "$code" = 200 ] && [ "$issuer" = "$EXPECTED_ISSUER" ]; then
        pass "/idp/realms/econ-graph discovery -> Keycloak, issuer $issuer"
    else
        fail "/idp/realms/econ-graph discovery: got $code, issuer '$issuer' (expected $EXPECTED_ISSUER)"
    fi
    for path in /idp/admin/ /idp/admin/master/console/ /idp/realms/master/.well-known/openid-configuration /idp/realms/econ-graph-x/.well-known/openid-configuration; do
        code="$(req notkc GET "$path")"
        # Not routed to Keycloak, so the frontend's catch-all route answers.
        if is_frontend_page notkc "$code"; then
            pass "$path -> the frontend's page, not Keycloak"
        else
            fail "$path: expected the frontend's page, got $code $(header notkc Location)"
        fi
    done
fi

# Rate limits: 20 requests a second per client, bursts of 100.
burst() {
    local path="$1" n="$2" args=()
    for _ in $(seq 1 "$n"); do
        args+=(--next -s -o /dev/null -w '%{http_code}\n' -X POST -H "Host: ${ROUTE_HOST}" "${GQL[@]}" "${BASE_URL}${path}")
    done
    curl --parallel --parallel-immediate --parallel-max 50 "${args[@]:1}" 2>/dev/null | sort | uniq -c | tr '\n' ' '
}
if [ "$RATE_LIMIT" = true ]; then
    for path in /graphql /data/graphql; do
        # Let the bucket refill after the requests above.
        sleep 6
        below="$(burst "$path" 90)"
        if [ "$(tr -s ' ' <<<" $below")" = " 90 200 " ]; then pass "$path: 90 requests at once, none limited ($below)"; else fail "$path: 90 requests at once were limited: $below"; fi
        sleep 6
        above="$(burst "$path" 250)"
        if grep -q ' 429' <<<" $above" && grep -q ' 200' <<<" $above"; then pass "$path: 250 requests at once, the excess limited ($above)"; else fail "$path: 250 requests at once: $above"; fi
    done
fi

echo "${passed} passed, ${failed} failed"
[ "$failed" -eq 0 ]
