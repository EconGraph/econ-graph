#!/usr/bin/env python3
"""Full-script reuse checks with synthetic files; never starts Docker or opens a dump.

Only Docker's rootless-info response and an uncontended flock are stubbed. The
builder performs its real input hashing, checksum checks, identity comparison
and byte counting. Run with Bash 4+ and GNU coreutils, as on the Linux CI runner.
"""

import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
BUILD = ROOT / "scripts/fixtures/build-sanitized-fixture.sh"


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


class FixtureReuseTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.inputs = self.root / "inputs"
        self.inputs.mkdir()
        for name in ("econ_graph", "keycloak"):
            (self.inputs / (name + ".dump")).write_bytes(b"synthetic input " + name.encode())
        self.output = self.root / "fixture"
        self.output.mkdir()
        (self.output / "econ_graph.dump").write_bytes(b"x" * 3072)
        (self.output / "keycloak.dump").write_bytes(b"y" * 1024)
        self.manifest()
        self.checksums()
        binary_dir = self.root / "bin"
        binary_dir.mkdir()
        stubs = {
            "docker": """#!/bin/sh
if [ "$1" = info ]; then
    printf '28.0.0 ["name=rootless"]\\n'
else
    echo 'No Docker operation except info is allowed by this reuse test' >&2
    exit 99
fi
""",
            "flock": """#!/bin/sh
# Each test owns its output parent; no other process can hold this fixture lock.
[ "$1" = -n ] && [ "$2" = 9 ]
""",
        }
        for name, content in stubs.items():
            binary = binary_dir / name
            binary.write_text(content)
            binary.chmod(0o700)
        self.env = dict(os.environ, PATH=str(binary_dir) + os.pathsep + os.environ["PATH"])

    def manifest(self, mode="ci-subset"):
        lines = ["mode=" + mode]
        if mode == "ci-subset":
            lines += ["subset.series=300", "subset.max_points=5000"]
        for name in ("econ_graph", "keycloak"):
            lines.append("input." + name + ".sha256=" + sha256(self.inputs / (name + ".dump")))
        # Deliberately wrong: cap enforcement must measure dumps, not trust metadata.
        lines.append("total_kib=1")
        (self.output / "MANIFEST").write_text("\n".join(lines) + "\n")

    def checksums(self):
        files = ("econ_graph.dump", "keycloak.dump", "MANIFEST")
        (self.output / "SHA256SUMS").write_text(
            "".join(sha256(self.output / name) + "  " + name + "\n" for name in files)
        )

    def snapshot(self):
        return {p.name: p.read_bytes() for p in self.output.iterdir()}

    def reuse(self, cap, subset=True):
        args = [shutil.which("bash"), str(BUILD),
                "--data", str(self.inputs / "econ_graph.dump"),
                "--keycloak", str(self.inputs / "keycloak.dump"),
                "--output-dir", str(self.output), "--subset-max-kb", str(cap)]
        if subset:
            args.append("--ci-subset")
        return subprocess.run(args, env=self.env, text=True, capture_output=True)

    def test_matching_fixture_obeys_lower_cap_without_overwriting(self):
        before = self.snapshot()
        result = self.reuse(1)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("4 KiB, over --subset-max-kb 1", result.stderr)
        self.assertEqual(self.snapshot(), before)

    def test_matching_fixture_is_reused_at_or_below_cap(self):
        before = self.snapshot()
        for cap in (4, 8):
            with self.subTest(cap=cap):
                result = self.reuse(cap)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn("nothing to do", result.stderr)
                self.assertEqual(self.snapshot(), before)

    def test_cap_counts_all_dumps_and_rounds_up_partial_kib(self):
        (self.output / "keycloak.dump").write_bytes(b"y" * 1025)
        self.checksums()
        before = self.snapshot()
        result = self.reuse(4)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("5 KiB, over --subset-max-kb 4", result.stderr)
        self.assertEqual(self.snapshot(), before)

    def test_bad_checksums_cannot_be_reused_or_overwritten(self):
        (self.output / "econ_graph.dump").write_bytes(b"changed after checksumming")
        before = self.snapshot()
        result = self.reuse(8)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("already exists and is not this build", result.stderr)
        self.assertEqual(self.snapshot(), before)

    def test_subset_cap_does_not_limit_full_fixture_reuse(self):
        self.manifest(mode="full")
        self.checksums()
        before = self.snapshot()
        result = self.reuse(1, subset=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.snapshot(), before)


if __name__ == "__main__":
    unittest.main()
