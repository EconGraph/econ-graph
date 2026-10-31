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


def catalog_read(name):
    """Identify catalog reads; other capabilities stay managed as structural configuration."""
    return name.startswith("catalog.") and name.endswith(":read")


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


def warn(message):
    """Report on stderr; stdout carries the merged import."""
    print("warning: " + message, file=sys.stderr)


# keycloak-config-cli records what it created in realm attributes named
# de.adorsys.keycloak.config.state-default-<kind>-<n>: one JSON list of names, split into numbered
# chunks. It deletes what it created and the realm file no longer declares, and nothing else.
# "default" is its import cache key, which the realm import Job leaves unset. When the list
# shrinks by a chunk, config-cli leaves the old last chunk behind and reads only the first JSON
# value of the joined text; so does config_cli_state.
CONFIG_CLI_STATE = "de.adorsys.keycloak.config.state-default-"


def config_cli_state(attributes, kind):
    """The names config-cli recorded creating, for one kind of resource."""
    key = CONFIG_CLI_STATE + kind + "-"
    chunks = sorted((int(name[len(key):]), value) for name, value in attributes.items()
                    if name.startswith(key) and name[len(key):].isdigit())
    if not chunks:
        return set()
    joined = "".join(value for _, value in chunks)
    try:
        names, end = json.JSONDecoder().raw_decode(joined)
    except ValueError:
        names = None
    if not isinstance(names, list) or not all(isinstance(n, str) for n in names):
        raise RuntimeError("Unreadable keycloak-config-cli state (realm attributes %s*)" % key)
    if joined[end:].strip():
        warn("ignoring %d characters after the keycloak-config-cli state list in realm attributes %s*, "
             "as config-cli does: a chunk left over from a longer list, harmless; deleting the realm "
             "attribute with the highest number silences this" % (len(joined) - end, key))
    return set(names)


# Clients every Keycloak realm has, which config-cli never deletes (KeycloakUtil.isDefaultClient).
KEYCLOAK_CLIENTS = {"account", "account-console", "admin-cli", "broker", "realm-management",
                    "security-admin-console"}


def clients_config_cli_deletes(realm, attributes):
    """Client ids config-cli created and the realm file no longer declares."""
    if realm.get("clients") is None:
        return set()  # Without a clients list config-cli manages, and deletes, no client.
    return (config_cli_state(attributes, "clients") - {client["clientId"] for client in realm["clients"]}
            - KEYCLOAK_CLIENTS)


def roles_config_cli_deletes(realm, attributes, client=None):
    """Roles of CLIENT (realm roles when None) that config-cli created and the realm file no
    longer declares. A grant of one written into the import would fail it part way ("Cannot
    find client role"). As config-cli 6.5.1 does it (RoleImportService): without a roles.realm
    or roles.client list it deletes none of that kind, and with roles.client every role of a
    client missing from it. A client config-cli deletes takes all its roles with it, whoever
    made them (clients_config_cli_deletes)."""
    roles = realm.get("roles") or {}
    if client is None:
        declared = roles.get("realm")
    else:
        declared = (roles.get("client") or {}).get(client, []) if roles.get("client") is not None else None
    if declared is None:
        return set()
    kind = "roles-realm" if client is None else "roles-client-" + client
    return config_cli_state(attributes, kind) - {role["name"] for role in declared}


def without_deleted(names, deleted, where, owner=""):
    """NAMES less the roles the import deletes, naming each grant that goes with its role.
    OWNER, when given, names the client the roles belong to."""
    gone = [n for n in names if n in deleted]
    if gone:
        shown = ", ".join((owner + " role " if owner else "") + n for n in gone)
        warn("%s %s no longer in the realm file, so the import deletes %s and its grant on %s"
             % (shown, "is" if len(gone) == 1 else "are", "it" if len(gone) == 1 else "them", where))
    return [n for n in names if n not in deleted]


