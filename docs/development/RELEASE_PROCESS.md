# Release process

How a release goes from `main` to a tag. What each release contains is tracked in
[Linear](https://linear.app/econgraph).

## Branches

- **`main`** holds all development until a release reaches QA, and is open to experimental work
  and later releases after that.
- **`release/vX.Y`** (for example `release/v4.0`) is cut from `main` when a release reaches QA. The
  QA cycle runs on it, and the release tag is made on it. Patch releases (`vX.Y.1`, ...) come
  from the same branch.

## Cutting the branch

Cut when all of the release's items are merged on `main` and CI is green. Check the release's
tracker in [Linear](https://linear.app/econgraph) for any further QA entry conditions; the
[v4.0.0 release tracker](https://linear.app/econgraph/issue/ECO-5/release-1-v400-broad-real-data-nothing-broken)
contains the release-specific conditions for that version.

```sh
git fetch origin main
git push origin origin/main:refs/heads/release/v4.0
```

Once the release ruleset's `creation` rule is applied (ECO-896; see [Merge queue](MERGE_QUEUE.md)),
only organization admins can create a `release/<name>` branch, so an admin does this push.
The release workflow and its provenance check trust every `release/<name>` branch as reviewed.

After the cut, `main` can take work for later releases, including flag and experimental
changes the release must not ship.

## During QA

The [Release Images workflow](../../.github/workflows/release.yml) builds release candidate
images for the backend, crawler-worker, frontend and chart-api. Push a `vX.Y.Z-rc.N` Git tag
on the candidate commit, or start it with GitHub's **Run workflow** button
(`workflow_dispatch`) from the release branch: a manual run builds the head of the branch it is
started from and has no input to pick another commit. To build an older commit, push an rc tag
on it. The resulting images receive `sha-<full-commit-SHA>` image tags; an RC Git tag push also
gives the images that RC tag. QA deploys these images and records the tested commit SHA.

The workflow builds only reviewed commits (ECO-896): a manual run must be started from `main`
or a `release/<name>` branch, and the commit must be on `origin/main` or an
`origin/release/<name>` branch. Anything else fails before building. Each image gets a signed
build provenance attestation for its commit, and the deploy side
(`scripts/deploy/load-release-images.sh`) refuses an image without one. Release builds use no
BuildKit cache. See the header of the workflow for the details.

For an rc tag push, the workflow file that runs is the tagged commit's own, so its checks are
defense in depth. The real control is the tag ruleset, which lets only admins push `v*` and
`train-*` tags. It is in the repository but takes effect only once an admin applies it:

1. Before relying on it for a release, run `scripts/github/apply-rulesets.sh --dry-run`
   against GitHub, check the diff, and apply it (see [Merge queue](MERGE_QUEUE.md)).

Release candidates for v4.2.0 and later must be built after ECO-896 merges; images built before
it have no attestation (see [Release images](../deployment/RELEASE_IMAGES.md)).
The QA and deploy hosts need `gh`, logged in, for the provenance check.

[Core CI](../../.github/workflows/ci-core.yml) and
[Release E2E](../../.github/workflows/release-e2e.yml) accept pull requests into any branch,
including `release/**`, subject to their path filters. Only fixes for QA findings go into the
release branch. No new features go in. A source that fails QA is turned off rather than fixed
on the branch; consult the release's Linear tracker for its exit conditions.

## Fix flow

The default is **branch first, then forward-port**:

1. The fix is a PR into `release/vX.Y`. It follows the normal PR process (draft, agent review,
   CodeRabbit for code, CI green).
2. After the fix merges, open a forward-port PR into `main` using
   `git cherry-pick -x <sha>`, adapting the fix if `main` has moved.
3. Before tagging the release, check that every fix merged into the release branch has a
   forward-port PR into `main`. That PR may be open or merged.

Fixing the release branch first keeps QA unblocked when `main` has already diverged. The `-x`
trailer records the source commit, and the tagging rule ensures that each release fix has a
corresponding PR into `main`.

## Tagging

Train 3 is a development milestone, not a product release. Use the separate
[Train 3 milestone procedure](TRAIN_3_MILESTONE.md) after acceptance on main.
It creates only an annotated `train-3` marker; the product-release instructions
below, including version/changelog changes and migration squashing, do not apply
to that milestone.

Before tagging, squash the release's eligible app plane migrations in
`backend/migrations/app` into one migration. A migration is eligible if it has not shipped in
a final release tag and sorts after the newest app plane migration that has shipped. Release
candidate tags do not count as shipped releases. Leave the two baselines and all previously
released app plane migrations unchanged. No squash is needed if the release added zero or one app plane
migration. Data plane migrations in `backend/migrations/data` are never squashed.

[Squashing at a release](./MIGRATIONS.md#squashing-at-a-release) has the command that lists
eligible migrations and explains how to handle unshipped migrations that sort before shipped
ones. Check the squash against the chain it replaces with
`backend/scripts/compare_migrations.sh <commit-before-the-squash>` from the repository root,
using PostgreSQL 18 as described there. Submit the squash as a PR into the release branch and
forward-port it to `main`, following the fix process above. Before forward-porting the squash,
forward-port any release migration that `main` still lacks.

When QA passes and the maintainer agrees, check out the tested commit on the release branch.
Before tagging, confirm that:

- the working tree is clean and the tested commit has been pushed to `origin`;
- the workspace version in `backend/Cargo.toml` and the versions in `frontend/package.json`
  and `frontend/package-lock.json` match the release version;
- `CHANGELOG.md` has a nonempty section headed `## [X.Y.Z] - YYYY-MM-DD` for that version;
- a candidate run of Release Images has successfully published all three images for this
  exact commit, and QA has tested them.

From the repository root, run
[`scripts/release/tag.sh`](../../scripts/release/tag.sh) as
`scripts/release/tag.sh <version> <train-number>`, with the version without a leading `v` and a
positive integer train number. For example, to tag `v4.0.0` with `train-1`:

```sh
scripts/release/tag.sh 4.0.0 1
```

The script checks the Git state, versions and changelog, and runs the feature flag `remove_by`
check for the specified train number. A flag due for removal blocks tagging. It creates the
`vX.Y.Z` and `train-N` Git tags on the checked-out commit and pushes both atomically. The
`train-N` marker records the shipped train for flag expiry checks.

A patch release (`vX.Y.Z` with `Z` greater than 0) is tagged the same way, from the same release
branch, with the train number the branch shipped as. For example, to tag `v4.0.1` on train 1:

```sh
scripts/release/tag.sh 4.0.1 1
```

The script refuses a patch release unless `train-N` already exists on `origin` and its commit is
an ancestor of the checked-out commit, so the patch is on the branch that train shipped from, and
unless `vX.Y.0` exists on `origin` on that same commit, so the train number matches the `X.Y`
being patched. It creates and pushes only the `vX.Y.Z` tag; `train-N` stays on the train's first release.

The script does not check image publication. The final version tag triggers Release Images,
which requires existing `sha-<full-commit-SHA>` images for that commit and fails if any is
missing. It resolves all three digests before applying version image tags, reusing the tested
images rather than rebuilding. It also updates `latest` if this is the newest release and
drafts the GitHub release from the changelog.
