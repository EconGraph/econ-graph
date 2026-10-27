#!/usr/bin/env python3
"""Keep operator-managed catalog policy while config-cli reconciles realm structure.

Read a rendered realm JSON file and write the merged import to stdout. Existing empty
mappings are authoritative. Only missing resources receive bootstrap defaults; any
other API/authentication error aborts rather than risking restored grants.
"""
import json
import os
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

INITIALIZED = "econgraph.catalog-policy-initialized"
PUBLIC_ROLE = "catalog-public"
ANONYMOUS_CLIENT = "econgraph-anonymous"
API_CLIENT = "econ-graph-api"


def catalog_read(name):
    return name.startswith("catalog.") and name.endswith(":read")


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, response, code, message, headers, new_url):
        return None


class Keycloak:
    def __init__(self, base_url, user, password, timeout=600):
        self.base_url = base_url.rstrip("/")
        self.token = None
        self.opener = urllib.request.build_opener(NoRedirect())
        data = urllib.parse.urlencode({"grant_type": "password", "client_id": "admin-cli",
                                       "username": user, "password": password}).encode()
        deadline = time.monotonic() + timeout
        while True:
            try:
                request = urllib.request.Request(self.base_url + "/realms/master/protocol/openid-connect/token", data=data)
                with self.opener.open(request, timeout=10) as response:
                    self.token = json.load(response)["access_token"]
                break
            except urllib.error.HTTPError as error:
                if error.code not in (404, 502, 503, 504) or time.monotonic() >= deadline:
                    raise RuntimeError("Keycloak administrator authentication failed") from None
            except (urllib.error.URLError, TimeoutError):
                if time.monotonic() >= deadline:
                    raise RuntimeError("Keycloak unavailable before catalog-policy snapshot") from None
            time.sleep(2)

    def get(self, path):
        request = urllib.request.Request(self.base_url + path, headers={"Authorization": "Bearer " + self.token})
        try:
            with self.opener.open(request, timeout=10) as response:
                return json.load(response)
        except urllib.error.HTTPError as error:
            if error.code == 404:
                return None
            raise RuntimeError("Keycloak catalog-policy snapshot failed (HTTP %s)" % error.code) from None
        except (urllib.error.URLError, TimeoutError, ValueError):
            raise RuntimeError("Keycloak catalog-policy snapshot unavailable or malformed") from None


def required(value):
    """Only entity lookups may return missing; a disappearing relation is an unsafe snapshot."""
    if value is None:
        raise RuntimeError("Keycloak resource disappeared during catalog-policy snapshot")
    return value


