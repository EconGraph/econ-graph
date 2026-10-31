#!/usr/bin/env bash
# The grep pattern for restart-k8s-rollout.sh is meant literally.
# shellcheck disable=SC2016
# Tests for check_public_host_urls in scripts/deploy/lib/public-host.sh (the URL summary at
# the end of restart-k8s-rollout.sh), offline: a stub curl first on PATH answers with
# canned statuses and redirect URLs, so nothing reaches the network.
#   - https://HOST answering 200 or 302 is accessible, anything else is not;
#   - www.HOST passes only with a 301 to https://HOST with the path and query kept and no
#     port (check-public-site.sh's rule); a 200, a 308, a port or a dropped query fails;
#   - the host is the one given, never a hard-coded econgraph.com;
#   - restart-k8s-rollout.sh uses it, with PUBLIC_HOST, and names no host itself.
#
# Usage: scripts/deploy/test-public-host-urls.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
W="$(mktemp -d)"
trap 'rm -rf "$W"' EXIT
mkdir -p "$W/bin"

cat >"$W/bin/curl" <<'STUB'
#!/usr/bin/env bash
# Stub curl: prints -w's format with %{http_code} and %{redirect_url} filled in from
# STUB_APEX_STATUS for https://$STUB_HOST/ and STUB_WWW_STATUS and STUB_WWW_LOCATION for
# https://www.$STUB_HOST/...; any other URL is an error.
fmt="" url=""
while [ $# -gt 0 ]; do
    case "$1" in
        -w) fmt="$2"; shift 2 ;;
        -o | --max-time) shift 2 ;;
        -*) shift ;;
        *) url="$1"; shift ;;
    esac
done
case "$url" in
    "https://${STUB_HOST}/") status="$STUB_APEX_STATUS" location="" ;;
    "https://www.${STUB_HOST}/"*) status="$STUB_WWW_STATUS" location="${STUB_WWW_LOCATION:-}" ;;
    *) echo "unexpected URL $url" >&2; exit 6 ;;
esac
fmt="${fmt//%\{http_code\}/$status}"
fmt="${fmt//%\{redirect_url\}/$location}"
printf "$fmt"
STUB
chmod +x "$W/bin/curl"

failures=0
ok() { echo "ok   $1"; }
bad() {
    echo "FAIL $1"
    failures=$((failures + 1))
}

# run APEX WWW_STATUS WWW_LOCATION [HOST]: check_public_host_urls's output in $W/out.
run() {
    local host="${4:-example.org}"
    (
        cd "$ROOT"
        export PATH="$W/bin:$PATH" STUB_HOST="$host" STUB_APEX_STATUS="$1" \
            STUB_WWW_STATUS="$2" STUB_WWW_LOCATION="$3"
        # shellcheck source=scripts/deploy/lib/public-host.sh
        source scripts/deploy/lib/public-host.sh
        check_public_host_urls "$host"
    ) >"$W/out" 2>&1
}
has() {
    if grep -qF -- "$1" "$W/out"; then ok "$2"; else
        bad "$2"
        sed 's/^/       /' "$W/out"
    fi
}

TARGET='https://example.org/series/smoke-check?from=www'
run 200 301 "$TARGET"
has "✅ HTTPS Production: https://example.org - Accessible" "the apex answering 200 is accessible"
has "✅ HTTPS Production: https://www.example.org - Redirects (301) to https://example.org" \
    "www's 301 to the apex, path and query kept, passes"
run 302 301 "$TARGET"
has "✅ HTTPS Production: https://example.org - Accessible" "a 302 from the apex is accessible"
run 502 301 "$TARGET"
has "❌ HTTPS Production: https://example.org - Not accessible (HTTP 502" "a 502 from the apex is not"
run 200 200 ""
has "❌ HTTPS Production: https://www.example.org - Expected a 301 to $TARGET, got HTTP 200" \
    "www answering 200 (the old success) now fails"
run 200 308 "$TARGET"
has "got HTTP 308 to $TARGET" "a 308 fails"
run 200 301 'https://example.org:443/series/smoke-check?from=www'
has "got HTTP 301 to https://example.org:443/" "a 301 that spells out a port fails"
run 200 301 'https://example.org/series/smoke-check'
has "got HTTP 301 to https://example.org/series/smoke-check" "a 301 that drops the query fails"
run 200 301 'https://econgraph.example/series/smoke-check?from=www' econgraph.example
has "https://www.econgraph.example - Redirects (301) to https://econgraph.example" \
    "the given host is the one checked"

restart="$ROOT/scripts/deploy/restart-k8s-rollout.sh"
if grep -q 'check_public_host_urls "\$PUBLIC_HOST"' "$restart" && ! grep -q 'econgraph\.com' "$restart"; then
    ok "restart-k8s-rollout.sh checks PUBLIC_HOST and names no host itself"
else
    bad "restart-k8s-rollout.sh: no check_public_host_urls \"\$PUBLIC_HOST\", or a hard-coded econgraph.com"
fi

if [ "$failures" -eq 0 ]; then
    echo "all passed"
else
    echo "${failures} failed"
    exit 1
fi
