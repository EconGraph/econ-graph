# /workflow-audit

Audit `.github/workflows` for correctness, hygiene, and CI efficiency, and propose concrete fixes. This command returns findings and suggested diffs; applying changes, committing them, and opening a PR are separate follow-up steps.

Read the [workflow inventory and validation guidance](../../.github/workflows/README.md) and [CI pipeline guide](../../docs/development/CI_CD_PIPELINE.md) before judging triggers or job selection. Use GitHub CLI for visibility (`gh workflow list/view`, `gh run list/view`); use `gh` for PR/CI operations rather than MCP.

Audit checklist:

- Parse every workflow (`.yml` and `.yaml`) with GitHub Actions semantics. Use actionlint and shellcheck at the versions pinned in [actionlint.yml](../../.github/workflows/actionlint.yml), with the repository's [actionlint configuration](../../.github/actionlint.yaml). The basic validation script uses PyYAML's default loader, which can interpret `on` as boolean true; verify missing-trigger warnings against the actual YAML before proposing a fix.
- Triggers and path filters:
  - Establish each workflow's purpose, intended events, inputs, and required-check role. Preserve intentional manual-only and scheduled execution; propose additional events only when they serve that purpose.
  - Propose `paths` or `paths-ignore` only after identifying all inputs that can affect the result, including workflow files and shared configuration. Check that skipping a workflow will not leave a required check pending or remove needed coverage.
  - Exclude documentation changes from expensive workflows only when those files cannot affect their results. Docs Checks must still validate documentation, and Feature Flags also depends on release tags; a documentation-only diff is not enough to establish that every check can be skipped.
- Names: propose descriptive workflow and job names where missing.
- Jobs:
  - For jobs that execute steps, check `runs-on`, nonempty `steps`, and that each step has either `uses:` or `run:`. Jobs that call reusable workflows use job-level `uses:` instead of `runs-on` and `steps`; validate them as reusable-workflow calls.
  - For jobs that execute steps, propose missing `timeout-minutes` values based on observed duration and enough headroom for normal variation.
- Permissions: propose least-privilege permissions for the operations performed. A read-only workflow can start with this workflow-level setting; publishing or repository writes need their own justified permissions:

  ```yaml
  permissions:
    contents: read
  ```

- Concurrency: identify which runs overlap and whether running work may safely be superseded. For PR validation that can be replaced by a newer commit, Core CI uses this pattern:

  ```yaml
  concurrency:
    group: ${{ github.workflow }}-${{ github.event_name == 'pull_request' && format('pr-{0}', github.event.pull_request.number) || github.run_id }}
    cancel-in-progress: ${{ github.event_name == 'pull_request' }}
  ```

  The group combines workflow identity with the PR number, so a newer run for the same PR cancels its predecessor. Other events get a unique run-id group. Preserve intentional serialization with `cancel-in-progress: false` for image publication, cost-document publication, and real crawls; do not apply the PR cancellation pattern to work that must finish.
- Caching: recommend language-appropriate caches where useful, checking that keys include the inputs needed to invalidate stale results.
- Action pinning: follow repository policy for major-version or commit-SHA pins and use the repository's approved current versions when proposing changes. Do not treat an illustrative version as an upgrade target.

Expected output:

- A concise table of findings with file, issue, severity, and suggested fix.
- Minimal inline suggested diffs for each affected workflow, with reasons for trigger, filter, permission, or cancellation changes.
- A proposed title and summary for one PR containing related fixes, such as `ci: workflow hygiene fixes`.

To implement the selected findings, apply the diffs on a branch and validate the affected workflows locally with the pinned actionlint and shellcheck tools. Commit the changes before handing them to the PR commands. When changes affect conditions or filters, verify actual job selection on both a PR and a push to main as described in the workflow validation guidance; local lint alone does not establish selection behavior.

Invoke these as separate Cursor commands, not shell commands:

1. Run `/workflow-audit` to obtain findings and proposed diffs.
2. After applying, validating, and committing the selected fixes, use [`/pr-ready`](pr-ready.md) to push the branch and ensure a PR exists:

   ```text
   /pr-ready "ci: workflow hygiene fixes"
   ```

3. Use [`/pr`](pr.md) to set the PR title and description from the implemented changes:

   ```text
   /pr "ci: workflow hygiene fixes" "<problem, resulting behavior, and validation>"
   ```

4. Use [`/pr-checks`](pr-checks.md) to watch checks until completion. For a status snapshot instead of watching, use [`/ci-latest`](ci-latest.md).
