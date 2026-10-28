# CI/CD Directory

This directory holds supporting scripts, Playwright configurations, Docker files, and documentation for EconGraph CI. GitHub Actions execution is defined in [`.github/workflows/`](../.github/workflows/README.md); see the [CI pipeline guide](../docs/development/CI_CD_PIPELINE.md) for job dependencies and selection rules.

## Directory Structure

Selected contents:

```text
ci/
├── configs/                      # Playwright test configurations
├── docker/                       # Docker configurations for CI
├── docs/                         # CI/CD documentation
│   ├── README.md                  # CI documentation index
│   ├── CI_FAILURE_TROUBLESHOOTING.md
│   └── workflow-status-report.md
├── scripts/                      # CI utility scripts
│   └── validate-ci-workflows.sh   # Local workflow checks
└── README.md                     # This file
```

## Scripts

### `scripts/validate-ci-workflows.sh`

This optional local helper parses workflow YAML and checks that each job has a nonempty `steps` section. It also warns about missing workflow names and triggers. The name check only tests whether `name` is nonempty. The trigger check uses PyYAML's default loader, which can interpret `on` as boolean true, so verify trigger warnings against the workflow YAML before treating a workflow as inactive.

Run from the repository root with Python 3 and PyYAML installed:

```bash
./ci/scripts/validate-ci-workflows.sh
```

Missing PyYAML is reported as invalid YAML by the helper. The helper is not invoked by the workflows; CI workflow linting uses [actionlint and shellcheck](../.github/workflows/actionlint.yml). See the [workflow validation guidance](../.github/workflows/README.md#validation) for those checks.

## Documentation

- [CI Documentation](docs/README.md) — CI documentation index and troubleshooting guides
- [GitHub Actions Workflows](../.github/workflows/README.md) — Workflow inventory and validation guidance
- [RelEng Persona](../personas/releng-engineer.md) — Release engineering practices
- [AI Developer Standards](../personas/ai-developer-standards.md) — Development guidelines

## Test Configurations

The Playwright files in [`configs/`](configs/) define test selection and browser projects:

- [Core](configs/playwright-core.config.ts) selects the `tests/e2e/core` directory.
- [Analysis](configs/playwright-analysis.config.ts) selects `*analysis*.spec.ts` files, including global analysis.
- [Comprehensive](configs/playwright-comprehensive.config.ts) selects the `tests/e2e` directory without excluding specialized suites.
- [Debug](configs/playwright-debug.config.ts) selects `*debug*.spec.ts` files in Chromium.
- Mobile configurations select [authentication, series explorer, and visual checks](configs/playwright-mobile-core.config.ts), [global analysis](configs/playwright-mobile-analysis.config.ts), or [complete workflows](configs/playwright-mobile-comprehensive.config.ts) on their configured devices.

These configuration files are separate from the jobs selected by Core CI. Its container E2E jobs require a manual dispatch with `run_e2e_tests=true`; `e2e_test_suite` selects desktop core, desktop comprehensive, or mobile execution. The mobile job runs mobile core even when `mobile-comprehensive` is selected. There is no separate global-analysis job in Core CI. See the [pipeline guide](../docs/development/CI_CD_PIPELINE.md#known-coverage-limitations) for current coverage limitations and the independent Release E2E workflow.
