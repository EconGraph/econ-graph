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
        self.assertEqual(merged["attributes"][preserve.INITIALIZED],"true")

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
            {"name":"custom-public-policy","clientRole":False},
        ]
        self.state[self.prefix + "/roles/user/composites"] = [{"name":"catalog-public","clientRole":False}]
        self.state[self.prefix + "/users/reader/role-mappings"] = {
            "realmMappings":[{"name":"custom-public-policy"}],
            "clientMappings":{"econ-graph-api":{"mappings":[{"name":"catalog.fred:read"}]}},
        }
        self.state[self.prefix + "/users/reader/groups"] = [{"path": "/public-readers"}]
        merged = self.merge()
        self.assertEqual(merged["users"][0]["groups"],["/public-readers"])
        self.assertEqual(self.role(merged,"catalog-public")["composites"],{"realm":["custom-public-policy"],"client":{"econ-graph-api":["catalog.bea:read"]}})
        self.assertEqual(merged["users"][0]["realmRoles"],["custom-public-policy"])
        self.assertEqual(merged["users"][0]["clientRoles"],{"econ-graph-api":["catalog.fred:read"]})
        self.assertIn("catalog-public",self.role(merged,"user")["composites"]["realm"])

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

    def test_missing_relations_or_failed_api_never_fall_back_to_defaults(self):
        self.state[self.prefix + "/roles/catalog-public/composites"] = None
        with self.assertRaises(RuntimeError): self.merge()
        def failed(_): raise RuntimeError("unavailable")
        with self.assertRaises(RuntimeError): preserve.merge_catalog_policy(self.realm,failed)


if __name__ == "__main__":
    unittest.main()
