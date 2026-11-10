"""Tests for catalog-access.py's refusal of roles the realm file marks never granted; no
Keycloak needed. The script's other behaviour is tested end to end by test-set-catalog-access.sh."""
import contextlib
import importlib.util
import io
import os
from pathlib import Path
from unittest.mock import patch
import unittest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("catalog_access", HERE / "catalog-access.py")
catalog_access = importlib.util.module_from_spec(spec)
spec.loader.exec_module(catalog_access)

MASSIVE = "catalog.massive_market_data:read"


class FakeKeycloak:
    """The reads check_role makes, and every call recorded; any write is a test failure."""
    def __init__(self, *_):
        self.calls = []

    def call(self, method, path, body=None, missing_ok=False):
        self.calls.append((method, path))
        if method != "GET":
            raise AssertionError("catalog-access.py wrote %s %s" % (method, path))
        if path == "/clients?clientId=econ-graph-api":
            return [{"id": "api", "clientId": "econ-graph-api"}]
        if path.startswith("/clients/api/roles/"):
            name = path[len("/clients/api/roles/"):].replace("%3A", ":")
            attributes = {catalog_access.NEVER_GRANTED: ["true"]} if name == MASSIVE else {}
            return {"id": "id-" + name, "name": name, "attributes": attributes}
        return {}


class NeverGrantedTests(unittest.TestCase):
    def run_script(self, *args):
        kc = FakeKeycloak()
        err = io.StringIO()
        env = {"KEYCLOAK_URL": "http://kc", "KEYCLOAK_USER": "admin", "KEYCLOAK_PASSWORD": "x"}
        with patch.object(catalog_access, "Keycloak", lambda *_: kc), patch.dict(os.environ, env), \
                contextlib.redirect_stderr(err), contextlib.redirect_stdout(io.StringIO()):
            status = catalog_access.main(["catalog-access.py"] + list(args))
        return status, err.getvalue(), kc

    def test_admin_only_and_make_public_refuse_a_never_granted_role(self):
        for mode in ("--admin-only", "--make-public"):
            for args in ([mode, MASSIVE], ["--dry-run", mode, MASSIVE],
                         ["--admin-only", "catalog.fred:read", mode, MASSIVE]):
                status, err, kc = self.run_script(*args)
                self.assertEqual(status, 3, args)
                self.assertIn("refused: %s is never granted" % MASSIVE, err)
                self.assertIn("nothing changed", err)
                self.assertIn("config/keycloak/README.md", err)
                # Every role is checked before any is changed, so FRED was left alone too.
                self.assertTrue(all(method == "GET" for method, _ in kc.calls))

    def test_check_role_allows_withdrawing_a_never_granted_role(self):
        kc = FakeKeycloak()
        api_id, role = catalog_access.check_role(kc, MASSIVE, "withdraw")
        self.assertEqual((api_id, role["name"]), ("api", MASSIVE))
        with self.assertRaises(catalog_access.Refused):
            catalog_access.check_role(kc, MASSIVE, "admin-only")
        catalog_access.check_role(kc, "catalog.fred:read", "admin-only")
        catalog_access.check_role(kc, "catalog.fred:read", "public")


if __name__ == "__main__":
    unittest.main()
