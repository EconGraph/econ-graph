# Train 5 company-research release evidence

Train 5 is accepted only after the complete company-research journey and its data
quality and operational gates pass. Run the checker against the exact candidate
whose required CI and reviews passed:

```sh
python3 scripts/train5/test_check_acceptance.py
python3 scripts/train5/check_acceptance.py /absolute/path/to/retained-evidence \
  --candidate "$(git rev-parse HEAD)"
```

The first command tests the checker using temporary, explicitly synthetic inputs.
It does not verify any company or establish release readiness. The second command
fails when evidence is missing, stale, mismatched, incomplete or unsuccessful. No
release evidence or completed manual audit is checked into this directory.

`corpus.json` fixes 20 company CIKs across technology, banking, insurance, payments,
healthcare, energy, retail, consumer products and media. The list is an audit plan,
not a claim that any company's values have been verified. Changing it invalidates
previous evidence through its SHA-256. SEC companyfacts covers standard-taxonomy,
whole-entity facts; original filing presentation, extension concepts and segments
remain Train 6 work.

## Evidence collection

Collect from a clean candidate checkout and an isolated release-candidate
environment using the actual companyfacts ingestion, data API and signed-in UI.
Enable the existing financial feature gate in that environment for both backend
and frontend builds. Leave the product release gate disabled until acceptance.
Do not substitute mocked responses, rerun one nightly load seven times, edit
recorded timestamps or label invented values as SEC fixtures.

Use an evidence directory separate from the checkout. Retain raw SEC payloads,
manual comparisons, actual API outputs, ingestion run exports, delivered alert
logs and the Playwright JSON report. Copy artifacts into that directory and compute
each file's SHA-256 after its final write. Artifact references always have this form:

```json
{"path": "relative/path.json", "sha256": "64 lowercase hexadecimal characters"}
```

Absolute paths, directory traversal, symlink files, files over 64 MiB, changed
contents and duplicate JSON object keys are rejected. Split larger evidence into
bounded extracts and retain their source archive hashes. Hashes establish that
retained evidence has not changed since the manifest was prepared; they are not
signatures or proof of how an artifact was produced. The release reviewer must
verify collection provenance and complete coverage.

`evidence.json` contains:

```json
{
  "schema_version": 1,
  "candidate_sha": "full 40-character lowercase candidate commit",
  "candidate_tree_clean": true,
  "collected_at": "UTC ISO-8601 timestamp",
  "corpus_sha256": "SHA-256 of scripts/train5/corpus.json",
  "company_audits": ["20 artifact reference objects, one per corpus CIK"],
  "balance_checks": {"path": "balances.json", "sha256": "..."},
  "nightly_loads": {"path": "loads.json", "sha256": "..."},
  "alert_drill": {"path": "alerts.json", "sha256": "..."},
  "workflow_report": {"path": "playwright.json", "sha256": "..."}
}
```

This is a shape example, not a valid evidence file. All artifacts containing
candidate outputs must identify the same candidate. Evidence collection must be
within the last 24 hours. Timestamps require timezones and are compared in UTC;
a five-minute clock-skew allowance applies to future times.

### Manual company audits

Each audit has `candidate_sha`, `cik`, a named `reviewer`, `reviewed_at`, `sources`
and `checks`. Each source is an artifact reference with an additional `url` naming
its original `https://data.sec.gov/` or `https://www.sec.gov/` payload or filing.
Retain actual source contents rather than a successful-download flag. Record
source accessions, periods, units and taxonomy concepts in the comparisons.

The checks contain exactly one of each kind: `balance_sheet`, `income_statement`,
`cash_flow`, `q4`, `ttm`, `per_share`, `restatement`, `ratios`, and `peers`. Each check
contains `kind`, a nonempty object or array in `expected`, and the candidate's
corresponding output in `actual`. The checker requires exact JSON equality.
Use decimal strings for financial values. The reviewer derives `expected` from
retained source evidence and documented calculation rules independently of the
candidate implementation. Copying actual output into expected is not an audit.

