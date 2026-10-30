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
PENDING = "econgraph.catalog-policy-bootstrap-pending"
PUBLIC_ROLE = "catalog-public"
ANONYMOUS_CLIENT = "econgraph-anonymous"
API_CLIENT = "econ-graph-api"

# Catalog read roles whose name changed in the realm file: old name -> new name. Live catalog
# policy names roles by their old name, so the render step carries each grant over to the new
# name, and keeps the old role declared for that one import so keycloak-config-cli (which
# deletes a client role it created once the file drops it) does not delete it, with every
# grant it carries, before those grants are moved. After the import, --complete grants the new
# role to every remaining holder of the old one and deletes the old role (see
# retire_renamed_roles for exactly which holders). A later deploy finds no old role and does
# nothing. A release from before a rename knows neither name change; see config/keycloak/README.md
# before redeploying one.
RENAMED_CATALOG_ROLES = {
    "catalog.boc:read": "catalog.bank_of_canada:read",
}


def catalog_read(name):
    """Identify catalog reads; other capabilities stay managed as structural configuration."""
    return name.startswith("catalog.") and name.endswith(":read")


def renamed(names):
    """Live catalog role names under their current names, in order, without duplicates."""
    return list(dict.fromkeys(RENAMED_CATALOG_ROLES.get(name, name) for name in names))


class NoRedirect(urllib.request.HTTPRedirectHandler):
    """Keep administrator credentials on the configured Keycloak server."""
    def redirect_request(self, request, response, code, message, headers, new_url):
        """Reject redirects instead of forwarding a credential or bearer header."""
        return None


class Keycloak:
    """Read Admin REST state without treating unavailable resources as bootstrap defaults."""
    def __init__(self, base_url, user, password, timeout=600):
        """Authenticate once, waiting for startup only on transient availability failures."""
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
        """Return JSON or None for HTTP 404; authentication and transport failures abort."""
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

    def put(self, path, body):
        """Write the completion attribute without following redirects or logging payloads."""
        request = urllib.request.Request(self.base_url + path, data=json.dumps(body).encode(),
            headers={"Authorization": "Bearer " + self.token, "Content-Type": "application/json"}, method="PUT")
        try:
            with self.opener.open(request, timeout=10):
                pass
        except (urllib.error.URLError, TimeoutError):
            raise RuntimeError("Could not record successful catalog bootstrap") from None

    def send(self, method, path, body=None):
        """POST, PUT or DELETE a catalog role change. A DELETE of something already gone
        succeeds, so a retried cleanup is harmless; every other failure aborts."""
        headers = {"Authorization": "Bearer " + self.token}
        data = None
        if body is not None:
            data = json.dumps(body).encode()
            headers["Content-Type"] = "application/json"
        request = urllib.request.Request(self.base_url + path, data=data, headers=headers, method=method)
        try:
            with self.opener.open(request, timeout=10):
                pass
        except urllib.error.HTTPError as error:
            if not (method == "DELETE" and error.code == 404):
                raise RuntimeError("Keycloak %s failed (HTTP %s)" % (method, error.code)) from None
        except (urllib.error.URLError, TimeoutError):
            raise RuntimeError("Keycloak %s failed" % method) from None


def required(value):
    """Only entity lookups may return missing; a disappearing relation is an unsafe snapshot."""
    if value is None:
        raise RuntimeError("Keycloak resource disappeared during catalog-policy snapshot")
    return value


def check_public_declaration(role):
    """The checked-in catalog-public defaults may also hold only catalog reads."""
    composites = role.get("composites") or {}
    clients = composites.get("client") or {}
    if composites.get("realm") or set(clients) - {API_CLIENT} or not all(
            catalog_read(name) for name in clients.get(API_CLIENT, [])):
        raise RuntimeError("The realm file's %s may contain only %s catalog.*:read client roles"
                           % (PUBLIC_ROLE, API_CLIENT))


def composite_catalog_reads(roles):
    """Names of catalog.*:read roles that are composites. Signed-in accounts and the anonymous
    reader inherit catalog reads through catalog-public, so a catalog read holding any other
    role, at any depth, would hand that role to every account. A non-composite role nests
    nothing, which rules out every depth at once."""
    return sorted(role["name"] for role in roles if catalog_read(role["name"]) and (
        role.get("composite") or role.get("composites")))


