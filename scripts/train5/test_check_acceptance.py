#!/usr/bin/env python3
"""Synthetic checker tests; these are not SEC fixtures or release evidence."""

from datetime import datetime, timedelta, timezone
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from uuid import UUID

SPEC = importlib.util.spec_from_file_location("check_acceptance", Path(__file__).with_name("check_acceptance.py"))
checker = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(checker)
SHA = "a" * 40
NOW = datetime(2026, 10, 31, 12, tzinfo=timezone.utc)


class AcceptanceTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.ciks = [entry["cik"] for entry in checker.load_json(Path(__file__).with_name("corpus.json"))["companies"]]
        self.manifest = {
            "schema_version": 1, "candidate_sha": SHA, "candidate_tree_clean": True,
            "collected_at": NOW.isoformat(),
            "corpus_sha256": hashlib.sha256(Path(__file__).with_name("corpus.json").read_bytes()).hexdigest(),
            "company_audits": [],
        }
        for cik in self.ciks:
            source = self.save(f"sources/{cik}.json", {"synthetic": True})
            source["url"] = f"https://data.sec.gov/api/xbrl/companyfacts/CIK{cik}.json"
            audit = {"candidate_sha": SHA, "cik": cik, "reviewer": "Synthetic test reviewer",
                     "reviewed_at": NOW.isoformat(), "sources": [source],
                     "checks": [{"kind": kind, "expected": {"value": "123.456"}, "actual": {"value": "123.456"}}
                                for kind in sorted(checker.REQUIRED_CHECKS)]}
            self.manifest["company_audits"].append(self.save(f"audits/{cik}.json", audit))
        periods = [{"cik": cik, "period_end": "2025-12-31", "accession": "synthetic", "unit": "USD",
                    "assets": "0.3", "liabilities": "0.1", "equity": "0.2"} for cik in self.ciks]
        self.manifest["balance_checks"] = self.save("balance.json", {"candidate_sha": SHA, "periods": periods})
        runs = []
        for i in range(7):
            started = NOW - timedelta(days=6-i, hours=2)
            runs.append({"id": str(UUID(int=i+1)), "started_at": started.isoformat(),
                         "completed_at": (started + timedelta(hours=1)).isoformat(), "status": "succeeded", "error": None,
                         "companies_count": 20, "facts_count": 100, "filings_count": 20,
                         "companyfacts_sha256": "b" * 64, "submissions_sha256": "c" * 64,
                         "source_mode": "downloaded", "companyfacts_last_modified": started.isoformat(),
                         "submissions_last_modified": started.isoformat()})
        self.manifest["nightly_loads"] = self.save("loads.json", {"candidate_sha": SHA, "runs": runs})
        events = []
        for kind in ("failed_load", "stale_load"):
            log = self.save(f"{kind}.json", {"synthetic": True, "notification": kind})
            events.append({"kind": kind, "notification_id": "synthetic-id", "recovered": True,
                           "detected_at": NOW.isoformat(), "log": log})
        self.manifest["alert_drill"] = self.save("alerts.json", {"candidate_sha": SHA, "events": events})
        report = {"config": {"metadata": {"candidate_sha": SHA, "backend_mode": "real", "authenticated": True}},
                  "errors": [], "suites": [{"suites": [{"specs": [{"file": "tests/e2e/train5/company-research.spec.ts",
                  "title": "company research workflow", "ok": True,
                  "tests": [{"expectedStatus": "passed", "status": "expected", "results": [{"status": "passed"}]}]}]}]}]}
        self.manifest["workflow_report"] = self.save("workflow.json", report)

    def save(self, name, content):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(content))
        return {"path": name, "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}

    def mutate(self, key, change):
        reference = self.manifest[key]
        data = checker.load_json(self.root / reference["path"])
        change(data)
        self.manifest[key] = self.save(reference["path"], data)

    def check(self):
        self.save("evidence.json", self.manifest)
        return checker.validate(self.root, SHA, NOW)

    def rejected(self, message):
        with self.assertRaisesRegex(checker.Rejected, message):
            self.check()

    def test_complete_synthetic_evidence_passes_and_uses_exact_decimals(self):
        result = self.check()
        self.assertEqual(result["balance_passing"], 20)
        self.assertEqual(len(result["nightly_dates"]), 7)

    def test_long_fractional_values_do_not_round_at_decimal_context_precision(self):
        self.mutate("balance_checks", lambda data: data["periods"][0].update(
            assets="123456789012345678901234567890.123456789012345678901234567891",
            liabilities="123456789012345678901234567890.123456789012345678901234567890",
            equity="0.000000000000000000000000000001"))
        self.assertEqual(self.check()["balance_passing"], 20)

    def test_long_cancellation_is_exact(self):
        self.mutate("balance_checks", lambda data: data["periods"][0].update(
            assets="0.000000000000000000000000000001",
            liabilities="123456789012345678901234567890.123456789012345678901234567891",
            equity="-123456789012345678901234567890.123456789012345678901234567890"))
        self.assertEqual(self.check()["balance_passing"], 20)

    def test_unavailable_audit_result_must_match_explicit_expectation(self):
        ref = self.manifest["company_audits"][0]
        audit = checker.load_json(self.root / ref["path"])
        audit["checks"][0].update(expected={"status": "unavailable", "reason": "No source input"},
                                   actual={"status": "unavailable", "reason": "No source input"})
        self.manifest["company_audits"][0] = self.save(ref["path"], audit)
        self.assertTrue(self.check()["accepted"])

    def test_missing_evidence_rejects(self):
        self.manifest["company_audits"] = []
        self.rejected("20 fixed-company")

    def test_different_candidate_rejects(self):
        self.manifest["candidate_sha"] = "d" * 40
        self.rejected("different candidate")

    def test_dirty_candidate_rejects(self):
        self.manifest["candidate_tree_clean"] = False
        self.rejected("must be clean")

    def test_changed_corpus_rejects(self):
        self.manifest["corpus_sha256"] = "f" * 64
        self.rejected("corpus changed")

    def test_tampered_artifact_rejects(self):
        (self.root / "balance.json").write_text("{}")
        self.rejected("SHA-256 mismatch")

    def test_path_escape_and_symlink_reject(self):
        self.manifest["balance_checks"]["path"] = "../outside.json"
        self.rejected("escapes evidence directory")
        (self.root / "link").symlink_to(self.root / "balance.json")
        self.manifest["balance_checks"]["path"] = "link"
        self.rejected("cannot be a symlink")

    def test_missing_manual_review_rejects(self):
        ref = self.manifest["company_audits"][0]
        audit = checker.load_json(self.root / ref["path"])
        audit["reviewer"] = ""
        self.manifest["company_audits"][0] = self.save(ref["path"], audit)
        self.rejected("named manual reviewer")

    def test_manual_comparison_mismatch_rejects(self):
        ref = self.manifest["company_audits"][0]
        audit = checker.load_json(self.root / ref["path"])
        audit["checks"][0]["actual"] = {"value": "123.457"}
        self.manifest["company_audits"][0] = self.save(ref["path"], audit)
        self.rejected("differs from manual expectation")

    def test_balance_missing_values_count_as_failures(self):
        self.mutate("balance_checks", lambda data: data["periods"][0].update(assets=None))
        self.rejected("19/20 periods")

    def test_balance_non_finite_rejects(self):
        self.mutate("balance_checks", lambda data: data["periods"][0].update(assets="NaN"))
        self.rejected("must be finite")

    def test_failed_and_running_loads_reject(self):
        for status in ("failed", "running"):
            self.mutate("nightly_loads", lambda data: data["runs"][-1].update(status=status))
            self.rejected("latest seven load attempts")

    def test_staged_fixture_import_cannot_count_as_nightly_evidence(self):
        self.mutate("nightly_loads", lambda data: data["runs"][-1].update(source_mode="staged"))
        self.rejected("never staged fixtures")

    def test_missing_upstream_freshness_rejects(self):
        self.mutate("nightly_loads", lambda data: data["runs"][-1].update(companyfacts_last_modified=None))
        self.rejected("invalid timestamp")

    def test_stale_upstream_archive_rejects_even_when_run_is_recent(self):
        self.mutate("nightly_loads", lambda data: data["runs"][-1].update(submissions_last_modified=(NOW-timedelta(days=2)).isoformat()))
        self.rejected("archive must be fresh")

    def test_future_upstream_archive_rejects(self):
        self.mutate("nightly_loads", lambda data: data["runs"][-1].update(companyfacts_last_modified=(NOW+timedelta(hours=1)).isoformat()))
        self.rejected("archive must be fresh")

    def test_duplicate_nights_reject(self):
        self.mutate("nightly_loads", lambda data: data["runs"][1].update(started_at=data["runs"][0]["started_at"], completed_at=data["runs"][0]["completed_at"], companyfacts_last_modified=data["runs"][0]["companyfacts_last_modified"], submissions_last_modified=data["runs"][0]["submissions_last_modified"]))
        self.rejected("overlap|distinct consecutive")

    def test_gap_in_nightly_dates_rejects(self):
        self.mutate("nightly_loads", lambda data: data["runs"][0].update(started_at=(NOW-timedelta(days=8, hours=2)).isoformat(), completed_at=(NOW-timedelta(days=8, hours=1)).isoformat(), companyfacts_last_modified=(NOW-timedelta(days=8, hours=2)).isoformat(), submissions_last_modified=(NOW-timedelta(days=8, hours=2)).isoformat()))
        self.rejected("distinct consecutive")

    def test_stale_load_rejects(self):
        self.mutate("nightly_loads", lambda data: [run.update(started_at=(checker.timestamp(run["started_at"])-timedelta(days=2)).isoformat(), completed_at=(checker.timestamp(run["completed_at"])-timedelta(days=2)).isoformat(), companyfacts_last_modified=(checker.timestamp(run["companyfacts_last_modified"])-timedelta(days=2)).isoformat(), submissions_last_modified=(checker.timestamp(run["submissions_last_modified"])-timedelta(days=2)).isoformat()) for run in data["runs"]])
        self.rejected("older than 24 hours")

    def test_missing_alert_delivery_rejects(self):
        self.mutate("alert_drill", lambda data: data["events"][0].update(notification_id=""))
        self.rejected("delivered notification")

    def test_mock_or_anonymous_workflow_rejects(self):
        self.mutate("workflow_report", lambda data: data["config"]["metadata"].update(backend_mode="mock"))
        self.rejected("real backend")

    def test_skips_and_retries_reject(self):
        self.mutate("workflow_report", lambda data: data["suites"][0]["suites"][0]["specs"][0]["tests"][0].update(results=[{"status": "failed"}, {"status": "passed"}]))
        self.rejected("without skips")

    def test_duplicate_json_keys_reject(self):
        path = self.root / "duplicate.json"
        path.write_text('{"value":1,"value":2}')
        with self.assertRaisesRegex(checker.Rejected, "duplicate JSON key"):
            checker.load_json(path)


if __name__ == "__main__":
    unittest.main()
