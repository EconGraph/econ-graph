# Release process

How a release train goes from `main` to a tag. This process was decided by Joe on 2026-09-27. The trains
and what each contains are in [release trains](../roadmap/releases.md).

## Branches

- **`main`** holds all development until a train reaches QA, and is open to experimental work and
  the next trains after that.
- **`release/vX.Y`** (for example `release/v4.0`) is cut from `main` when a train reaches QA. The
  QA cycle runs on it, and the release tag is made on it. Patch releases (`vX.Y.1`, ...) come
  from the same branch.

## Cutting the branch

Cut when all of the train's items are merged on `main`, CI is green, and the release image
workflow (version bump, changelog, image builds; release plan item 19, REL-5, not started as of
2026-09-27) is merged. For train 1 these are the "Before QA starts"
conditions in the [v4.0.0 QA plan](../release/v4.0.0-qa.md).

```sh
git fetch origin main
git push origin origin/main:refs/heads/release/v4.0
```

After the cut, `main` can take work for the next train, including flag and experimental
changes the release must not ship.

## During QA

- Release candidate images are built from the release branch by running the release image
  workflow by hand (`workflow_dispatch`) or by pushing a `vX.Y.0-rc.N` tag, tagged with the
  commit SHA. QA deploys those images.
- Core CI and the `Release E2E` workflow must run on pull requests into `release/**` as well as
  `main`. This is a precondition in the QA plan: it holds once PR #242 (Core CI without a
  branch filter) and PR #218 (the `Release E2E` workflow) have merged.
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

When QA passes and Joe agrees, tag the tested commit on the release branch, `vX.Y.0` plus the
train tag (`train-N`, which the feature flag `remove_by` check reads). Before pushing `train-N`
the flags `remove_by` check runs with that tag, so a flag that is due for removal blocks the tag.
The tag workflow reuses the tested image digests instead of rebuilding.

## Next train

Once a release branch is cut, the area threads start scoping the next release cut on `main`, and
a separate Fable review agent reviews each scope until it converges.