def check_catalog_reads_are_plain(realm, get, prefix, api_id):
    """Refuse when a catalog read role, in the realm file or live in Keycloak, is a composite."""
    declared = composite_catalog_reads(realm.get("roles", {}).get("client", {}).get(API_CLIENT, []))
    if declared:
        raise RuntimeError("The realm file's catalog read roles may not be composites: "
                           + ", ".join(declared))
    if api_id is None:
        return
    live = composite_catalog_reads(required(
        get(prefix + "/clients/" + api_id + "/roles?briefRepresentation=false")))
    if live:
        raise RuntimeError("Catalog read roles may not be composites; remove their associated "
                           "roles in Keycloak before deploying: " + ", ".join(live))


def keep_renamed_roles_for_this_import(realm, get, prefix, api_id):
    """Declare each old name of a renamed catalog role that still exists in Keycloak, so the
    import does not delete it before --complete moves its remaining grants (see
    RENAMED_CATALOG_ROLES). The import's own edges already use the new names."""
    if api_id is None:
        return
    live = {r["name"] for r in required(
        get(prefix + "/clients/" + api_id + "/roles?briefRepresentation=false"))}
    declared = realm.setdefault("roles", {}).setdefault("client", {}).setdefault(API_CLIENT, [])
    names = {role["name"] for role in declared}
    for old, new in RENAMED_CATALOG_ROLES.items():
        if new not in names:
            raise RuntimeError("The realm file must define %s, the new name of %s" % (new, old))
        if old in live and old not in names:
            declared.append({"name": old, "description": "Renamed to %s; removed by this deploy" % new})


def retire_renamed_roles(realm, get, send, page=100):
    """After the import: grant each renamed role's new name to every remaining holder of its
    old name, then delete the old role. The holders covered are exactly: users and groups the
    old role is mapped to directly, composite realm roles, composite client roles of every
    client, and every client's own scope mappings. Client scopes' scope mappings are not
    covered (the import already replaces them from the realm file), nor is anything else that
    names the role; deleting the role removes it from those. Does nothing once the old role is
    gone, so it is safe on every deploy."""
    prefix = "/admin/realms/" + urllib.parse.quote(realm["realm"], safe="")
    api_clients = required(get(prefix + "/clients?clientId=" + API_CLIENT))
    if not api_clients:
        return []
    api_id = api_clients[0]["id"]
    roles_path = prefix + "/clients/" + api_id + "/roles/"
    retired = []

    def paged(path):
        items, first = [], 0
        while True:
            batch = required(get("%s?first=%d&max=%d" % (path, first, page)))
            items += batch
            if len(batch) < page:
                return items
            first += page

    for old, new in RENAMED_CATALOG_ROLES.items():
        old_path = roles_path + urllib.parse.quote(old, safe="")
        old_role = get(old_path)
        if old_role is None:
            continue
        new_role = required(get(roles_path + urllib.parse.quote(new, safe="")))
        grant = [new_role]

        def holds(roles, name):
            return any(r["name"] == name and r.get("clientRole", True) and
                       r.get("containerId", api_id) == api_id for r in roles)

        for user in paged(old_path + "/users"):
            mapped = required(get(prefix + "/users/" + user["id"] + "/role-mappings/clients/" + api_id))
            if not holds(mapped, new):
                send("POST", prefix + "/users/" + user["id"] + "/role-mappings/clients/" + api_id, grant)
        for group in paged(old_path + "/groups"):
            mapped = required(get(prefix + "/groups/" + group["id"] + "/role-mappings/clients/" + api_id))
            if not holds(mapped, new):
                send("POST", prefix + "/groups/" + group["id"] + "/role-mappings/clients/" + api_id, grant)
        clients = required(get(prefix + "/clients"))
        candidates = required(get(prefix + "/roles?briefRepresentation=false"))
        for client in clients:
            candidates += required(get(prefix + "/clients/" + client["id"] + "/roles?briefRepresentation=false"))
        for role in candidates:
            if not role.get("composite") or role["id"] == old_role["id"]:
                continue
            composites = required(get(prefix + "/roles-by-id/" + role["id"] + "/composites"))
            if holds(composites, old) and not holds(composites, new):
                send("POST", prefix + "/roles-by-id/" + role["id"] + "/composites", grant)
        for client in clients:
            scope_path = prefix + "/clients/" + client["id"] + "/scope-mappings/clients/" + api_id
            scope = required(get(scope_path))
            if holds(scope, old) and not holds(scope, new):
                send("POST", scope_path, grant)
        send("DELETE", old_path)
        retired.append(old)
    return retired


