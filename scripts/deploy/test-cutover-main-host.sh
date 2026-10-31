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
        printf '%s https://%s/some/path' "$STUB_STATUS" "$MAIN_HOST"
    else
        printf '%s https://%s/some/path?from=redirect' "$STUB_STATUS" "$MAIN_HOST"
    fi
}
PUBLIC_HOST=econgraph.com
# The redirects to the apex are permanent; those to www temporary until
# MAIN_HOST_PERMANENT names www.
for spec in 'www.econgraph.com 302 -' 'www.econgraph.com 301 www.econgraph.com' \
    'econgraph.com 301 -' 'econgraph.com 301 www.econgraph.com'; do
    read -r MAIN_HOST STUB_STATUS MAIN_HOST_PERMANENT <<<"$spec"
    [ "$MAIN_HOST_PERMANENT" != - ] || MAIN_HOST_PERMANENT=''
    : >"$WORK/requests"
    check_public_redirects http://127.0.0.1:19080 https://127.0.0.1:19443
    alternate=econgraph.com
    [ "$MAIN_HOST" != econgraph.com ] || alternate=www.econgraph.com
    expected="$(printf '%s\n' \
        'econgraph.com http://127.0.0.1:19080/some/path?from=redirect' \
        'www.econgraph.com http://127.0.0.1:19080/some/path?from=redirect' \
        "$alternate https://127.0.0.1:19443/some/path?from=redirect")"
    [ "$(cat "$WORK/requests")" = "$expected" ] || { echo 'FAIL redirect entry matrix'; exit 1; }
    echo "ok redirect entry matrix for $MAIN_HOST (${STUB_STATUS})"
done
MAIN_HOST=www.econgraph.com MAIN_HOST_PERMANENT='' STUB_STATUS=302
if DROP_QUERY=true check_public_redirects http://127.0.0.1:19080 https://127.0.0.1:19443; then
    echo 'FAIL dropped query accepted'
    exit 1
fi
for spec in '301 -' '302 www.econgraph.com'; do
    read -r STUB_STATUS MAIN_HOST_PERMANENT <<<"$spec"
    [ "$MAIN_HOST_PERMANENT" != - ] || MAIN_HOST_PERMANENT=''
    if check_public_redirects http://127.0.0.1:19080 https://127.0.0.1:19443 >/dev/null; then
        echo "FAIL a ${STUB_STATUS} to www accepted with MAIN_HOST_PERMANENT='${MAIN_HOST_PERMANENT}'"
        exit 1
    fi
done
echo 'ok the wrong redirect status to www fails'
echo 'cutover main host checks: all passed'
