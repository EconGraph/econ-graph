#!/bin/bash

# The realm import against a real Keycloak: what a deploy keeps and what it removes.
#
# Usage: scripts/keycloak/test-realm-import.sh
#
# Starts a throwaway Keycloak in dev mode in Docker and runs the realm import Job's steps
# against it, as k8s/manifests/keycloak/realm-import-job.yaml runs them: render-realm.sh,
# preserve-catalog-grants.py, keycloak-config-cli, then preserve-catalog-grants.py
# --complete. The first import's realm file also declares a client, temp-x. After it, the
# script makes the changes an operator makes, imports the realm file as it is (without
# temp-x) and checks:
#
#   - catalog reads granted through the scope mappings of a client the realm file doesn't
#     declare, of a client scope, and of a declared client survive the import;
#   - other econ-graph-api roles in those mappings are removed, and the import's log
#     names each one first;
#   - realm-role scope mappings on those targets are left alone;
#   - what scripts/keycloak/catalog-access.py (set-catalog-access.sh's helper) set on
#     catalog-public, admin and the anonymous reader's scope survives;
#   - temp-x, which config-cli created and now deletes, doesn't fail the import although
#     it held a catalog read;
#   - a further import changes nothing and warns about nothing.
#
# Needs docker, python3 and curl; touches no cluster. Each run uses its own container
# names, network and a random local port, so runs can overlap. DOCKER_HOST is honoured.
# The images are pinned by digest; the script refuses to run when the manifests deploy a
# different Keycloak or keycloak-config-cli tag than it tests.

set -euo pipefail

REPO="$(cd "$(dirname "$0")/../.." && pwd)"
KC_TAG=quay.io/keycloak/keycloak:26.8.1
KC_IMAGE="$KC_TAG@sha256:a78d6b79572168cb25886b80a8b5b8d4ed4390743e3eecb810538683e209dd6b"
CLI_TAG=adorsys/keycloak-config-cli:6.5.1-26.5.5
CLI_IMAGE="$CLI_TAG@sha256:0955d98c8a341898b7aa177477edf8a1e90569ae50bbe7598141c1270b773274"
ADMIN="admin"
PASS="scratch-$RANDOM$RANDOM"
REALM=econ-graph
ID="realm-import-test-$$-$RANDOM"
NET="$ID-net"
KC="$ID-kc"

# Every image: line, exactly; a pinned digest or another tag there means updating this script.
check_images() { # MANIFEST IMAGE
    local found
    found="$(sed -n 's/^ *image: *//p' "$REPO/$1" | grep -F "${2%%:*}" | sort -u)"
    [ "$found" = "$2" ] || { echo "$1 runs '${found:-nothing}', not $2; update this script" >&2; exit 1; }
}
check_images k8s/manifests/keycloak/deployment.yaml "$KC_TAG"
check_images k8s/manifests/keycloak/realm-import-job.yaml "$CLI_TAG"

WORK="$(mktemp -d)"
cleanup() {
    docker rm -fv "$KC" >/dev/null 2>&1 || true
    docker network rm "$NET" >/dev/null 2>&1 || true
    rm -rf "$WORK"
}
trap cleanup EXIT

failures=0
fail() { echo "FAIL: $*" >&2; failures=$((failures + 1)); }

docker network create "$NET" >/dev/null
docker run -d --name "$KC" --network "$NET" -p 127.0.0.1::8080 \
    -e KC_BOOTSTRAP_ADMIN_USERNAME="$ADMIN" -e KC_BOOTSTRAP_ADMIN_PASSWORD="$PASS" \
    -e KC_HTTP_RELATIVE_PATH=/idp "$KC_IMAGE" start-dev >/dev/null
PORT="$(docker port "$KC" 8080/tcp | head -n1 | sed 's/.*://')" || true
[ -n "$PORT" ] || { echo "Keycloak didn't start" >&2; docker logs "$KC" 2>&1 | tail -20 >&2; exit 1; }
BASE="http://127.0.0.1:${PORT}/idp"
for _ in $(seq 1 150); do curl -sf "$BASE/realms/master" >/dev/null && break; sleep 2; done
curl -sf "$BASE/realms/master" >/dev/null || { docker logs "$KC" 2>&1 | tail -20; exit 1; }

