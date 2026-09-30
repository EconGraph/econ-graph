# CI Failure Troubleshooting

Start with the [pipeline guide](../../docs/development/CI_CD_PIPELINE.md) and [workflow inventory](../../.github/workflows/README.md). Confirm the workflow revision and event before reproducing a failure.

## Inspect the run

```bash
gh run list --repo EconGraph/econ-graph --branch main --limit 20
gh run view RUN_ID --repo EconGraph/econ-graph --json jobs
gh run view RUN_ID --repo EconGraph/econ-graph --log-failed
gh run view RUN_ID --repo EconGraph/econ-graph --job JOB_ID --log
```

If every relevant job skipped, there may be no failure log. Inspect job conditions and the full dependency chain. Check continue-on-error and shell constructs that replace a nonzero exit with a successful echo before interpreting a green badge.

## Job-selection problems

1. Check workflow branches/paths and dispatch inputs.
2. Check the changes job and its frontend_only/backend_only outputs.
3. Inspect skipped ancestors, not only the immediate dependency.
4. Preserve required dependency success checks when overriding default status conditions.
5. Confirm behavior on a real main run as well as a PR.

The [pipeline guide](../../docs/development/CI_CD_PIPELINE.md#known-coverage-limitations) records the current main dependency-skip defect and advisory checks.

## Build or setup failures

- Compare the job's Rust/Node versions and locked dependencies with local reproduction.
- For npm peer conflicts, reconcile declared dependency versions; avoid hiding them with --force.
- Check Docker build output paths against Vite's dist directory. A missing COPY source fails before browser tests execute.
- Separate container image build, application startup and test assertion failures.
- A YAML syntax check does not validate GitHub expressions or job-selection behavior. Use actionlint where available and examine the actual run.

## Database failures

Use the DATABASE_URL, PostgreSQL version, migrations and test serialisation from the failing job. Core CI service ports vary by job. Do not point cleanup commands at a development or production database.

For unique-constraint errors, inspect test isolation, deterministic identifiers and cleanup handling. Concurrent tests sharing mutable fixtures need isolation; random short identifiers alone do not eliminate collision risk. Never ignore a failed cleanup result.

## E2E failures

Check application/container logs and readiness before changing browser timeouts. Verify the configured base URL, backend URL, authentication issuer and database connection. Inspect the generated HTML for Vite asset paths instead of assuming CRA static/js paths.

Use the [release suite guide](../../frontend/tests/e2e/release/README.md) to reproduce the recorded-fixture release stack. Legacy nightly and manual suites use different setups; do not substitute one suite's environment for another.

## Scheduled automation failures

A scheduled cost-update push can fail because the token has insufficient permissions. Validate generated calculations and diffs before changing publishing permissions. The accessibility workflow can fail during dependency installation or configuration loading before any accessibility assertion runs.

Retry transient network failures only after establishing that they are transient. Deterministic dependency, path and configuration errors need fixes rather than repeated reruns.
