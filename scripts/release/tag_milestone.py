#!/usr/bin/env python3
"""Validate, then optionally publish the non-product train-3 milestone.

The default is a dry run. Evidence is an externally stored JSON document, since
a commit cannot contain a manifest naming its own SHA. See RELEASE_PROCESS.md.
This path intentionally has no version, changelog, image or deployment handling.
"""

import argparse
import json
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import urlsplit


TAG = "train-3"
PACKAGES = tuple(f"P3-{n}" for n in range(1, 7))
LANES = ("platform", "company", "workspace")
SUBPROCESS_TIMEOUT_SECONDS = 300


class Refused(Exception):
    """A required acceptance condition has not been established."""


def run(*args):
    try:
        result = subprocess.run(
            args, text=True, capture_output=True, check=False,
            timeout=SUBPROCESS_TIMEOUT_SECONDS,
        )
    except subprocess.TimeoutExpired as error:
        # Do not expose captured output or full command arguments: either can
        # contain credentials when a remote command stalls.
        raise Refused(f"{args[0]} {args[1]} timed out after {SUBPROCESS_TIMEOUT_SECONDS}s") from error
    if result.returncode:
        # Authentication errors may contain credentials; do not echo stderr.
        raise Refused(f"{args[0]} {args[1]} failed (exit {result.returncode})")
    return result.stdout.strip()


def ancestor(older, newer):
    try:
        result = subprocess.run(
            ["git", "merge-base", "--is-ancestor", older, newer],
            capture_output=True,
            check=False,
            timeout=SUBPROCESS_TIMEOUT_SECONDS,
        )
    except subprocess.TimeoutExpired as error:
        raise Refused(f"cannot establish commit ancestry: git merge-base timed out after {SUBPROCESS_TIMEOUT_SECONDS}s") from error
    if result.returncode not in (0, 1):
        raise Refused("cannot establish commit ancestry")
    return result.returncode == 0


def url(value):
    if not isinstance(value, str) or any(c.isspace() for c in value):
        raise Refused("evidence references must be HTTPS URLs without whitespace")
    try:
        parsed = urlsplit(value)
    except ValueError as error:
        raise Refused("invalid evidence URL") from error
    if parsed.scheme != "https" or not parsed.hostname or parsed.username or parsed.password:
        raise Refused("evidence references must be HTTPS URLs without credentials")
    return value


def load_evidence(path, commit):
    try:
        evidence = json.loads(path.read_text())
    except (OSError, ValueError) as error:
        raise Refused("cannot read evidence JSON") from error
    if not isinstance(evidence, dict) or evidence.get("format_version") != 1:
        raise Refused("evidence format_version must be 1")
    if evidence.get("commit") != commit:
        raise Refused("evidence must name the exact candidate commit")
    references = [url(evidence.get("url"))]
    pull_requests = set()
    for section, names in (("packages", PACKAGES), ("lanes", LANES)):
        entries = evidence.get(section)
        if not isinstance(entries, dict) or set(entries) != set(names):
            raise Refused(f"evidence {section} must contain exactly {', '.join(names)}")
        for name in names:
            item = entries[name]
            if not isinstance(item, dict) or item.get("commit") != commit or item.get("result") != "passed":
                raise Refused(f"{name} needs passing evidence for the exact candidate commit")
            references.append(url(item.get("url")))
            if section == "packages":
                number = item.get("pull_request")
                if type(number) is not int or number <= 0:
                    raise Refused(f"{name} needs its merged implementation pull_request number")
                pull_requests.add(number)
    return references, pull_requests


def github_json(*args):
    try:
        return json.loads(run("gh", "api", "--hostname", "github.com", *args))
    except ValueError as error:
        raise Refused("GitHub returned invalid JSON") from error


def origin_repository():
    """Bind acceptance to the push destination, ignoring GH_REPO/GH_HOST."""
    origin = run("git", "remote", "get-url", "origin")
    if origin.startswith("git@github.com:"):
        path = origin.removeprefix("git@github.com:")
    else:
        try:
            parsed = urlsplit(origin)
        except ValueError as error:
            raise Refused("cannot parse origin GitHub URL") from error
        if parsed.scheme not in ("https", "ssh") or parsed.hostname != "github.com" or parsed.password:
            raise Refused("origin must identify a github.com repository")
        path = parsed.path.lstrip("/")
    repository = path.removesuffix(".git")
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
        raise Refused("cannot establish the origin GitHub repository")
    # A distinct push URL could publish a tag somewhere other than the audited
    # fetch remote. Refuse it rather than treating repository names as aliases.
    if run("git", "remote", "get-url", "--push", "--all", "origin") != origin:
        raise Refused("origin fetch and push URLs must be identical")
    return repository


def verify_ci(repository, commit):
    try:
        rules = json.loads(Path("scripts/github/merge-queue-rules.json").read_text())
        required = {
            check["context"]: check["integration_id"]
            for rule in rules if rule["type"] == "required_status_checks"
            for check in rule["parameters"]["required_status_checks"]
        }
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise Refused("cannot read checked-in required CI policy") from error
    if not required:
        raise Refused("required CI policy is empty")
    # These milestone checks are mandatory even if a future ruleset edit
    # accidentally omits them. 15368 is GitHub Actions' integration ID.
    required.update({"Feature Flags Check": 15368, "Split-plane compatibility": 15368})
    pages = github_json("--paginate", "--slurp", f"repos/{repository}/commits/{commit}/check-runs?per_page=100")
    checks = [check for page in pages for check in page["check_runs"]]
    for context, integration in sorted(required.items()):
        matching = [check for check in checks if check["name"] == context and check.get("app", {}).get("id") == integration]
        if not matching or not any(check.get("head_sha") == commit and check.get("status") == "completed" and check.get("conclusion") == "success" for check in matching):
            raise Refused(f"candidate lacks successful required check from its configured integration: {context}")
    for check in checks:
        if check.get("head_sha") != commit or check.get("status") != "completed" or check.get("conclusion") not in ("success", "neutral", "skipped"):
            raise Refused(f"candidate check is pending or unsuccessful: {check['name']}")
    pages = github_json("--paginate", "--slurp", f"repos/{repository}/commits/{commit}/statuses?per_page=100")
    # Status contexts may have older failures followed by success. The API is
    # newest-first, so only the latest observation of each context is relevant.
    latest = {}
    for page in pages:
        for status in page:
            latest.setdefault(status["context"], status)
    for context, status in latest.items():
        if status.get("state") != "success":
            raise Refused(f"candidate status is pending or unsuccessful: {context}")


