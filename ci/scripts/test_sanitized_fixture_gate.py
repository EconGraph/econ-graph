"""A sanitizer failure must propagate through the required Core CI result."""

import json
import os
from pathlib import Path
import subprocess
import stat
import tempfile
import unittest

import yaml


ROOT = Path(__file__).resolve().parents[2]


class SanitizedFixtureGateTest(unittest.TestCase):
    def setUp(self):
        workflow = yaml.safe_load((ROOT / ".github/workflows/ci-core.yml").read_text())
        self.fixture_job = workflow["jobs"]["sanitized-fixture"]
        self.changes_job = workflow["jobs"]["changes"]
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

    def test_fixture_checkout_honors_requested_manual_branch(self):
        checkout = next(step for step in self.fixture_job["steps"]
                        if step.get("uses", "").startswith("actions/checkout@"))
        self.assertEqual(checkout["with"]["ref"],
                         "${{ github.event.inputs.branch || github.ref }}")

    def test_harness_cleanup_preserves_caller_scratch_parent(self):
        # Run the actual harness through an early network-create failure. The
        # stub is the only Docker implementation reachable; no daemon is used.
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            parent = root / "caller-owned"
            parent.mkdir(mode=0o750)
            marker = parent / "keep.txt"
            marker.write_bytes(b"caller data")
            marker.chmod(0o400)
            before_modes = (stat.S_IMODE(parent.stat().st_mode),
                            stat.S_IMODE(marker.stat().st_mode))
            binary_dir = root / "bin"
            binary_dir.mkdir()
            docker = binary_dir / "docker"
            docker.write_text("""#!/bin/sh
printf '%s\n' "$*" >>"$FIXTURE_SCRATCH_DOCKER_LOG"
case "$1 $2" in
    'info '*) echo '["name=rootless"]'; exit 0 ;;
    'network create') echo 'synthetic network failure' >&2; exit 73 ;;
    'rm -fv'|'network rm') exit 0 ;;
    *) echo 'unexpected Docker invocation' >&2; exit 99 ;;
esac
""")
            docker.chmod(0o700)
            log = root / "docker.log"
            result = subprocess.run(
                ["bash", str(ROOT / "scripts/fixtures/test-build-sanitized-fixture.sh")],
                env=dict(os.environ, PATH=str(binary_dir) + os.pathsep + os.environ["PATH"],
                         FIXTURE_TEST_DIR=str(parent), FIXTURE_TEST_KEEP="",
                         FIXTURE_SCRATCH_DOCKER_LOG=str(log), CI="false"),
                capture_output=True, text=True, timeout=30,
            )
            self.assertEqual(result.returncode, 73, result.stdout + result.stderr)
            self.assertIn("synthetic network failure", result.stderr)
            self.assertIn("network create ", log.read_text())
            self.assertIn("rm -fv ", log.read_text())
            self.assertTrue(parent.is_dir(), "cleanup removed the caller's directory")
            self.assertEqual(sorted(path.name for path in parent.iterdir()), [marker.name])
            self.assertEqual(marker.read_bytes(), b"caller data")
            self.assertEqual((stat.S_IMODE(parent.stat().st_mode),
                              stat.S_IMODE(marker.stat().st_mode)), before_modes)

    def test_sanitizer_failure_and_cancellation_fail_required_result(self):
        for event in ("pull_request", "merge_group"):
            for result in ("failure", "cancelled"):
                with self.subTest(event=event, result=result):
                    self.assertNotEqual(self.gate(result, event), 0)

    def test_docs_only_pr_can_skip_sanitizer(self):
        self.assertIn("changes", self.fixture_job["needs"])
        self.assertEqual(self.fixture_job["if"], "${{ needs.changes.outputs.code == 'true' }}")
        # Non-PR events must force code=true, keeping the queue strict.
        self.assertEqual(
            self.changes_job["outputs"]["code"],
            "${{ github.event_name != 'pull_request' || steps.code.outputs.code == 'true' }}",
        )
        self.assertEqual(self.gate("skipped", "pull_request"), 0)

    def test_merge_queue_cannot_pass_a_skipped_sanitizer(self):
        self.assertNotEqual(self.gate("skipped", "merge_group"), 0)

    def test_successful_sanitizer_allows_required_result(self):
        for event in ("pull_request", "merge_group"):
            self.assertEqual(self.gate("success", event), 0)


if __name__ == "__main__":
    unittest.main()
