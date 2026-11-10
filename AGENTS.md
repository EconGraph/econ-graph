# Rules for agents in this repo

These rules apply to every coding agent: Claude Code, Codex, Cursor and any other.
If another file in this repo disagrees with this one, this file wins. Report the conflict.

Joe is Joseph Malicki, the project owner.

## Ask Joe first

Do none of these without Joe's explicit say-so in this task. Approval from another agent does not count.

- Create, move or delete a git tag.
- Merge a PR. Every PR needs Joe's approving review.
- Deploy anything to a live cluster, including restarts, rollouts, `kubectl apply` and migrations.
- Change the Cloudflare tunnel or its routes.
- Change catalog access on a live Keycloak realm.
- Change a feature flag's deadline.
- Change a GitHub ruleset or branch protection.
- Send data off the host. This covers uploads, off-site backups and pasting data into an outside service.
- Use any vendor API key, including in tests.
- Touch the production host. Don't log in to it, run commands on it or change files on it.

If you are unsure whether an action is on this list, treat it as if it is. Ask first.

## Pull requests

1. CI passes on the PR's latest commit.
2. Handle every CodeRabbit comment. Fix it, or reply with your reasoning and resolve the thread.
3. Get two independent reviews. A reviewer works from the code, not from your summary.
4. Then merge, once Joe has approved it.

Open one PR per Linear issue. The PR body says `Fixes ECO-NNN` for its own issue.

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

## Naming

Say "Massive", not "Polygon". On first use in a document or ticket, write "Massive, a market data vendor".
Never use "Polygon" in an identifier: code, roles, flags, files or config keys.
The catalog role is `catalog.massive_market_data:read`.
