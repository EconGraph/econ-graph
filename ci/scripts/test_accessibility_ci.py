"""Offline failure-propagation contracts for accessibility and required Core CI."""
import json
import os
from pathlib import Path
import subprocess
import unittest

import yaml


ROOT = Path(__file__).resolve().parents[2]
AUDITS = (
    "accessibility-static-analysis",
    "accessibility-runtime-tests",
    "accessibility-browser-tests",
    "accessibility-manual-tests",
)


def workflow(name):
    # BaseLoader retains the literal `on` key on PyYAML's YAML 1.1 parser.
    return yaml.load((ROOT / ".github/workflows" / name).read_text(), Loader=yaml.BaseLoader)


class AccessibilityGateTests(unittest.TestCase):
    def setUp(self):
        self.results = {name: {"result": "success"} for name in AUDITS}
        self.results["accessibility-manual-tests"]["result"] = "skipped"

    def audit_gate(self, results=None, event="pull_request", mode="all"):
        return subprocess.run(
            [str(ROOT / "ci/scripts/check-accessibility-jobs.sh")],
            env=dict(os.environ, NEEDS=json.dumps(self.results if results is None else results),
                     EVENT=event, TEST_TYPE=mode),
            text=True, capture_output=True,
        ).returncode

    def core_gate(self, accessibility_result, event="pull_request"):
        core = workflow("ci-core.yml")["jobs"]["ci-result"]
        results = {name: {"result": "success"} for name in core["needs"]}
        results["accessibility"] = {"result": accessibility_result}
        step = next(s for s in core["steps"] if s.get("name") == "Check the job results")
        return subprocess.run(
            [str(ROOT / "ci/scripts/check-required-jobs.sh")],
            env=dict(os.environ, NEEDS=json.dumps(results), EVENT=event,
                     MUST_RUN=step["env"]["MUST_RUN"]),
            text=True, capture_output=True,
        ).returncode

    def test_each_automated_audit_failure_or_cancellation_reaches_core_result(self):
        for audit in AUDITS[:3]:
            for result in ("failure", "cancelled"):
                with self.subTest(audit=audit, result=result):
                    self.results[audit]["result"] = result
                    rc = self.audit_gate()
                    self.assertNotEqual(rc, 0)
                    self.assertNotEqual(self.core_gate("failure" if rc else "success"), 0)
                    self.results[audit]["result"] = "success"

    def test_selected_audits_cannot_pass_as_skipped_or_missing(self):
        for audit in AUDITS[:3]:
            with self.subTest(audit=audit):
                self.results[audit]["result"] = "skipped"
                self.assertNotEqual(self.audit_gate(), 0)
                del self.results[audit]
                self.assertNotEqual(self.audit_gate(), 0)
                self.results[audit] = {"result": "success"}

    def test_empty_or_invalid_results_fail(self):
        for results in ({}, None, [], "success"):
            with self.subTest(results=results):
                encoded = json.dumps(results)
                cp = subprocess.run(
                    [str(ROOT / "ci/scripts/check-accessibility-jobs.sh")],
                    env=dict(os.environ, NEEDS=encoded, EVENT="pull_request", TEST_TYPE="all"),
                    text=True, capture_output=True,
                )
                self.assertNotEqual(cp.returncode, 0)

    def test_all_audits_pass_and_docs_filter_skip_is_allowed_only_outside_queue(self):
        self.assertEqual(self.audit_gate(), 0)
        self.assertEqual(self.core_gate("success", "merge_group"), 0)
        self.assertEqual(self.core_gate("skipped", "pull_request"), 0)
        self.assertNotEqual(self.core_gate("skipped", "merge_group"), 0)

    def test_dispatch_static_does_not_require_runtime_browser_or_manual(self):
        for name in AUDITS[1:]:
            self.results[name]["result"] = "skipped"
        self.assertEqual(self.audit_gate(event="workflow_dispatch", mode="static"), 0)

    def test_dispatch_runtime_still_requires_browser(self):
        self.results[AUDITS[0]]["result"] = "skipped"
        self.assertEqual(self.audit_gate(event="workflow_dispatch", mode="runtime"), 0)
        self.results[AUDITS[2]]["result"] = "skipped"
        self.assertNotEqual(self.audit_gate(event="workflow_dispatch", mode="runtime"), 0)

    def test_manual_checklist_modes_and_schedule(self):
        self.results[AUDITS[3]]["result"] = "success"
        self.assertEqual(self.audit_gate(event="schedule"), 0)
        for name in AUDITS[:3]:
            self.results[name]["result"] = "skipped"
        self.assertEqual(self.audit_gate(event="workflow_dispatch", mode="manual"), 0)
        self.results[AUDITS[3]]["result"] = "skipped"
        self.assertNotEqual(self.audit_gate(event="workflow_dispatch", mode="manual"), 0)
        self.assertNotEqual(self.audit_gate(mode="manual"), 0)
        self.assertNotEqual(self.audit_gate(mode="unknown"), 0)

    def test_core_calls_same_workflow_and_gate_is_wired_to_all_dependencies(self):
        core = workflow("ci-core.yml")
        self.assertIn("merge_group", core["on"])
        caller = core["jobs"]["accessibility"]
        self.assertEqual(caller["uses"], "./.github/workflows/accessibility-tests.yml")
        self.assertEqual(caller["with"]["test_type"], "all")
        self.assertIn("needs.changes.outputs.code == 'true'", caller["if"])
        self.assertIn("needs.changes.outputs.backend_only != 'true'", caller["if"])
        self.assertIn("accessibility", core["jobs"]["ci-result"]["needs"])
        audits = workflow("accessibility-tests.yml")
        self.assertIn("workflow_call", audits["on"])
        self.assertIn("workflow_dispatch", audits["on"])
        self.assertIn("schedule", audits["on"])
        self.assertNotIn("pull_request", audits["on"])
        self.assertNotIn("push", audits["on"])
        summary = audits["jobs"]["accessibility-summary"]
        self.assertEqual(set(summary["needs"]), set(AUDITS))
        gate = summary["steps"][-1]
        self.assertEqual(gate["if"], "always()")
        self.assertEqual(gate["run"], "ci/scripts/check-accessibility-jobs.sh")


if __name__ == "__main__":
    unittest.main()
