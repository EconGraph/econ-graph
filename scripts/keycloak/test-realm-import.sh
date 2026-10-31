#!/bin/bash

# The realm import against a real Keycloak: what a deploy keeps, removes and refuses.
#
# Usage: scripts/keycloak/test-realm-import.sh
#
# Starts a throwaway Keycloak in dev mode in Docker and replays production's upgrade path.
# First the previous release's realm import (PREVIOUS_RELEASE, default v4.0.1: its
# render-realm.sh and realm file, then keycloak-config-cli). Then this tree's import Job steps,
# as k8s/manifests/keycloak/realm-import-job.yaml runs them: render-realm.sh,
# preserve-catalog-grants.py, keycloak-config-cli, then preserve-catalog-grants.py --complete.
# That first import's realm file also declares a client, temp-x, with a client role
# temp-role, and ten roles catalog.retired_<n>:read. The script checks it seeds the realm file's catalog policy, then
# makes the changes an operator makes and imports the realm file without temp-x, the retired
# roles and catalog.ilo:read, checking:
#
#   - catalog reads granted through the scope mappings of a client the realm file doesn't
#     declare, of a client scope, and of a declared client survive the import;
#   - other econ-graph-api roles in those mappings are removed, and the import's log
#     names each one first;
#   - realm-role scope mappings on those targets are left alone;
#   - what scripts/keycloak/catalog-access.py (set-catalog-access.sh's helper) set on
#     catalog-public, admin and the anonymous reader's scope survives;
#   - a catalog read mapped on `basic`, a client scope anonymous tokens are built with, makes
#     catalog-access.py warn and the import refuse, changing nothing, until it is removed; so
#     does catalog-public mapped there, which hands an anonymous token (checked) a catalog
#     taken off the reader's own scope;
#   - temp-x, which config-cli created and now deletes, doesn't fail the import although
#     it held a catalog read and the anonymous service account holds its role directly;
#   - neither do catalog.ilo:read and the retired roles, deleted the same way although granted
#     (one directly to the anonymous service account): their grants are named and dropped,
#     and a catalog role an operator made keeps its grants;
#   - an anonymous token carries exactly the anonymous scope's catalogs;
#   - a further import changes nothing and warns only that config-cli's role state, now a
#     chunk shorter, has a chunk left over, which it ignores as config-cli does.
#
# The previous release's files come from PREVIOUS_RELEASE_DIR when set (CI checks the tag
# out there), otherwise from the tag in this repository. Needs docker, python3, curl and
# git; touches no cluster. Each run uses its own container names, network and a random
# local port, so runs can overlap; its files go under $XDG_CACHE_HOME (default ~/.cache), not
# /tmp. DOCKER_HOST is honoured. The images are pinned by digest; the script refuses to run
# when the manifests deploy a different Keycloak or keycloak-config-cli tag than it tests.

set -euo pipefail

REPO="$(cd "$(dirname "$0")/../.." && pwd)"
PREVIOUS_RELEASE="${PREVIOUS_RELEASE:-v4.0.1}"
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
    found="$(sed -n 's/^ *image: *//p' "$REPO/$1" | { grep -F "${2%%:*}" || true; } | sort -u)"
    [ "$found" = "$2" ] || { echo "$1 runs '${found:-nothing}', not $2; update this script" >&2; exit 1; }
}
check_images k8s/manifests/keycloak/deployment.yaml "$KC_TAG"
check_images k8s/manifests/keycloak/realm-import-job.yaml "$CLI_TAG"

CACHE="${XDG_CACHE_HOME:-$HOME/.cache}"
mkdir -p "$CACHE"
WORK="$(mktemp -d "$CACHE/econgraph-realm-import-test.XXXXXX")"
cleanup() {
    docker rm -fv "$KC" >/dev/null 2>&1 || true
    docker network rm "$NET" >/dev/null 2>&1 || true
    rm -rf "$WORK"
}
trap cleanup EXIT

