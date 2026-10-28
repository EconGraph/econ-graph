# Merge queue

`main` and the `release/*` branches take changes through GitHub's merge queue. A
PR that is approved and green is added to the queue; GitHub merges it onto the
tip of the branch (after the PRs ahead of it) on a temporary
`gh-readonly-queue/...` branch, runs the required checks on that merge group,
and merges the PR only when they pass. So the branch only ever gets commits
whose combined result was tested, not just each PR on its own base.

## Required checks

Branch protection requires one check per required workflow. Each is a job that
always reports, so a skipped job or a path-filtered workflow can neither block
a PR nor pass it wrongly:

| Check | Workflow | Required on | Passes when |
| --- | --- | --- | --- |
| `Core CI Result` | [ci-core.yml](../../.github/workflows/ci-core.yml) | `main`, `release/*` | No job failed or was cancelled. On a PR, jobs skipped by path selection (all of them on a docs-only PR) pass; in a merge group every job of the ordinary suite must have succeeded |
| `Release E2E Result` | [release-e2e.yml](../../.github/workflows/release-e2e.yml) | `main`, `release/*` | The suite passed, or a PR touched none of its paths; in a merge group the suite must have run and passed |
| `Markdown lint and links` | [docs-checks.yml](../../.github/workflows/docs-checks.yml) | `main`, `release/*` | Markdown lint and relative links pass (every PR and merge group) |
| `Feature Flags Check` | [feature-flags.yml](../../.github/workflows/feature-flags.yml) | `main`, `release/*` | Flag validation passes (every PR and merge group) |
| `Lint workflows` | `.github/workflows/actionlint.yml` | `main` | actionlint, action pins and the result-job gate pass (every PR and merge group). `release/v4.0` predates this workflow, so the release rule leaves it out |

Each of these workflows has a `merge_group:` trigger. Merge groups are never
path-filtered: Core CI runs its whole ordinary suite (as on a push to `main`,
plus Admin Frontend Tests) and Release E2E always runs. The result jobs use
[check-required-jobs.sh](../../ci/scripts/check-required-jobs.sh), tested by
[test-check-required-jobs.sh](../../ci/scripts/test-check-required-jobs.sh)
(in Lint workflows on `main`, in Quality Checks on `release/v4.0`).

A new required workflow needs the same three things: a `merge_group:` trigger,
no trigger-level path filter on `pull_request`, and a result job with
`if: always()` that needs every other job. Add its check to the rulesets below.

## Landing a PR

1. Open the PR, get CI green and the review approved, and resolve the
   CodeRabbit threads.
