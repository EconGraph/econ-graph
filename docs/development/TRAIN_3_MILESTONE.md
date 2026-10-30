# Train 3 milestone acceptance

Train 3 establishes shared development contracts for the platform/storage,
company and workspace lanes. Its implementation can be reviewed and tested on
a feature branch, but that does not complete the milestone. The acceptance and
tag steps below happen only after the implementation is merged to main and a
maintainer authorizes publication. See the [Train 3 specification](https://linear.app/econgraph/document/train-3-specification-parallel-development-foundations-5b8127e1adb3).

## Collect acceptance evidence

Run the store/API/auth, durable-reference, migration and split-plane compatibility
checks against the exact candidate main commit. Prove three independent scratch
lanes build and test that same commit. The platform and company lanes must
demonstrate branch-local data writes without altering primary or other-branch
data; the workspace lane uses its own app database and identity state. Include
the real isolation run and bounded fixture/capacity/cleanup evidence. Passing
renderer unit tests alone does not establish isolation.

Store a version 1 JSON acceptance manifest outside the checkout. Every result
records the exact full commit SHA, `"result": "passed"` and an HTTPS evidence
URL. Evidence should be retained CI runs or durable audit artifacts with the
commands, fixture checksums, migration versions, baseline commit and assertions,
not just a statement that testing happened. Each work package also identifies
its merged implementation pull request. One PR may implement several packages.

```json
{
  "format_version": 1,
  "commit": "FULL_CANDIDATE_SHA",
  "url": "https://github.com/EconGraph/econ-graph/issues/ACCEPTANCE_RECORD",
  "packages": {
    "P3-1": {"commit": "FULL_CANDIDATE_SHA", "result": "passed", "url": "https://example.invalid/store-evidence", "pull_request": 123},
    "P3-2": {"commit": "FULL_CANDIDATE_SHA", "result": "passed", "url": "https://example.invalid/api-evidence", "pull_request": 123},
    "P3-3": {"commit": "FULL_CANDIDATE_SHA", "result": "passed", "url": "https://example.invalid/reference-evidence", "pull_request": 123},
    "P3-4": {"commit": "FULL_CANDIDATE_SHA", "result": "passed", "url": "https://example.invalid/migration-evidence", "pull_request": 123},
    "P3-5": {"commit": "FULL_CANDIDATE_SHA", "result": "passed", "url": "https://example.invalid/compatibility-evidence", "pull_request": 123},
    "P3-6": {"commit": "FULL_CANDIDATE_SHA", "result": "passed", "url": "https://example.invalid/isolation-evidence", "pull_request": 123}
  },
  "lanes": {
    "platform": {"commit": "FULL_CANDIDATE_SHA", "result": "passed", "url": "https://example.invalid/platform-build"},
    "company": {"commit": "FULL_CANDIDATE_SHA", "result": "passed", "url": "https://example.invalid/company-build"},
    "workspace": {"commit": "FULL_CANDIDATE_SHA", "result": "passed", "url": "https://example.invalid/workspace-build"}
  }
}
```

Replace all placeholders with real evidence. The helper validates manifest
structure and commit identity; the maintainer must inspect the linked artifacts
for the semantic acceptance criteria. It does not infer a successful isolation
run from an arbitrary URL or a green unrelated CI job.

## Validate and publish

Use a clean checkout at the fetched origin/main tip, with Git, Python 3, Node
and an authenticated GitHub CLI available. From the repository root:

```sh
python3 scripts/release/tag_milestone.py --evidence /path/to/acceptance.json --dry-run
```

The default is also a dry run. The helper fetches origin/main, requires HEAD to
equal that candidate, and refuses any existing local or remote `train-3` tag.
It binds GitHub API calls to the effective origin repository and github.com,
ignoring `GH_REPO`/`GH_HOST` overrides, and refuses differing or multiple push
destinations. It checks GitHub check runs and the latest status for each context
on that exact SHA. Every check in the checked-in
[main required-check policy](../../scripts/github/merge-queue-rules.json), plus
`Split-plane compatibility`, must be present and successful from its configured
integration. Missing workflows cannot pass by omission. Additional conditional
neutral/skipped checks are allowed; no observed check may be pending/failing.
GitHub API failures fail closed. Dispatch the applicable workflows on the exact
main candidate when they do not run on main pushes automatically.

Every referenced implementation PR must be merged into the candidate's ancestry,
have no outstanding required reviews or unresolved review threads, and have a
completed review of its final head. The helper refuses PRs exceeding its 100-review
or 100-thread audit window rather than silently skipping additional pages. A
larger audit requires extending the verifier with pagination before publication.
These GitHub checks complement the maintainer's acceptance review; they do not
verify the contents of arbitrary evidence URLs.

Before publication it runs `node scripts/check-flags --shipped-train 3` and
`node scripts/flags-merge --check`. The ordinary Feature Flags workflow also runs
the flag checker, checker fixtures, merge fixtures and generated-output check.
[The deadline audit](../../config/flags/README.md#train-3-deadline-audit) moves
MCP to Train 7 and global analysis tabs to Train 8, retains company financial
components at Train 5 and leaves the unscheduled flags unchanged. Production
flag values do not change. `train-3` counts as a completed train for expiry even
though it ships no product.

After acceptance and explicit publication authorization, repeat with `--push`:

```sh
python3 scripts/release/tag_milestone.py --evidence /path/to/acceptance.json --push
```

This creates one annotated `train-3` tag containing the candidate commit and
evidence URLs, then pushes only that tag without force. It never replaces an
existing tag. A push failure retains the local annotation for investigation;
inspect the remote before taking any recovery action. Do not force or replace
a milestone tag.

The helper creates no `v*` tag, product version change, changelog heading,
GitHub release, image publication/promotion, deployment, migration squash or
workstream branch. The existing product and patch tag helper remains separate.
The `train-*` tag triggers Feature Flags; product image publication continues
to listen to `v*` tags.

## Cut independent workstreams after the milestone

Only after the immutable tag is published and verified, cut the Train 4
platform/storage, Train 5 company and Train 7 workspace branches from
`train-3^{commit}`. Verify all three tips equal that baseline before work begins,
and record the tag and branch evidence on ECO-795. Train 6 builds on Train 5.
Shared prerequisite changes still land through main and are integrated into
each lane. Product-release gates later validate company/workspace behavior on
the completed Train 4 stack; they do not block earlier independent development.

Train 3 does not renumber products: Train 4 is v4.2.0, Train 5 v4.3.0, Train 6
v4.4.0, Train 7 v4.5.0, Train 8 v4.6.0 and Train 9 v4.7.0.