previous() { # PATH: a file of the previous release
    if [ -n "${PREVIOUS_RELEASE_DIR:-}" ]; then
        cat "$PREVIOUS_RELEASE_DIR/$1"
    else
        git -C "$REPO" show "$PREVIOUS_RELEASE:$1"
    fi
}
mkdir -p "$WORK/previous"
previous scripts/keycloak/render-realm.sh >"$WORK/previous/render-realm.sh"
previous config/keycloak/econ-graph-realm.json >"$WORK/previous/econ-graph-realm.json"

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
as_admin() { KEYCLOAK_URL="$BASE" KEYCLOAK_USER="$ADMIN" KEYCLOAK_PASSWORD="$PASS" "$@"; }

config_cli() { # TAG: import WORK/econ-graph-realm.json
    chmod a+r "$WORK/econ-graph-realm.json"
    chmod a+rx "$WORK"
    docker run --rm --network "$NET" -v "$WORK/econ-graph-realm.json:/rendered/econ-graph-realm.json:ro" \
        -e KEYCLOAK_URL="http://$KC:8080/idp" -e KEYCLOAK_USER="$ADMIN" -e KEYCLOAK_PASSWORD="$PASS" \
        -e IMPORT_BEHAVIORS_REMOVEDEFAULTROLEFROMUSER=true \
        -e IMPORT_FILES_LOCATIONS=/rendered/econ-graph-realm.json \
        "$CLI_IMAGE" >"$WORK/config-cli-$1.log" 2>&1 \
        || { grep -m5 ' ERROR ' "$WORK/config-cli-$1.log" >&2 || tail -30 "$WORK/config-cli-$1.log" >&2; exit 1; }
}

# The render and catalog-policy steps; their warnings go to WORK/warnings-TAG and the status
# is the catalog-policy step's. Patches to the realm file: extra-client declares a client
# temp-x and the retired roles, drop-ilo leaves out the role catalog.ilo:read.
prepare() { # TAG [PATCH]
    KC_WEB_BASE_URL=http://localhost KC_WEB_E2E_BASE_URL=http://localhost \
        KC_ANONYMOUS_CLIENT_SECRET=scratch-anonymous-secret \
        bash "$REPO/scripts/keycloak/render-realm.sh" "$REPO/config/keycloak/econ-graph-realm.json" \
        >"$WORK/realm-template.json" || return 1
    python3 -c 'import json, sys
path, patch = sys.argv[1:]
realm = json.load(open(path))
if patch == "extra-client":
    realm["clients"].append({"clientId": "temp-x", "publicClient": False, "fullScopeAllowed": False})
    realm["roles"]["client"]["temp-x"] = [{"name": "temp-role"}]
    realm["roles"]["client"]["econ-graph-api"] += [{"name": "catalog.retired_%d:read" % n} for n in range(10)]
if patch == "drop-ilo":
    roles = realm["roles"]["client"]["econ-graph-api"]
    roles[:] = [r for r in roles if r["name"] != "catalog.ilo:read"]
json.dump(realm, open(path, "w"))' "$WORK/realm-template.json" "${2:-}" || return 1
    as_admin python3 "$REPO/scripts/keycloak/preserve-catalog-grants.py" "$WORK/realm-template.json" \
        >"$WORK/econ-graph-realm.json" 2>"$WORK/warnings-$1"
}

# The Job's steps, in its order.
import_realm() { # TAG [PATCH]
    prepare "$@" || { cat "$WORK/warnings-$1" >&2; exit 1; }
    config_cli "$1"
    as_admin python3 "$REPO/scripts/keycloak/preserve-catalog-grants.py" --complete "$WORK/econ-graph-realm.json" >/dev/null
}

# The previous release's Job: render, then config-cli, with no catalog policy yet.
KC_WEB_BASE_URL=http://localhost KC_WEB_E2E_BASE_URL=http://localhost bash "$WORK/previous/render-realm.sh" "$WORK/previous/econ-graph-realm.json" \
    >"$WORK/econ-graph-realm.json"