token() {
    curl -sf -d grant_type=password -d client_id=admin-cli -d username="$ADMIN" -d password="$PASS" \
        "$BASE/realms/master/protocol/openid-connect/token" \
        | python3 -c 'import json,sys; print(json.load(sys.stdin)["access_token"])'
}
api() { # METHOD PATH [JSON]
    local args=(-sf -X "$1" -H "Authorization: Bearer $(token)")
    [ -z "${3:-}" ] || args+=(-H 'Content-Type: application/json' -d "$3")
    curl "${args[@]}" "$BASE/admin/realms/$REALM$2"
}
json() { python3 -c "import json,sys; d=json.load(sys.stdin); print($1)"; }
names() { json '",".join(sorted(r["name"] for r in d))'; }

# The Job's steps, in its order; the catalog-policy step's warnings go to WORK/warnings-TAG.
# With extra-client, the realm file also declares a client temp-x.
import_realm() {
    local tag="$1"
    KC_WEB_BASE_URL=http://localhost KC_WEB_E2E_BASE_URL=http://localhost \
        KC_ANONYMOUS_CLIENT_SECRET=scratch-anonymous-secret \
        bash "$REPO/scripts/keycloak/render-realm.sh" "$REPO/config/keycloak/econ-graph-realm.json" \
        >"$WORK/realm-template.json"
    if [ "${2:-}" = extra-client ]; then
        python3 -c 'import json, sys
realm = json.load(open(sys.argv[1]))
realm["clients"].append({"clientId": "temp-x", "publicClient": False, "fullScopeAllowed": False})
json.dump(realm, open(sys.argv[1], "w"))' "$WORK/realm-template.json"
    fi
    KEYCLOAK_URL="$BASE" KEYCLOAK_USER="$ADMIN" KEYCLOAK_PASSWORD="$PASS" \
        python3 "$REPO/scripts/keycloak/preserve-catalog-grants.py" "$WORK/realm-template.json" \
        >"$WORK/econ-graph-realm.json" 2>"$WORK/warnings-$tag" \
        || { cat "$WORK/warnings-$tag" >&2; exit 1; }
    chmod a+r "$WORK/econ-graph-realm.json"
    chmod a+rx "$WORK"
    docker run --rm --network "$NET" -v "$WORK/econ-graph-realm.json:/rendered/econ-graph-realm.json:ro" \
        -e KEYCLOAK_URL="http://$KC:8080/idp" -e KEYCLOAK_USER="$ADMIN" -e KEYCLOAK_PASSWORD="$PASS" \
        -e IMPORT_BEHAVIORS_REMOVEDEFAULTROLEFROMUSER=true \
        -e IMPORT_FILES_LOCATIONS=/rendered/econ-graph-realm.json \
        "$CLI_IMAGE" >"$WORK/config-cli-$tag.log" 2>&1 \
        || { grep -m5 ' ERROR ' "$WORK/config-cli-$tag.log" >&2 || tail -30 "$WORK/config-cli-$tag.log" >&2; exit 1; }
    KEYCLOAK_URL="$BASE" KEYCLOAK_USER="$ADMIN" KEYCLOAK_PASSWORD="$PASS" \
        python3 "$REPO/scripts/keycloak/preserve-catalog-grants.py" --complete "$WORK/econ-graph-realm.json" >/dev/null
}

import_realm first extra-client
[ ! -s "$WORK/warnings-first" ] || fail "a first import warned: $(cat "$WORK/warnings-first")"

client_id() { api GET "/clients?clientId=$1" | json 'd[0]["id"]'; }
API_ID="$(client_id econ-graph-api)"
WEB_ID="$(client_id econ-graph-web)"
ANON_ID="$(client_id econgraph-anonymous)"
role() { api GET "/clients/$API_ID/roles/$1" || { echo "no role $1" >&2; exit 1; }; }

