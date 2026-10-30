"""Offline tests: real Git histories and tags, recorded GitHub identity/responses."""

import copy
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import tag_milestone


SOURCE = Path(__file__).resolve().parents[2]
HELPER = SOURCE / "scripts/release/tag_milestone.py"


class SubprocessTimeoutTests(unittest.TestCase):
    def test_command_timeout_fails_closed_without_captured_credentials(self):
        error = subprocess.TimeoutExpired(
            ["gh", "api", "sensitive-argument"], 300,
            output="sensitive-output\n", stderr="sensitive-credentials\n",
        )
        with patch.object(tag_milestone.subprocess, "run", side_effect=error) as process:
            with self.assertRaises(tag_milestone.Refused) as refused:
                tag_milestone.run("gh", "api", "sensitive-argument")
        self.assertEqual(str(refused.exception), "gh api timed out after 300s")
        self.assertEqual(process.call_args.kwargs["timeout"], 300)

    def test_ancestry_timeout_fails_closed_without_captured_credentials(self):
        error = subprocess.TimeoutExpired(
            ["git", "merge-base", "--is-ancestor", "older", "newer"], 300,
            output=b"sensitive-output\n", stderr=b"sensitive-credentials\n",
        )
        with patch.object(tag_milestone.subprocess, "run", side_effect=error) as process:
            with self.assertRaises(tag_milestone.Refused) as refused:
                tag_milestone.ancestor("older", "newer")
        self.assertEqual(str(refused.exception), "cannot establish commit ancestry: git merge-base timed out after 300s")
        self.assertEqual(process.call_args.kwargs["timeout"], 300)


class MilestoneTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="train3-milestone-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.repo = self.root / "repo"
        self.origin = self.root / "origin.git"
        self.responses = self.root / "responses"
        self.responses.mkdir()
        self.repo.mkdir()
        self.env = dict(os.environ, GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1")
        self.command("git", "init", "--bare", "--initial-branch=main", str(self.origin))
        self.git("init", "--initial-branch=main")
        self.git("config", "user.name", "Milestone tests")
        self.git("config", "user.email", "milestone@example.invalid")
        self.git("config", "commit.gpgsign", "false")
        self.git("config", "tag.gpgsign", "false")
        self.git("remote", "add", "origin", str(self.origin))
        for relative in ("scripts/check-flags", "scripts/flags-merge", "config/flags", "backend/flags", "frontend/flags"):
            shutil.copytree(SOURCE / relative, self.repo / relative)
        (self.repo / "scripts/release").mkdir()
        shutil.copy2(HELPER, self.repo / "scripts/release/tag_milestone.py")
        (self.repo / "scripts/github").mkdir()
        shutil.copy2(SOURCE / "scripts/github/merge-queue-rules.json", self.repo / "scripts/github/merge-queue-rules.json")
        (self.repo / "PRODUCT_VERSION").write_text("4.1.0\n")
        self.git("add", ".")
        self.git("commit", "-m", "candidate")
        self.commit = self.git("rev-parse", "HEAD")
        self.git("push", "origin", "main")
        self.evidence = {
            "format_version": 1,
            "commit": self.commit,
            "url": "https://example.invalid/acceptance",
            "packages": {
                f"P3-{n}": {"commit": self.commit, "result": "passed", "url": f"https://example.invalid/p3-{n}", "pull_request": 1}
                for n in range(1, 7)
            },
            "lanes": {
                lane: {"commit": self.commit, "result": "passed", "url": f"https://example.invalid/{lane}"}
                for lane in ("platform", "company", "workspace")
            },
        }
        self.checks = [{"check_runs": [
            {"name": name, "head_sha": self.commit, "status": "completed", "conclusion": "success", "app": {"id": 15368}}
            for name in ("Feature Flags Check", "Core CI Result", "Release E2E Result", "Markdown lint and links", "Lint workflows", "Split-plane compatibility")
        ]}]
        self.statuses = [[{"context": "CodeRabbit", "state": "success"}]]
        self.pr = {
            "merged": True, "headRefOid": self.commit, "mergeCommit": {"oid": self.commit}, "reviewDecision": "APPROVED",
            "reviews": {"pageInfo": {"hasPreviousPage": False}, "nodes": [{"author": {"login": "reviewer"}, "state": "APPROVED", "commit": {"oid": self.commit}}]},
            "reviewThreads": {"pageInfo": {"hasNextPage": False}, "nodes": []},
        }
        bin_dir = self.root / "bin"
        bin_dir.mkdir()
        real_git = shutil.which("git", path=self.env["PATH"])
        # Only remote identity is recorded. All fetch, ancestry, tag and push
        # operations run the real Git against the scratch bare origin.
        git = bin_dir / "git"
        git.write_text(f"#!{sys.executable}\n" + """import os, sys
args = sys.argv[1:]
if args[:2] == ['remote', 'get-url']:
    key = 'MILESTONE_PUSH_ORIGIN' if '--push' in args else 'MILESTONE_FETCH_ORIGIN'
    print(os.environ.get(key, 'git@github.com:test/repository.git'))
else:
    os.execv(os.environ['MILESTONE_REAL_GIT'], [os.environ['MILESTONE_REAL_GIT'], *args])
""")
        git.chmod(0o755)
        gh = bin_dir / "gh"
        gh.write_text(f"#!{sys.executable}\n" + """import json, os, pathlib, sys
root = pathlib.Path(os.environ['MILESTONE_RESPONSES'])
args = sys.argv[1:]
with (root / 'requests').open('a') as out:
    out.write(json.dumps(args) + '\\n')
name = 'reviews' if 'graphql' in args else ('checks' if '/check-runs?' in args[-1] else 'statuses')
print((root / (name + '.json')).read_text())
""")
        gh.chmod(0o755)
        self.env.update(PATH=str(bin_dir) + os.pathsep + self.env["PATH"], MILESTONE_RESPONSES=str(self.responses), MILESTONE_REAL_GIT=real_git)

    def command(self, *args, **kwargs):
        result = subprocess.run(args, cwd=self.repo, env=self.env, capture_output=True, text=True, **kwargs)
        if result.returncode:
            self.fail(f"command failed: {args}: {result.stderr}")
        return result.stdout.strip()

    def git(self, *args):
        return self.command("git", *args)

    def run_helper(self, *args, evidence=None):
        path = self.root / "evidence.json"
        path.write_text(json.dumps(self.evidence if evidence is None else evidence))
        for name, value in (("checks", self.checks), ("statuses", self.statuses), ("reviews", {"data": {"repository": {"pullRequest": self.pr}}})):
            (self.responses / f"{name}.json").write_text(json.dumps(value))
        return subprocess.run(
            [sys.executable, "scripts/release/tag_milestone.py", "--evidence", str(path), *args],
            cwd=self.repo, env=self.env, capture_output=True, text=True,
        )

    def assert_refused(self, message, *args, evidence=None):
        result = self.run_helper(*args, evidence=evidence)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn(message, result.stderr)
        self.assertEqual(self.git("tag", "--list"), "")
        self.assertEqual(self.git("ls-remote", "--tags", "origin"), "")

    def test_dry_run_does_not_tag_or_change_product(self):
        result = self.run_helper("--dry-run")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Dry run passed", result.stdout)
        self.assertEqual(self.git("tag", "--list"), "")
        self.assertEqual(self.git("ls-remote", "--tags", "origin"), "")
        self.assertEqual(self.git("status", "--porcelain"), "")
        self.assertEqual((self.repo / "PRODUCT_VERSION").read_text(), "4.1.0\n")
        requests = (self.responses / "requests").read_text()
        self.assertIn(f"/commits/{self.commit}/check-runs", requests)
        self.assertIn(f"/commits/{self.commit}/statuses", requests)

    def test_default_is_dry_run(self):
        result = self.run_helper()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.git("tag", "--list"), "")

    def test_github_environment_cannot_redirect_acceptance(self):
        self.env.update(GH_REPO="unrelated/fork", GH_HOST="other.example.invalid")
        result = self.run_helper()
        self.assertEqual(result.returncode, 0, result.stderr)
        requests = [json.loads(line) for line in (self.responses / "requests").read_text().splitlines()]
        for request in requests:
            self.assertEqual(request[:3], ["api", "--hostname", "github.com"])
        self.assertIn(f"repos/test/repository/commits/{self.commit}/check-runs?per_page=100", requests[0])
        self.assertIn("owner=test", requests[2])
        self.assertIn("name=repository", requests[2])
        self.assertNotIn("unrelated/fork", str(requests))

    def test_distinct_push_destination_refused(self):
        self.env["MILESTONE_PUSH_ORIGIN"] = "git@github.com:unrelated/fork.git"
        self.assert_refused("fetch and push URLs must be identical")

    def test_multiple_push_destinations_refused(self):
        self.env["MILESTONE_PUSH_ORIGIN"] = "git@github.com:test/repository.git\ngit@github.com:unrelated/fork.git"
        self.assert_refused("fetch and push URLs must be identical")

    def test_publish_only_annotated_train_tag(self):
        result = self.run_helper("--push")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.git("tag", "--list"), "train-3")
        self.assertEqual(self.git("cat-file", "-t", "train-3"), "tag")
        self.assertEqual(self.git("rev-parse", "train-3^{commit}"), self.commit)
        self.assertIn("https://example.invalid/acceptance", self.git("cat-file", "-p", "train-3"))
        self.assertIn(self.commit, self.git("cat-file", "-p", "train-3"))
        self.assertEqual(len(self.git("ls-remote", "--tags", "origin").splitlines()), 2)
        self.assertEqual(self.git("status", "--porcelain"), "")
        self.assertEqual(self.git("branch", "--format=%(refname:short)"), "main")

    def test_local_duplicate_refused(self):
        self.git("tag", "train-3")
        result = self.run_helper("--push")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("already exists locally", result.stderr)
        self.assertEqual(self.git("cat-file", "-t", "train-3"), "commit")

    def test_remote_duplicate_refused(self):
        self.git("tag", "-a", "train-3", "-m", "original")
        self.git("push", "origin", "train-3")
        self.git("tag", "-d", "train-3")
        before = self.git("ls-remote", "--tags", "origin")
        result = self.run_helper("--push")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("already exists on origin", result.stderr)
        self.assertEqual(self.git("ls-remote", "--tags", "origin"), before)
        self.assertEqual(self.git("tag", "--list"), "")

    def test_unmerged_feature_head_refused(self):
        self.git("checkout", "-b", "feature")
        self.git("commit", "--allow-empty", "-m", "unmerged feature")
        self.git("push", "origin", "feature")
        self.assert_refused("freshly fetched origin/main")

    def test_stale_origin_main_is_fetched(self):
        self.git("commit", "--allow-empty", "-m", "new main")
        self.git("push", "origin", "main")
        self.git("checkout", "--detach", self.commit)
        self.git("update-ref", "refs/remotes/origin/main", self.commit)
        self.assert_refused("freshly fetched origin/main")

    def test_dirty_tree_refused(self):
        (self.repo / "untracked").write_text("dirty")
        self.assert_refused("working tree must be clean")

    def test_wrong_evidence_commit_refused(self):
        self.evidence["commit"] = "0" * 40
        self.assert_refused("exact candidate commit")

    def test_incomplete_package_and_lane_evidence_refused(self):
        for section, name in (("packages", "P3-4"), ("lanes", "company")):
            with self.subTest(section=section):
                evidence = copy.deepcopy(self.evidence)
                evidence[section][name]["result"] = "pending"
                self.assert_refused("passing evidence", evidence=evidence)
                evidence[section][name]["result"] = "passed"
                evidence[section][name]["commit"] = "0" * 40
                self.assert_refused("exact candidate commit", evidence=evidence)

    def test_evidence_requires_https_links(self):
        self.evidence["url"] = "file:///tmp/unverified"
        self.assert_refused("HTTPS")

    def test_pending_and_failed_checks_refused(self):
        for status, conclusion in (("in_progress", None), ("completed", "failure"), ("completed", "skipped")):
            with self.subTest(status=status, conclusion=conclusion):
                self.checks[0]["check_runs"][0].update(status=status, conclusion=conclusion)
                self.assert_refused("tag_milestone:")

    def test_ci_from_different_commit_refused(self):
        self.checks[0]["check_runs"][0]["head_sha"] = "0" * 40
        self.assert_refused("required check")

    def test_failure_on_later_ci_page_refused(self):
        self.checks.append({"check_runs": [{"name": "Contract tests", "head_sha": self.commit, "status": "completed", "conclusion": "failure"}]})
        self.assert_refused("Contract tests")

    def test_latest_status_per_context(self):
        self.statuses[0].append({"context": "CodeRabbit", "state": "failure"})
        self.assertEqual(self.run_helper().returncode, 0)
        self.statuses[0].insert(0, {"context": "CodeRabbit", "state": "pending"})
        self.assert_refused("CodeRabbit")

    def test_missing_workflow_refused(self):
        self.checks[0]["check_runs"] = self.checks[0]["check_runs"][1:]
        self.assert_refused("Feature Flags Check")

    def test_single_green_workflow_is_not_sufficient(self):
        self.checks[0]["check_runs"] = self.checks[0]["check_runs"][:1]
        self.assert_refused("Core CI Result")

    def test_missing_compatibility_check_refused(self):
        self.checks[0]["check_runs"].pop()
        self.assert_refused("Split-plane compatibility")

    def test_required_check_cannot_be_spoofed_by_another_app(self):
        self.checks[0]["check_runs"][0]["app"]["id"] = 42
        self.assert_refused("configured integration: Feature Flags Check")

    def test_unmerged_pr_refused(self):
        self.pr["merged"] = False
        self.assert_refused("merged into the candidate")

    def test_unresolved_review_thread_refused(self):
        self.pr["reviewThreads"]["nodes"] = [{"isResolved": False}]
        self.assert_refused("unresolved review threads")

    def test_pending_required_review_refused(self):
        self.pr["reviewDecision"] = "REVIEW_REQUIRED"
        self.assert_refused("outstanding required reviews")

    def test_changes_requested_review_refused(self):
        self.pr["reviews"]["nodes"].append({"author": {"login": "bot"}, "state": "CHANGES_REQUESTED", "commit": {"oid": self.commit}})
        self.assert_refused("unfinished or rejected review")

    def test_stale_review_refused(self):
        self.pr["reviews"]["nodes"][0]["commit"]["oid"] = "0" * 40
        self.assert_refused("completed review of its final head")

    def test_review_pagination_is_not_silently_ignored(self):
        self.pr["reviewThreads"]["pageInfo"]["hasNextPage"] = True
        self.assert_refused("review audit page limit")

    def test_real_flag_expiry_blocks_tag(self):
        flags_path = self.repo / "config/flags/flags.flagd.json"
        flags = json.loads(flags_path.read_text())
        flags["flags"]["mcp"]["metadata"]["remove_by"] = "train 3"
        flags_path.write_text(json.dumps(flags))
        self.git("add", ".")
        self.git("commit", "-m", "expired flag")
        self.git("push", "origin", "main")
        self.commit = self.git("rev-parse", "HEAD")
        self.evidence["commit"] = self.commit
        for items in (self.evidence["packages"], self.evidence["lanes"]):
            for item in items.values():
                item["commit"] = self.commit
        for check in self.checks[0]["check_runs"]:
            check["head_sha"] = self.commit
        self.assert_refused("node scripts/check-flags failed")


if __name__ == "__main__":
    unittest.main()