def warn_removed(target, live, declared):
    """Name the live non-catalog API roles the import removes from one target's scope mappings."""
    removed = sorted(r for r in live if not catalog_read(r) and r not in declared)
    if removed:
        warn("the realm import removes %s scope mappings the realm file doesn't declare on %s: %s; "
             "declare them in clientScopeMappings to keep them" % (API_CLIENT, target, ", ".join(removed)))


def preserve_scope_mappings(realm, get, prefix, api_id, attributes, seed_defaults, deleted_roles):
    """Keep live catalog reads in the scope mappings of every client and client scope.

    config-cli replaces all scope mappings of the API client's roles with the realm file's
    clientScopeMappings, on every client and client scope, declared or not. Catalog reads
    there are operator policy: each target's live ones are written into the import. The
    file's declared ones seed only a new realm, like the composites' catalog edges. Other
    API roles stay configuration as code; a live one the file doesn't declare is named
    before the import removes it. The anonymous reader is handled with its service account
    below. Clients config-cli is about to delete are skipped: their grants go with them,
    and an entry naming one would fail the import.
    """
    entries = realm.setdefault("clientScopeMappings", {}).setdefault(API_CLIENT, [])
    seen = set()
    for entry in entries:
        key = next(((kind, entry[kind]) for kind in ("client", "clientScope") if kind in entry), None)
        if key is None or ("client" in entry and "clientScope" in entry):
            raise RuntimeError("Each realm file %s scope mapping names one client or one client scope"
                               % API_CLIENT)
        if key in seen:
            raise RuntimeError("The realm file declares %s scope mappings twice for %s %s"
                               % (API_CLIENT, key[0], key[1]))
        seen.add(key)
    deleted = clients_config_cli_deletes(realm, attributes)
    targets = [("client", c["id"], c["clientId"], "/clients/")
               for c in required(get(prefix + "/clients"))
               if c["clientId"] != ANONYMOUS_CLIENT and c["clientId"] not in deleted]
    targets += [("clientScope", s["id"], s["name"], "/client-scopes/")
                for s in required(get(prefix + "/client-scopes"))]
    live_targets = set()
    for kind, target_id, name, path in sorted(targets, key=lambda t: (t[0], t[2])):
        live_targets.add((kind, name))
        label = ("client " if kind == "client" else "client scope ") + name
        live = [r["name"] for r in required(get(prefix + path + target_id + "/scope-mappings/clients/" + api_id))]
        live = without_deleted(live, deleted_roles, label)
        entry = next((e for e in entries if e.get(kind) == name), None)
        declared = entry.get("roles", []) if entry else []
        warn_removed(label, live, declared)
        kept = declared if seed_defaults else [r for r in declared if not catalog_read(r)]
        roles = list(dict.fromkeys(kept + [r for r in live if catalog_read(r)]))
        if entry:
            entry["roles"] = roles
        elif roles:
            entries.append({kind: name, "roles": roles})
    if not seed_defaults:
        # A declared target the import creates (or recreates after an operator deleted it)
        # gets no catalog read from the file.
        for entry in entries:
            for kind in ("client", "clientScope"):
                if kind in entry and (kind, entry[kind]) not in live_targets and (kind, entry[kind]) != ("client", ANONYMOUS_CLIENT):
                    entry["roles"] = [r for r in entry.get("roles", []) if not catalog_read(r)]


def declared_catalog_reads(realm, role_names):
    """The catalog reads the merged realm file's realm roles ROLE_NAMES grant, at any depth."""
    roles = {role["name"]: role for role in realm.get("roles", {}).get("realm", [])}
    found, pending, seen = set(), list(role_names), set()
    while pending:
        name = pending.pop()
        if name in seen or name not in roles:
            continue
        seen.add(name)
        composites = roles[name].get("composites") or {}
        found.update(r for r in (composites.get("client") or {}).get(API_CLIENT, []) if catalog_read(r))
        pending.extend(composites.get("realm") or [])
    return found


