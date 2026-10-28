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
   `gh pr merge <number> --squash` (with the queue on, this enqueues the PR
   rather than merging it). The squash commit takes the PR title and
   description (the repository's squash-merge setting), so write them as the
   commit message.
3. Wait. The PR shows its queue position and the merge-group run. When the
   required checks pass on the merge group, GitHub merges the PR.
4. If the merge-group run fails, the PR leaves the queue and the PRs behind it
   are re-tested without it. Read the run (`gh run list --event merge_group`),
   fix the PR, and queue it again.

To take a PR out of the queue: **Remove from queue** on the PR page, or
`gh pr merge <number> --disable-auto`.

A stacked PR is queued only after it is retargeted onto `main` or the release
branch; the queue only merges into the branches the rulesets name.

## Branch protection settings

These are repository settings; a maintainer applies them. They extend the
existing ruleset `main only by pr` (id 8429203) and add one for the release
branches. Both keep the existing pull-request rule, add the required checks
(all from GitHub Actions, integration id 15368), and turn on the queue with
squash merges.

### main

Save as `main-ruleset.json` and apply with
`gh api -X PUT repos/EconGraph/econ-graph/rulesets/8429203 --input main-ruleset.json`:

```json
{
  "name": "main only by pr",
  "target": "branch",
  "enforcement": "active",
  "conditions": { "ref_name": { "include": ["~DEFAULT_BRANCH"], "exclude": [] } },
  "bypass_actors": [
    { "actor_id": null, "actor_type": "OrganizationAdmin", "bypass_mode": "always" }
  ],
  "rules": [
    { "type": "non_fast_forward" },
    {
      "type": "pull_request",
      "parameters": {
        "required_approving_review_count": 1,
        "dismiss_stale_reviews_on_push": false,
        "required_reviewers": [],
        "require_code_owner_review": false,
        "require_last_push_approval": false,
        "required_review_thread_resolution": false,
        "allowed_merge_methods": ["merge", "squash", "rebase"]
      }
    },
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
}
```

### release/*

Save as `release-ruleset.json` and create it with
`gh api -X POST repos/EconGraph/econ-graph/rulesets --input release-ruleset.json`:

```json
{
  "name": "release branches by pr",
  "target": "branch",
  "enforcement": "active",
  "conditions": { "ref_name": { "include": ["refs/heads/release/*"], "exclude": [] } },
  "bypass_actors": [
    { "actor_id": null, "actor_type": "OrganizationAdmin", "bypass_mode": "always" }
  ],
  "rules": [
    { "type": "non_fast_forward" },
    {
      "type": "pull_request",
      "parameters": {
        "required_approving_review_count": 1,
        "dismiss_stale_reviews_on_push": false,
        "required_reviewers": [],
        "require_code_owner_review": false,
        "require_last_push_approval": false,
        "required_review_thread_resolution": false,
        "allowed_merge_methods": ["merge", "squash", "rebase"]
      }
    },
    {
      "type": "required_status_checks",
      "parameters": {
        "strict_required_status_checks_policy": false,
        "do_not_enforce_on_create": false,
        "required_status_checks": [
          { "context": "Core CI Result", "integration_id": 15368 },
          { "context": "Release E2E Result", "integration_id": 15368 },
          { "context": "Markdown lint and links", "integration_id": 15368 },
          { "context": "Feature Flags Check", "integration_id": 15368 }
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
}
```

Apply each ruleset only once the branches it covers have these workflows: the
queue runs the workflows of the branch it merges into, and a required check
that no workflow there reports keeps every PR waiting.

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