Include complete supported statement-line and period lists in statement checks,
including explicit unavailable results with reasons where source inputs are
missing. For derivations and ratios, retain their contributing fact identities
and values. Verify fiscal periods using dates, not SEC `fy`/`fp` or calendar-frame
labels alone. Verify original, later comparative and amended accessions without
moving a filed note to a later revision. Select representative historical periods
with actual restatements; do not invent a restatement for a company without one.
Reviewers must assess whether the cases meet the feature's acceptance requirements;
the checker cannot infer the sufficiency of manually selected examples.

### Balance-sheet reconciliation

The balance artifact contains `candidate_sha` and `periods`. Every supported
balance-sheet period for the fixed corpus must be represented, with each entry's
`cik`, `period_end`, `unit`, `accession`, `assets`, `liabilities`, and `equity`.
Values are exact decimal strings; unavailable inputs are `null` and count as
failed periods rather than disappearing from the denominator. Each company must
appear, and duplicate company/period/unit/accession entries are rejected.

The checker recomputes assets = liabilities + equity using exact rational arithmetic converted from decimal strings,
requires at least 99% passing periods, and applies no rounding tolerance. If SEC
rounding requires a tolerance, decide and review that policy explicitly before
changing this gate. An export that omits troublesome periods is invalid evidence,
even if its remaining rows pass; completeness is part of the release review.

### Nightly ingestion and freshness

The load artifact contains `candidate_sha` and a `runs` array exported from
`company_fact_load_runs`. Retain all attempts in the observation interval rather
than just selecting successful rows. Each row contains:

- `id`, `started_at`, `completed_at`, `status`, and nullable `error`;
- positive `companies_count`, `facts_count`, and `filings_count`;
- `companyfacts_sha256` and `submissions_sha256` from the actual downloaded archives.

The latest seven attempts must succeed, have distinct UUIDs, not overlap and
start on seven distinct consecutive UTC dates. The latest completion must be no
older than 24 hours. A failed or unfinished most-recent attempt blocks acceptance.
The candidate identifier binds the export and collector to this candidate; run
rows do not themselves attest which executable ran. Retain deployment/image and
scheduler logs so the reviewer can verify that the observation interval exercises
the intended ingestion code. Do not claim seven nights of a candidate based on
seven rows created with a different implementation.

### Failure and staleness alert drills

The alert artifact contains `candidate_sha` and `events`, one each for `failed_load`
and `stale_load`. Each event includes `detected_at`, an actual delivered
`notification_id`, `recovered: true` and `log`, an artifact reference to the
nonempty retained monitor/notification log. Both drills must have occurred within
the previous seven days. A process exit code or configured alert rule alone does
not demonstrate delivery. Use a disposable fixture or test alert route to exercise
failure, staleness and recovery without disrupting production ingestion.

### Signed-in research workflow

Run `frontend/tests/e2e/train5/company-research.spec.ts` against the real candidate
backend with the financial feature enabled, an isolated populated companyfacts
database and authenticated browser storage state. The workflow must actually
assert lookup/header, all three statements, period selection, a selected-line
chart, attributed CSV values, ratios/compatible peers and note permissions.

Retain the Playwright JSON reporter output. Its `config.metadata` must include
`candidate_sha`, `backend_mode: "real"` and `authenticated: true`. The checker
requires the `company research workflow` test from that spec and rejects global
errors, failed specs, skipped/expected-failure tests, retries and flaky outcomes.
Metadata describes collection intent; the test assertions and environment review
establish that real requests and sign-in occurred.

## Remaining release gates

This checker validates evidence consistency and the explicit numerical/operational
conditions above. It does not approve a merge, deploy, create the accepted Train 3
tag, prove Train 4's production acceptance or replace required CI and review.
Those gates remain independent. A successful checker run without authentic,
complete evidence does not satisfy ECO-782.
