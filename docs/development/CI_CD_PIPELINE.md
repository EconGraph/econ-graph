# CI Pipeline

[Core CI](../../.github/workflows/ci-core.yml) is the primary build/test workflow. The [workflow inventory](../../.github/workflows/README.md) lists the separate documentation, flags, release E2E, scheduled and manual workflows. YAML is the source of truth; this guide describes its conditions and limitations, not a guarantee of full coverage.

## Triggers and selection

Core CI runs on pushes to main, develop, optimize/ci-core-primary and release/**, PRs into any branch, merge groups ([merge queue](MERGE_QUEUE.md)) and manual dispatch. The push trigger's paths-ignore excludes top-level Markdown, docs/**, LICENSE, .gitignore and the workflow README. PRs have no trigger path filter, because Core CI Result is a required check and must report on every PR: on a PR that changes only those paths, Detect changes sets `code` to false, every other job is skipped, and Core CI Result passes. Docs Checks runs on every PR.

PR runs share a concurrency group by PR number: a new push cancels the previous run. Push, merge-group and manual Core CI runs have independent groups.

| Event/change | Selection |
| --- | --- |
| Docs-only PR | Only Detect changes and Core CI Result |
| Frontend-only PR | Skip backend build and dependent tests; frontend/admin checks run |
| Backend-only PR | Skip frontend/admin test jobs; backend checks run |
| Shared paths or neither side alone | Run both sides |
| Dependency changes on a PR | Also run security and license jobs |
| Docker/build-input changes on a PR | Also run Docker Build Test |
| Migration changes on a PR | Also run migration-order validation |
| Push to main/develop/release/** | Intended to run backend/frontend checks and audit/license/migration/Docker jobs; admin checks and optional E2E are excluded |
| Merge group | No path filtering; the push selection plus admin tests; Core CI Result requires every job of that suite to succeed |
| Manual dispatch | Run ordinary checks plus admin tests; optional E2E requires run_e2e_tests=true |

Exact paths are in the changes job. Shared paths include CI configuration, scripts, infrastructure, Keycloak/flags configuration, the committed GraphQL schema, and the backend/frontend Docker inputs. An arbitrary config change is not automatically docs-only.

## Core jobs and dependencies

| Job(s) | Dependency | Work |
| --- | --- | --- |
| Detect changes | None | Complete successfully on every Core CI run; on PRs, decide `code` (false for a docs-only change); filter feature refs only, not merge groups |
| Backend Build Cache | Detect changes | Compile backend and test targets with cargo build --all-targets; prepare the shared Rust cache |
| Backend Smoke Tests (Fast) | Backend Build Cache | Formatting, Clippy, rustdoc, fast tests, release-flag checks, schema snapshot and Keycloak checks |
| Backend Workspace Tests (all tests) | Backend Build Cache | Required crawler/queue/ratio checks and release CLI checks; broader workspace and doctest runs are advisory |
| Backend Database Tests | Backend Build Cache | Consolidated database-backed library tests, with a fresh database for each group |
| Backend Integration Tests | Backend Smoke Tests | Backend integration checks |
| Chart API Integration Tests | Backend Smoke Tests | Build and test chart service |
| Backend MCP Integration Tests | Backend Smoke Tests and Chart API Integration Tests | Backend/MCP integration |
| Frontend Tests | Detect changes | Type checking, Vitest coverage and build-flag bundle checks |
| Frontend Integration Tests | Frontend Tests | Frontend integration suite |
| Admin Frontend Tests | Detect changes | Admin lint, type checking and Vitest coverage; selected PRs/manual runs only |
| Quality Checks | Detect changes | Rust and frontend formatting/linting checks, and the tests of the required-jobs gate |
| Security Audit / License Compliance Check / Migration Validation | Detect changes | Selected PR checks; enabled for non-PR runs |
| Docker Build Test | Detect changes and Backend Smoke Tests | Load built backend/frontend images and check their runtime health |
| Optional E2E container build and suites | Frontend integration, plus backend dependencies for suites | Manual opt-in desktop/mobile Playwright |
| Core CI Result | Every job above, whatever its result | The required check ([merge queue](MERGE_QUEUE.md)): fails when a job failed or was cancelled, and in a merge group when a job of the suite did not succeed ([check-required-jobs.sh](../../ci/scripts/check-required-jobs.sh)) |

The cache build compiles test binaries; it does not execute tests. Rust test jobs restore the shared Swatinem/rust-cache cache; only the build job saves it. Diesel CLI is downloaded from a pinned release and checksum-verified in the relevant jobs. Core CI uses Rust 1.98.1, Node 24 and PostgreSQL 18; database credentials and ports are job-specific.

## Known coverage limitations

- The broad workspace and doctest steps use continue-on-error. Required crawler/queue checks in the same job still fail the job. Inspect the broader step logs even when the job is green.
- Admin tests are excluded from ordinary pushes by an explicit event condition.
- Core CI E2E jobs are disabled unless explicitly selected on manual dispatch. Independent Release E2E runs its suite on matching PRs, every merge group and manual dispatch, not on ordinary main pushes.
- The scheduled accessibility runtime commands currently mask failures; their green job status is not evidence of successful execution.
- Scheduled security audit steps include continue-on-error; inspect their artifacts and logs.
- The cost-update workflow has separate failures. See its latest runs rather than assuming scheduled coverage is healthy.

These limitations describe the checked-in configuration as of September 29, 2026. Update this section when fixes merge.

## Run and inspect CI

Run these commands from a repository checkout with an authenticated GitHub CLI:

```bash
gh run list --repo EconGraph/econ-graph --branch main --limit 20
gh run view RUN_ID --repo EconGraph/econ-graph
gh run view RUN_ID --repo EconGraph/econ-graph --log-failed
gh run view RUN_ID --repo EconGraph/econ-graph --json jobs
gh workflow run ci-core.yml --repo EconGraph/econ-graph --ref BRANCH
gh workflow run ci-core.yml --repo EconGraph/econ-graph --ref BRANCH -f run_e2e_tests=true -f e2e_test_suite=core
gh workflow run release-e2e.yml --repo EconGraph/econ-graph --ref BRANCH
gh workflow run playwright-tests-deployed.yml --repo EconGraph/econ-graph -f base_url=https://YOUR_DEPLOYED_TARGET
```

Prefer --ref to select the workflow revision and checkout together. Core CI also exposes a branch input which overrides checkout, but dispatch-time conditions still use the event ref; mixing refs can select the wrong jobs.

## Diagnosing skipped or green jobs

Inspect individual job and step conclusions, not only the workflow badge. Distinguish workflow path exclusions, explicit job selection, dependency skips and allowed failures.

GitHub applies a default success() status condition. A skipped ancestor can skip dependent jobs even after an intermediate job overrides that skip. If an upstream gate is optional, either let it complete successfully without performing its optional work or use an explicit status condition in each affected downstream job, preserving checks that required dependencies succeeded.

Detect changes completes on main/develop/release refs and merge groups while bypassing only its filter step, avoiding propagation of an intentional skip into mandatory tests. Adding !cancelled() to one intermediate job alone is insufficient.

A required check is a job that always reports: a workflow skipped by a trigger path filter, or a job skipped by its condition, leaves the check pending (blocking a PR) or, for a skipped job, counts as passed. So branch protection requires only the result jobs, which run with `if: always()` and decide from the results of the jobs they need.

A YAML parser can detect syntax errors; it cannot establish which jobs GitHub will run. Validate main/develop/release selection, frontend-only and backend-only PRs, shared changes, failed dependencies and cancellation. Do not claim main coverage from a successful PR run alone.

## Related guides

- [Merge queue](MERGE_QUEUE.md)
- [CI optimization notes](CI_OPTIMIZATION_NOTES.md)
- [CI failure troubleshooting](../../ci/docs/CI_FAILURE_TROUBLESHOOTING.md)
- [E2E testing strategy](e2e-testing-strategy.md)
- [Release E2E suite](../../frontend/tests/e2e/release/README.md)
- [Release process](RELEASE_PROCESS.md)
