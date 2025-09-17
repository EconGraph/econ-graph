# Test Documentation

This is a test documentation file to verify that CI/CD workflows skip on documentation-only changes.

## Test Purpose

This file exists solely to test the path filtering configuration in GitHub Actions workflows. When this file is modified or added, the CI/CD pipeline should skip running tests since it's a documentation-only change.

## Expected Behavior

- CI workflows should skip when only this file is changed
- The workflow should still run when code files are changed
- Manual workflow dispatch should always run regardless of paths

## Test Scenarios

1. **Documentation-only change**: Modify this file → CI should skip
2. **Code change**: Modify any `.rs`, `.tsx`, `.ts`, `.js` file → CI should run
3. **Mixed change**: Modify both docs and code → CI should run
4. **Manual trigger**: Manual workflow dispatch → CI should always run

## Implementation Details

The path filtering is implemented using GitHub Actions `paths-ignore` feature:

```yaml
paths-ignore:
  - '*.md'
  - 'docs/**'
  - 'README.md'
  - 'LICENSE'
  - '.gitignore'
  - '.github/workflows/README.md'
```

This configuration ensures that CI/CD resources are not wasted on documentation-only changes while maintaining full testing coverage for actual code changes.

## Update Test

This line was added to test the path filtering functionality. If CI/CD skips running when this change is pushed, the path filtering is working correctly.
