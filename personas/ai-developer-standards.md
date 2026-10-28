# AI developer standards

Use these standards alongside the role personas when contributing to EconGraph. Preserve existing features and test intent, investigate failures with evidence, and carry each task through review and verification.

## Task branches and focused changes

Work in the assigned worktree and branch. For a new ordinary task without a supplied base, start from fetched `origin/main`. In a clean worktree, create and switch to a task branch with:

```bash
git fetch origin
git switch -c docs/eco-NNN-task-description origin/main
```

If the task needs a separate worktree, use this alternative from the repository:

```bash
git fetch origin
git worktree add -b docs/eco-NNN-task-description ../econ-graph-task origin/main
```

Replace the example branch name with the task's issue ID and concern. Use an explicitly assigned base when provided; release fixes follow the [release fix flow](../docs/development/RELEASE_PROCESS.md#fix-flow). Separate branches and worktrees let independent tasks proceed without changing another task's checkout. Do not switch or reset a worktree containing someone else's changes.

Use `git mv old_path new_path` when moving a tracked file. It moves the file and stages both path changes together. Git detects renames from content when presenting diffs and history; `git mv` does not store a special rename record. Use noninteractive Git commands so a task cannot stall waiting for an editor or prompt.

Keep each PR focused on one concern. Independent bug fixes, features, formatting cleanups, and test reorganizations belong on separate branches from the appropriate base, rather than being collected into the current task. Keep changes together when they implement one feature or must work together to solve one problem; a fix may require code, configuration, and tests in the same PR. Explain dependencies when PRs must land in a particular order.

Follow the established directory structure and the placement of similar tools: CI tooling belongs in `ci/scripts/`, general project scripts in `scripts/`, Rust code in `backend/`, and React code in `frontend/`. Prefer the relevant domain directory when adding validation or automation.

## Commits, pull requests, and releases

Follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) for commits and PR titles. Use `doc:` for documentation-only commits and PR titles. Do not include CI-skip markers, personal names, AI attribution, coauthor trailers for agents, or session links. Run the applicable pre-commit checks and correct failures rather than bypassing them.

Write PR descriptions for a reviewer who has not read the conversation. Lead with the problem and resulting behavior, explain why the change matters, and include relevant trade-offs and a short validation note. Use connected paragraphs sized to the change; file-by-file inventories and mandatory section templates obscure small changes. Keep review bookkeeping and running status in comments. Reread the whole final description before publishing or editing it.

The Cursor slash commands are optional helpers defined in this repository:

- [`/commit`](../.cursor/commands/commit.md) creates a Conventional Commit and reports status.
- [`/pr`](../.cursor/commands/pr.md) creates or updates a PR with a supplied or generated body.
- [`/pr-ready`](../.cursor/commands/pr-ready.md) pushes the current branch and ensures an open PR; it does not curate the description.
- [`/pr-checks`](../.cursor/commands/pr-checks.md) watches checks, and [`/ci-latest`](../.cursor/commands/ci-latest.md) lists recent runs.

Apply the standards above when supplying helper arguments or reviewing generated text. Outside Cursor, use available noninteractive Git and GitHub tools. For example, with an authorized push and the GitHub CLI available, use `gh pr create --title 'doc: clarify developer standards' --body-file /tmp/pr-body.md`, `gh pr checks --watch`, and `gh run list --branch BRANCH`. If a needed tool or permission is unavailable, use another authorized interface or report the specific blocker.

Implementation work is complete only after a PR exists, CI is green for its latest commit, and all CodeRabbit and other automated review findings have been addressed. Fix findings you agree with; for disagreements, post the technical reasoning and resolve the thread when appropriate. Mark the PR ready when it is ready for review. Do not declare completion while checks or automated reviews remain pending, and do not merge without authorization.