config_cli previous

import_realm first extra-client
[ ! -s "$WORK/warnings-first" ] || fail "the first import warned: $(cat "$WORK/warnings-first")"

client_id() { api GET "/clients?clientId=$1" | json 'd[0]["id"]'; }
API_ID="$(client_id econ-graph-api)"
WEB_ID="$(client_id econ-graph-web)"
ANON_ID="$(client_id econgraph-anonymous)"
role() { api GET "/clients/$API_ID/roles/$1" || { echo "no role $1" >&2; exit 1; }; }

# The realm file's policy, seeded on upgrading from the previous release.
PUBLIC_CATALOGS="catalog.bea:read,catalog.bls:read,catalog.census:read,catalog.fhfa:read,catalog.world_bank:read"
[ "$(api GET "/roles/catalog-public/composites" | names)" = "$PUBLIC_CATALOGS" ] ||
    fail "the upgrade seeded catalog-public with $(api GET "/roles/catalog-public/composites" | names)"
[ "$(api GET "/clients/$ANON_ID/scope-mappings/clients/$API_ID" | names)" = "$PUBLIC_CATALOGS" ] ||
    fail "the upgrade seeded the anonymous scope with $(api GET "/clients/$ANON_ID/scope-mappings/clients/$API_ID" | names)"
api GET "/roles/admin/composites" | names | grep -q 'catalog\.fred:read' || fail "the upgrade left FRED off admin"

# An operator's targets: a client and a client scope the realm file doesn't declare, and a
# catalog role of their own.
api POST /clients '{"clientId":"partner-x","fullScopeAllowed":false,"serviceAccountsEnabled":true}' >/dev/null
api POST /client-scopes '{"name":"catalog-extra","protocol":"openid-connect"}' >/dev/null
api POST "/clients/$API_ID/roles" '{"name":"catalog.partner:read"}' >/dev/null
PARTNER_ID="$(client_id partner-x)"
SCOPE_ID="$(api GET /client-scopes | json '[s["id"] for s in d if s["name"] == "catalog-extra"][0]')"
BASIC_ID="$(api GET /client-scopes | json '[s["id"] for s in d if s["name"] == "basic"][0]')"
IMF="$(role catalog.imf:read)"
BANK_OF_CANADA="$(role catalog.bank_of_canada:read)"
MCP="$(role api:mcp)"
WTO="$(role catalog.world_trade_organization:read)"
ILO="$(role catalog.ilo:read)"
FRED="$(role catalog.fred:read)"
PARTNER="$(role catalog.partner:read)"
GRANTS="[$IMF,$BANK_OF_CANADA,$MCP]"
PUBLIC="$(api GET /roles/catalog-public)"
api POST "/clients/$PARTNER_ID/scope-mappings/clients/$API_ID" "[$IMF,$BANK_OF_CANADA,$MCP,$ILO,$PARTNER]" >/dev/null
api POST "/client-scopes/$SCOPE_ID/scope-mappings/clients/$API_ID" "$GRANTS" >/dev/null
api POST "/clients/$WEB_ID/scope-mappings/clients/$API_ID" "[$WTO]" >/dev/null
api POST "/clients/$(client_id temp-x)/scope-mappings/clients/$API_ID" "[$IMF]" >/dev/null
api POST "/roles/admin/composites" "[$PARTNER]" >/dev/null
api POST "/clients/$PARTNER_ID/scope-mappings/realm" "[$PUBLIC]" >/dev/null
api POST "/client-scopes/$SCOPE_ID/scope-mappings/realm" "[$PUBLIC]" >/dev/null
# A retired role held directly by the anonymous service account.
READER_ID="$(api GET "/clients/$ANON_ID/service-account-user" | json 'd["id"]')"
api POST "/users/$READER_ID/role-mappings/clients/$API_ID" "[$(role catalog.retired_0:read)]" >/dev/null
TEMP_ID="$(client_id temp-x)"
api POST "/users/$READER_ID/role-mappings/clients/$TEMP_ID" "[$(api GET "/clients/$TEMP_ID/roles/temp-role")]" >/dev/null
# A catalog read on `basic`, which anonymous tokens are built with.
api POST "/client-scopes/$BASIC_ID/scope-mappings/clients/$API_ID" "[$FRED]" >/dev/null
# What set-catalog-access.sh does, through its helper.
as_admin python3 "$REPO/scripts/keycloak/catalog-access.py" \
    --admin-only catalog.bls:read --make-public catalog.imf:read --withdraw catalog.census:read \
    --make-public catalog.ilo:read --withdraw catalog.fred:read >"$WORK/catalog-access.log"
