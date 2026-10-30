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

    def test_missing_relations_or_failed_api_never_fall_back_to_defaults(self):
        self.state[self.prefix + "/roles/catalog-public/composites"] = None
        with self.assertRaises(RuntimeError): self.merge()
        def failed(_): raise RuntimeError("unavailable")
        with self.assertRaises(RuntimeError): preserve.merge_catalog_policy(self.realm,failed)


OLD = "catalog.boc:read"
NEW = "catalog.bank_of_canada:read"


class RenamedCatalogRoleTests(unittest.TestCase):
    """A renamed catalog role's live grants move to the new name (RENAMED_CATALOG_ROLES)."""
    setUp = CatalogPolicyTests.setUp
    merge = CatalogPolicyTests.merge
    role = staticmethod(CatalogPolicyTests.role)

    def test_the_bank_of_canada_rename_is_listed(self):
        self.assertEqual(preserve.RENAMED_CATALOG_ROLES[OLD], NEW)

    def test_realm_file_defines_every_new_name_and_no_old_name(self):
        declared = {role["name"] for role in self.realm["roles"]["client"]["econ-graph-api"]}
        text = json.dumps(self.realm)
        for old, new in preserve.RENAMED_CATALOG_ROLES.items():
            self.assertIn(new, declared)
            self.assertNotIn('"%s"' % old, text)

    def test_live_grants_of_the_old_name_move_to_the_new_name(self):
        self.state[self.prefix + "/clients/api/roles?briefRepresentation=false"].append(
            {"name": OLD, "composite": False})
        self.state[self.prefix + "/roles/catalog-public/composites"] = [
            {"name": OLD, "clientRole": True, "containerId": "api"},
            {"name": "catalog.bea:read", "clientRole": True, "containerId": "api"},
        ]
        self.state[self.prefix + "/roles/user/composites"] = [
            {"name": OLD, "clientRole": True, "containerId": "api"},
        ]
        self.state[self.prefix + "/clients/anon/scope-mappings/clients/api"] = [{"name": OLD}]
        self.state[self.prefix + "/users/reader/role-mappings"] = {
            "realmMappings": [],
            "clientMappings": {"econ-graph-api": {"mappings": [{"name": OLD}, {"name": NEW}]}},
        }
        merged = self.merge()
        self.assertEqual(self.role(merged, "catalog-public")["composites"],
                         {"realm": [], "client": {"econ-graph-api": [NEW, "catalog.bea:read"]}})
        self.assertIn(NEW, self.role(merged, "user")["composites"]["client"]["econ-graph-api"])
        self.assertEqual(merged["clientScopeMappings"]["econ-graph-api"][0]["roles"], [NEW])
        self.assertEqual(merged["users"][0]["clientRoles"], {"econ-graph-api": [NEW]})
        # The old role stays declared for this import only, so config-cli does not delete it
        # before --complete moves the grants it still carries; no edge names it.
        declared = merged["roles"]["client"]["econ-graph-api"]
        old_roles = [role for role in declared if role["name"] == OLD]
        self.assertEqual(len(old_roles), 1)
        self.assertFalse(old_roles[0].get("composite"))
        edges = copy.deepcopy(merged)
        edges["roles"]["client"]["econ-graph-api"] = [r for r in declared if r["name"] != OLD]
        self.assertNotIn('"%s"' % OLD, json.dumps(edges))

    def test_without_the_old_role_the_import_is_unchanged(self):
        merged = self.merge()
        self.assertEqual(merged["roles"]["client"], self.realm["roles"]["client"])

    def test_a_realm_file_missing_the_new_name_fails(self):
        self.realm["roles"]["client"]["econ-graph-api"] = [
            role for role in self.realm["roles"]["client"]["econ-graph-api"] if role["name"] != NEW]
        with self.assertRaises(RuntimeError) as raised:
            self.merge()
        self.assertIn(NEW, str(raised.exception))


