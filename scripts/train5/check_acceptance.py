#!/usr/bin/env python3
"""Validate retained Train 5 release evidence. No evidence means no acceptance."""

import argparse
from datetime import datetime, timedelta, timezone
from decimal import Decimal, InvalidOperation
from fractions import Fraction
import hashlib
import json
from pathlib import Path
import re
import sys
from uuid import UUID

SHA = re.compile(r"[0-9a-f]{64}\Z")
COMMIT = re.compile(r"[0-9a-f]{40}\Z")
REQUIRED_CHECKS = {"balance_sheet", "income_statement", "cash_flow", "q4", "ttm", "per_share", "restatement", "ratios", "peers"}


class Rejected(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise Rejected(message)


def timestamp(value):
    try:
        result = datetime.fromisoformat(value.replace("Z", "+00:00"))
        require(result.tzinfo is not None, "timestamps must include a timezone")
        return result.astimezone(timezone.utc)
    except (AttributeError, TypeError, ValueError) as exc:
        raise Rejected(f"invalid timestamp: {value!r}") from exc


def decimal(value):
    require(isinstance(value, str), "financial evidence values must be exact decimal strings")
    try:
        result = Decimal(value)
        require(result.is_finite(), "financial evidence values must be finite")
        return result
    except InvalidOperation as exc:
        raise Rejected(f"invalid decimal: {value!r}") from exc


def load_json(path):
    def unique_pairs(pairs):
        result = {}
        for key, value in pairs:
            require(key not in result, f"duplicate JSON key: {key}")
            result[key] = value
        return result
    return json.loads(path.read_text(), object_pairs_hook=unique_pairs,
                      parse_constant=lambda value: (_ for _ in ()).throw(Rejected(f"invalid JSON number {value}")))


class Evidence:
    def __init__(self, directory):
        self.directory = directory.resolve()

    def artifact(self, reference, json_document=True):
        require(isinstance(reference, dict), "artifact reference must be an object")
        name = reference.get("path", "")
        require(isinstance(name, str) and name and not Path(name).is_absolute(), "artifact path must be relative")
        path = self.directory / name
        require(not path.is_symlink(), f"artifact cannot be a symlink: {name}")
        require(path.resolve().is_relative_to(self.directory), f"artifact escapes evidence directory: {name}")
        require(path.is_file(), f"missing artifact: {name}")
        require(path.stat().st_size <= 64 * 1024 * 1024, f"artifact exceeds 64 MiB: {name}")
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        require(SHA.fullmatch(str(reference.get("sha256", ""))) is not None and digest == reference["sha256"],
                f"artifact SHA-256 mismatch: {name}")
        return load_json(path) if json_document else path.read_text()


def validate(directory, candidate, now):
    require(COMMIT.fullmatch(candidate) is not None, "--candidate must be a full lowercase Git commit SHA")
    evidence = Evidence(directory)
    manifest = load_json(directory / "evidence.json")
    require(manifest.get("schema_version") == 1, "unsupported evidence schema_version")
    require(manifest.get("candidate_sha") == candidate, "evidence belongs to a different candidate")
    require(manifest.get("candidate_tree_clean") is True, "candidate tree must be clean when evidence is collected")
    captured = timestamp(manifest["collected_at"])
    require(now - timedelta(hours=24) <= captured <= now + timedelta(minutes=5), "evidence collection is stale or in the future")
    corpus_path = Path(__file__).with_name("corpus.json")
    require(manifest.get("corpus_sha256") == hashlib.sha256(corpus_path.read_bytes()).hexdigest(), "fixed-company corpus changed or is missing")
    corpus = {entry["cik"] for entry in load_json(corpus_path)["companies"]}
    require(len(corpus) == 20, "fixed-company corpus must contain exactly 20 distinct CIKs")

    audits = manifest.get("company_audits", [])
    require(len(audits) == 20, "20 fixed-company manual audits are required")
    audited = set()
    for reference in audits:
        audit = evidence.artifact(reference)
        cik = audit.get("cik")
        require(cik in corpus and cik not in audited, f"unexpected or duplicate audited CIK: {cik}")
        audited.add(cik)
        require(audit.get("candidate_sha") == candidate, f"{cik}: API observations belong to a different candidate")
        require(isinstance(audit.get("reviewer"), str) and audit["reviewer"].strip(), f"{cik}: named manual reviewer required")
        require(timestamp(audit["reviewed_at"]) <= now + timedelta(minutes=5), f"{cik}: audit is in the future")
        sources = audit.get("sources", [])
        require(sources, f"{cik}: retained SEC source payload required")
        for source in sources:
            require(re.fullmatch(r"https://(?:data|www)\.sec\.gov/[^\s]+", source.get("url", "")) is not None,
                    f"{cik}: source URL must identify an SEC payload or filing")
            evidence.artifact(source, json_document=False)
        checks = audit.get("checks", [])
        covered = set()
        for check in checks:
            kind = check.get("kind")
            require(kind in REQUIRED_CHECKS and kind not in covered, f"{cik}: unknown or duplicate audit check {kind}")
            covered.add(kind)
            expected, actual = check.get("expected"), check.get("actual")
            require(isinstance(expected, (dict, list)) and expected, f"{cik}/{kind}: nonempty manually expected data required")
            require(actual == expected, f"{cik}/{kind}: actual output differs from manual expectation")
        require(covered == REQUIRED_CHECKS, f"{cik}: missing manual checks: {sorted(REQUIRED_CHECKS - covered)}")
    require(audited == corpus, "manual audits do not match the fixed-company corpus")

    balances = evidence.artifact(manifest["balance_checks"])
    require(balances.get("candidate_sha") == candidate, "balance checks belong to a different candidate")
    rows = balances.get("periods", [])
    require(rows, "balance checks contain no periods")
    seen, represented, passing = set(), set(), 0
    for row in rows:
        cik = row["cik"]
        require(cik in corpus, f"unexpected balance-check CIK: {cik}")
        key = (cik, row["period_end"], row["unit"], row["accession"])
        require(key not in seen, f"duplicate balance-check period: {key}")
        seen.add(key)
        represented.add(cik)
        # Missing values count as failed periods; the collector must include every supported period.
        if all(row.get(field) is not None for field in ("assets", "liabilities", "equity")):
            passing += Fraction(decimal(row["assets"])) == Fraction(decimal(row["liabilities"])) + Fraction(decimal(row["equity"]))
    require(represented == corpus, "balance checks must include every fixed company")
    require(passing * 100 >= len(rows) * 99, f"balance equation passes {passing}/{len(rows)} periods; at least 99% required")

    loads = evidence.artifact(manifest["nightly_loads"])
    require(loads.get("candidate_sha") == candidate, "nightly load export belongs to a different candidate")
    runs = loads.get("runs", [])
    require(len(runs) >= 7, "seven consecutive nightly loads are required")
    require(len({run["id"] for run in runs}) == len(runs), "duplicate nightly run IDs")
    runs = sorted(runs, key=lambda run: timestamp(run["started_at"]))
    latest = runs[-7:]
    dates = []
    previous_completion = None
    for run in latest:
        try:
            UUID(run["id"])
        except (ValueError, TypeError) as exc:
            raise Rejected("nightly run ID must be a UUID") from exc
        started, completed = timestamp(run["started_at"]), timestamp(run.get("completed_at"))
        require(run.get("status") == "succeeded" and run.get("error") is None, "latest seven load attempts must all succeed")
        require(started <= completed <= now + timedelta(minutes=5), "invalid nightly run completion time")
        require(run.get("source_mode") == "downloaded", "nightly evidence must use downloaded archives, never staged fixtures")
        for field in ("companyfacts_last_modified", "submissions_last_modified"):
            modified = timestamp(run.get(field))
            require(completed - timedelta(hours=24) <= modified <= completed + timedelta(minutes=5),
                    f"{field}: downloaded archive must be fresh at load completion")
        require(previous_completion is None or previous_completion <= started, "nightly load attempts overlap")
        previous_completion = completed
        require(all(isinstance(run.get(field), int) and not isinstance(run[field], bool) and run[field] > 0
                    for field in ("companies_count", "facts_count", "filings_count")), "successful load must contain companies, facts and filings")
        require(all(SHA.fullmatch(str(run.get(field, ""))) is not None
                    for field in ("companyfacts_sha256", "submissions_sha256")), "nightly load archive hashes are required")
        dates.append(started.date())
    require(all(b - a == timedelta(days=1) for a, b in zip(dates, dates[1:])), "nightly successes must cover seven distinct consecutive UTC dates")
    require(now - timestamp(latest[-1]["completed_at"]) <= timedelta(hours=24), "latest successful nightly load is older than 24 hours")

    alerts = evidence.artifact(manifest["alert_drill"])
    require(alerts.get("candidate_sha") == candidate, "alert drill belongs to a different candidate")
    kinds = set()
    for event in alerts.get("events", []):
        kind = event.get("kind")
        require(kind in {"failed_load", "stale_load"} and kind not in kinds, "alert drill has duplicate or unknown events")
        kinds.add(kind)
        require(event.get("notification_id") and event.get("recovered") is True, f"{kind}: delivered notification and recovery required")
        require(now - timedelta(days=7) <= timestamp(event["detected_at"]) <= now + timedelta(minutes=5), f"{kind}: alert drill must be recent")
        require(evidence.artifact(event["log"], json_document=False).strip(), f"{kind}: alert log is empty")
    require(kinds == {"failed_load", "stale_load"}, "failed-load and stale-load alert drills are required")

    workflow = evidence.artifact(manifest["workflow_report"])
    metadata = workflow.get("config", {}).get("metadata", {})
    require(metadata.get("candidate_sha") == candidate and metadata.get("backend_mode") == "real" and metadata.get("authenticated") is True,
            "workflow report must identify this candidate, a real backend and authenticated execution")
    require(not workflow.get("errors"), "workflow report contains global errors")
    matching = []
    def visit(suite):
        for spec in suite.get("specs", []):
            if spec.get("file", "").replace("\\", "/").endswith("train5/company-research.spec.ts"):
                matching.append(spec)
        for child in suite.get("suites", []):
            visit(child)
    for suite in workflow.get("suites", []):
        visit(suite)
    require(matching, "workflow report does not include the Train 5 company-research spec")
    require(any(spec.get("title") == "company research workflow" for spec in matching), "complete company research workflow was not executed")
    for spec in matching:
        tests = spec.get("tests", [])
        require(tests and spec.get("ok") is True, "company workflow spec did not pass")
        for test in tests:
            results = test.get("results", [])
            require(test.get("expectedStatus") == "passed" and test.get("status") == "expected" and len(results) == 1 and results[0].get("status") == "passed",
                    "workflow must pass without skips, expected failures, retries or flaky outcomes")
    return {"accepted": True, "candidate_sha": candidate, "companies": len(audited),
            "balance_periods": len(rows), "balance_passing": passing, "nightly_dates": [str(day) for day in dates]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("evidence_dir", type=Path)
    parser.add_argument("--candidate", required=True, help="full commit SHA checked by CI and deployed for the research workflow")
    args = parser.parse_args()
    try:
        result = validate(args.evidence_dir, args.candidate, datetime.now(timezone.utc))
    except (Rejected, OSError, KeyError, TypeError, json.JSONDecodeError) as exc:
        print(f"Train 5 acceptance rejected: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
