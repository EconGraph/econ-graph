"""Deterministic tests for catalog policy preservation; no Keycloak credentials needed."""
import copy
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
            self.prefix + "/clients": [{"id": "api", "clientId": "econ-graph-api"}],
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

    def merge(self):
        return preserve.merge_catalog_policy(copy.deepcopy(self.realm), lambda path: copy.deepcopy(self.state.get(path)))

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

    def test_realm_file_public_catalogs_are_exactly_the_reviewed_ones(self):
        expected = {"catalog.%s:read" % source
                    for source in ("bls", "bea", "census", "fhfa", "world_bank", "wto", "boc")}
        public = self.role(self.realm, "catalog-public")["composites"]["client"]["econ-graph-api"]
        self.assertEqual(set(public), expected)
        self.assertEqual(len(public), len(expected))
        scope = next(e for e in self.realm["clientScopeMappings"]["econ-graph-api"]
                     if e["client"] == "econgraph-anonymous")
        self.assertEqual(set(scope["roles"]), expected)
        self.assertEqual({r for r in self.realm_granted("user") if r.startswith("catalog.")}, expected)

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
            merged = preserve.merge_catalog_policy(copy.deepcopy(realm), lambda path: copy.deepcopy(self.state.get(path)))
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

    def test_missing_relations_or_failed_api_never_fall_back_to_defaults(self):
        self.state[self.prefix + "/roles/catalog-public/composites"] = None
        with self.assertRaises(RuntimeError): self.merge()
        def failed(_): raise RuntimeError("unavailable")
        with self.assertRaises(RuntimeError): preserve.merge_catalog_policy(self.realm,failed)


if __name__ == "__main__":
    unittest.main()
