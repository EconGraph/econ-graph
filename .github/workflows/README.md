# CI/CD Workflows

The workflow YAML files in this directory define what runs. See the [CI pipeline guide](../../docs/development/CI_CD_PIPELINE.md) for job dependencies, selection rules, commands and limitations.

## Workflow inventory

| File | Automatic triggers | Purpose |
| --- | --- | --- |
| [ci-core.yml](ci-core.yml) | Push to main, develop, optimize/ci-core-primary and release/**; PRs into any branch, subject to paths-ignore | Backend build and tests, frontend tests, quality, audit, licenses and Docker validation |
| [docs-checks.yml](docs-checks.yml) | Push to main/develop; all PRs, including docs-only and stacked PRs | Markdown lint and offline relative-link checks |
| [feature-flags.yml](feature-flags.yml) | Push to main/develop/release/**; train-* tags; all PRs; Monday 06:17 UTC | Flag validation and generated-file consistency |
| [release-e2e.yml](release-e2e.yml) | PRs touching its listed application, Keycloak, release-stack or workflow paths | Chromium against a release frontend, real backend and PostgreSQL 18 with recorded fixtures |
| [e2e-tests-nightly.yml](e2e-tests-nightly.yml) | Daily 02:00 UTC | Container-based desktop and mobile E2E suites |
| [security.yml](security.yml) | Daily 02:00 UTC | Dependency audits, Trivy, CodeQL, licenses and Dockerfile scanning |
| [accessibility-tests.yml](accessibility-tests.yml) | Monday 09:00 UTC | Accessibility static/runtime checks and manual-testing guidance |
| [update-cost-analysis.yml](update-cost-analysis.yml) | Daily 06:00 UTC | Recalculate cost documentation and attempt to publish changes |
| [crawler-integration-test.yml](crawler-integration-test.yml) | None | Enqueue and execute a real source crawl; needs network access and source credentials where required |
| [playwright-tests.yml](playwright-tests.yml) | None | Legacy Playwright and Grafana checks |
| [playwright-tests-comprehensive.yml](playwright-tests-comprehensive.yml) | None | Legacy comprehensive Playwright suite |
| [playwright-tests-deployed.yml](playwright-tests-deployed.yml) | None | Playwright against an explicitly deployed target |
| [ci-experimental.yml](ci-experimental.yml) | None | Optional RAM-disk experiment |
| [ramdisk-build-cache.yml](ramdisk-build-cache.yml) | None | Manual build-cache experiment |

All listed workflows support manual dispatch. Scheduled times are UTC; GitHub may delay scheduled runs. The Playwright workflows do not run automatically on version tags. Admin Frontend Tests is a job in Core CI, not a separate workflow file.

## Operational notes

A green workflow does not imply every suite ran or every step passed. Inspect job conclusions and any continue-on-error steps. The [pipeline guide](../../docs/development/CI_CD_PIPELINE.md#known-coverage-limitations) records current coverage limitations.

Core CI builds and smoke-tests Docker images; it does not publish or deploy them. Release E2E is independent of the optional Core CI E2E jobs.

## Validation

Run Markdown lint and relative-link checks as defined in [docs-checks.yml](docs-checks.yml). The repository's [workflow validation script](../../ci/scripts/validate-ci-workflows.sh) checks YAML and basic job structure; it does not prove job-selection semantics. Its trigger check uses PyYAML's default loader, which can interpret the key on as boolean true, so trigger warnings require verification against the YAML.

When changing workflow conditions, check actual jobs on both a PR and a push to main. A passing PR alone does not validate main-specific conditions.
