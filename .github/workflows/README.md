# CI/CD Workflows

The workflow YAML files in this directory define what runs. See the [CI pipeline guide](../../docs/development/CI_CD_PIPELINE.md) for job dependencies, selection rules, commands and limitations.

## Workflow inventory

All 23 workflow files. **Required** means a job of the workflow is a required check in the
`main only by pr` ruleset, as listed in
[merge-queue-rules.json](../../scripts/github/merge-queue-rules.json) (the check name follows in
parentheses). Every other workflow is advisory: it can be red, or not run, without blocking a
merge.

| File | Required | Automatic triggers | Purpose |
| --- | --- | --- | --- |
| [ci-core.yml](ci-core.yml) | Yes (`Core CI Result`) | Push to main, develop, optimize/ci-core-primary and release/** (push subject to paths-ignore); all PRs (a docs-only PR runs only Detect changes and Core CI Result); merge groups | Backend build and tests, frontend tests, quality, audit, licenses and Docker validation |
| [release-e2e.yml](release-e2e.yml) | Yes (`Release E2E Result`) | All PRs (the suite runs when its listed application, Keycloak, release-stack or workflow paths change); merge groups | Chromium against a release frontend, real backend and PostgreSQL 18 with recorded fixtures |
| [docs-checks.yml](docs-checks.yml) | Yes (`Markdown lint and links`) | Push to main/develop; all PRs, including docs-only and stacked PRs; merge groups | Markdown lint and offline relative-link checks |
| [feature-flags.yml](feature-flags.yml) | Yes (`Feature Flags Check`) | Push to main/develop/release/**; train-* tags; all PRs; merge groups; Monday 06:17 UTC | Flag validation and generated-file consistency |
| [actionlint.yml](actionlint.yml) | Yes (`Lint workflows`; main only, not release branches) | Push to main/develop that changes workflows, local actions or their validation; all PRs; merge groups | Immutable action references, actionlint with shellcheck on `run:` scripts, and the offline tests of the ruleset and release provenance scripts |
| [train3-compatibility.yml](train3-compatibility.yml) | No | All PRs; merge groups; push to main | Split-plane compatibility contracts |
| [train4-platform.yml](train4-platform.yml) | No | PRs and pushes to main that change platform, k8s, Keycloak or the Train 4 tool installer; merge groups | Platform guards and pinned schemas |
| [train4-talos.yml](train4-talos.yml) | No | PRs and pushes to main that change infra/talos, scripts/talos or the Train 4 tool installer; merge groups | Talos ownership and plan guards; pinned Talos provider configuration |
| [train4-objectstore.yml](train4-objectstore.yml) | No | PRs and pushes to main that change the object-store scripts, backup snapshot scripts or their docs; merge groups | Offline object-store ownership and backup guards |
| [train4-npm-compat.yml](train4-npm-compat.yml) | No | PRs and pushes to main that change npm manifests, the npm compatibility scripts, ci/docker or the frontend Dockerfile; merge groups | Patched npm registry-matcher compatibility and audits |
| [app-data-conversion.yml](app-data-conversion.yml) | No | PRs and pushes to main that change the app/data split or conversion scripts, the deploy library, create-secrets.sh or backend migrations | Rehearse the app/data database conversion on synthetic PostgreSQL records |
| [train5.yml](train5.yml) | No | PRs that change the company research code, backend manifests, migrations or the GraphQL roots | Company research contracts and enabled APIs |
| [visual-layout.yml](visual-layout.yml) | No | PRs that change frontend, admin-frontend or config/flags | Public and admin layout checks in Chromium |
| [release.yml](release.yml) | No (not a PR check) | Push of `v*.*.*` tags | Manual dispatch (from main or a release branch, building its head; no ref input) or `vX.Y.Z-rc.N` tag pushes build and publish missing commit-tagged images, only for commits on main or a release branch, with signed build provenance for the commit and no BuildKit cache; final version tags promote existing, verified image digests without rebuilding and draft a GitHub release. See the [release process](../../docs/development/RELEASE_PROCESS.md). |
| [security.yml](security.yml) | No | Daily 02:00 UTC | Dependency audits, Trivy, CodeQL, licenses and Dockerfile scanning |
| [accessibility-tests.yml](accessibility-tests.yml) | No | Monday 09:00 UTC | Accessibility static/runtime checks and manual-testing guidance |
| [update-cost-analysis.yml](update-cost-analysis.yml) | No | Manual dispatch; matching PRs | Validate calculations and generated-output repeatability; manual runs on main attempt to publish changes |
| [crawler-integration-test.yml](crawler-integration-test.yml) | No | None | Enqueue and execute a real source crawl; needs network access and source credentials where required |
| [playwright-tests.yml](playwright-tests.yml) | No | None | Legacy Playwright and Grafana checks |
| [playwright-tests-comprehensive.yml](playwright-tests-comprehensive.yml) | No | None | Legacy comprehensive Playwright suite |
| [playwright-tests-deployed.yml](playwright-tests-deployed.yml) | No | None | Legacy suite: its `base_url` input sets `PLAYWRIGHT_BASE_URL`, but the default config reads `FRONTEND_URL`, so the input does not select the target. Use the [release suite’s deployed mode](../../frontend/tests/e2e/release/README.md#against-a-deployed-build-rel-qa-check-3). |
| [ci-experimental.yml](ci-experimental.yml) | No | None | Optional RAM-disk experiment |
| [ramdisk-build-cache.yml](ramdisk-build-cache.yml) | No | None | Manual build-cache experiment |

The merge queue's required checks are the result jobs of Core CI and Release E2E, and the Docs Checks, Feature Flags and Actionlint jobs; see [Merge queue](../../docs/development/MERGE_QUEUE.md). The `release branches by pr` ruleset requires the same checks except `Lint workflows`. All listed workflows support manual dispatch. Scheduled times are UTC; GitHub may delay scheduled runs. The Playwright workflows do not run automatically on version tags. Admin Frontend Tests is a job in Core CI, not a separate workflow file.

### Required checks: proposed changes (not applied)

This is a proposal for a maintainer (ECO-896). Nothing here changes GitHub; the required set
changes only when `merge-queue-rules.json` is edited and `scripts/github/apply-rulesets.sh` is
run by an admin.

A workflow with a path filter on `pull_request` cannot simply be made required: on a PR that
does not touch its paths it never reports, and the PR waits forever. Core CI and Release E2E
avoid this with an always-running result job (`ci/scripts/check-required-jobs.sh`).

- **Require now: `Split-plane compatibility`** (train3-compatibility.yml). It runs on every PR
  and merge group with no path filter, and a release depends on the app/data split it checks.
  `release/v4.1` has no such workflow, so the release ruleset must leave it out, the way it
  leaves out `Lint workflows`.
- **Require after adding a result job:** train4-platform.yml, train4-talos.yml,
  train4-objectstore.yml and app-data-conversion.yml. The platform release deploys through the
  Talos cluster and Flux, keeps its backups in the object store, and runs the app/data
  conversion, so each guards something a release depends on. Each needs a job that always
  runs and fails when the path-selected job failed, as in Release E2E.
- **Decide with its owner:** train4-npm-compat.yml. Its paths include the frontend Dockerfile
  and ci/docker, so it may guard what goes into a release image; if it does, give it a result
  job and require it.
- **Keep advisory:** train5.yml (Train 5 is unscheduled, and its paths are one feature area),
  visual-layout.yml (layout checks for review; a release does not depend on them), and the
  scheduled, manual and legacy workflows. release.yml is not a PR check: it runs on tags and dispatch, and its failures
  show in the existing CI alerts.

## Operational notes

A green workflow does not imply every suite ran or every step passed. Inspect job conclusions and any continue-on-error steps. The [pipeline guide](../../docs/development/CI_CD_PIPELINE.md#known-coverage-limitations) records current coverage limitations.

Core CI builds and smoke-tests Docker images; it does not publish or deploy them. Release E2E is independent of the optional Core CI E2E jobs.

External actions and reusable workflows are pinned to full commit SHAs, with version comments for readability. Resolve updates from the action's upstream repository and update the SHA and comment together. Docker actions require an image digest; local actions use the calling commit. The Actionlint workflow enforces these rules in workflows and composite actions.

Every `actions/checkout` must set `persist-credentials: false`. Steps that need authenticated Git or GitHub API access must receive credentials explicitly for that step. The same validation script enforces this for workflows and composite actions.

The manual crawler workflow accepts only source and series inputs. A maintainer configures its free-tier credentials with `gh secret set FRED_API_KEY`, `gh secret set BLS_API_KEY`, `gh secret set BEA_API_KEY` and `gh secret set CENSUS_API_KEY`, entering values at the hidden prompt. FRED, BEA and Census require their keys; BLS can run without one. Keep paid-source and production authentication credentials out of these repository secrets. Never pass keys as dispatch inputs, which appear in workflow metadata.

## Validation

Run Markdown lint and relative-link checks as defined in [docs-checks.yml](docs-checks.yml). Lint workflow files with actionlint and shellcheck at the versions pinned in [actionlint.yml](actionlint.yml); per-file ignores live in [actionlint.yaml](../actionlint.yaml).

The repository's [workflow validation script](../../ci/scripts/validate-ci-workflows.sh) checks YAML and basic job structure; it does not prove job-selection semantics. Its trigger check uses PyYAML's default loader, which can interpret the key `on` as boolean `true`, so trigger warnings require verification against the YAML.

With `PyYAML==6.0.3` installed, run `python3 ci/scripts/check_action_pins.py` and `python3 -m unittest discover -s ci/scripts -p 'test_action_pins.py'` to check action references and exercise the enforcement rules locally.

When changing workflow conditions, check actual jobs on both a PR and a push to main. A passing PR alone does not validate main-specific conditions.
