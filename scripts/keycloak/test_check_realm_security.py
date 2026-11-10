#!/usr/bin/env python3
"""Tests for check-realm-security.py: the repo's realm passes, and each weakening fails.

Usage: python3 scripts/keycloak/test_check_realm_security.py
"""

import copy
import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
SCRIPT = HERE / "check-realm-security.py"
spec = importlib.util.spec_from_file_location("check_realm_security", SCRIPT)
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)
REALM = json.loads(checker.DEFAULT_REALM.read_text())


def flow(realm, alias):
    return next(f for f in realm["authenticationFlows"] if f["alias"] == alias)


class RealmSecurityTest(unittest.TestCase):
    def problems(self, change):
        realm = copy.deepcopy(REALM)
        change(realm)
        return checker.check(realm)

    def assert_fails(self, change, expected):
        problems = self.problems(change)
        self.assertTrue(any(expected in p for p in problems), f"wanted {expected!r} in {problems}")

    def test_the_repo_realm_passes(self):
        self.assertEqual(checker.check(REALM), [])
        self.assertEqual(REALM["accessTokenLifespan"], checker.MAX_ACCESS_TOKEN_LIFESPAN)

    def test_a_longer_or_missing_token_lifespan_fails(self):
        self.assert_fails(lambda r: r.update(accessTokenLifespan=3600), "accessTokenLifespan is 3600s")
        self.assert_fails(lambda r: r.pop("accessTokenLifespan"), "accessTokenLifespan is not set")

    def test_a_client_lifespan_override_fails(self):
        def change(realm):
            realm["clients"][0].setdefault("attributes", {})["access.token.lifespan"] = "86400"

        self.assert_fails(change, "sets access.token.lifespan to '86400'")

    def test_a_shorter_client_lifespan_passes(self):
        def change(realm):
            realm["clients"][0].setdefault("attributes", {})["access.token.lifespan"] = "60"

        self.assertEqual(self.problems(change), [])

    def test_the_password_grant_fails_except_on_keycloak_clients(self):
        def web(realm):
            realm["clients"][0]["directAccessGrantsEnabled"] = "${KC_WEB_DIRECT_GRANTS:true}"

        self.assert_fails(web, "allows the password grant")

        def admin_cli(realm):
            realm["clients"].append({"clientId": "admin-cli", "directAccessGrantsEnabled": True})

        self.assertEqual(self.problems(admin_cli), [])

    def test_the_default_browser_flow_fails(self):
        self.assert_fails(lambda r: r.pop("browserFlow"), "browserFlow is not set")
        self.assert_fails(lambda r: r.update(browserFlow="browser"), "is not in authenticationFlows")

    def test_a_browser_flow_without_the_staff_otp_fails(self):
        def drop(realm):
            forms = flow(realm, "browser with staff otp forms")
            forms["authenticationExecutions"] = [
                e for e in forms["authenticationExecutions"] if e.get("flowAlias") != "staff otp"
            ]

        self.assert_fails(drop, "no conditional subflow requiring an OTP")

        def optional_otp(realm):
            for item in flow(realm, "staff otp")["authenticationExecutions"]:
                if item.get("authenticator") == "auth-otp-form":
                    item["requirement"] = "ALTERNATIVE"

        self.assert_fails(optional_otp, "no conditional subflow requiring an OTP")

    def test_a_staff_condition_on_another_role_or_negated_fails(self):
        def other_role(realm):
            realm["authenticatorConfig"][0]["config"]["condUserRole"] = "admin"

        self.assert_fails(other_role, "no conditional subflow requiring an OTP")

        def negated(realm):
            realm["authenticatorConfig"][0]["config"]["negate"] = "true"

        self.assert_fails(negated, "no conditional subflow requiring an OTP")

    def test_an_identity_provider_without_the_staff_otp_fails(self):
        self.assert_fails(
            lambda r: r["identityProviders"][0].pop("postBrokerLoginFlowAlias"),
            "has no post-login flow",
        )

        def plain(realm):
            realm["identityProviders"][1]["postBrokerLoginFlowAlias"] = "browser with staff otp forms"
            forms = flow(realm, "browser with staff otp forms")
            forms["authenticationExecutions"] = forms["authenticationExecutions"][:1]

        self.assert_fails(plain, "does not require an OTP")

    def test_a_staff_role_outside_support_fails(self):
        def change(realm):
            realm["roles"]["realm"].append(
                {"name": "auditor", "composite": True,
                 "composites": {"client": {"econ-graph-api": ["admin.audit:read"]}}}
            )

        self.assert_fails(change, "realm role 'auditor' grants admin.audit:read without including 'support'")

    def test_a_live_export_is_read_from_a_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            good = Path(tmp) / "good.json"
            bad = Path(tmp) / "bad.json"
            good.write_text(json.dumps(REALM))
            weak = copy.deepcopy(REALM)
            weak["accessTokenLifespan"] = 1800
            bad.write_text(json.dumps(weak))
            run = lambda path: subprocess.run(  # noqa: E731
                [sys.executable, str(SCRIPT), "--realm", str(path)], capture_output=True, text=True
            )
            self.assertEqual(run(good).returncode, 0, run(good).stderr)
            result = run(bad)
            self.assertEqual(result.returncode, 1)
            self.assertIn("accessTokenLifespan is 1800s", result.stderr)
            self.assertEqual(run(Path(tmp) / "missing.json").returncode, 2)


if __name__ == "__main__":
    unittest.main()
