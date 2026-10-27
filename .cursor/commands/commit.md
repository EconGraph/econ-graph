# /commit

Create a Conventional Commit and show a concise status summary.

- message (string, required): Use Conventional Commits: `type(scope): summary` (see [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/))
  - Types: `feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert`
  - Examples: `feat(backend): add series search by tag`, `docs(personas): add Cursor slash commands`

Invoke in Cursor:

```text
/commit "fix(frontend): scope drawer queries to prevent duplicate element errors"
```

Before committing, run the relevant checks below (strongly recommended). Start each command at the repository root; the subshells keep your shell in that directory.

- Frontend formatting/lint (auto-fix):
  - `(cd frontend && npm run prettier-fix && npm run lint:fix && npm run typecheck)`
- Frontend tests (targeted to changed areas):
  - `(cd frontend && npm run test:unit)` or `(cd frontend && npm run test:unit -- path/to/test.tsx)` for specific files
- Backend formatting/lints:
  - `(cd backend && cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings)`
- Backend tests (targeted):
  - `(cd backend && cargo test -p <crate> <module_or_test_name>)`

Pre-commit reminder:

- Install once: `pre-commit install`
- Run locally for staged files: `pre-commit run`
- Run across repo: `pre-commit run --all-files`

Notes:

- Docs-only PRs still receive Markdown and relative-link checks through [Docs Checks](../../.github/workflows/docs-checks.yml). [Core CI](../../.github/workflows/ci-core.yml) uses path filters to skip documentation changes.
- If hooks fail, fix the underlying issues; do not bypass with `--no-verify`
