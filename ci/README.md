# CI/CD Directory

This directory contains CI/CD related scripts, configurations, and tools for the EconGraph project.

## Directory Structure

```
ci/
├── configs/                    # Playwright test configurations
├── docker/                     # Docker configurations for CI
├── docs/                       # CI/CD documentation
│   ├── README.md              # CI documentation index
│   ├── CI_FAILURE_ANALYSIS_AND_FIXES.md
│   ├── CI_FAILURE_TROUBLESHOOTING.md
│   ├── E2E_TEST_FAILURE_ANALYSIS.md
│   └── workflow-status-report.md
├── scripts/                    # CI/CD automation scripts
│   └── validate-ci-workflows.sh  # GitHub Actions workflow validation
└── README.md                   # This file
```

## Scripts

### `scripts/validate-ci-workflows.sh`

Validates GitHub Actions CI/CD workflow files for common issues.

**Usage:**
```bash
./ci/scripts/validate-ci-workflows.sh
```

**Features:**
- YAML syntax validation
- Job structure validation (ensures all jobs have steps)
- Orphaned workflow detection
- Naming consistency checks
- Clear error reporting with color-coded output

**Integration:**
- Can be run as a pre-commit hook
- Integrated into CI pipeline validation
- Follows RelEng persona best practices

## Modular Organization

This directory follows the project's modular codebase organization principles:

- **Domain-Specific**: All CI/CD related tools are organized under `ci/`
- **Clear Separation**: Separates CI/CD tools from general project scripts
- **Maintainable**: Easy to find and maintain CI/CD specific tooling
- **Consistent**: Follows established patterns for domain-specific directories

## Documentation

- **[CI Documentation](docs/README.md)** - Comprehensive CI/CD documentation and troubleshooting guides
- **[GitHub Actions Workflows](../.github/workflows/README.md)** - Workflow documentation
- **[RelEng Persona](../personas/releng-engineer.md)** - Release engineering practices
- **[AI Developer Standards](../personas/ai-developer-standards.md)** - Development guidelines

## Test Suite Organization

The CI pipeline includes multiple specialized test suites that run in parallel:

- **Core Tests**: Basic functionality (navigation, authentication, dashboard)
- **Global Analysis Tests**: World map, country selection, economic indicators (162 tests)
- **Professional Analysis Tests**: Advanced charting, technical indicators (39 tests)
- **Mobile Tests**: Mobile versions of all test suites
- **Comprehensive Tests**: Integration/workflow tests (excludes specialized suites)

For detailed information about test organization and troubleshooting, see the [CI Documentation](docs/README.md).
