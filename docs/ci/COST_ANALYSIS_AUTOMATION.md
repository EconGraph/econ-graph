# Cost Analysis Automation

The cost scripts retain a historical development-cost model for inspection. Its
calculated comparisons are estimates derived from fixed assumptions and current
file counts, rather than measured replacement costs, invoices, productivity gains,
or evidence of product quality.

## Inputs and Implementation

- `data/usage-events-2025-09-26.csv` is the historical usage export.
- `scripts/codebase-stats-corrected.sh` reports selected tracked-file statistics,
  with structured JSON available through `--json`.
- `scripts/update-cost-analysis.sh` delegates to `scripts/cost_analysis.py`.
- The Python implementation uses standard-library CSV parsing, decimal arithmetic,
  and JSON serialization. Python 3 and Git are required; Node, Rust, and `bc`
  are not required to run the updater.
- `scripts/tests/test_cost_analysis.py` covers calculations, malformed inputs,
  failure handling, output scope, and repeatability.

Missing or malformed inputs fail the update. Usage exports are parsed by column
name, token components must reconcile, and undefined zero-denominator calculations
are rejected. Reported usage costs can include events marked Included or Not Charged;
they are not verified bills and may overlap subscription charges.

## Generated Outputs

The updater writes only:

- `data/cost-analysis.json` — structured calculation data.
- `docs/archive/generated/development-costs/analysis.md` — a standalone archived report.

The archived report remains unlinked from project, product, and business
documentation. The updater does not modify the README, business collateral,
demo summaries, or other documentation. Report generation must continue working
when those documents are removed.

The model preserves the existing per-line rates, overhead, staffing, historical
usage, and subscription assumptions. Those assumptions belong with the generated
report; they do not establish savings, return on investment, or development novelty
for the project.

## Run and Validate

From the repository root:

```bash
python3 -m unittest discover -s scripts/tests -p 'test_cost_analysis.py' -v
./scripts/update-cost-analysis.sh
python3 -m json.tool data/cost-analysis.json > /dev/null
git diff --check
```

For calculation fixtures, the updater also accepts `--stats-json <path>` and
`--usage-csv <path>`. Review the generated file diff before committing.
A second run with unchanged inputs should produce no changes.

## GitHub Actions

`.github/workflows/update-cost-analysis.yml` runs on its daily schedule at
06:00 UTC, manual dispatch, and relevant pull-request changes.

The validation job runs regression tests, generation, JSON validation, and
repeatability checks with read-only repository permissions. For scheduled or
manual runs on main, a separate publishing job receives the validated outputs,
rejects an outdated source revision, and creates or updates the
`automation/update-cost-analysis` pull request. Only that publishing job has
`contents: write` and `pull-requests: write`.

Artifacts, repeatability checks, and the publishing allowlist cover only the two
generated outputs above. Automation must not restore deleted promotional material
or write calculated figures into project-facing documentation.

GitHub repository settings must permit Actions to create pull requests.
Pull requests created with `GITHUB_TOKEN` may require a manual downstream CI run.
Calculation validation runs before publishing; publishing failures remain visible
in workflow logs.

## Troubleshooting

Inspect the failing test or workflow step and its input validation message. Restore
the required source export when it is missing; do not substitute invented data.
For unexpected changed outputs, compare the tracked-file statistics, CSV totals,
and retained model assumptions. Follow the workflow's actual commands and output
allowlist when changing automation.