Version numbers move forward. Follow [Semantic Versioning](https://semver.org/) and the existing release series: the [backend workspace](../backend/Cargo.toml) declares version `4.0.0`. Change versions or tags only as part of the authorized [release process](../docs/development/RELEASE_PROCESS.md).

## Features, tests, and database isolation

Cover new behavior and regressions with tests that can distinguish correct behavior from the failure being fixed. Do not delete difficult tests or existing features to make a task easier. When replacing a test, preserve its original intent unless approved requirements change that intent. Reversible prose edits do not need implementation tests that merely repeat the wording.

Choose test scope by dependencies separately from Cargo's target layout. Fast smoke selections should require no database, network, external API, or service credentials; temporary files are acceptable. Parsing, calculations, configuration validation, and data-structure logic are useful fast-test candidates. Tests of database CRUD, API integrations, authentication, and full workflows need their documented service setup or disposable resources.

The Rust workspace is under `backend/`. Library tests live in crate modules such as `src/.../tests.rs`; Cargo integration targets live in the owning crate's `tests/` directory, not a workspace-wide root directory. Use `#[test]` for synchronous tests and `#[tokio::test]` when async execution is needed. Async execution alone does not make a test an integration test. Name tests clearly and document their prerequisites.

Existing database-backed tests also run as library tests. For example, [dataset tests](../backend/crates/econ-graph-core/src/models/dataset/tests.rs) use migrations once per test binary and unique fixture rows. [Core CI](../.github/workflows/ci-core.yml) selects these with `--lib` and also runs a workspace suite. Consult the [CI/CD guide](../docs/development/CI_CD_PIPELINE.md) and workflow for actual selections, feature flags, and triggers. Do not move database tests wholesale to change their Cargo category. If a dependency-bound test enters a fast smoke selection, correct that selection or the test's dependency boundary while retaining its coverage elsewhere.

For database changes, follow [database testing guidance](../docs/testing/DATABASE_TESTING.md) and the target module's setup and locking conventions. Use disposable databases, coordinate cleanup with other database users, and give independent fixtures unique identifiers. A `#[serial]` attribute or `--test-threads=1` does not coordinate separate Cargo processes. When modifying a database schema, run the existing schema validation integration target, with its documented PostgreSQL, Docker, and Diesel CLI prerequisites:

```bash
cd backend
cargo test -p econ-graph-core --test schema_validation_test -- --nocapture
```

Database cleanup helpers should return errors to the caller. Use `?` and, where needed, `map_err()` in fallible utilities; at the test boundary, check the result and fail with useful context if cleanup fails. Continuing after failed cleanup conceals the original failure and can contaminate later tests.

A duplicate-key constraint violation is evidence to investigate isolation: check whether cleanup failed, parallel tests shared a database, or fixtures reused an identifier. Inspect the pool, migration target, and lock before changing test data. Unique identifiers help independent fixtures, but cannot repair destructive cleanup racing with another test.

## Rust warnings

Address compiler warnings rather than suppressing them by default. Prefix a variable with `_` when it is intentionally unused. Handle a returned `Result` with propagation, `match`, or `if let`; use `expect` at a test boundary where failure should stop the test with context. Use `let _ = result;` only when discarding the outcome is intentional and safe, never to continue after failed setup or cleanup.

`cargo fix --lib -p PACKAGE --tests` can apply compiler suggestions. Review its changes and run the affected checks; automatic suggestions do not establish correctness.

## Ports and service configuration

Make service ports configurable and keep each stack's listeners, client URLs, health checks, and container mappings in agreement. Separate environments and concurrently running suites may use different ports. If a port is occupied, use dynamic allocation where supported or choose an available random port through the consumer's environment override. Do not kill an unrelated process or repeatedly increment a hardcoded port.

The repository has several configuration consumers. [CI environment configuration](../config/ci-env.config) defines service defaults, derived endpoints, and distinct backend test database ports. Loading that file is explicit; its existence does not make other tools inherit its values. The E2E core job in [Core CI](../.github/workflows/ci-core.yml) sources [`ci-ports.env`](../ci-ports.env) and exports selected values through `GITHUB_ENV`. The [release Playwright configuration](../frontend/playwright.release.config.ts) instead reads `RELEASE_BACKEND_PORT` and `RELEASE_FRONTEND_PORT`, defaulting to `18080` and `18081`, and derives service URLs from them.

Before changing a port, inspect how the affected launcher and test configuration load values. Change the relevant consumer's override and all dependent endpoints together. Do not assume an environment loader expands shell expressions or that changing a port automatically updates a separately assigned URL.

For a Bash-sourced configuration, this illustrative pattern applies an override before deriving and exporting URLs:

```bash
BACKEND_PORT=${BACKEND_PORT:-18080}
BACKEND_URL="http://localhost:${BACKEND_PORT}"
BACKEND_HEALTH_URL="${BACKEND_URL}/health"
GRAPHQL_ENDPOINT="${BACKEND_URL}/graphql"
export BACKEND_PORT BACKEND_URL BACKEND_HEALTH_URL GRAPHQL_ENDPOINT
```

This is a Bash example, not a universal `.env` syntax contract. Verify the affected service, health check, and test client use the selected values.

## Debugging and verification

When the maintainer questions an approach, explain the reasoning and consult the relevant library documentation before changing it reflexively. When a failure occurs, investigate one concrete hypothesis at a time:

1. Capture the exact failing command, test name, error, and relevant environment. For CI, read the failed logs with `gh run view RUN_ID --log-failed` when the CLI is available. Read the full error context.
2. Establish expected behavior from the requirements and code. Trace the request or data flow to the first point where observed behavior diverges, checking the relevant application, proxy, or container logs and whether configuration actually took effect.
3. Reproduce the failure locally where feasible. If the full scenario is hard to reason about, create a smaller reproduction or disposable proof of concept to test a specific assumption. Predict what evidence would confirm or reject the hypothesis.
4. Make a focused change supported by that evidence. One hypothesis can require several files; avoid unrelated changes that obscure which behavior mattered.
5. Rerun the original failing scenario and affected tests. A small reproduction supports diagnosis but does not establish that the original scenario now works. Check related callers when shared functionality changes.
6. Push within the authorized workflow and confirm CI on the latest PR commit. Address automated findings and repeat the affected verification after changes. Report what passed and any specific limits; do not infer success from a plausible patch or partial results.

If local reproduction is infeasible, identify the missing prerequisite and use authorized CI or another suitable disposable environment to run the failing scenario. Keep the verification gap explicit until that run completes. Persist while experiments produce new evidence. If investigation stops producing evidence, summarize what is known, failed hypotheses, and the specific information needed before asking for help.

Common CI symptoms suggest starting points, not diagnoses: container timeouts can involve resources or networking; compilation errors can involve dependencies or syntax; smoke failures can expose external dependencies; constraint violations can expose shared state. Confirm the cause rather than applying a stock fix. Record useful decisions and investigation evidence in the issue or PR, with progress and unbuilt plans in Linear rather than checked-in documentation.

### Long-running command output

Keep output visible and retain the complete log. Prefer the execution tool's native PTY support when a program buffers progress in a pipe. A PTY may change the program's output behavior; it does not guarantee that all output becomes unbuffered. Monitor the process through completion and inspect its final exit status.

On Linux with util-linux `script`, `-c` selects the child command and `-e` returns its exit status. For example, this harmless command prints progress and returns status 7:

```bash
script -q -e -c 'printf "progress\n"; exit 7' /dev/null
```

Check the installed tool's help on other platforms. Do not pipe a running test, installation, or build into `head`: that truncates evidence and can stop the producer. When logging through `tee`, preserve failures as described below.

### E2E readiness and failure signals

Before interpreting an E2E assertion failure, check frontend and backend readiness in the authorized test environment. Use bounded health-check retries, for example up to 30 attempts at two-second intervals, with failure logs on timeout. Keep service logs available during the test. Confirm static JavaScript and CSS are served, API endpoints are reachable from the test's network, ports agree, and container networking supports the required connections. Database connection errors also require checking the test's connection setup without exposing credentials.

Preserve test failures through every wrapper. When a wrapper's final action is to invoke its arguments and no subsequent cleanup is needed, use:

```bash
exec "$@"
```

When cleanup must follow, capture the command status even if the script uses `set -e`, perform cleanup, and return a failure if either operation failed. This Bash example takes a cleanup command (or an exported Bash function) as its first argument and the child command as the remaining arguments:

```bash
cleanup_command=$1
shift
status=0
"$@" || status=$?
cleanup_status=0
"$cleanup_command" || cleanup_status=$?
if [ "$status" -eq 0 ]; then
  status=$cleanup_status
fi
exit "$status"
```

For a Bash pipeline that writes a complete log, `pipefail` prevents a successful `tee` from hiding the child's failure. This harmless example returns a failure:

```bash
set -o pipefail
bash -c 'printf "test output\n"; exit 7' | tee /tmp/example-test.log
```

A background process starting or a log containing no visible errors does not establish test completion. Confirm that the test process exits and its status reaches CI.

## Resolving test and component disagreements

When tests expect different behavior from a component, investigate both before changing either. Neither passing an existing test nor matching the current component establishes the intended product behavior.

Compare the test assertions and actual behavior with approved specifications, design decisions, user stories, and acceptance criteria. Consider the user problem, business requirements, and technical constraints those sources identify. General UX principles or another product's conventions can suggest questions; they do not establish this product's requirements. If the sources leave the behavior unclear, ask for product clarification rather than choosing the easier implementation.

Record the disagreement, supporting evidence, and decision in the issue or PR so reviewers can assess why behavior or coverage changed. Preserve useful coverage when updating tests, and change the component when evidence shows it violates the approved behavior.

For example, suppose a ratio-analysis test expects every ratio in one flat table, while the component presents summary cards and category tabs. If approved acceptance criteria require summary cards and category tabs, update the tests to exercise those views and verify all required ratios remain accessible. If the criteria require every ratio to be visible at once, investigate changing the component. Without those criteria, the interface alone cannot establish which side is correct or prove better mobile usability or reduced abandonment.
