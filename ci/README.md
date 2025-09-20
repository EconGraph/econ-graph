# CI/CD Directory

This directory contains CI/CD related scripts, configurations, and tools for the EconGraph project.

## Directory Structure

```
ci/
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

## Related Documentation

- [GitHub Actions Workflows](../.github/workflows/README.md) - Workflow documentation
- [RelEng Persona](../personas/releng-engineer.md) - Release engineering practices
- [AI Developer Standards](../personas/ai-developer-standards.md) - Development guidelines