# An operator's targets: a client and a client scope the realm file doesn't declare.
api POST /clients '{"clientId":"partner-x","fullScopeAllowed":false,"serviceAccountsEnabled":true}' >/dev/null
api POST /client-scopes '{"name":"catalog-extra","protocol":"openid-connect"}' >/dev/null
PARTNER_ID="$(client_id partner-x)"
SCOPE_ID="$(api GET /client-scopes | json '[s["id"] for s in d if s["name"] == "catalog-extra"][0]')"
IMF="$(role catalog.imf:read)"
BANK_OF_CANADA="$(role catalog.bank_of_canada:read)"
MCP="$(role api:mcp)"
WTO="$(role catalog.world_trade_organization:read)"
GRANTS="[$IMF,$BANK_OF_CANADA,$MCP]"
api POST "/clients/$PARTNER_ID/scope-mappings/clients/$API_ID" "$GRANTS" >/dev/null
api POST "/client-scopes/$SCOPE_ID/scope-mappings/clients/$API_ID" "$GRANTS" >/dev/null
api POST "/clients/$WEB_ID/scope-mappings/clients/$API_ID" "[$WTO]" >/dev/null
api POST "/clients/$(client_id temp-x)/scope-mappings/clients/$API_ID" "[$IMF]" >/dev/null
PUBLIC="$(api GET /roles/catalog-public)"
api POST "/clients/$PARTNER_ID/scope-mappings/realm" "[$PUBLIC]" >/dev/null
api POST "/client-scopes/$SCOPE_ID/scope-mappings/realm" "[$PUBLIC]" >/dev/null
# What set-catalog-access.sh does, through its helper.
KEYCLOAK_URL="$BASE" KEYCLOAK_USER="$ADMIN" KEYCLOAK_PASSWORD="$PASS" \
    python3 "$REPO/scripts/keycloak/catalog-access.py" \
    --admin-only catalog.bls:read --make-public catalog.imf:read --withdraw catalog.census:read >/dev/null

snapshot() {
    local path
    for path in "clients/$PARTNER_ID" "client-scopes/$SCOPE_ID" "clients/$WEB_ID" "clients/$ANON_ID"; do
        echo "$path api: $(api GET "/$path/scope-mappings/clients/$API_ID" | names)"
        echo "$path realm: $(api GET "/$path/scope-mappings/realm" | names)"
    done
    echo "catalog-public: $(api GET "/roles/catalog-public/composites" | names)"
    echo "admin: $(api GET "/roles/admin/composites" | names)"
}
expect() { # LINE
    grep -qxF "$1" "$WORK/after" || fail "after a deploy, expected '$1'; got: $(grep -F "${1%%:*}:" "$WORK/after")"
}

import_realm second
snapshot >"$WORK/after"
expect "clients/$PARTNER_ID api: catalog.bank_of_canada:read,catalog.imf:read"
expect "client-scopes/$SCOPE_ID api: catalog.bank_of_canada:read,catalog.imf:read"
expect "clients/$WEB_ID api: catalog.world_trade_organization:read"
expect "clients/$PARTNER_ID realm: catalog-public"
expect "client-scopes/$SCOPE_ID realm: catalog-public"
expect "clients/$ANON_ID api: catalog.bea:read,catalog.fhfa:read,catalog.imf:read,catalog.world_bank:read"
expect "catalog-public: catalog.bea:read,catalog.fhfa:read,catalog.imf:read,catalog.world_bank:read"
grep -q '^admin: .*catalog\.bls:read' "$WORK/after" || fail "admin lost catalog.bls:read: $(grep '^admin:' "$WORK/after")"
for target in "client partner-x: api:mcp" "client scope catalog-extra: api:mcp"; do
    grep -qF "$target" "$WORK/warnings-second" || fail "no warning for $target: $(cat "$WORK/warnings-second")"
done
! grep -q 'catalog\.\|temp-x' "$WORK/warnings-second" || fail "unexpected warning: $(cat "$WORK/warnings-second")"
[ "$(api GET "/clients?clientId=temp-x")" = "[]" ] || fail "temp-x, dropped from the realm file, still exists"

import_realm third
mv "$WORK/after" "$WORK/after-second"
snapshot >"$WORK/after"
diff "$WORK/after-second" "$WORK/after" >&2 || fail "a further import changed the realm"
[ ! -s "$WORK/warnings-third" ] || fail "a further import warned: $(cat "$WORK/warnings-third")"

if [ "$failures" -ne 0 ]; then
    echo "$failures check(s) failed" >&2
    exit 1
fi
echo "realm import: catalog grants kept, other undeclared scope mappings named and removed"
