#!/usr/bin/env bash
# Exercise cutover's redirect matrix with curl stubbed, and its check of MAIN_HOST against
# Keycloak's live KC_HOSTNAME with kc stubbed; no cluster or sockets.
# shellcheck disable=SC2015,SC2016,SC2034,SC2329
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
# The cut-over's MAIN_HOST must match Keycloak's live public issuer.
sed -n '/^main_host_matches_keycloak() {$/,/^}$/p' "$ROOT/scripts/deploy/cutover-to-gateway.sh" >"$WORK/keycloak.sh"
# shellcheck source=/dev/null
source "$WORK/keycloak.sh"
NAMESPACE=econ-graph
kc() {
    [ "$*" = '-n econ-graph get deployment keycloak --ignore-not-found -o jsonpath={.spec.template.spec.containers[?(@.name=="keycloak")].env[?(@.name=="KC_HOSTNAME")].value}' ] ||
        { echo "unexpected kc $*" >&2; return 1; }
    [ "${STUB_KC_FAILS:-false}" != true ] || return 1
    printf '%s' "$STUB_KC_HOSTNAME"
}
for spec in 'www.econgraph.com https://www.econgraph.com/idp ok' 'econgraph.com https://econgraph.com/idp ok' \
    'econgraph.com - ok' 'econgraph.com http://localhost/idp ok' \
    'www.econgraph.com https://econgraph.com/idp refused' 'econgraph.com https://www.econgraph.com/idp refused'; do
    read -r MAIN_HOST STUB_KC_HOSTNAME expected <<<"$spec"
    [ "$STUB_KC_HOSTNAME" != - ] || STUB_KC_HOSTNAME=''
    if reason="$(main_host_matches_keycloak)"; then got=ok; else got=refused; fi
    [ "$got" = "$expected" ] || { echo "FAIL MAIN_HOST=$MAIN_HOST with KC_HOSTNAME '$STUB_KC_HOSTNAME': $got ($reason)"; exit 1; }
    [ "$got" = ok ] || grep -q 'switch-main-host.sh' <<<"$reason" || { echo "FAIL the refusal does not name the switch: $reason"; exit 1; }
done
MAIN_HOST=www.econgraph.com STUB_KC_HOSTNAME='' STUB_KC_FAILS=true
if main_host_matches_keycloak >/dev/null; then echo 'FAIL a failed Keycloak lookup was accepted'; exit 1; fi
echo 'ok the cut-over refuses a MAIN_HOST other than Keycloak'"'"'s live public issuer, and a failed lookup'
# The cut-over resolves MAIN_HOST with main_host_init (deploy-config aware), then checks Keycloak.
grep -q '^    main_host_init || die' "$ROOT/scripts/deploy/cutover-to-gateway.sh" &&
    grep -q '^    reason="$(main_host_matches_keycloak)" || die' "$ROOT/scripts/deploy/cutover-to-gateway.sh" ||
    { echo 'FAIL cutover-to-gateway.sh does not resolve MAIN_HOST with main_host_init and check Keycloak'; exit 1; }
echo 'ok cutover-to-gateway.sh resolves MAIN_HOST with main_host_init and checks it against Keycloak'
echo 'cutover main host checks: all passed'
