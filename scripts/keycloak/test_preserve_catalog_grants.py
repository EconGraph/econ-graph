"""Deterministic tests for catalog policy preservation; no Keycloak credentials needed."""
import contextlib
import copy
import io
import importlib.util
import json
from pathlib import Path
import unittest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("preserve", HERE / "preserve-catalog-grants.py")
preserve = importlib.util.module_from_spec(spec)
spec.loader.exec_module(preserve)


class CatalogPolicyTests(unittest.TestCase):
    def setUp(self):
        self.realm = json.loads((HERE.parent.parent / "config/keycloak/econ-graph-realm.json").read_text())
        self.prefix = "/admin/realms/econ-graph"
        self.state = {
            self.prefix: {"attributes": {preserve.INITIALIZED: "true"}},
            self.prefix + "/clients?clientId=econ-graph-api": [{"id": "api", "clientId": "econ-graph-api"}],
            self.prefix + "/clients?clientId=econgraph-anonymous": [{"id": "anon", "clientId": "econgraph-anonymous", "enabled": True, "serviceAccountsEnabled": True}],
            self.prefix + "/clients": [{"id": "api", "clientId": "econ-graph-api"},
                                       {"id": "anon", "clientId": "econgraph-anonymous"}],
            self.prefix + "/clients/api/scope-mappings/clients/api": [],
            self.prefix + "/client-scopes": [],
            self.prefix + "/clients/anon/default-client-scopes": [{"id": "basic", "name": "basic"}],
            self.prefix + "/clients/anon/optional-client-scopes": [],
            self.prefix + "/default-default-client-scopes": [{"id": "profile", "name": "profile"}],
            self.prefix + "/default-optional-client-scopes": [],
            self.prefix + "/client-scopes/basic/scope-mappings/clients/api": [],
            self.prefix + "/client-scopes/profile/scope-mappings/clients/api": [],
            self.prefix + "/clients/anon/service-account-user": {"id": "reader", "enabled": True},
            self.prefix + "/users/reader/role-mappings": {"realmMappings": []},
            self.prefix + "/users/reader/groups": [],
            self.prefix + "/clients/anon/scope-mappings/clients/api": [],
            self.prefix + "/clients/api/roles?briefRepresentation=false": [
                {"name": role["name"], "composite": False}
                for role in self.realm["roles"]["client"]["econ-graph-api"]
            ],
        }
        for role in self.realm["roles"]["realm"]:
            path = self.prefix + "/roles/" + role["name"]
            self.state[path] = {"name": role["name"], "composite": False}
            self.state[path + "/composites"] = []

    def get(self, path):
        """The fake Keycloak: a composite listing not set by a test is the direct one."""
        if path not in self.state and path.endswith("/composite"):
            path = path[:-len("/composite")]
        return copy.deepcopy(self.state.get(path))

    def merge(self):
        return preserve.merge_catalog_policy(copy.deepcopy(self.realm), self.get)

    @staticmethod
    def role(realm, name):
        return next(role for role in realm["roles"]["realm"] if role["name"] == name)

    def test_new_realm_seeds_defaults(self):
        self.state[self.prefix] = None
        merged = self.merge()
        self.assertEqual(self.role(merged,"catalog-public")["composites"],self.role(self.realm,"catalog-public")["composites"])
        self.assertNotIn(preserve.INITIALIZED,merged["attributes"])
        self.assertEqual(json.loads(merged["attributes"][preserve.PENDING]),{"policy":True,"reader":True})

    def test_existing_empty_sets_and_removed_memberships_are_authoritative(self):
        merged = self.merge()
        self.assertEqual(self.role(merged,"catalog-public")["composites"],{"realm":[],"client":{}})
        self.assertNotIn("catalog-public",self.role(merged,"user")["composites"]["realm"])
        self.assertEqual(merged["users"][0]["realmRoles"],[])
        self.assertEqual(merged["clientScopeMappings"]["econ-graph-api"][0]["roles"],[])
        # Operational defaults remain configuration as code.
        self.assertIn("annotation:create",self.role(merged,"user")["composites"]["client"]["econ-graph-api"])
        self.assertIn("admin.users:delete",self.role(merged,"admin")["composites"]["client"]["econ-graph-api"])

    def test_selective_grants_and_custom_reader_memberships_survive(self):
        self.state[self.prefix + "/roles/catalog-public/composites"] = [
            {"name":"catalog.bea:read","clientRole":True,"containerId":"api"},
        ]
        self.state[self.prefix + "/roles/user/composites"] = [{"name":"catalog-public","clientRole":False}]
        self.state[self.prefix + "/users/reader/role-mappings"] = {
            "realmMappings":[{"name":"custom-public-policy"}],
            "clientMappings":{"econ-graph-api":{"mappings":[{"name":"catalog.fred:read"}]}},
        }
        self.state[self.prefix + "/users/reader/groups"] = [{"path": "/public-readers"}]
        merged = self.merge()
        self.assertEqual(merged["users"][0]["groups"],["/public-readers"])
        self.assertEqual(self.role(merged,"catalog-public")["composites"],{"realm":[],"client":{"econ-graph-api":["catalog.bea:read"]}})
        self.assertEqual(merged["users"][0]["realmRoles"],["custom-public-policy"])
        self.assertEqual(merged["users"][0]["clientRoles"],{"econ-graph-api":["catalog.fred:read"]})
        self.assertIn("catalog-public",self.role(merged,"user")["composites"]["realm"])

    def test_public_policy_holding_anything_but_catalog_reads_fails_the_deploy(self):
        # Every account inherits catalog-public: a broader edge must stop the deploy, never be
        # preserved (granting it to everyone) or silently dropped.
        self.state[self.prefix + "/clients"].append({"id": "other", "clientId": "other-client"})
        for edge, shown in [
            ({"name":"custom-public-policy","clientRole":False}, "custom-public-policy"),
            ({"name":"admin","clientRole":False}, "admin"),
            ({"name":"annotation:create","clientRole":True,"containerId":"api"}, "econ-graph-api/annotation:create"),
            ({"name":"admin.users:delete","clientRole":True,"containerId":"api"}, "econ-graph-api/admin.users:delete"),
            ({"name":"catalog.fred:read","clientRole":True,"containerId":"other"}, "other-client/catalog.fred:read"),
        ]:
            self.state[self.prefix + "/roles/catalog-public/composites"] = [
                {"name":"catalog.bea:read","clientRole":True,"containerId":"api"}, edge]
            with self.assertRaises(RuntimeError) as raised:
                self.merge()
            self.assertIn(shown, str(raised.exception))

    def test_a_composite_catalog_read_role_fails_the_deploy(self):
        # admin.users:delete nested under catalog.fred:read would reach every account through
        # catalog-public, however deep the nesting: a catalog read must not be a composite.
        roles = self.state[self.prefix + "/clients/api/roles?briefRepresentation=false"]
        fred = next(role for role in roles if role["name"] == "catalog.fred:read")
        fred["composite"] = True
        with self.assertRaises(RuntimeError) as raised:
            self.merge()
        self.assertIn("catalog.fred:read", str(raised.exception))
        self.assertNotIn("catalog.bea:read", str(raised.exception))
        fred["composite"] = False
        # Other roles may be composites, as the realm file's own staff roles are.
        next(role for role in roles if not role["name"].startswith("catalog."))["composite"] = True
        self.merge()

    def test_a_composite_catalog_read_in_the_realm_file_fails_even_for_a_new_realm(self):
        self.state[self.prefix] = None
        fred = next(role for role in self.realm["roles"]["client"]["econ-graph-api"]
                    if role["name"] == "catalog.fred:read")
        fred.update(composite=True, composites={"client": {"econ-graph-api": ["admin.users:delete"]}})
        with self.assertRaises(RuntimeError) as raised:
            self.merge()
        self.assertIn("catalog.fred:read", str(raised.exception))

    def test_realm_file_public_policy_is_checked_too(self):
        self.state[self.prefix] = None  # a new realm seeds the declared defaults
        self.role(self.realm, "catalog-public")["composites"]["client"]["econ-graph-api"].append("annotation:create")
        with self.assertRaises(RuntimeError):
            self.merge()
        self.role(self.realm, "catalog-public")["composites"]["client"]["econ-graph-api"].pop()
        self.role(self.realm, "catalog-public")["composites"]["realm"] = ["user"]
        with self.assertRaises(RuntimeError):
            self.merge()

    def test_reader_does_not_inherit_signed_in_default_grants(self):
        self.state[self.prefix + "/users/reader/role-mappings"] = {"realmMappings": [
            {"name": "default-roles-econ-graph"}, {"name": "catalog-public"},
        ]}
        self.assertEqual(self.merge()["users"][0]["realmRoles"],["catalog-public"])

    def test_disabled_reader_is_not_reenabled(self):
        self.state[self.prefix + "/clients?clientId=econgraph-anonymous"][0]["enabled"] = False
        self.state[self.prefix + "/clients/anon/service-account-user"]["enabled"] = False
        merged = self.merge()
        reader=next(c for c in merged["clients"] if c["clientId"]=="econgraph-anonymous")
        self.assertFalse(reader["enabled"])
        self.assertFalse(merged["users"][0]["enabled"])

    def test_deleted_policy_or_client_is_not_bootstrapped_again(self):
        self.state[self.prefix + "/roles/catalog-public"] = None
        self.state[self.prefix + "/clients?clientId=econgraph-anonymous"] = []
        merged=self.merge()
        self.assertFalse(self.role(merged,"catalog-public")["composite"])
        self.assertEqual(self.role(merged,"catalog-public")["composites"]["client"]["econ-graph-api"],[])
        self.assertFalse(next(c for c in merged["clients"] if c["clientId"]=="econgraph-anonymous")["enabled"])
        self.assertEqual(merged["users"][0]["realmRoles"],[])

    def test_existing_realm_first_rollout_seeds_only_absent_resources(self):
        self.state[self.prefix]["attributes"] = {}
        self.state[self.prefix + "/roles/catalog-public"] = None
        self.state[self.prefix + "/clients?clientId=econgraph-anonymous"] = []
        merged=self.merge()
        self.assertEqual(self.role(merged,"catalog-public")["composites"],self.role(self.realm,"catalog-public")["composites"])
        self.assertEqual(merged["users"][0]["realmRoles"],["catalog-public"])
        self.assertIn("catalog-public",self.role(merged,"user")["composites"]["realm"])

    def test_realm_attributes_survive_without_premature_completion(self):
        self.state[self.prefix]["attributes"]={"frontendUrl":"https://custom.test","cli-state":"existing-state"}
        merged=self.merge()
        self.assertEqual(merged["attributes"]["frontendUrl"],"https://custom.test")
        self.assertEqual(merged["attributes"]["cli-state"],"existing-state")
        self.assertNotIn(preserve.INITIALIZED,merged["attributes"])

    def test_interrupted_old_realm_bootstrap_retries_only_originally_missing_resources(self):
        self.state[self.prefix]["attributes"]={}
        self.state[self.prefix+"/roles/catalog-public"]=None
        self.state[self.prefix+"/clients?clientId=econgraph-anonymous"]=[]
        first=self.merge()
        # Config-cli writes the pending descriptor before creating clients and roles. Fail
        # after it creates the policy role and client but before setting their default grants.
        self.state[self.prefix]["attributes"]=first["attributes"]
        self.state[self.prefix+"/roles/catalog-public"]={"name":"catalog-public","composite":False}
        self.state[self.prefix+"/clients?clientId=econgraph-anonymous"]=[{"id":"anon","clientId":"econgraph-anonymous","enabled":False,"serviceAccountsEnabled":False}]
        retried=self.merge()
        self.assertEqual(self.role(retried,"catalog-public")["composites"],self.role(self.realm,"catalog-public")["composites"])
        self.assertIn("catalog-public",self.role(retried,"user")["composites"]["realm"])
        self.assertEqual(retried["users"][0]["realmRoles"],["catalog-public"])
        self.assertTrue(next(c for c in retried["clients"] if c["clientId"]=="econgraph-anonymous")["enabled"])
        self.assertNotIn(preserve.INITIALIZED,retried["attributes"])

    def test_disabled_service_accounts_preserve_live_scope_restrictions(self):
        self.state[self.prefix+"/clients?clientId=econgraph-anonymous"][0]["serviceAccountsEnabled"]=False
        self.state[self.prefix+"/clients/anon/scope-mappings/clients/api"]=[{"name":"catalog.fred:read"}]
        merged=self.merge()
        self.assertEqual(merged["clientScopeMappings"]["econ-graph-api"][0]["roles"],["catalog.fred:read"])
        self.assertEqual(merged["users"],[])

    def test_completion_commits_marker_after_import_and_keeps_fresh_attributes(self):
        self.state[self.prefix]["attributes"]={"frontendUrl":"https://custom.test","cli-state":"new-state",preserve.PENDING:json.dumps({"policy":True,"reader":True})}
        writes=[]
        preserve.complete_bootstrap(self.realm,self.state.get,lambda path,body:writes.append((path,body)))
        self.assertEqual(len(writes),1)
        attributes=writes[0][1]["attributes"]
        self.assertEqual(attributes["frontendUrl"],"https://custom.test")
        self.assertEqual(attributes["cli-state"],"new-state")
        self.assertEqual(attributes[preserve.INITIALIZED],"true")
        self.assertNotIn(preserve.PENDING,attributes)
        self.state[self.prefix]["attributes"]=attributes
        preserve.complete_bootstrap(self.realm,self.state.get,lambda path,body:writes.append((path,body)))
        self.assertEqual(len(writes),1,"completion is idempotent")

    def test_incomplete_import_cannot_be_marked_complete(self):
        self.state[self.prefix]["attributes"]={}
        self.state[self.prefix+"/roles/catalog-public"]=None
        writes=[]
        with self.assertRaises(RuntimeError):
            preserve.complete_bootstrap(self.realm,self.state.get,lambda path,body:writes.append(body))
        self.assertEqual(writes,[])

    def realm_granted(self, name):
        """The econ-graph-api roles the realm file's role NAME grants, through nested composites."""
        realm_roles = {role["name"]: role for role in self.realm["roles"]["realm"]}
        found, pending = set(), [name]
        while pending:
            role = realm_roles.get(pending.pop())
            if role is None:
                continue
            composites = role.get("composites") or {}
            found.update((composites.get("client") or {}).get("econ-graph-api", []))
            pending.extend(composites.get("realm") or [])
        return found

    def assert_not_public(self, role):
        """No default grant reaches signed-in users or the anonymous reader."""
        for name in ("catalog-public", "user", "support", self.realm["defaultRole"]["name"]):
            self.assertNotIn(role, self.realm_granted(name), name)
        reader = next(u for u in self.realm["users"] if u.get("serviceAccountClientId") == "econgraph-anonymous")
        for name in reader.get("realmRoles", []):
            self.assertNotIn(role, self.realm_granted(name), name)
        self.assertNotIn(role, (reader.get("clientRoles") or {}).get("econ-graph-api", []))
        scope = next(e for e in self.realm["clientScopeMappings"]["econ-graph-api"]
                     if e["client"] == "econgraph-anonymous")
        self.assertNotIn(role, scope["roles"])

    def test_realm_file_makes_fred_admin_only(self):
        self.assert_not_public("catalog.fred:read")
        for name in ("admin", "super_admin"):
            self.assertIn("catalog.fred:read", self.realm_granted(name), name)

    def test_realm_file_grants_unreviewed_catalogs_to_nobody_by_default(self):
        for source in ("imf", "ecb", "oecd", "boe", "boj", "rba", "snb", "unstats", "ilo"):
            role = "catalog.%s:read" % source
            self.assert_not_public(role)
            for name in ("admin", "super_admin"):
                self.assertNotIn(role, self.realm_granted(name), name)

    def test_realm_file_restricts_the_world_trade_organization_catalog(self):
        # Defined so it can be granted, but public to nobody by default.
        self.assertIn("catalog.world_trade_organization:read",
                      {r["name"] for r in self.realm["roles"]["client"]["econ-graph-api"]})
        self.assert_not_public("catalog.world_trade_organization:read")

    def test_realm_file_restricts_the_bank_of_canada_catalog(self):
        # Defined so it can be granted, but public to nobody by default.
        self.assertIn("catalog.bank_of_canada:read",
                      {r["name"] for r in self.realm["roles"]["client"]["econ-graph-api"]})
        self.assert_not_public("catalog.bank_of_canada:read")
        for name in ("admin", "super_admin"):
            self.assertNotIn("catalog.bank_of_canada:read", self.realm_granted(name), name)

    def test_realm_file_public_catalogs_are_exactly_the_reviewed_ones(self):
        expected = {"catalog.%s:read" % source
                    for source in ("bls", "bea", "census", "fhfa", "world_bank")}
        public = self.role(self.realm, "catalog-public")["composites"]["client"]["econ-graph-api"]
        self.assertEqual(set(public), expected)
        self.assertEqual(len(public), len(expected))
        scope = next(e for e in self.realm["clientScopeMappings"]["econ-graph-api"]
                     if e["client"] == "econgraph-anonymous")
        self.assertEqual(set(scope["roles"]), expected)
        self.assertEqual({r for r in self.realm_granted("user") if r.startswith("catalog.")},
                         expected | {"catalog.sec:read"})
        self.assertNotIn("catalog.sec:read", scope["roles"])

    def test_live_admin_only_catalog_survives_a_deploy(self):
        # The state scripts/keycloak/set-catalog-access.sh --admin-only leaves: FRED out of
        # catalog-public and the anonymous scope, granted to admin. A deploy must keep it,
        # whatever the realm file declares.
        public = [{"name": n, "clientRole": True, "containerId": "api"}
                  for n in ("catalog.bls:read", "catalog.bea:read")]
        self.state[self.prefix + "/roles/catalog-public/composites"] = public
        self.state[self.prefix + "/roles/user/composites"] = [{"name": "catalog-public", "clientRole": False}]
        self.state[self.prefix + "/roles/admin/composites"] = [
            {"name": "support", "clientRole": False},
            {"name": "catalog.fred:read", "clientRole": True, "containerId": "api"}]
        self.state[self.prefix + "/clients/anon/scope-mappings/clients/api"] = [{"name": "catalog.bls:read"}]
        for realm in (self.realm, self.with_fred_public()):
            merged = preserve.merge_catalog_policy(copy.deepcopy(realm), self.get)
            self.assertEqual(self.role(merged, "catalog-public")["composites"]["client"]["econ-graph-api"],
                             ["catalog.bls:read", "catalog.bea:read"])
            self.assertIn("catalog.fred:read", self.role(merged, "admin")["composites"]["client"]["econ-graph-api"])
            scope = merged["clientScopeMappings"]["econ-graph-api"][0]["roles"]
            self.assertEqual(scope, ["catalog.bls:read"])

    def test_live_public_catalog_is_not_withdrawn_or_granted_to_admin_by_a_deploy(self):
        # Until an operator runs set-catalog-access.sh, a cluster keeps its live grants: the
        # realm file's admin-only FRED changes only a new realm.
        self.state[self.prefix + "/roles/catalog-public/composites"] = [
            {"name": "catalog.fred:read", "clientRole": True, "containerId": "api"}]
        self.state[self.prefix + "/roles/admin/composites"] = [{"name": "support", "clientRole": False}]
        merged = self.merge()
        self.assertEqual(self.role(merged, "catalog-public")["composites"]["client"]["econ-graph-api"],
                         ["catalog.fred:read"])
        self.assertNotIn("catalog.fred:read", self.role(merged, "admin")["composites"]["client"]["econ-graph-api"])

    def with_fred_public(self):
        """The realm file as it was with FRED in catalog-public and the anonymous scope."""
        realm = copy.deepcopy(self.realm)
        self.role(realm, "catalog-public")["composites"]["client"]["econ-graph-api"].insert(0, "catalog.fred:read")
        realm["clientScopeMappings"]["econ-graph-api"][0]["roles"].insert(0, "catalog.fred:read")
        return realm

    def add_target(self, kind, target_id, name, live):
        """A live client or client scope whose scope mappings hold LIVE econ-graph-api roles."""
        listing, path = ("/clients", "/clients/") if kind == "client" else ("/client-scopes", "/client-scopes/")
        key = "clientId" if kind == "client" else "name"
        self.state[self.prefix + listing].append({"id": target_id, key: name})
        self.state[self.prefix + path + target_id + "/scope-mappings/clients/api"] = [{"name": r} for r in live]

    def merge_with_warnings(self, realm=None):
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            merged = preserve.merge_catalog_policy(copy.deepcopy(realm or self.realm),
                                                   self.get)
        return merged, err.getvalue()

    @staticmethod
    def scope_entries(merged):
        return {(kind, entry[kind]): entry["roles"] for entry in merged["clientScopeMappings"]["econ-graph-api"]
                for kind in ("client", "clientScope") if kind in entry}

    def test_catalog_reads_on_undeclared_clients_and_client_scopes_survive_a_deploy(self):
        # config-cli would remove every scope mapping of an econ-graph-api role on a client or
        # client scope the realm file doesn't list; catalog reads are operator policy and stay.
        self.add_target("client", "partner", "partner-x", ["catalog.imf:read", "api:mcp"])
        self.add_target("clientScope", "extra", "catalog-extra", ["catalog.imf:read", "catalog.ecb:read"])
        self.add_target("client", "web", "econ-graph-web", ["catalog.bank_of_canada:read"])
        merged, warnings = self.merge_with_warnings()
        entries = self.scope_entries(merged)
        self.assertEqual(entries[("client", "partner-x")], ["catalog.imf:read"])
        self.assertEqual(entries[("clientScope", "catalog-extra")], ["catalog.imf:read", "catalog.ecb:read"])
        self.assertEqual(entries[("client", "econ-graph-web")], ["catalog.bank_of_canada:read"])
        # The non-catalog mapping is removed, and the deploy log names it.
        self.assertIn("client partner-x: api:mcp", warnings)
        self.assertNotIn("catalog.", warnings)
        # Targets holding nothing get no entry, and the anonymous reader keeps its own.
        self.assertNotIn(("client", "econ-graph-api"), entries)
        self.assertEqual(entries[("client", "econgraph-anonymous")], [])
        self.assertEqual(len(merged["clientScopeMappings"]["econ-graph-api"]), 4)

    def test_preserved_scope_mappings_are_stable_across_deploys(self):
        self.add_target("clientScope", "extra", "catalog-extra", ["catalog.imf:read"])
        self.add_target("client", "partner", "partner-x", ["catalog.ecb:read"])
        first, _ = self.merge_with_warnings()
        second, warnings = self.merge_with_warnings(first)
        self.assertEqual(first["clientScopeMappings"], second["clientScopeMappings"])
        self.assertEqual(warnings, "")

    def test_declared_scope_mappings_keep_their_other_roles_and_take_live_catalog_reads(self):
        realm = copy.deepcopy(self.realm)
        realm["clientScopeMappings"]["econ-graph-api"] += [
            {"client": "partner-x", "roles": ["api:mcp", "catalog.fred:read"]},
            {"clientScope": "not-created-yet", "roles": ["catalog.fred:read"]},
        ]
        self.add_target("client", "partner", "partner-x", ["catalog.ecb:read", "api:mcp", "annotation:create"])
        merged, warnings = self.merge_with_warnings(realm)
        entries = self.scope_entries(merged)
        # Once the realm is initialized the live grants are the policy: a declared catalog read
        # neither returns to a live target nor reaches one the import creates or recreates.
        self.assertEqual(entries[("client", "partner-x")], ["api:mcp", "catalog.ecb:read"])
        self.assertEqual(entries[("clientScope", "not-created-yet")], [])
        self.assertIn("client partner-x: annotation:create", warnings)
        self.assertNotIn("api:mcp", warnings)
        # An emptied live mapping on a declared target stays empty.
        self.state[self.prefix + "/clients/partner/scope-mappings/clients/api"] = [{"name": "api:mcp"}]
        merged, _ = self.merge_with_warnings(realm)
        self.assertEqual(self.scope_entries(merged)[("client", "partner-x")], ["api:mcp"])

    def test_a_retried_first_import_seeds_declared_scope_mappings(self):
        # A first import that stopped part way leaves the target live but unmapped; the retry
        # applies the declared catalog reads, as it does the composites' defaults.
        self.state[self.prefix]["attributes"] = {preserve.PENDING: json.dumps({"policy": True, "reader": False})}
        realm = copy.deepcopy(self.realm)
        realm["clientScopeMappings"]["econ-graph-api"] += [
            {"client": "partner-x", "roles": ["catalog.fred:read"]},
            {"clientScope": "not-created-yet", "roles": ["catalog.fred:read"]}]
        self.add_target("client", "partner", "partner-x", ["catalog.ecb:read"])
        entries = self.scope_entries(self.merge_with_warnings(realm)[0])
        self.assertEqual(entries[("client", "partner-x")], ["catalog.fred:read", "catalog.ecb:read"])
        self.assertEqual(entries[("clientScope", "not-created-yet")], ["catalog.fred:read"])

    def test_clients_config_cli_deletes_get_no_scope_mapping_entry(self):
        # config-cli deletes a client it created once the realm file drops it, then could not
        # resolve an entry naming it; the client's grants go with it.
        state = 'de.adorsys.keycloak.config.state-default-clients-'
        self.state[self.prefix]["attributes"].update({
            state + "0": '["econ-graph-web","econ-graph-api","econgraph-anon',
            state + "1": 'ymous","temp-x"]'})
        self.add_target("client", "temp", "temp-x", ["catalog.imf:read", "api:mcp"])
        self.add_target("client", "partner", "partner-x", ["catalog.imf:read"])
        merged, warnings = self.merge_with_warnings()
        entries = self.scope_entries(merged)
        self.assertNotIn(("client", "temp-x"), entries)
        self.assertEqual(entries[("client", "partner-x")], ["catalog.imf:read"])
        self.assertEqual(warnings, "")
        # Still declared, it is kept like any other client.
        realm = copy.deepcopy(self.realm)
        realm["clients"].append({"clientId": "temp-x"})
        self.assertEqual(self.scope_entries(self.merge_with_warnings(realm)[0])[("client", "temp-x")],
                         ["catalog.imf:read"])
        self.state[self.prefix]["attributes"][state + "1"] = "not json"
        with self.assertRaises(RuntimeError) as raised:
            self.merge()
        self.assertIn(state, str(raised.exception))

    def test_scope_mapping_entries_without_a_target_fail_the_deploy(self):
        self.realm["clientScopeMappings"]["econ-graph-api"].append({"roles": ["api:mcp"]})
        with self.assertRaises(RuntimeError):
            self.merge()

    def test_duplicate_scope_mapping_entries_fail_the_deploy(self):
        self.realm["clientScopeMappings"]["econ-graph-api"].append(
            {"client": "econgraph-anonymous", "roles": ["catalog.fred:read"]})
        with self.assertRaises(RuntimeError) as raised:
            self.merge()
        self.assertIn("econgraph-anonymous", str(raised.exception))

    def test_undeclared_roles_removed_from_the_anonymous_scope_are_named(self):
        self.state[self.prefix + "/clients/anon/scope-mappings/clients/api"] = [
            {"name": "catalog.bea:read"}, {"name": "api:mcp"}]
        merged, warnings = self.merge_with_warnings()
        self.assertIn("client econgraph-anonymous: api:mcp", warnings)
        self.assertEqual(self.scope_entries(merged)[("client", "econgraph-anonymous")], ["catalog.bea:read"])

    def test_catalog_access_changes_survive_alongside_preserved_scope_mappings(self):
        # set-catalog-access.sh --withdraw empties the anonymous scope; a partner's grant of
        # the same catalog is separate policy, and neither restores the other.
        self.add_target("client", "partner", "partner-x", ["catalog.bls:read"])
        self.state[self.prefix + "/clients/anon/scope-mappings/clients/api"] = [{"name": "catalog.bea:read"}]
        merged, _ = self.merge_with_warnings()
        entries = self.scope_entries(merged)
        self.assertEqual(entries[("client", "econgraph-anonymous")], ["catalog.bea:read"])
        self.assertEqual(entries[("client", "partner-x")], ["catalog.bls:read"])

    def test_unreadable_scope_mappings_fail_the_deploy(self):
        # Never import a file that would drop grants it couldn't read.
        self.add_target("clientScope", "extra", "catalog-extra", ["catalog.imf:read"])
        self.state[self.prefix + "/client-scopes/extra/scope-mappings/clients/api"] = None
        with self.assertRaises(RuntimeError):
            self.merge()
        self.state[self.prefix + "/client-scopes"] = None
        with self.assertRaises(RuntimeError):
            self.merge()

    def test_catalog_reads_on_a_client_scope_the_reader_uses_fail_the_deploy(self):
        # Anonymous tokens are built with the reader's client scopes: a catalog read mapped on
        # one reaches visitors whatever the reader's own scope mapping says.
        self.add_target("clientScope", "basic", "basic", ["catalog.fred:read"])
        with self.assertRaises(RuntimeError) as raised:
            self.merge()
        self.assertIn("basic (catalog.fred:read)", str(raised.exception))
        # Optional scopes, and scopes the realm file assigns, count too.
        self.state[self.prefix + "/client-scopes/basic/scope-mappings/clients/api"] = []
        self.state[self.prefix + "/clients/anon/optional-client-scopes"] = [{"id": "extra", "name": "catalog-extra"}]
        self.add_target("clientScope", "extra", "catalog-extra", ["catalog.imf:read"])
        with self.assertRaises(RuntimeError) as raised:
            self.merge()
        self.assertIn("catalog-extra (catalog.imf:read)", str(raised.exception))
        self.state[self.prefix + "/clients/anon/optional-client-scopes"] = []
        self.merge()
        realm = copy.deepcopy(self.realm)
        next(c for c in realm["clients"] if c["clientId"] == "econgraph-anonymous")[
            "defaultClientScopes"].append("catalog-extra")
        with self.assertRaises(RuntimeError):
            self.merge_with_warnings(realm)
        # A new realm is checked against the realm file alone.
        self.state[self.prefix] = None
        realm["clientScopeMappings"]["econ-graph-api"].append({"clientScope": "catalog-extra", "roles": ["catalog.imf:read"]})
        with self.assertRaises(RuntimeError):
            self.merge_with_warnings(realm)

    def test_other_api_roles_on_the_readers_client_scopes_and_partner_scopes_are_allowed(self):
        self.add_target("clientScope", "basic", "basic", ["api:mcp"])
        self.add_target("clientScope", "extra", "catalog-extra", ["catalog.imf:read"])
        self.merge()

    def test_catalog_reads_on_a_realm_default_client_scope_are_named(self):
        self.add_target("clientScope", "profile", "profile", ["catalog.imf:read"])
        merged, warnings = self.merge_with_warnings()
        self.assertIn("realm default client scope profile admits catalog.imf:read", warnings)
        self.assertEqual(self.scope_entries(merged)[("clientScope", "profile")], ["catalog.imf:read"])

    def test_grants_of_roles_the_import_deletes_are_dropped_and_named(self):
        # A role config-cli created and the realm file dropped is deleted by the import; a grant
        # of it written into the import would fail it part way.
        self.state[self.prefix]["attributes"]["de.adorsys.keycloak.config.state-default-roles-client-econ-graph-api-0"] = \
            json.dumps([r["name"] for r in self.realm["roles"]["client"]["econ-graph-api"]])
        self.realm["roles"]["client"]["econ-graph-api"] = [
            r for r in self.realm["roles"]["client"]["econ-graph-api"] if r["name"] != "catalog.ilo:read"]
        edge = {"name": "catalog.ilo:read", "clientRole": True, "containerId": "api"}
        operator_made = {"name": "catalog.partner:read", "clientRole": True, "containerId": "api"}
        self.state[self.prefix + "/roles/catalog-public/composites"] = [edge, operator_made]
        self.state[self.prefix + "/roles/admin/composites"] = [edge]
        self.state[self.prefix + "/clients/anon/scope-mappings/clients/api"] = [{"name": "catalog.ilo:read"}]
        self.state[self.prefix + "/users/reader/role-mappings"] = {
            "realmMappings": [], "clientMappings": {"econ-graph-api": {"mappings": [{"name": "catalog.ilo:read"}]}}}
        self.add_target("client", "partner", "partner-x", ["catalog.ilo:read", "catalog.imf:read"])
        merged, warnings = self.merge_with_warnings()
        # An operator-made role isn't config-cli's to delete, so its grant stays.
        self.assertEqual(self.role(merged, "catalog-public")["composites"]["client"]["econ-graph-api"],
                         ["catalog.partner:read"])
        self.assertNotIn("catalog.ilo:read", self.role(merged, "admin")["composites"]["client"]["econ-graph-api"])
        entries = self.scope_entries(merged)
        self.assertEqual(entries[("client", "partner-x")], ["catalog.imf:read"])
        self.assertEqual(entries[("client", "econgraph-anonymous")], [])
        self.assertEqual(merged["users"][0]["clientRoles"], {"econ-graph-api": []})
        for where in ("role catalog-public", "role admin", "client partner-x", "client econgraph-anonymous",
                      "the econgraph-anonymous service account"):
            self.assertIn("catalog.ilo:read is no longer in the realm file, so the import deletes it and its grant on "
                          + where, warnings)
        self.assertNotIn("catalog.partner:read", warnings)

    def test_realm_and_composite_roles_admitting_catalog_reads_to_the_reader_fail_the_deploy(self):
        # Keycloak expands composites when it builds a token: catalog-public (or user, or a
        # composite API role) mapped on `basic` gives visitors every catalog read it holds.
        composite = self.prefix + "/client-scopes/basic/scope-mappings/clients/api/composite"
        self.state[composite] = [{"name": "catalog.bls:read"}]
        with self.assertRaises(RuntimeError) as raised:
            self.merge()
        self.assertIn("client scope basic (catalog.bls:read)", str(raised.exception))
        del self.state[composite]
        # The reader's own scope may admit a catalog read only by mapping it directly.
        own = self.prefix + "/clients/anon/scope-mappings/clients/api"
        self.state[own] = [{"name": "catalog.bea:read"}]
        self.state[own + "/composite"] = [{"name": "catalog.bea:read"}, {"name": "catalog.bls:read"}]
        with self.assertRaises(RuntimeError) as raised:
            self.merge()
        self.assertIn("its own scope, through realm roles, its own client roles or full scope (catalog.bls:read)",
                      str(raised.exception))
        self.assertNotIn("catalog.bea:read", str(raised.exception))
        del self.state[own + "/composite"]
        self.merge()

    def test_realm_file_realm_role_scope_mappings_are_checked_through_its_composites(self):
        self.state[self.prefix] = None
        self.realm["scopeMappings"] = [{"clientScope": "basic", "roles": ["user"]}]
        with self.assertRaises(RuntimeError) as raised:
            self.merge()
        self.assertIn("client scope basic (catalog.bea:read", str(raised.exception))
        self.realm["scopeMappings"] = [{"clientScope": "basic", "roles": ["offline_access"]}]
        self.merge()

    def test_leftover_state_chunks_are_ignored_as_config_cli_ignores_them(self):
        # Shrinking by a chunk, config-cli rewrites the earlier chunks and leaves the last one.
        state = "de.adorsys.keycloak.config.state-default-roles-client-econ-graph-api-"
        names = [r["name"] for r in self.realm["roles"]["client"]["econ-graph-api"]] + ["catalog.gone:read"]
        self.state[self.prefix]["attributes"].update({
            state + "0": json.dumps(names), state + "1": 'atalog.unstats:read","catalog.ilo:read"]'})
        self.add_target("client", "partner", "partner-x", ["catalog.gone:read", "catalog.imf:read"])
        merged, warnings = self.merge_with_warnings()
        self.assertEqual(self.scope_entries(merged)[("client", "partner-x")], ["catalog.imf:read"])
        self.assertIn("ignoring 40 characters after the keycloak-config-cli state list", warnings)
        self.assertNotIn("unstats", warnings)
        # A state that doesn't start with a readable list still stops the deploy.
        self.state[self.prefix]["attributes"][state + "0"] = '{"not": "a list"}'
        with self.assertRaises(RuntimeError):
            self.merge()

    def test_the_readers_deleted_realm_and_other_client_roles_are_dropped(self):
        attributes = self.state[self.prefix]["attributes"]
        attributes["de.adorsys.keycloak.config.state-default-roles-realm-0"] = json.dumps(
            [r["name"] for r in self.realm["roles"]["realm"]] + ["retired-policy"])
        self.state[self.prefix + "/users/reader/role-mappings"] = {
            "realmMappings": [{"name": "retired-policy"}, {"name": "custom-public-policy"}],
            "clientMappings": {"econ-graph-web": {"mappings": [{"name": "web-only"}]}}}
        merged, warnings = self.merge_with_warnings()
        self.assertEqual(merged["users"][0]["realmRoles"], ["custom-public-policy"])
        # config-cli recorded creating no econ-graph-web role, so it deletes none.
        self.assertEqual(merged["users"][0]["clientRoles"], {"econ-graph-web": ["web-only"]})
        self.assertIn("retired-policy is no longer in the realm file", warnings)

    def test_roles_of_a_client_the_realm_file_drops_are_dropped_from_the_reader(self):
        # config-cli 6.5.1 deletes every role it created for a client missing from roles.client,
        # and a client it created, once the realm file drops it, with every role on it.
        attributes = self.state[self.prefix]["attributes"]
        attributes["de.adorsys.keycloak.config.state-default-roles-client-temp-x-0"] = '["temp-role"]'
        self.state[self.prefix + "/users/reader/role-mappings"] = {
            "realmMappings": [{"name": "custom-public-policy"}],
            "clientMappings": {"temp-x": {"mappings": [{"name": "temp-role"}, {"name": "op-role"}]}}}
        merged, warnings = self.merge_with_warnings()
        # The client stays: only the roles config-cli made go; an operator's stays.
        self.assertEqual(merged["users"][0]["clientRoles"], {"temp-x": ["op-role"]})
        self.assertIn("temp-x role temp-role is no longer in the realm file", warnings)
        # The client goes too: every role on it goes with it, whoever made it.
        attributes["de.adorsys.keycloak.config.state-default-clients-0"] = json.dumps(
            [c["clientId"] for c in self.realm["clients"]] + ["temp-x"])
        merged, warnings = self.merge_with_warnings()
        self.assertEqual(merged["users"][0]["clientRoles"], {"temp-x": []})
        self.assertIn("client temp-x is no longer in the realm file, so the import deletes it with its roles, "
                      "dropping grants of temp-role, op-role", warnings)
        # Keycloak's own clients are never deleted, whatever the state says.
        attributes["de.adorsys.keycloak.config.state-default-clients-0"] = '["account"]'
        self.assertEqual(preserve.clients_config_cli_deletes(self.realm, attributes), set())
        # Without roles.client config-cli deletes no client role.
        del self.realm["roles"]["client"]
        self.assertEqual(preserve.roles_config_cli_deletes(self.realm, attributes, "temp-x"), set())

    def test_a_scope_the_realm_file_newly_assigns_to_the_reader_is_read(self):
        self.add_target("clientScope", "extra", "catalog-extra", [])
        self.state[self.prefix + "/client-scopes/extra/scope-mappings/clients/api/composite"] = [
            {"name": "catalog.bls:read"}]
        self.merge()  # Not assigned to the reader: nothing reaches its tokens.
        realm = copy.deepcopy(self.realm)
        next(c for c in realm["clients"] if c["clientId"] == "econgraph-anonymous")[
            "optionalClientScopes"] = ["catalog-extra"]
        with self.assertRaises(RuntimeError) as raised:
            self.merge_with_warnings(realm)
        self.assertIn("client scope catalog-extra (catalog.bls:read)", str(raised.exception))

    def test_catalog_reads_the_import_deletes_are_not_refused(self):
        self.state[self.prefix]["attributes"]["de.adorsys.keycloak.config.state-default-roles-client-econ-graph-api-0"] = \
            json.dumps([r["name"] for r in self.realm["roles"]["client"]["econ-graph-api"]])
        self.realm["roles"]["client"]["econ-graph-api"] = [
            r for r in self.realm["roles"]["client"]["econ-graph-api"] if r["name"] != "catalog.ilo:read"]
        self.add_target("clientScope", "basic", "basic", ["catalog.ilo:read"])
        merged, warnings = self.merge_with_warnings()
        self.assertIn("client scope basic admits catalog.ilo:read to econgraph-anonymous tokens until the "
                      "import deletes it", warnings)
        self.state[self.prefix + "/client-scopes/basic/scope-mappings/clients/api"] = [
            {"name": "catalog.ilo:read"}, {"name": "catalog.bls:read"}]
        with self.assertRaises(RuntimeError) as raised:
            self.merge()
        self.assertNotIn("catalog.ilo:read", str(raised.exception))
        # The same through the reader's own scope, by a realm role mapped there.
        self.state[self.prefix + "/client-scopes/basic/scope-mappings/clients/api"] = []
        self.state[self.prefix + "/clients/anon/scope-mappings/clients/api/composite"] = [{"name": "catalog.ilo:read"}]
        merged, warnings = self.merge_with_warnings()
        self.assertIn("its own scope admits catalog.ilo:read to econgraph-anonymous tokens until the import "
                      "deletes it", warnings)

    def test_scope_mapping_entries_naming_both_a_client_and_a_scope_fail_the_deploy(self):
        self.realm["clientScopeMappings"]["econ-graph-api"].append(
            {"client": "partner-x", "clientScope": "basic", "roles": ["api:mcp"]})
        with self.assertRaises(RuntimeError):
            self.merge()

    def test_missing_relations_or_failed_api_never_fall_back_to_defaults(self):
        self.state[self.prefix + "/roles/catalog-public/composites"] = None
        with self.assertRaises(RuntimeError): self.merge()
        def failed(_): raise RuntimeError("unavailable")
        with self.assertRaises(RuntimeError): preserve.merge_catalog_policy(self.realm,failed)


if __name__ == "__main__":
    unittest.main()
