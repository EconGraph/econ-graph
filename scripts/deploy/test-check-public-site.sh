#!/usr/bin/env bash
# Tests for scripts/deploy/check-public-site.sh's www redirect checks (ECO-709) and its
# localhost redirect URI check (ECO-753), offline:
# a stub curl first on PATH answers www.econgraph.com with a canned status and Location
# (and every other request with 404), so nothing reaches the network. Only the www
# check and the spoofed X-Forwarded-* www check are asserted; the other checks fail
# against the stub and are not looked at.
#
#   - the gateway's 301 to https://econgraph.com/<path>?<query>, no port: passes;
#   - ingress-nginx's 308 to the same with :443 and nginx's redirect page, until the
#     gateway cut-over (ECO-800): passes, named as ECO-709's known case;
#   - the same 308 without nginx's page (a Cloudflare redirect rule) fails, as does any
#     other port, status, or a dropped query, and a spoofed request that gets anything
#     but the 301;
#   - plain http's redirect must name https://econgraph.com/ with no port;
#   - the sign-in page must refuse a http://localhost redirect URI.
#
# Usage: scripts/deploy/test-check-public-site.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CHECK="${ROOT}/scripts/deploy/check-public-site.sh"
W="$(mktemp -d)"
trap 'rm -rf "$W"' EXIT
mkdir -p "$W/bin"

cat >"$W/bin/curl" <<'STUB'
#!/usr/bin/env bash
# Stub curl: -o BODY -D HEADERS -w FORMAT [-H HEADER]... URL. Answers www.econgraph.com
# with STUB_WWW_STATUS, STUB_WWW_LOCATION and the body STUB_WWW_BODY (STUB_SPOOF_STATUS
# and STUB_SPOOF_LOCATION instead, where set, when the request spoofs
# X-Forwarded-Port), http://econgraph.com with STUB_PLAIN_STATUS and
# STUB_PLAIN_LOCATION, and everything else with 404.
out=/dev/null hdr=/dev/null url="" spoof=false
while [ $# -gt 0 ]; do
    case "$1" in
        -o) out="$2"; shift 2 ;;
        -D) hdr="$2"; shift 2 ;;
        -w) shift 2 ;;
        -H) [[ "$2" == X-Forwarded-Port:* ]] && spoof=true; shift 2 ;;
        -X | --max-time | --connect-to) shift 2 ;;
        -*) shift ;;
        *) url="$1"; shift ;;
    esac
done
: >"$out"
case "$url" in
    https://www.econgraph.com/*)
        status="$STUB_WWW_STATUS"
        location="$STUB_WWW_LOCATION"
        if [ "$spoof" = true ]; then
            status="${STUB_SPOOF_STATUS:-$status}"
            location="${STUB_SPOOF_LOCATION:-$location}"
        fi
        printf '%s' "${STUB_WWW_BODY:-}" >"$out"
        printf 'HTTP/1.1 %s Redirect\r\nLocation: %s\r\n\r\n' "$status" "$location" >"$hdr" ;;
    https://econgraph.com/idp/realms/econ-graph/protocol/openid-connect/auth\?*redirect_uri=http%3A%2F%2Flocalhost*)
        status="${STUB_LOCAL_LOGIN_STATUS:-404}"
        printf '%s' "${STUB_LOCAL_LOGIN_BODY:-}" >"$out"
        printf 'HTTP/1.1 %s\r\n\r\n' "$status" >"$hdr" ;;
    http://econgraph.com/*)
        status="${STUB_PLAIN_STATUS:-404}"
        printf 'HTTP/1.1 %s Redirect\r\nLocation: %s\r\n\r\n' "$status" "${STUB_PLAIN_LOCATION:-}" >"$hdr" ;;
    *)
        status=404
        printf 'HTTP/1.1 404 Not Found\r\n\r\n' >"$hdr" ;;
esac
printf '%s' "$status"
STUB
chmod +x "$W/bin/curl"

failures=0
ok() { echo "ok   $1"; }
bad() {
    echo "FAIL $1"
    failures=$((failures + 1))
}

TARGET='https://econgraph.com/series/smoke-check?from=www'
INGRESS_TARGET='https://econgraph.com:443/series/smoke-check?from=www'
# ingress-nginx's own 308 page (observed through Cloudflare, ECO-709)
NGINX_BODY='<html><head><title>308 Permanent Redirect</title></head><body><center><h1>308 Permanent Redirect</h1></center><hr><center>nginx</center></body></html>'
WWW_LINE='www.econgraph.com redirects'
SPOOF_LINE='spoofed X-Forwarded-\* headers leave'

