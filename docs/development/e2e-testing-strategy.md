# E2E Testing Strategy

E2E tests exercise browser workflows against running application services. The suites have different scopes; requiring live third-party data sources for every test would make ordinary release validation depend on those services.

## Suites and triggers

| Suite | Trigger | Environment |
| --- | --- | --- |
| [Release E2E](../../.github/workflows/release-e2e.yml) | Matching application/stack/workflow PR changes; manual dispatch | Release frontend build, real Rust backend and PostgreSQL 18, seeded recorded fixtures; no source API keys |
| [Core CI optional E2E](../../.github/workflows/ci-core.yml) | Manual dispatch with run_e2e_tests=true | Container-based desktop/mobile suites |
| [Nightly E2E](../../.github/workflows/e2e-tests-nightly.yml) | Daily 02:00 UTC; manual dispatch | Container build followed by selected desktop/mobile suites |
| [Deployed Playwright](../../.github/workflows/playwright-tests-deployed.yml) | Manual dispatch | Explicit deployed base URL |
| [Legacy Playwright](../../.github/workflows/playwright-tests.yml) and [comprehensive suite](../../.github/workflows/playwright-tests-comprehensive.yml) | Manual dispatch | Older setups; review workflow comments before use |

Release E2E is independent of Core CI's disabled-by-default E2E jobs. Recorded fixtures keep upstream data reproducible while still exercising the real frontend/backend/database boundary. Real source networking is tested separately by the [manual crawler integration workflow](../../.github/workflows/crawler-integration-test.yml).

## Run the maintained release suite

Follow the [release suite README](../../frontend/tests/e2e/release/README.md) for local prerequisites, fixture seeding and test scope. For GitHub runs:

```bash
gh workflow run release-e2e.yml --repo EconGraph/econ-graph --ref BRANCH
gh workflow run ci-core.yml --repo EconGraph/econ-graph --ref BRANCH -f run_e2e_tests=true -f e2e_test_suite=core
gh workflow run e2e-tests-nightly.yml --repo EconGraph/econ-graph --ref BRANCH -f test_suite=core
gh workflow run playwright-tests-deployed.yml --repo EconGraph/econ-graph -f base_url=https://YOUR_DEPLOYED_TARGET
```

The optional Core and nightly workflows expose different suite choices. Use each workflow's declared inputs; do not assume all legacy suite labels map to an active job.

## Interpret results

Distinguish image build failures, service readiness failures, browser startup failures and failed application assertions. If a container build fails, dependent suites may never execute. Scheduled runs can be delayed, and a schedule does not imply successful coverage.

Keep browser assertions meaningful. Do not skip required application behavior because a service is unavailable or replace failures with successful shell output. For a suite that explicitly covers an optional external integration, document its prerequisites and scope separately.

Inspect Playwright reports and service logs, using the workflow's own ports and URLs. Vite builds emit dist assets; legacy CRA build/static paths are not interchangeable.

See the [CI pipeline guide](CI_CD_PIPELINE.md) for job-selection limitations and [troubleshooting guide](../../ci/docs/CI_FAILURE_TROUBLESHOOTING.md) for diagnosis.
