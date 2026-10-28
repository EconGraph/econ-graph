# Test Documentation

This file is a probe for the documentation-only path filter in [Core CI Tests](../.github/workflows/ci-core.yml).

## Test Scenarios

For the automatic-trigger scenarios below, open a PR into any branch, push commits to an existing PR, or push to a branch enabled by Core CI: `main`, `develop`, `optimize/ci-core-primary` or `release/**`. A push to another branch without a PR cannot test this path filter because Core CI does not run on that push event.

1. **Documentation-only change**: Modify only this file. Core CI should skip. On a PR, Docs Checks and Feature Flags should still run; see the [workflow inventory](../.github/workflows/README.md#workflow-inventory) for other workflows and their triggers.
2. **Code change**: Modify a code file outside the ignored paths, such as a `.rs`, `.tsx`, `.ts` or `.js` file in an application directory. Core CI should start.
3. **Mixed change**: Modify both this file and a code file outside the ignored paths. Core CI should start.
4. **Manual trigger**: Use GitHub's **Run workflow** button (`workflow_dispatch`) for Core CI. This starts the workflow regardless of changed paths.

Starting Core CI does not ensure that every job runs. Jobs have their own selection conditions, and manual dispatch inputs also control optional tests.

## Implementation Details

Core CI applies the same `paths-ignore` list to its `push` and `pull_request` triggers:

```yaml
paths-ignore:
  - '*.md'
  - 'docs/**'
  - 'README.md'
  - 'LICENSE'
  - '.gitignore'
  - '.github/workflows/README.md'
```

When all changed paths match this list, Core CI skips the automatic run. Changes outside these paths remain eligible for the push and PR triggers described above.

## Update Test

Edit this line to probe Core CI's path filtering. When this file is the only change in a PR or an eligible branch push, Core CI should skip; other workflows may still run.
