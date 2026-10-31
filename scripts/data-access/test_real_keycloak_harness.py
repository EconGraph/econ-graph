#!/usr/bin/env python3
"""Failure-path contracts; no Docker daemon or external service is contacted."""

import importlib.util
import json
from pathlib import Path
import subprocess
import traceback
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    "real_keycloak", Path(__file__).with_name("test-real-keycloak.py")
)
harness = importlib.util.module_from_spec(spec)
spec.loader.exec_module(harness)


class DisposableHarnessTests(unittest.TestCase):
    def test_docker_failure_redacts_argv_and_daemon_output(self):
        lab = harness.Lab()
        secret = "fixture-password-must-never-appear"
        argv = ["docker", "create", "--env=POSTGRES_PASSWORD=" + secret]
        error = subprocess.CalledProcessError(125, argv, output=secret, stderr=secret)
        with patch.object(
            harness.subprocess, "check_output", side_effect=error
        ) as call:
            try:
                lab.docker(*argv[1:])
            except harness.DockerFailure:
                rendered = traceback.format_exc()
            else:
                self.fail("Docker failure escaped sanitization")
        self.assertNotIn(secret, rendered)
        self.assertNotIn("POSTGRES_PASSWORD", rendered)
        self.assertNotIn("CalledProcessError", rendered)
        self.assertIn("Disposable Docker operation failed", rendered)
        self.assertEqual(call.call_args.kwargs["stderr"], subprocess.PIPE)

    def test_failed_start_retains_created_id_for_owned_cleanup(self):
        lab = harness.Lab()
        calls = []

        def daemon(*args):
            calls.append(args)
            if args[0] == "create":
                return "immutable-created-id"
            if args[0] == "start":
                raise harness.DockerFailure("Disposable Docker operation failed")
            if args[0] == "inspect":
                return json.dumps({harness.OWNER_LABEL: lab.owner})
            return ""

        with patch.object(lab, "docker", side_effect=daemon):
            with self.assertRaises(harness.DockerFailure):
                lab.run("postgres", "immutable-test-image")
            self.assertEqual(lab.containers, ["immutable-created-id"])
            self.assertEqual(lab.cleanup(), 0)
        self.assertEqual(calls[0][0], "create")
        self.assertIn(("rm", "--force", "immutable-created-id"), calls)

    def test_cleanup_continues_after_missing_or_unowned_resources(self):
        lab = harness.Lab()
        lab.containers = ["owned", "foreign", "already-gone"]
        lab.network = "owned-network"
        calls = []

        def daemon(*args):
            calls.append(args)
            if args[0] == "inspect":
                if args[-1] == "already-gone":
                    raise harness.DockerFailure("Disposable Docker operation failed")
                owner = "other-owner" if args[-1] == "foreign" else lab.owner
                return json.dumps({harness.OWNER_LABEL: owner})
            if args[:2] == ("network", "inspect"):
                return json.dumps({harness.OWNER_LABEL: lab.owner})
            return ""

        original = RuntimeError("original acceptance failure")
        with patch.object(lab, "docker", side_effect=daemon):
            try:
                raise original
            except RuntimeError as caught:
                self.assertEqual(lab.cleanup(), 2)
                self.assertIs(caught, original)
        self.assertIn(("rm", "--force", "owned"), calls)
        self.assertIn(("network", "rm", "owned-network"), calls)
        self.assertFalse(
            any(args[:2] == ("rm", "--force") and args[-1] != "owned" for args in calls)
        )


if __name__ == "__main__":
    unittest.main()
