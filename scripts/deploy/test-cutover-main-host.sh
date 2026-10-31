#!/usr/bin/env bash
# Exercise cutover's redirect matrix with curl stubbed; no cluster or sockets.
# shellcheck disable=SC2016,SC2034,SC2329
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
sed -n '/^check_public_redirects() {$/,/^}$/p' "$ROOT/scripts/deploy/cutover-to-gateway.sh" >"$WORK/functions.sh"
# shellcheck source=/dev/null
source "$WORK/functions.sh"
curl() {
    local arg host='' url=''
    while [ $# -gt 0 ]; do
        arg="$1"; shift
        case "$arg" in
            -H) host="${1#Host: }"; shift ;;
            -o | -w | --max-time) shift ;;
            http*) url="$arg" ;;
        esac
    done
    printf '%s %s\n' "$host" "$url" >>"$WORK/requests"
    if [ "${DROP_QUERY:-false}" = true ]; then
        printf '301 https://%s/some/path' "$MAIN_HOST"
    else
        printf '301 https://%s/some/path?from=redirect' "$MAIN_HOST"
    fi
}
PUBLIC_HOST=econgraph.com
for MAIN_HOST in www.econgraph.com econgraph.com; do
    : >"$WORK/requests"
    check_public_redirects http://127.0.0.1:19080 https://127.0.0.1:19443
    alternate=econgraph.com
    [ "$MAIN_HOST" != econgraph.com ] || alternate=www.econgraph.com
    expected="$(printf '%s\n' \
        'econgraph.com http://127.0.0.1:19080/some/path?from=redirect' \
        'www.econgraph.com http://127.0.0.1:19080/some/path?from=redirect' \
        "$alternate https://127.0.0.1:19443/some/path?from=redirect")"
    [ "$(cat "$WORK/requests")" = "$expected" ] || { echo 'FAIL redirect entry matrix'; exit 1; }
    echo "ok redirect entry matrix for $MAIN_HOST"
done
if DROP_QUERY=true check_public_redirects http://127.0.0.1:19080 https://127.0.0.1:19443; then
    echo 'FAIL dropped query accepted'
    exit 1
fi
echo 'cutover main host checks: all passed'
