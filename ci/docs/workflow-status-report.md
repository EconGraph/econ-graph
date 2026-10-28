# GitHub Actions Workflow Report: September 2025

Generated: 2025-09-17 08:25:07 PDT

This historical report records an attempted workflow cache refresh and a defect in
the report generator. It is not a current workflow inventory. See the
[workflow definitions](../../.github/workflows/) for current configuration and
[CI failure troubleshooting](CI_FAILURE_TROUBLESHOOTING.md) for investigation guidance.

## Original diagnosis and evidence

The original report attributed CI parsing failures to GitHub retaining workflows
from deleted test branches. It claimed that GitHub showed 22 active workflows
while only eight workflow files existed locally. Those counts and the cache
diagnosis were hard-coded in the
[generator](../../scripts/fix-github-workflow-cache.sh), rather than derived from
its checks. The report did not identify a failing run or parser error that
connected the workflow listing to a CI failure.

## Why the reported orphan list was unreliable

The generator requested workflow display names with
`gh workflow list --all --json name,id --jq '.[].name'`. Its unquoted shell loop
split those names on whitespace and compared each word with local filenames
after removing their `.yml` extensions. It did not compare workflow paths or
request workflow state. The `--all` option includes disabled workflows; it does
not select only active workflows.

The resulting list had 36 entries, including `.github/workflows/ci-core.yml`,
`Experimental`, `Features`, `RAM`, `Disk`, `and`, and repeated `Playwright` and
`Tests` entries. Even a path-shaped name such as `.github/workflows/ci-core.yml`
could not match the local basename `ci-core`. The report also listed `ci-core.yml`
as present locally. These entries demonstrate the comparison error; they do not
identify missing workflow records. The report did not preserve workflow records
with full names, IDs, paths, and states from which to recover an authoritative
historical inventory.

The report listed these eight local files under `.github/workflows/`:

- `ci-core.yml`
- `ci-experimental.yml`
- `crawler-integration-test.yml`
- `playwright-tests-comprehensive.yml`
- `playwright-tests-deployed.yml`
- `playwright-tests.yml`
- `ramdisk-build-cache.yml`
- `security.yml`

## Refresh attempt and validation limits

The refresh attempt added `# Workflow cache refresh: 2025-09-17 08:25:07 PDT`
comments to the eight files. The comments and original report are retained in
[the September 2025 revision](https://github.com/EconGraph/econ-graph/tree/8a4853621).
Their presence establishes the timestamp edits, but does not show that GitHub
refreshed a cache or that a parsing failure was resolved.

The generator wrote its claim of successful workflow validation before reaching
the validation step. Its checks only parse YAML with Python's `yaml.safe_load`
and search for lines beginning with `on:` and `jobs:`. They do not validate GitHub
Actions trigger semantics. The report retained no validation output or before-and-after
CI results, so it does not establish a successful validation run or incident
resolution. Its branch-cleanup and branch-naming recommendations were not
supported by a demonstrated cause.

For a new investigation, `gh workflow list --all` lists workflows, including
disabled ones. Request `--json id,name,path,state` to preserve complete records
before comparing paths with local files. A new listing describes GitHub's state
at the time of that query; it cannot reconstruct the September 2025 records or
validate workflow syntax.