REVIEW_QUERY = """query($owner:String!, $name:String!, $number:Int!) {
  repository(owner:$owner, name:$name) { pullRequest(number:$number) {
    merged headRefOid reviewDecision mergeCommit { oid }
    reviews(last:100) { pageInfo { hasPreviousPage } nodes {
      state author { login } commit { oid }
    } }
    reviewThreads(first:100) { pageInfo { hasNextPage } nodes { isResolved } }
  } }
}"""


def verify_reviews(repository, numbers, commit):
    owner, name = repository.split("/")
    for number in sorted(numbers):
        response = github_json(
            "graphql", "-f", f"query={REVIEW_QUERY}", "-f", f"owner={owner}",
            "-f", f"name={name}", "-F", f"number={number}",
        )
        if response.get("errors"):
            raise Refused(f"cannot establish review state for PR #{number}")
        pr = response["data"]["repository"]["pullRequest"]
        if not pr or not pr["merged"] or not pr.get("mergeCommit") or not ancestor(pr["mergeCommit"]["oid"], commit):
            raise Refused(f"PR #{number} must be merged into the candidate's ancestry")
        if pr.get("reviewDecision") in ("CHANGES_REQUESTED", "REVIEW_REQUIRED"):
            raise Refused(f"PR #{number} has outstanding required reviews")
        # Refuse rather than silently ignoring additional pages. Large PRs
        # should provide an audit-specific paginated verifier before tagging.
        if pr["reviews"]["pageInfo"]["hasPreviousPage"] or pr["reviewThreads"]["pageInfo"]["hasNextPage"]:
            raise Refused(f"PR #{number} exceeds the review audit page limit")
        if any(not thread["isResolved"] for thread in pr["reviewThreads"]["nodes"]):
            raise Refused(f"PR #{number} has unresolved review threads")
        reviews = pr["reviews"]["nodes"]
        latest = {}
        for review in reviews:
            if review.get("author"):
                latest[review["author"]["login"]] = review
        if any(review["state"] in ("PENDING", "CHANGES_REQUESTED") for review in latest.values()):
            raise Refused(f"PR #{number} has an unfinished or rejected review")
        if not any(review["state"] in ("APPROVED", "COMMENTED") and review.get("commit", {}).get("oid") == pr["headRefOid"] for review in reviews):
            raise Refused(f"PR #{number} lacks a completed review of its final head")


def check_target(commit):
    run("git", "fetch", "--quiet", "--no-tags", "origin", "+refs/heads/main:refs/remotes/origin/main")
    main = run("git", "rev-parse", "refs/remotes/origin/main^{commit}")
    if main != commit:
        raise Refused("HEAD must be the freshly fetched origin/main candidate")
    if TAG in run("git", "tag", "--list", TAG).splitlines():
        raise Refused(f"{TAG} already exists locally; replacement is forbidden")
    # An unsuccessful lookup is an error, not proof that the remote tag is absent.
    if run("git", "ls-remote", "--tags", "origin", f"refs/tags/{TAG}"):
        raise Refused(f"{TAG} already exists on origin; replacement is forbidden")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", type=Path, required=True)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--dry-run", action="store_true", help="validate only (the default)")
    mode.add_argument("--push", action="store_true", help="create and push the annotated train-3 tag after validation")
    args = parser.parse_args()
    if run("git", "rev-parse", "--show-toplevel") != str(Path.cwd().resolve()):
        raise Refused("run from the repository root")
    if run("git", "status", "--porcelain"):
        raise Refused("working tree must be clean, including untracked files")
    commit = run("git", "rev-parse", "HEAD^{commit}")
    check_target(commit)
    references, pull_requests = load_evidence(args.evidence, commit)
    repository = origin_repository()
    verify_ci(repository, commit)
    verify_reviews(repository, pull_requests, commit)
    run("node", "scripts/check-flags", "--shipped-train", "3")
    run("node", "scripts/flags-merge", "--check")
    # Repeat the remote checks after CI/review inspection, immediately before
    # publication. Git's ordinary non-force tag push rejects a concurrent tag.
    check_target(commit)
    if not args.push:
        print(f"Dry run passed: {TAG} would annotate {commit}; no tag created or pushed.")
        return
    annotation = f"Train 3 development milestone\n\nCommit: {commit}\nEvidence:\n" + "\n".join(dict.fromkeys(references))
    run("git", "tag", "-a", TAG, commit, "-m", annotation)
    try:
        run("git", "push", "origin", f"refs/tags/{TAG}:refs/tags/{TAG}")
    except Refused as error:
        raise Refused(f"push failed; local {TAG} retained for inspection; never replace or force-push it") from error
    print(f"Published annotated {TAG} at {commit}; no product release or workstream branches created.")


if __name__ == "__main__":
    try:
        main()
    except (Refused, KeyError, TypeError, AttributeError, OSError) as error:
        print(f"tag_milestone: {error}", file=sys.stderr)
        sys.exit(1)