def check_anonymous_client_scopes(realm, get, prefix, anon_id, api_id, live=True, deleted_roles=()):
    """Refuse catalog reads that reach anonymous tokens other than through the anonymous
    client's own direct scope mapping of each catalog read.

    The reader's token carries every role it holds that any of its client scopes, or its own
    scope mapping, admits, with composite roles expanded. A catalog read admitted by `basic`,
    or by a realm role such as catalog-public mapped on a scope, would reach visitors even
    after it was removed from the reader's own scope mapping, the supported switch for
    anonymous access. The client scopes are those assigned to it live and in the realm file;
    realm default scopes, assigned to a recreated reader, are named with a warning. Live
    scopes are read with composites expanded; the realm file's mappings through the
    declared realm composites. Without a live realm only the file is read. Catalog reads the
    import deletes (DELETED_ROLES) reach no token after it, so they aren't refused.
    """
    admitted = {}  # (kind, name) -> catalog reads admitted other than directly by the reader
    skipped = {}  # the same for catalog reads the import deletes
    def admit(key, roles):
        roles = {r for r in roles if catalog_read(r)}
        if roles - set(deleted_roles):
            admitted.setdefault(key, set()).update(roles - set(deleted_roles))
        if roles & set(deleted_roles):
            skipped.setdefault(key, set()).update(roles & set(deleted_roles))
    for entry in realm.get("clientScopeMappings", {}).get(API_CLIENT, []):
        if "clientScope" in entry:
            admit(("clientScope", entry["clientScope"]), entry.get("roles", []))
    for entry in realm.get("scopeMappings", []):
        target = ("clientScope", entry["clientScope"]) if "clientScope" in entry else ("client", entry.get("client"))
        admit(target, declared_catalog_reads(realm, entry.get("roles", [])))
    assigned, defaults, ids = set(), set(), {}
    for client in realm.get("clients", []):
        if client["clientId"] == ANONYMOUS_CLIENT:
            assigned.update(client.get("defaultClientScopes", []), client.get("optionalClientScopes", []))
    defaults.update(realm.get("defaultDefaultClientScopes", []), realm.get("defaultOptionalClientScopes", []))
    if live:
        for kind in ("default", "optional"):
            scopes = required(get(prefix + "/default-" + kind + "-client-scopes"))
            defaults.update(s["name"] for s in scopes)
            ids.update((s["name"], s["id"]) for s in scopes)
            if anon_id:
                scopes = required(get(prefix + "/clients/" + anon_id + "/" + kind + "-client-scopes"))
                assigned.update(s["name"] for s in scopes)
                ids.update((s["name"], s["id"]) for s in scopes)
    if live and api_id:
        # A scope the realm file newly assigns is read too, if it exists already.
        ids.update((s["name"], s["id"]) for s in required(get(prefix + "/client-scopes"))
                   if s["name"] in assigned | defaults)
    if api_id:
        for name, scope_id in ids.items():
            admit(("clientScope", name), [r["name"] for r in required(
                get(prefix + "/client-scopes/" + scope_id + "/scope-mappings/clients/" + api_id + "/composite"))])
        if anon_id:
            # The reader's own scope may admit catalog reads only directly, one by one.
            own = prefix + "/clients/" + anon_id + "/scope-mappings/clients/" + api_id
            direct = {r["name"] for r in required(get(own))}
            admit(("client", ANONYMOUS_CLIENT),
                  {r["name"] for r in required(get(own + "/composite"))} - direct)
    reaching = [("clientScope", name) for name in sorted(assigned)] + [("client", ANONYMOUS_CLIENT)]
    for key in reaching:
        if key in skipped:
            warn("%s admits %s to %s tokens until the import deletes %s"
                 % ("client scope " + key[1] if key[0] == "clientScope" else "its own scope",
                    ", ".join(sorted(skipped[key])), ANONYMOUS_CLIENT, "it" if len(skipped[key]) == 1 else "them"))
    refused = sorted("client scope %s (%s)" % (name, ", ".join(sorted(admitted[("clientScope", name)])))
                     for name in assigned if ("clientScope", name) in admitted)
    if ("client", ANONYMOUS_CLIENT) in admitted:
        refused.append("its own scope, through realm roles, its own client roles or full scope (%s)"
                       % ", ".join(sorted(admitted[("client", ANONYMOUS_CLIENT)])))
    if refused:
        raise RuntimeError(
            "Catalog reads reach %s's tokens other than through its own scope mapping, so they would "
            "reach anonymous visitors whatever that says; remove these scope mappings in Keycloak (or "
            "the realm file) before deploying, and use scripts/keycloak/set-catalog-access.sh "
            "--make-public for public catalogs: %s" % (ANONYMOUS_CLIENT, "; ".join(refused)))
    for name in sorted(defaults - assigned):
        if ("clientScope", name) in admitted:
            warn("realm default client scope %s admits %s; a recreated %s would be assigned it and give "
                 "anonymous visitors those catalogs" % (name, ", ".join(sorted(admitted[("clientScope", name)])),
                                                         ANONYMOUS_CLIENT))


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
    # The deploy owns the realm-level frontendUrl, which overrides KC_HOSTNAME: a
    # public deploy sets it to its issuer URL (KC_REALM_FRONTEND_URL), and any other
    # import (localhost only, compose, a production dump restored elsewhere) clears
    # it, so a stale public URL never keeps being issued. Keycloak treats an empty
    # frontendUrl as unset; an absent key would leave the stored value in place.
    frontend_url = os.environ.get("KC_REALM_FRONTEND_URL")
    if frontend_url:
        attributes["frontendUrl"] = frontend_url
    elif attributes.get("frontendUrl"):
        attributes["frontendUrl"] = ""
    # Completion is recorded by a separate container only after config-cli succeeds.
    attributes.pop(INITIALIZED, None)
    attributes.pop(PENDING, None)
    initialized = observed_attrs.get(INITIALIZED) == "true"
    if initialized:
        attributes[INITIALIZED] = "true"
    if observed is None:
        attributes[PENDING] = json.dumps({"policy": True, "reader": True}, sort_keys=True)
        realm["attributes"] = attributes
        check_anonymous_client_scopes(realm, get, prefix, None, None, live=False)
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
    deleted_roles = roles_config_cli_deletes(realm, observed_attrs, API_CLIENT)

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
        live_catalog = without_deleted([r["name"] for r in composites if r.get("clientRole") and
                                        r.get("containerId") == api_id and catalog_read(r["name"])],
                                       deleted_roles, "role " + role["name"])
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

    if api_id:
        preserve_scope_mappings(realm, get, prefix, api_id, observed_attrs, seed_defaults, deleted_roles)
    check_anonymous_client_scopes(realm, get, prefix, anonymous[0]["id"] if anonymous else None, api_id,
                                  deleted_roles=deleted_roles)

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
        for entry in realm.get("clientScopeMappings", {}).get(API_CLIENT, []):
            if entry.get("client") == ANONYMOUS_CLIENT:
                live = without_deleted([r["name"] for r in live_scope], deleted_roles, "client " + ANONYMOUS_CLIENT)
                warn_removed("client " + ANONYMOUS_CLIENT, live, entry.get("roles", []))
                entry["roles"] = [r for r in entry.get("roles", []) if not catalog_read(r)]
                entry["roles"] += [r for r in live if catalog_read(r)]
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
            account_label = "the %s service account" % ANONYMOUS_CLIENT
            user["realmRoles"] = without_deleted(
                user["realmRoles"], roles_config_cli_deletes(realm, observed_attrs), account_label)
            deleted_clients = clients_config_cli_deletes(realm, observed_attrs)
            for name in user["clientRoles"]:
                if name in deleted_clients:
                    # The client goes, with every role on it; a grant left in would fail the import.
                    warn("client %s is no longer in the realm file, so the import deletes it with its roles, "
                         "dropping grants of %s on %s" % (name, ", ".join(user["clientRoles"][name]), account_label))
                    user["clientRoles"][name] = []
                    continue
                deleted = deleted_roles if name == API_CLIENT else roles_config_cli_deletes(realm, observed_attrs, name)
                user["clientRoles"][name] = without_deleted(user["clientRoles"][name], deleted, account_label,
                                                            "" if name == API_CLIENT else name)
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
