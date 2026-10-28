# /pr

Create or update a GitHub pull request for the current branch, with a description that explains the change and its impact.

Use the GitHub CLI (`gh`) for all PR and CI operations. Do not use MCP for PR creation or CI checks.

## Arguments

- `title` (string, required): PR title in Conventional Commits format.
- `body` (string, optional): PR description in Markdown. On creation, generate an omitted body from the branch's commits and diff against the base. On update, preserve an omitted body.
- `base` (string, optional): Base branch. On creation, default to `main`. On update, preserve the existing base unless explicitly supplied.
- `draft` (boolean, optional): Whether the PR is a draft. On creation, default to `false` (ready for review). On update, preserve the current state unless explicitly supplied.

```text
/pr "feat(frontend): scope drawer queries with within() to fix flakiness"
```

To supply all arguments, including an explicit draft state:

```text
/pr "feat(frontend): scope drawer queries with within() to fix flakiness" "Scope drawer queries to avoid matching controls outside the drawer." main true
```

## Create or update

Find an open PR for the current branch with `gh pr list --head "$(git branch --show-current)" --state open`, and inspect it with `gh pr view`. Match the head repository as well as the branch before updating. If no open PR exists, create one with the creation defaults above. A failed lookup is not evidence that no PR exists; resolve the error before creating another PR.

For an existing PR, update the title and only the optional fields explicitly supplied. Use `gh pr edit` for the title, body, and base; use `gh pr ready` for an explicit `draft: false` or `gh pr ready --undo` for an explicit `draft: true`. Pass multiline descriptions through a file with `--body-file` to preserve formatting.

## Write the description

The PR title and description become the squash commit message. Write for a reviewer who has not read the conversation: lead with the concrete problem and resulting behavior, then explain why the change matters. A small PR usually needs one or two connected paragraphs and a short validation note. For a complex change, use a few purposeful bullets or sections when they make the explanation easier to follow.

Include design choices, compatibility concerns, security effects, migrations, rollout or rollback details only when they help assess the change. Summarize relevant validation and its practical limits; link to detailed evidence when needed. Keep checklists, review bookkeeping, retry history, and running status in PR comments.

## Link the Linear issue

Each PR has exactly one PR-level Linear issue. Use the existing issue, or create it before the PR if none exists, following [How agents keep Linear up to date](https://linear.app/econgraph/document/how-agents-keep-linear-up-to-date-7d547a9af496). Include `Fixes ECO-NNN` for that issue in the description. If it has a parent, use `Part of ECO-MMM` to link the parent without closing it; reserve closing words for the PR's own issue.

Once the PR exists, attach its URL through the Linear issue's `links` field and keep the issue's state current as described in that guidance. A link-only comment does not establish this relationship. Preserve useful issue references and related links when replacing a description.

## Repository conventions

Follow the [developer standards](../../personas/ai-developer-standards.md) and [repository conventions](https://linear.app/econgraph/document/repository-conventions-63c5891a9777).

- Use Conventional Commits titles: `feat`, `fix`, `docs`, `chore`, `refactor`, `perf`, `test`, `style`, `ci`, `build`, or `revert`, with an optional scope. Examples: `feat(backend): add series search by tag`, `docs(personas): clarify Cursor slash commands`.
- Keep each PR focused on one concern; split independent fixes or features into separate PRs.
- Validate the change with appropriate checks. Add or update unit, integration, or end-to-end tests when behavior changes; documentation changes need relevant documentation checks. Do not bypass pre-commit.
- Update documentation and workflows when the behavior they describe changes.
- Mark finished work ready for review; CodeRabbit skips drafts. On an update that omits `draft`, preserve its state and use an explicit readiness operation when appropriate.
- Watch CI and address automated review comments before considering the PR complete. [Docs Checks](../../.github/workflows/docs-checks.yml) runs on every PR. [Core CI](../../.github/workflows/ci-core.yml) skips a PR only when every changed path matches its exclusions; Markdown under `.cursor/` still triggers Core CI. Do not merge without authorization.

## Helper commands

- [Create or find a PR and view CI](pr-ready.md): `/pr-ready "feat(...): ..."`
- [Watch PR checks](pr-checks.md): `/pr-checks`
- [Show latest CI runs](ci-latest.md): `/ci-latest`
