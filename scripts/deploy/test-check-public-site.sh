#!/usr/bin/env bash
# Tests for scripts/deploy/check-public-site.sh's www redirect checks (ECO-709), offline:
# a stub curl first on PATH answers www.econgraph.com with a canned status and Location
# (and every other request with 404), so nothing reaches the network. Only the www
# check and the spoofed X-Forwarded-* www check are asserted; the other checks fail
# against the stub and are not looked at.
#
#   - the gateway's 301 to https://econgraph.com/<path>?<query>, no port: passes;
#   - ingress-nginx's 308 to the same with :443, until the gateway cut-over (ECO-800):
#     passes, named as ECO-709's known case;
#   - any other port, status, or a dropped query fails, as does a spoofed
#     X-Forwarded-Port that reaches the redirect.
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
# with STUB_WWW_STATUS and STUB_WWW_LOCATION (STUB_SPOOF_LOCATION instead when the
# request spoofs X-Forwarded-Port), everything else with 404.
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
        if [ "$spoof" = true ] && [ -n "${STUB_SPOOF_LOCATION:-}" ]; then
            location="$STUB_SPOOF_LOCATION"
        fi
        printf 'HTTP/1.1 %s Redirect\r\nLocation: %s\r\n\r\n' "$status" "$location" >"$hdr" ;;
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
WWW_LINE='www.econgraph.com redirects'
SPOOF_LINE='spoofed X-Forwarded-\* headers leave'

# run NAME STATUS LOCATION [SPOOF_LOCATION]: the check's output against the stub.
run() {
    STUB_WWW_STATUS="$2" STUB_WWW_LOCATION="$3" STUB_SPOOF_LOCATION="${4:-}" \
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

run ingress 308 'https://econgraph.com:443/series/smoke-check?from=www'
expect ingress ok "$WWW_LINE \(308\) through ingress-nginx.*ECO-709: production not yet cut over" \
    "ingress-nginx's 308 with :443 is the named known case"
expect ingress ok "$SPOOF_LINE ingress-nginx's www redirect.*ECO-709" "and so is its spoofed check"

run port-301 301 'https://econgraph.com:443/series/smoke-check?from=www'
expect port-301 FAIL "$WWW_LINE" "a 301 that spells out :443 fails"

run plain-308 308 "$TARGET"
expect plain-308 FAIL "$WWW_LINE" "a 308 without the port fails: only ingress-nginx's exact form is known"

run other-port 308 'https://econgraph.com:8443/series/smoke-check?from=www'
expect other-port FAIL "$WWW_LINE" "a 308 to another port fails"

run no-query 301 'https://econgraph.com/series/smoke-check'
expect no-query FAIL "$WWW_LINE" "a redirect that drops the query fails"

run found 302 "$TARGET"
expect found FAIL "$WWW_LINE" "a 302 fails"

run spoofed 301 "$TARGET" 'https://econgraph.com:8443/series/smoke-check?from=www'
expect spoofed ok "$WWW_LINE \(301\)" "with a spoofed X-Forwarded-Port reaching the redirect, the www check passes"
expect spoofed FAIL "with spoofed X-Forwarded-\* headers www redirects" "but the spoofed check fails"

if [ "$failures" -eq 0 ]; then
    echo "all passed"
else
    echo "${failures} failed"
    exit 1
fi
