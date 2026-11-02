"""A sanitizer failure must propagate through the required Core CI result."""

import json
import os
from pathlib import Path
import subprocess
import unittest

import yaml


ROOT = Path(__file__).resolve().parents[2]


class SanitizedFixtureGateTest(unittest.TestCase):
    def setUp(self):
        workflow = yaml.safe_load((ROOT / ".github/workflows/ci-core.yml").read_text())
        self.result_job = workflow["jobs"]["ci-result"]
        self.step = next(s for s in self.result_job["steps"] if s.get("name") == "Check the job results")

    def gate(self, fixture_result, event="pull_request"):
        # Build the same needs context the actual workflow supplies. Omitting
        # sanitized-fixture from needs must make the failure tests fail too.
        results = {name: {"result": fixture_result if name == "sanitized-fixture" else "success"}
                   for name in self.result_job["needs"]}
        return subprocess.run(
            [str(ROOT / "ci/scripts/check-required-jobs.sh")],
            env=dict(os.environ, NEEDS=json.dumps(results), EVENT=event,
                     MUST_RUN=self.step["env"]["MUST_RUN"]),
            capture_output=True, text=True,
        ).returncode

    def test_sanitizer_failure_and_cancellation_fail_required_result(self):
        for event in ("pull_request", "merge_group"):
            for result in ("failure", "cancelled"):
                with self.subTest(event=event, result=result):
                    self.assertNotEqual(self.gate(result, event), 0)

    def test_merge_queue_cannot_pass_a_skipped_sanitizer(self):
        self.assertNotEqual(self.gate("skipped", "merge_group"), 0)

    def test_successful_sanitizer_allows_required_result(self):
        for event in ("pull_request", "merge_group"):
            self.assertEqual(self.gate("success", event), 0)


if __name__ == "__main__":
    unittest.main()