def merge_catalog_policy(realm, get):
    """Merge current operator policy into a rendered import, seeding defaults only once."""
    for role in realm.get("roles", {}).get("realm", []):
        if role["name"] == PUBLIC_ROLE:
            check_public_declaration(role)
    prefix = "/admin/realms/" + urllib.parse.quote(realm["realm"], safe="")
    check_catalog_reads_are_plain(realm, get, prefix, None)
    observed = get(prefix)
    observed_attrs = (observed or {}).get("attributes") or {}
    attributes = dict(observed_attrs)
    attributes.update(realm.get("attributes") or {})
    # Completion is recorded by a separate container only after config-cli succeeds.
    attributes.pop(INITIALIZED, None)
    attributes.pop(PENDING, None)
    initialized = observed_attrs.get(INITIALIZED) == "true"
    if initialized:
        attributes[INITIALIZED] = "true"
    if observed is None:
        attributes[PENDING] = json.dumps({"policy": True, "reader": True}, sort_keys=True)
        realm["attributes"] = attributes
        return realm
    anonymous = required(get(prefix + "/clients?clientId=" + ANONYMOUS_CLIENT))
    pending = json.loads(observed_attrs[PENDING]) if PENDING in observed_attrs else None
    if PENDING in observed_attrs and (not isinstance(pending, dict) or set(pending) != {"policy", "reader"}
                                or any(type(v) is not bool for v in pending.values())):
        raise RuntimeError("Invalid catalog bootstrap recovery state")
    seed = {"policy": False, "reader": False}
    if not initialized:
        # Pending flags identify resources absent at the first attempt. They permit retrying
        # a partial initial import without resetting an already operator-managed resource.
        seed = pending if pending is not None else {
            "policy": get(prefix + "/roles/" + PUBLIC_ROLE) is None,
            "reader": not anonymous,
        }
        if any(seed.values()):
            attributes[PENDING] = json.dumps(seed, sort_keys=True)
    realm["attributes"] = attributes
    seed_defaults = seed["policy"]
    api_clients = required(get(prefix + "/clients?clientId=" + API_CLIENT))
    api_id = api_clients[0]["id"] if api_clients else None
    check_catalog_reads_are_plain(realm, get, prefix, api_id)
    file_roles = {r["name"] for r in realm.get("roles", {}).get("client", {}).get(API_CLIENT, [])}

    def check_declared(names, where):
        """Refuse to carry a live catalog grant of a role the realm file does not define:
        keycloak-config-cli would delete the role, with every grant it carries, and then fail
        on the edge. This is what deploying a release from before a role rename looks like."""
        undeclared = sorted(n for n in names if catalog_read(n) and n not in file_roles)
        if undeclared:
            raise RuntimeError(
                "%s holds catalog roles the realm file does not define: %s. Importing would delete "
                "them with every grant. Deploying a release from before a catalog role rename? "
                "Rename the role back first (config/keycloak/README.md, Renaming a catalog role)"
                % (where, ", ".join(undeclared)))

    keep_renamed_roles_for_this_import(realm, get, prefix, api_id)

    # Keep catalog-specific edges on managed composites. Their non-catalog behavior
    # remains configuration as code. catalog-public itself belongs entirely to operators.
    for role in realm.get("roles", {}).get("realm", []):
        path = prefix + "/roles/" + urllib.parse.quote(role["name"], safe="")
        existing = get(path)
        if role["name"] == PUBLIC_ROLE and seed_defaults:
            continue  # Retry the originally declared policy defaults after a partial import.
        if existing is None:
            if not seed_defaults:
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
        live_catalog = renamed(r["name"] for r in composites if r.get("clientRole") and
                               r.get("containerId") == api_id and catalog_read(r["name"]))
        check_declared(live_catalog, "Realm role " + role["name"])
        desired = role.setdefault("composites", {})
        if role["name"] == PUBLIC_ROLE:
            # Every account inherits catalog-public, so it may hold nothing but catalog reads.
            # Preserve exactly those edges and refuse the deploy on anything else, rather than
            # silently importing (or silently dropping) a broader grant an operator added.
            unexpected = [edge for edge in composites if not (
                edge.get("clientRole") and edge.get("containerId") == api_id and catalog_read(edge["name"]))]
            if unexpected:
                clients = {c["id"]: c["clientId"] for c in required(get(prefix + "/clients"))}
                names = sorted(clients.get(edge.get("containerId"), "?") + "/" + edge["name"]
                               if edge.get("clientRole") else edge["name"] for edge in unexpected)
                raise RuntimeError(
                    "%s may contain only %s catalog.*:read client roles; remove these composites "
                    "in Keycloak before deploying: %s" % (PUBLIC_ROLE, API_CLIENT, ", ".join(names)))
            role["composites"] = {"realm": [], "client": {API_CLIENT: live_catalog} if live_catalog else {}}
            role["composite"] = bool(live_catalog)
        else:
            desired_realm = desired.get("realm", [])
            desired["realm"] = [r for r in desired_realm if r != PUBLIC_ROLE]
            desired["realm"] += [r for r in live_realm if r == PUBLIC_ROLE]
            if seed_defaults and PUBLIC_ROLE in desired_realm and PUBLIC_ROLE not in desired["realm"]:
                desired["realm"].append(PUBLIC_ROLE)
            api_edges = desired.setdefault("client", {}).get(API_CLIENT, [])
            declared = api_edges if seed_defaults else [r for r in api_edges if not catalog_read(r)]
            desired["client"][API_CLIENT] = list(dict.fromkeys(declared + live_catalog))

    if seed["reader"]:
        return realm  # The reader was absent at bootstrap; retry its declared defaults.
    if not anonymous:
        if not seed["reader"]:
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
    # Scope restrictions on the anonymous client are also operator policy. Preserving them
    # prevents a deploy from undoing a restriction made in the client's scope settings.
    if api_id:
        live_scope = required(get(anon_path + "/scope-mappings/clients/" + api_id))
        live_scope = renamed(r["name"] for r in live_scope if catalog_read(r["name"]))
        check_declared(live_scope, "The %s scope mapping" % ANONYMOUS_CLIENT)
        for entry in realm.get("clientScopeMappings", {}).get(API_CLIENT, []):
            if entry.get("client") == ANONYMOUS_CLIENT:
                entry["roles"] = [r for r in entry.get("roles", []) if not catalog_read(r)]
                entry["roles"] += live_scope
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
            user["clientRoles"] = {
                name: renamed(r["name"] for r in value["mappings"]) if name == API_CLIENT
                else [r["name"] for r in value["mappings"]]
                for name, value in mappings.get("clientMappings", {}).items()}
            check_declared(user["clientRoles"].get(API_CLIENT, []),
                           "The %s service account" % ANONYMOUS_CLIENT)
    return realm