class RetireRenamedRolesTests(unittest.TestCase):
    """--complete moves every remaining holder of an old name to the new one, then deletes it."""

    def setUp(self):
        self.realm = {"realm": "econ-graph"}
        p = "/admin/realms/econ-graph"
        old = p + "/clients/api/roles/catalog.boc%3Aread"
        self.prefix, self.old = p, old
        self.state = {
            p + "/clients?clientId=econ-graph-api": [{"id": "api", "clientId": "econ-graph-api"}],
            old: {"id": "old-id", "name": OLD},
            p + "/clients/api/roles/catalog.bank_of_canada%3Aread": {"id": "new-id", "name": NEW},
            old + "/users?first=0&max=100": [{"id": "u1"}, {"id": "u2"}],
            p + "/users/u1/role-mappings/clients/api": [{"name": OLD}],
            p + "/users/u2/role-mappings/clients/api": [{"name": OLD}, {"name": NEW}],
            old + "/groups?first=0&max=100": [{"id": "g1"}],
            p + "/groups/g1/role-mappings/clients/api": [{"name": OLD}],
            p + "/roles?briefRepresentation=false": [
                {"id": "cp", "name": "catalog-public", "composite": True},
                {"id": "ur", "name": "user", "composite": True},
                {"id": "pl", "name": "plain", "composite": False},
            ],
            p + "/clients/api/roles?briefRepresentation=false": [
                {"id": "old-id", "name": OLD, "composite": False},
                {"id": "ad", "name": "admin.users:read", "composite": True},
            ],
            p + "/roles-by-id/cp/composites": [{"name": OLD, "clientRole": True, "containerId": "api"}],
            p + "/roles-by-id/ur/composites": [{"name": "annotation:create", "clientRole": True, "containerId": "api"}],
            p + "/roles-by-id/ad/composites": [{"name": OLD, "clientRole": True, "containerId": "other"}],
            p + "/clients": [{"id": "api"}, {"id": "anon"}],
            p + "/clients/api/scope-mappings/clients/api": [],
            p + "/clients/anon/scope-mappings/clients/api": [{"name": OLD}],
        }
        self.sends = []

    def retire(self, **kwargs):
        return preserve.retire_renamed_roles(
            self.realm, lambda path: copy.deepcopy(self.state.get(path)),
            lambda method, path, body=None: self.sends.append((method, path, body)), **kwargs)

    def test_remaining_holders_get_the_new_role_then_the_old_role_is_deleted(self):
        self.assertEqual(self.retire(), [OLD])
        new_role = [{"id": "new-id", "name": NEW}]
        p = self.prefix
        self.assertEqual(self.sends, [
            ("POST", p + "/users/u1/role-mappings/clients/api", new_role),
            ("POST", p + "/groups/g1/role-mappings/clients/api", new_role),
            ("POST", p + "/roles-by-id/cp/composites", new_role),
            ("POST", p + "/clients/anon/scope-mappings/clients/api", new_role),
            ("DELETE", self.old, None),
        ])

    def test_holders_are_paged(self):
        old = self.old
        del self.state[old + "/users?first=0&max=100"]
        del self.state[old + "/groups?first=0&max=100"]
        self.state[old + "/users?first=0&max=1"] = [{"id": "u1"}]
        self.state[old + "/users?first=1&max=1"] = [{"id": "u3"}]
        self.state[old + "/users?first=2&max=1"] = []
        self.state[old + "/groups?first=0&max=1"] = []
        self.state[self.prefix + "/users/u3/role-mappings/clients/api"] = []
        self.retire(page=1)
        users = [path for method, path, _ in self.sends if "/users/" in path]
        self.assertEqual(users, [self.prefix + "/users/u1/role-mappings/clients/api",
                                 self.prefix + "/users/u3/role-mappings/clients/api"])

    def test_nothing_happens_once_the_old_role_is_gone(self):
        self.state[self.old] = None
        self.assertEqual(self.retire(), [])
        self.assertEqual(self.sends, [])

    def test_a_missing_new_role_aborts_before_any_change(self):
        self.state[self.prefix + "/clients/api/roles/catalog.bank_of_canada%3Aread"] = None
        with self.assertRaises(RuntimeError):
            self.retire()
        self.assertEqual(self.sends, [])


if __name__ == "__main__":
    unittest.main()
