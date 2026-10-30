# Release process

How a release goes from `main` to a tag. What each release contains is tracked in
[Linear](https://linear.app/econgraph/project/releases-and-release-engineering-ee56b60d9c7a).

## Branches

- **`main`** holds all development until a release reaches QA, and is open to experimental work
  and later releases after that.
- **`release/vX.Y`** (for example `release/v4.0`) is cut from `main` when a release reaches QA. The
  QA cycle runs on it, and the release tag is made on it. Patch releases (`vX.Y.1`, ...) come
  from the same branch.

## Cutting the branch

Cut when all of the release's items are merged on `main` and CI is green. For `v4.0.0` these are
the "Before QA starts" conditions in the [v4.0.0 QA plan](../release/v4.0.0-qa.md).

```sh
git fetch origin main
git push origin origin/main:refs/heads/release/v4.0
```

After the cut, `main` can take work for later releases, including flag and experimental
changes the release must not ship.

## During QA

- Release candidate images are built from the release branch by running the release image
  workflow by hand (`workflow_dispatch`) or by pushing a `vX.Y.0-rc.N` tag, tagged with the
  commit SHA. QA deploys those images.
- Core CI and the `Release E2E` workflow run on pull requests into any branch, including
  `release/**`, so fixes into the release branch get the same checks as `main`.
- Only fixes for QA findings go into the release branch. No new features go in. A source that fails QA is
  turned off rather than fixed on the branch (see the QA plan's exit rules).

## Fix flow

The default is **branch first, then forward-port**:

1. The fix is a PR into `release/vX.Y`. It follows the normal PR process (draft, agent review,
   CodeRabbit for code, CI green).
2. After it merges, the same thread opens a forward-port PR into `main` with
   `git cherry-pick -x <sha>`, adapting it if `main` has moved.
3. The release isn't tagged while any fix merged to the branch lacks a forward-port PR, open or
   merged.

Branch first keeps QA unblocked when `main` has already diverged. The `-x` trailer and rule 3
make sure no fix is lost on `main`.

## Tagging

Before tagging, the database migrations added since the previous tag are squashed into one
migration for the release, and checked against the chain they replace with
`backend/scripts/compare_migrations.sh` (see [database migrations](./MIGRATIONS.md)). The squash
is a PR into the release branch like any fix, forward-ported to `main`. No squash is needed when
the release added no migrations, or exactly one.

When QA passes and the maintainer agrees, tag the tested commit on the release branch with
`scripts/release/tag.sh`: `vX.Y.0` plus the marker tag `train-N`, which the feature flag
`remove_by` check reads. Before pushing `train-N`
the flags `remove_by` check runs with that tag, so a flag that is due for removal blocks the tag.
The tag workflow reuses the tested image digests instead of rebuilding.

A patch release (`vX.Y.Z` with `Z` greater than 0) is tagged the same way, from the same release
branch, with the train number the branch shipped as (`scripts/release/tag.sh 4.0.1 1`). It needs a
dated `## [X.Y.Z]` CHANGELOG section and the version bumped in `backend/Cargo.toml`,
`frontend/package.json` and `frontend/package-lock.json`, like any release. `tag.sh` refuses it
unless `train-N` already exists on origin, its commit is an ancestor of the tagged commit, and
`vX.Y.0` exists on origin on that same commit, so the train number matches the `X.Y` being
patched. It then pushes only `vX.Y.Z`; `train-N` stays on the train's first release.

## Scoping the next release

Once a release branch is cut, scoping of the next release starts on `main`, and a separate
review agent reviews each scope until it converges.