def complete_bootstrap(realm, get, put):
    """Commit the lifecycle marker after the import, preserving freshly read realm attributes."""
    prefix = "/admin/realms/" + urllib.parse.quote(realm["realm"], safe="")
    observed = required(get(prefix))
    attributes = dict(observed.get("attributes") or {})
    if attributes.get(INITIALIZED) == "true":
        return
    required(get(prefix + "/roles/" + PUBLIC_ROLE))
    if not required(get(prefix + "/clients?clientId=" + ANONYMOUS_CLIENT)):
        raise RuntimeError("Anonymous reader bootstrap is incomplete")
    attributes[INITIALIZED] = "true"
    attributes.pop(PENDING, None)
    put(prefix, {"attributes": attributes})


def main():
    """Prepare the import, or finalize its lifecycle only after config-cli has succeeded."""
    complete = len(sys.argv) == 3 and sys.argv[1] == "--complete"
    if not (len(sys.argv) == 2 or complete):
        raise RuntimeError("usage: preserve-catalog-grants.py [--complete] <rendered-realm.json>")
    with open(sys.argv[2] if complete else sys.argv[1], encoding="utf-8") as source:
        realm = json.load(source)
    client = Keycloak(os.environ["KEYCLOAK_URL"], os.environ["KEYCLOAK_USER"], os.environ["KEYCLOAK_PASSWORD"])
    if complete:
        complete_bootstrap(realm, client.get, client.put)
        print("Catalog policy initialization recorded")
        for old in retire_renamed_roles(realm, client.get, client.send):
            print("Renamed catalog role retired: %s -> %s" % (old, RENAMED_CATALOG_ROLES[old]))
    else:
        json.dump(merge_catalog_policy(realm, client.get), sys.stdout, indent=2)
        sys.stdout.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (KeyError, ValueError, TypeError, AttributeError, OSError, RuntimeError) as error:
        # Never print response bodies, tokens, secrets or partially rendered configuration.
        print("catalog-policy preservation failed: " + (str(error) if isinstance(error, RuntimeError) else "invalid configuration or API response"), file=sys.stderr)
        sys.exit(1)