def merge_catalog_policy(realm, get):
    prefix = "/admin/realms/" + urllib.parse.quote(realm["realm"], safe="")
    observed = get(prefix)
    realm.setdefault("attributes", {})[INITIALIZED] = "true"
    if observed is None:
        return realm  # First realm creation: seed its declared defaults.
    initialized = observed.get("attributes", {}).get(INITIALIZED) == "true"
    # An existing pre-feature realm needs the user->catalog-public edge seeded once too.
    seed_defaults = not initialized and get(prefix + "/roles/" + PUBLIC_ROLE) is None
    api_clients = required(get(prefix + "/clients?clientId=" + API_CLIENT))
    api_id = api_clients[0]["id"] if api_clients else None

    # Keep catalog-specific edges on managed composites. Their non-catalog behavior
    # remains configuration as code. catalog-public itself belongs entirely to operators.
    for role in realm.get("roles", {}).get("realm", []):
        path = prefix + "/roles/" + urllib.parse.quote(role["name"], safe="")
        existing = get(path)
        if existing is None:
            if initialized:
                # Deleting a policy role must not restore its bootstrap grants.
                desired = role.setdefault("composites", {})
                desired["realm"] = [r for r in desired.get("realm", []) if r != PUBLIC_ROLE]
                for client, edges in desired.get("client", {}).items():
                    if client == API_CLIENT:
                        desired["client"][client] = [r for r in edges if not catalog_read(r)]
                if role["name"] == PUBLIC_ROLE:
                    role["composite"] = False
            continue
        composites = required(get(path + "/composites"))
        live_realm = [r["name"] for r in composites if not r.get("clientRole")]
        live_catalog = [r["name"] for r in composites if r.get("clientRole") and
                        r.get("containerId") == api_id and catalog_read(r["name"])]
        desired = role.setdefault("composites", {})
        if role["name"] == PUBLIC_ROLE:
            # Preserve all current edges, including custom composite roles. Resolve client
            # UUIDs to names because the import format addresses clients by clientId.
            clients = {c["id"]: c["clientId"] for c in required(get(prefix + "/clients"))}
            client_edges = {}
            for edge in composites:
                if edge.get("clientRole"):
                    client_edges.setdefault(clients[edge["containerId"]], []).append(edge["name"])
            role["composites"] = {"realm": live_realm, "client": client_edges}
            role["composite"] = bool(composites)
        else:
            desired_realm = desired.get("realm", [])
            desired["realm"] = [r for r in desired_realm if r != PUBLIC_ROLE]
            desired["realm"] += [r for r in live_realm if r == PUBLIC_ROLE]
            if seed_defaults and PUBLIC_ROLE in desired_realm and PUBLIC_ROLE not in desired["realm"]:
                desired["realm"].append(PUBLIC_ROLE)
            api_edges = desired.setdefault("client", {}).get(API_CLIENT, [])
            declared = api_edges if seed_defaults else [r for r in api_edges if not catalog_read(r)]
            desired["client"][API_CLIENT] = list(dict.fromkeys(declared + live_catalog))

    anonymous = required(get(prefix + "/clients?clientId=" + ANONYMOUS_CLIENT))
    if not anonymous:
        if initialized:
            # A deleted reader stays denied even though the managed client is recreated.
            for client in realm.get("clients", []):
                if client["clientId"] == ANONYMOUS_CLIENT:
                    client["enabled"] = False
            for user in realm.get("users", []):
                if user.get("serviceAccountClientId") == ANONYMOUS_CLIENT:
                    user.update(enabled=False, realmRoles=[], clientRoles={})
            for entry in realm.get("clientScopeMappings", {}).get(API_CLIENT, []):
                if entry.get("client") == ANONYMOUS_CLIENT:
                    entry["roles"] = []
        return realm  # Only the first feature rollout receives bootstrap mappings.
    live_client = anonymous[0]
    for client in realm.get("clients", []):
        if client["clientId"] == ANONYMOUS_CLIENT:
            for field in ("enabled", "serviceAccountsEnabled"):
                if field in live_client:
                    client[field] = live_client[field]
    anon_path = prefix + "/clients/" + live_client["id"]
    if not live_client.get("serviceAccountsEnabled", False):
        # Do not recreate a deliberately disabled service account through the users import.
        realm["users"] = [u for u in realm.get("users", []) if u.get("serviceAccountClientId") != ANONYMOUS_CLIENT]
        return realm
    account = required(get(anon_path + "/service-account-user"))
    mappings = required(get(prefix + "/users/" + account["id"] + "/role-mappings"))
    groups = required(get(prefix + "/users/" + account["id"] + "/groups"))
    for user in realm.get("users", []):
        if user.get("serviceAccountClientId") == ANONYMOUS_CLIENT:
            user["enabled"] = account["enabled"]
            user["groups"] = [group["path"] for group in groups]
            default_roles = {realm.get("defaultRole", {}).get("name"), observed.get("defaultRole", {}).get("name")}
            # Service accounts created through the Admin API inherit the signed-in default
            # role. It must not provide a second path around anonymous-only revocations.
            user["realmRoles"] = [r["name"] for r in mappings.get("realmMappings", []) if r["name"] not in default_roles]
            user["clientRoles"] = {name: [r["name"] for r in value["mappings"]]
                                   for name, value in mappings.get("clientMappings", {}).items()}
    # Scope restrictions on the anonymous client are also operator policy. Preserving them
    # prevents a deploy from undoing a restriction made in the client's scope settings.
    if api_id:
        live_scope = required(get(anon_path + "/scope-mappings/clients/" + api_id))
        for entry in realm.get("clientScopeMappings", {}).get(API_CLIENT, []):
            if entry.get("client") == ANONYMOUS_CLIENT:
                entry["roles"] = [r for r in entry.get("roles", []) if not catalog_read(r)]
                entry["roles"] += [r["name"] for r in live_scope if catalog_read(r["name"])]
    return realm


def main():
    if len(sys.argv) != 2:
        raise RuntimeError("usage: preserve-catalog-grants.py <rendered-realm.json>")
    with open(sys.argv[1], encoding="utf-8") as source:
        realm = json.load(source)
    client = Keycloak(os.environ["KEYCLOAK_URL"], os.environ["KEYCLOAK_USER"], os.environ["KEYCLOAK_PASSWORD"])
    json.dump(merge_catalog_policy(realm, client.get), sys.stdout, indent=2)
    sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (KeyError, ValueError, TypeError, AttributeError, OSError, RuntimeError) as error:
        # Never print response bodies, tokens, secrets or partially rendered configuration.
        print("catalog-policy preservation failed: " + (str(error) if isinstance(error, RuntimeError) else "invalid configuration or API response"), file=sys.stderr)
        sys.exit(1)
