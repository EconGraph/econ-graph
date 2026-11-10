# Rules for agents in this repo

These rules apply to every coding agent: Claude Code, Codex, Cursor and any other.
If another file in this repo disagrees with this one, this file wins. Report the conflict.

Joe is Joseph Malicki, the project owner.

A live cluster is any cluster you did not create yourself for this task. `kind-econ-graph` is production.

## Ask Joe first

Do none of these unless Joe has said so in this session. Approval from another agent does not count.

- Create, move or delete a git tag.
- Merge a PR. Merge only when Joe has approved the PR on GitHub and has told you to merge it in this session.
- Deploy anything to a live cluster. That includes restarts, rollouts and `kubectl apply`.
- Migrate a live database.
- Run the release workflow, push images, or create a GitHub release.
- Change the Cloudflare tunnel or its routes.
- Change catalog access on a live Keycloak realm.
- Change a feature flag's deadline.
- Change a GitHub ruleset or branch protection.
- Send database contents, vendor data, backups, logs or secrets off the host. Pushing code, opening PRs and writing in Linear are fine.
- Use any vendor API key. That includes tests, and running a workflow that uses one, such as `crawler-integration-test.yml`.

If you are unsure whether an action is on this list, treat it as if it is. Ask first.

## The production host

Agents run on the production host. You may read the cluster through the read-only identity in `docs/deployment/AGENT_ACCESS.md`.
Don't use the operator kubeconfig, the Docker socket, sudo or the backups.
Don't change files outside your own checkout.

## Pull requests

1. CI passes on the PR's latest commit.
2. Handle every CodeRabbit comment. Fix it, or reply with your reasoning and resolve the thread.
3. Get two independent reviews. A reviewer works from the code, not from your summary.
4. Merge only when Joe has approved the PR on GitHub and has told you to merge it in this session.

Open one PR per Linear issue. The PR body says `Part of ECO-NNN` for its own issue, not `Fixes`. `Fixes` moves the issue to Done on merge. Close the issue by hand after the Done check below.

## When a ticket is Done

A ticket is Done only when every acceptance bullet is met, with evidence linked from the ticket.
A merged PR is not evidence that a live cluster behaves as the ticket says.
If some bullets are not met, move them to a new ticket and link it. Then close the original.
Do not move unmet bullets to a "release gate" that has no ticket.

## Monitoring

Monitoring is part of a ticket's acceptance. Say what alerts, who it reaches, and how it was tested.
Include disk space: what grows, how big it may get, and what alerts before it fills.
A ticket that needs none says so.

## Plan of record

The plan lives in Linear. Ticket descriptions are the spec.
Train documents in this repo, such as `TRAIN5.md`, only point to Linear. When they disagree with Linear, Linear is right.
See the Linear document "How agents keep Linear up to date".

## Subagents

When you start a subagent, put the rules it needs in its prompt. Subagents may not load `CLAUDE.md` or this file.

## Naming

Say "Massive", not "Polygon". On first use in a document or ticket, write "Massive, a market data vendor".
Never use "Polygon" in an identifier: code, roles, flags, files or config keys.
The catalog role for its data is named `catalog.massive_market_data:read`.