grep -qF "warning: econgraph-anonymous tokens admit catalog.fred:read through client scope basic," "$WORK/catalog-access.log" ||
    fail "catalog-access.py didn't name basic: $(cat "$WORK/catalog-access.log")"

# The catalogs in an anonymous token, whatever claim carries them.
anonymous_catalogs() {
    curl -sf -d grant_type=client_credentials -d client_id=econgraph-anonymous \
        -d client_secret=scratch-anonymous-secret "$BASE/realms/$REALM/protocol/openid-connect/token" \
        | python3 -c 'import base64, json, re, sys
payload = json.load(sys.stdin)["access_token"].split(".")[1]
claims = base64.urlsafe_b64decode(payload + "=" * (-len(payload) % 4)).decode()
print(",".join(sorted(set(re.findall(r"catalog\.[a-z0-9_]+:read", claims)))))'
}
# BEA signed-in only: off the reader's own scope, still in catalog-public. Then catalog-public
# mapped on basic gives anonymous tokens BEA back.
api DELETE "/clients/$ANON_ID/scope-mappings/clients/$API_ID" "[$(role catalog.bea:read)]" >/dev/null
TOKEN_CATALOGS="$(anonymous_catalogs)"
[[ ",$TOKEN_CATALOGS," != *,catalog.bea:read,* ]] || fail "BEA reached an anonymous token before the leak"
api POST "/client-scopes/$BASIC_ID/scope-mappings/realm" "[$PUBLIC]" >/dev/null
TOKEN_CATALOGS="$(anonymous_catalogs)"
[[ ",$TOKEN_CATALOGS," == *,catalog.bea:read,* ]] || fail "catalog-public on basic didn't reach the token; the refusal test is vacuous"

snapshot() {
    local path
    for path in "clients/$PARTNER_ID" "client-scopes/$SCOPE_ID" "clients/$WEB_ID" "clients/$ANON_ID" \
        "client-scopes/$BASIC_ID"; do
        echo "$path api: $(api GET "/$path/scope-mappings/clients/$API_ID" | names)"
        echo "$path realm: $(api GET "/$path/scope-mappings/realm" | names)"
    done
    echo "catalog-public: $(api GET "/roles/catalog-public/composites" | names)"
    echo "admin: $(api GET "/roles/admin/composites" | names)"
}

# The catalog reads basic admits stop the deploy at the catalog-policy step.
if prepare refused drop-ilo; then
    fail "the import accepted catalog reads on basic"
else
    for name in catalog.bea:read catalog.fred:read; do
        grep -q "client scope basic ([^)]*$name" "$WORK/warnings-refused" ||
            fail "the refusal doesn't name $name on basic: $(cat "$WORK/warnings-refused")"
    done
fi
api DELETE "/client-scopes/$BASIC_ID/scope-mappings/clients/$API_ID" "[$FRED]" >/dev/null
api DELETE "/client-scopes/$BASIC_ID/scope-mappings/realm" "[$PUBLIC]" >/dev/null

expect() { # LINE
    grep -qxF "$1" "$WORK/after" || fail "after a deploy, expected '$1'; got: $(grep -F "${1%%:*}:" "$WORK/after")"
}