# run NAME STATUS LOCATION: the check's output against the stub, with any STUB_* from the
# caller's environment (env STUB_WWW_BODY=... run ...).
run() {
    STUB_WWW_STATUS="$2" STUB_WWW_LOCATION="$3" \
        PATH="$W/bin:$PATH" "$CHECK" --host econgraph.com >"$W/$1.out" 2>&1 || true
}
# expect NAME ok|FAIL PATTERN DESCRIPTION: a line of that outcome matching PATTERN.
expect() {
    if grep -Eq "^  $2 +$3" "$W/$1.out"; then
        ok "$4"
    else
        bad "$4"
        grep -E "www|spoofed" "$W/$1.out" | sed 's/^/       /'
    fi
}

run gateway 301 "$TARGET"
expect gateway ok "$WWW_LINE \(301\) to https://econgraph.com/series/smoke-check\?from=www$" \
    "the gateway's 301 without a port, path and query kept, passes"
expect gateway ok "$SPOOF_LINE the www redirect" "and the spoofed check passes"

STUB_WWW_BODY="$NGINX_BODY" run ingress 308 "$INGRESS_TARGET"
expect ingress ok "$WWW_LINE \(308\) through ingress-nginx.*ECO-709: production not yet cut over" \
    "ingress-nginx's 308 with :443 is the named known case"
expect ingress ok "$SPOOF_LINE ingress-nginx's www redirect.*ECO-709" "and so is its spoofed check"

STUB_WWW_BODY='<p>Redirecting</p>' run cloudflare-rule 308 "$INGRESS_TARGET"
expect cloudflare-rule FAIL "$WWW_LINE" "the same 308 without nginx's page (a Cloudflare rule, say) fails"
expect cloudflare-rule FAIL "with spoofed X-Forwarded-\* headers www redirects" "and so does its spoofed check"

run port-301 301 "$INGRESS_TARGET"
expect port-301 FAIL "$WWW_LINE" "a 301 that spells out :443 fails"

run plain-308 308 "$TARGET"
expect plain-308 FAIL "$WWW_LINE" "a 308 without the port fails: only ingress-nginx's exact form is known"

run other-port 308 'https://econgraph.com:8443/series/smoke-check?from=www'
expect other-port FAIL "$WWW_LINE" "a 308 to another port fails"

run no-query 301 'https://econgraph.com/series/smoke-check'
expect no-query FAIL "$WWW_LINE" "a redirect that drops the query fails"

run found 302 "$TARGET"
expect found FAIL "$WWW_LINE" "a 302 fails"

STUB_SPOOF_LOCATION='https://econgraph.com:8443/series/smoke-check?from=www' run spoofed 301 "$TARGET"
expect spoofed ok "$WWW_LINE \(301\)" "with a spoofed X-Forwarded-Port reaching the redirect, the www check passes"
expect spoofed FAIL "with spoofed X-Forwarded-\* headers www redirects" "but the spoofed check fails"

STUB_SPOOF_STATUS=302 run spoofed-302 301 "$TARGET"
expect spoofed-302 FAIL "with spoofed X-Forwarded-\* headers www redirects \(302\)" \
    "a spoofed request answered with a 302, even to the right Location, fails"

PLAIN='plain http \(even claiming https\)'
STUB_PLAIN_STATUS=301 STUB_PLAIN_LOCATION='https://econgraph.com/idp/realms/econ-graph/.well-known/openid-configuration' \
    run plain-ok 301 "$TARGET"
expect plain-ok ok "$PLAIN is redirected" "plain http redirected to https://econgraph.com/ passes"
STUB_PLAIN_STATUS=301 STUB_PLAIN_LOCATION='https://econgraph.com:443/idp/realms/econ-graph/.well-known/openid-configuration' \
    run plain-port 301 "$TARGET"
expect plain-port FAIL "plain http redirects" "plain http redirected with :443 spelled out fails"

LOCAL='sign-in page'
STUB_LOCAL_LOGIN_STATUS=400 STUB_LOCAL_LOGIN_BODY='<p class="instruction">Invalid parameter: redirect_uri</p>' \
    run local-refused 301 "$TARGET"
expect local-refused ok "$LOCAL refuses http://localhost/auth/callback" \
    "a realm that refuses the localhost redirect URI passes"
STUB_LOCAL_LOGIN_STATUS=200 STUB_LOCAL_LOGIN_BODY='<form id="kc-form-login">' \
    run local-accepted 301 "$TARGET"
expect local-accepted FAIL "$LOCAL returned 200 for http://localhost/auth/callback" \
    "a realm that still shows the sign-in form for it fails"

if [ "$failures" -eq 0 ]; then
    echo "all passed"
else
    echo "${failures} failed"
    exit 1
fi
