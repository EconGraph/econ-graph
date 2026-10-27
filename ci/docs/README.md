# CI/CD Documentation

Use these guides to choose a CI workflow, reproduce a test failure, or inspect the CI infrastructure.

## Guides and infrastructure

- [Workflow inventory](../../.github/workflows/README.md): workflow triggers and purposes.
- [CI pipeline guide](../../docs/development/CI_CD_PIPELINE.md): job selection, dependencies, commands and coverage limitations.
- [CI failure troubleshooting](CI_FAILURE_TROUBLESHOOTING.md): diagnosing failed or skipped checks.
- [CI infrastructure](../README.md): configurations, containers and utility scripts.
- [Frontend Playwright configurations](../../frontend/ci/configs/): configurations used by the specialized npm scripts in [frontend/package.json](../../frontend/package.json).
- [CI configurations](../configs/), [Docker configurations](../docker/) and [utility scripts](../scripts/): supporting CI files.

## Browser test coverage

[Core CI](../../.github/workflows/ci-core.yml) runs its browser jobs only when dispatched with `run_e2e_tests=true`; `e2e_test_suite` selects among `e2e-core-tests`, `e2e-comprehensive-tests` and `mobile-e2e-tests`. The mobile job invokes `test:e2e:mobile:core`, including when the input is `mobile-comprehensive`. Core CI has no separate global-analysis job. See the [pipeline guide](../../docs/development/CI_CD_PIPELINE.md) for the other backend, frontend, quality, container and dependency checks.

[Release E2E](../../.github/workflows/release-e2e.yml) runs independently on PRs matching its application, Keycloak, release-stack or workflow paths, and supports manual dispatch. It exercises a release frontend, real backend and disposable PostgreSQL database seeded through crawler adapters from recorded responses. It needs no live data-source APIs or API keys. Its Playwright projects include desktop Chromium and an emulated Pixel 5; the mobile project covers a subset of the release specs.

Browser specs can also intercept requests deliberately. The world-map UI specs `world-map-mocked.spec.ts` and `world-atlas.spec.ts`, included by the [global-analysis configuration](../../frontend/ci/configs/playwright-global-analysis.config.ts), mock GraphQL responses to test map interactions and error handling. These checks do not establish backend or upstream API behavior. The release suite separately checks map values against its seeded backend. The [release suite guide](../../frontend/tests/e2e/release/README.md) describes both kinds of specs and the mobile scope.

Checks against a deployed build require an explicitly selected target. Follow the release guide's [deployed-build instructions](../../frontend/tests/e2e/release/README.md#against-a-deployed-build-rel-qa-check-3) for `RELEASE_BASE_URL`, QA authentication and the assertions that use live data. A passing mocked or fixture-seeded run does not verify that deployed environment or live upstream services.

## Run browser tests locally

For the specialized npm scripts, use the Node version in [.nvmrc](../../.nvmrc), install frontend dependencies and the browsers selected by the configuration, and start the application target separately. These configurations do not start a server. They use `BASE_URL`, defaulting to `http://localhost:3000`; the target must provide the backend and authentication services needed by the selected specs. Use disposable test databases for local stacks.

From the repository root:

```bash
cd frontend
npm ci
npx playwright install --with-deps chromium

# With an application already running at the configured target:
npm run test:e2e:core
npm run test:e2e:comprehensive
```

For the fixture-seeded release stack, follow the [release suite guide](../../frontend/tests/e2e/release/README.md#run-it) for Docker, build and browser requirements, then run from the repository root:

```bash
scripts/release-e2e.sh
```

That script builds and starts the release stack and seeds its test database. Keep existing services off its backend and frontend ports so Playwright does not reuse a stale build.

## Historical investigations

- [CI failure analysis and fixes](../../docs/archive/ci/docs/CI_FAILURE_ANALYSIS_AND_FIXES.md): archived failure investigation and resolutions.
- [E2E test failure analysis](../../docs/archive/ci/docs/E2E_TEST_FAILURE_ANALYSIS.md): archived browser-test investigation.
- [September 2025 workflow incident report](workflow-status-report.md): the workflow investigation generated on September 17, 2025.