import_realm second drop-ilo
snapshot >"$WORK/after"
expect "clients/$PARTNER_ID api: catalog.bank_of_canada:read,catalog.imf:read,catalog.partner:read"
expect "client-scopes/$SCOPE_ID api: catalog.bank_of_canada:read,catalog.imf:read"
expect "clients/$WEB_ID api: catalog.world_trade_organization:read"
expect "clients/$PARTNER_ID realm: catalog-public"
expect "client-scopes/$SCOPE_ID realm: catalog-public"
expect "client-scopes/$BASIC_ID api: "
expect "clients/$ANON_ID api: catalog.fhfa:read,catalog.imf:read,catalog.world_bank:read"
expect "client-scopes/$BASIC_ID realm: "
TOKEN_CATALOGS="$(anonymous_catalogs)"
[ "$TOKEN_CATALOGS" = "catalog.fhfa:read,catalog.imf:read,catalog.world_bank:read" ] ||
    fail "an anonymous token carries $TOKEN_CATALOGS"
expect "catalog-public: catalog.bea:read,catalog.fhfa:read,catalog.imf:read,catalog.world_bank:read"
grep -q '^admin: .*catalog\.bls:read' "$WORK/after" || fail "admin lost catalog.bls:read: $(grep '^admin:' "$WORK/after")"
grep -q '^admin: .*catalog\.partner:read' "$WORK/after" || fail "admin lost catalog.partner:read: $(grep '^admin:' "$WORK/after")"
[ "$(api GET "/clients/$API_ID/roles/catalog.partner:read" | json 'd["name"]')" = catalog.partner:read ] ||
    fail "the operator's catalog role was deleted"
for gone in catalog.ilo:read catalog.retired_0:read; do
    [ "$(curl -s -o /dev/null -w '%{http_code}' -H "Authorization: Bearer $(token)" \
        "$BASE/admin/realms/$REALM/clients/$API_ID/roles/$gone")" = 404 ] ||
        fail "$gone, dropped from the realm file, still exists"
done
for target in "client partner-x: api:mcp" "client scope catalog-extra: api:mcp" \
    "catalog.ilo:read is no longer in the realm file, so the import deletes it and its grant on role catalog-public" \
    "catalog.ilo:read is no longer in the realm file, so the import deletes it and its grant on client partner-x" \
    "catalog.ilo:read is no longer in the realm file, so the import deletes it and its grant on client econgraph-anonymous" \
    "catalog.retired_0:read is no longer in the realm file, so the import deletes it and its grant on the econgraph-anonymous service account" \
    "temp-role is no longer in the realm file, so the import deletes it and its grant on the econgraph-anonymous service account"; do
    grep -qF "$target" "$WORK/warnings-second" || fail "no warning for $target: $(cat "$WORK/warnings-second")"
done
[ "$(grep -v 'is no longer in the realm file' "$WORK/warnings-second" | grep -c 'catalog\.\|temp-x')" = 0 ] ||
    fail "unexpected warning: $(cat "$WORK/warnings-second")"
[ "$(api GET "/clients?clientId=temp-x")" = "[]" ] || fail "temp-x, dropped from the realm file, still exists"

import_realm third drop-ilo
mv "$WORK/after" "$WORK/after-second"
snapshot >"$WORK/after"
diff "$WORK/after-second" "$WORK/after" >&2 || fail "a further import changed the realm"
grep -q "^warning: ignoring [0-9]* characters after the keycloak-config-cli state list in realm attributes de\.adorsys\.keycloak\.config\.state-default-roles-client-econ-graph-api-\*" \
    "$WORK/warnings-third" || fail "the role state didn't shrink by a chunk; the leftover-chunk check is vacuous"
[ "$(grep -vc '^warning: ignoring' "$WORK/warnings-third")" = 0 ] ||
    fail "a further import warned: $(cat "$WORK/warnings-third")"

if [ "$failures" -ne 0 ]; then
    echo "$failures check(s) failed" >&2
    exit 1
fi
echo "realm import: upgrade seeded, catalog grants kept, deleted roles and clients dropped, anonymous client-scope grants refused"
