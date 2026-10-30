# /pr-checks

Watch PR checks for the current branch and keep streaming updates until completion.

Use `gh pr checks --watch` to stream status for the open PR tied to the current branch. If no PR exists yet, use [`/pr-ready`](pr-ready.md) to push the branch and create a PR first.

In Cursor, invoke:

```text
/pr-checks
```

Alternatives and tips:

- Specific workflow run: use `gh run list --branch "$(git rev-parse --abbrev-ref HEAD)"` to choose a run ID, then run `gh run watch <run-id>`.
- One‑shot summary (no stream): `gh pr checks`
- Full PR status: `gh pr view --json url,headRefName,statusCheckRollup`
- For monorepos or matrix builds, open the Checks tab from the PR URL printed by [`/pr-ready`](pr-ready.md) to inspect individual jobs.
