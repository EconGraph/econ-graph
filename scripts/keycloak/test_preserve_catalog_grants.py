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


if __name__ == "__main__":
    unittest.main()