2. Add it to the queue: **Merge when ready** on the PR page, or
   `gh pr merge <number>` (no merge method is needed: the queue's is used).
   With the required checks passed, that adds the PR to the queue; before
   they pass, it turns on auto-merge, which adds the PR once they do. This
   works whether or not the repository allows auto-merge, so
   `allow_auto_merge` can stay off. The squash commit takes the PR title and
   description (the repository's squash-merge setting), so write them as the
   commit message.
3. Wait. The PR shows its queue position and the merge-group run. When the
   required checks pass on the merge group, GitHub merges the PR.
4. If the merge-group run fails, the PR leaves the queue and the PRs behind it
   are re-tested without it. Read the run (`gh run list --event merge_group`),
   fix the PR, and queue it again.

To take a PR out of the queue: **Remove from queue** on the PR page, or on the
branch's merge queue page (**...** next to the PR). GitHub CLI has no command
for it; the API's GraphQL mutation is `dequeuePullRequest`
(`gh api graphql -f query='mutation { dequeuePullRequest(input: {id: "<PR node id>"}) { clientMutationId } }'`,
with the node id from `gh pr view <number> --json id`). A PR still waiting for
its checks (auto-merge on, not yet queued) is stopped with **Disable
auto-merge**, or `gh pr merge <number> --disable-auto`.

A stacked PR is queued only after it is retargeted onto `main` or the release
branch; the queue only merges into the branches the rulesets name.

Sources: GitHub's [merging a pull request with a merge queue](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/incorporating-changes-from-a-pull-request/merging-a-pull-request-with-a-merge-queue)
and [`gh pr merge`](https://cli.github.com/manual/gh_pr_merge); the mutation is
in the GraphQL schema. Not yet tried on this repository: the rulesets below
are not applied.

### When a new advisory blocks the queue

Security Audit (cargo audit and npm audit) and License Compliance Check run in
every merge group, whatever the PRs change: a merge group, like a push to
`main`, is not path-filtered. So an advisory published today fails every merge
group, and the queue merges nothing, until the advisory is dealt with on the
branch. To unblock it:

1. Open a PR that fixes it: update the dependency (`cargo update -p <crate>`,
   or the npm lockfile entry or an `overrides` pin), or, when no fixed version
   exists, add an entry with a reason and a review date to the audit allowlist
   (`backend/.cargo/audit.toml`, `frontend/scripts/npm-audit-allowlist.json`,
   `admin-frontend/scripts/npm-audit-allowlist.json`).
2. Queue it. Its own merge group passes, since it carries the fix, and the
   PRs queued after it pass once it is ahead of them. Remove other PRs from
   the queue first if they keep failing in front of it.

An organization admin can also merge the fix directly (below), but a reviewed
fix through the queue is the normal way.

## Branch protection settings

These are repository settings; a maintainer applies them. They add rules to
the existing ruleset `main only by pr` (id 8429203) and create one for the
release branches. Both keep the live pull-request rule exactly as it is
(including `require_extra_approval_for_unattributed_changes` and its
`dismissal_restriction`), and add:

- `deletion`: the branch can't be deleted;
- the required checks, all from GitHub Actions (integration id 15368);
- the merge queue, with squash merges.

Both keep the live `bypass_actors`: organization admins, always. An admin can
therefore bypass the queue, merging a PR directly ("Merge without waiting for
requirements to be met"), or `gh pr merge --admin`, and push to the branch
without the queue's checks. Everyone else lands through the queue.

The commands read the live ruleset with `gh api` and add only the new rules
with `jq`, so nothing the live ruleset sets is lost; running them again
replaces the added rules rather than duplicating them. Check the output file
before the `PUT` or `POST`. They need `gh` and `jq`.

### main

Save the new rules as `main-new-rules.json`:

```json
[
  { "type": "deletion" },
  {
    "type": "required_status_checks",
    "parameters": {
      "strict_required_status_checks_policy": false,
      "do_not_enforce_on_create": false,
      "required_status_checks": [
        { "context": "Core CI Result", "integration_id": 15368 },
        { "context": "Release E2E Result", "integration_id": 15368 },
        { "context": "Markdown lint and links", "integration_id": 15368 },
        { "context": "Feature Flags Check", "integration_id": 15368 },
        { "context": "Lint workflows", "integration_id": 15368 }
      ]
    }
  },
  {
    "type": "merge_queue",
    "parameters": {
      "merge_method": "SQUASH",
      "grouping_strategy": "ALLGREEN",
      "max_entries_to_build": 5,
      "min_entries_to_merge": 1,
      "max_entries_to_merge": 5,
      "min_entries_to_merge_wait_minutes": 5,
      "check_response_timeout_minutes": 120
    }
  }
]
```

and apply them:

```sh
gh api repos/EconGraph/econ-graph/rulesets/8429203 |
  jq --slurpfile new main-new-rules.json '{name, target, enforcement, conditions, bypass_actors,
      rules: ([.rules[] | select(.type as $t | [$new[0][].type] | index($t) | not)] + $new[0])}' \
  >main-ruleset.json
gh api -X PUT repos/EconGraph/econ-graph/rulesets/8429203 --input main-ruleset.json
```

### release/*

The release ruleset copies the main ruleset's live `non_fast_forward` and
`pull_request` rules and `bypass_actors`, for `refs/heads/release/*`. Its
required checks leave out `Lint workflows`: `release/v4.0` has no Actionlint
workflow, and a required check that no workflow reports keeps every PR
waiting. Save its new rules as `release-new-rules.json` (the main rules
without that check):

```sh
jq '(.[] | select(.type == "required_status_checks") | .parameters.required_status_checks) |=
      map(select(.context != "Lint workflows"))' main-new-rules.json >release-new-rules.json
```

and create it:

```sh
gh api repos/EconGraph/econ-graph/rulesets/8429203 |
  jq --slurpfile new release-new-rules.json '{name: "release branches by pr", target, enforcement,
      bypass_actors, conditions: {ref_name: {include: ["refs/heads/release/*"], exclude: []}},
      rules: ([.rules[] | select(.type == "non_fast_forward" or .type == "pull_request")] + $new[0])}' \
  >release-ruleset.json
gh api -X POST repos/EconGraph/econ-graph/rulesets --input release-ruleset.json
```

Apply each ruleset only once the branches it covers have these workflows: the
queue runs the workflows of the branch it merges into.

### Notes on the values

- `strict_required_status_checks_policy: false`: the queue tests each PR on the
  current tip anyway, so "branch must be up to date" would only add rebases.
- `grouping_strategy: ALLGREEN`: every group must pass; a failing PR is removed
  and the ones behind it are re-tested without it.
- `max_entries_to_build: 5`: up to five merge groups run at once; each runs the
  whole suite (about 20 minutes for Core CI and 10 for Release E2E).
- `check_response_timeout_minutes: 120`: a group whose checks haven't all
  reported after two hours fails, so a stuck runner can't stall the queue.
- `merge_method: SQUASH`: with the repository's squash setting (PR title and
  description), the queued commit message is the PR's.
