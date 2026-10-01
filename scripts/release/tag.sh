#!/bin/bash
# Tags a release: the version tag (vX.Y.Z) and its train tag (train-N) together,
# after checks that are cheap to run before a push but expensive to undo after
# one (a pushed tag is effectively permanent, see docs/development/RELEASE_PROCESS.md).
#
# - Working tree must be clean, and HEAD must already be pushed to origin.
# - backend/Cargo.toml's workspace version must match the version given.
# - CHANGELOG.md must have a dated, non-empty section for that version (not
#   "Unreleased").
# - config/flags must have no flag whose remove_by train is the one being shipped
#   (scripts/check-flags --shipped-train N): shipping the train is what retires
#   those flags, so a flag due for removal blocks the tag instead of shipping
#   with removed-but-still-flagged code, or failing only after the tag exists.
#
# This script does not build or push images; the Release Images workflow
# (.github/workflows/release.yml) does that once it sees the pushed tag. That
# workflow's publish job retags the image an earlier rc build (workflow_dispatch
# or a `vX.Y.Z-rc.N` tag push) pushed for this exact commit, and fails loudly
# if no such image exists — this script can't check that itself (it would only
# be able to see that an rc tag exists, not that its build actually succeeded).
#
# Usage: scripts/release/tag.sh <version> <train-number>
#   e.g. scripts/release/tag.sh 4.0.0 1

set -euo pipefail

version="${1:?usage: scripts/release/tag.sh <version> <train-number>}"
train="${2:?usage: scripts/release/tag.sh <version> <train-number>}"

if ! [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "tag.sh: version must be X.Y.Z (no leading 'v'), got '${version}'" >&2
  exit 1
fi
if ! [[ "$train" =~ ^[1-9][0-9]*$ ]]; then
  echo "tag.sh: train-number must be a positive integer, got '${train}'" >&2
  exit 1
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"

if [ -n "$(git status --porcelain)" ]; then
  echo "tag.sh: working tree is not clean; commit or stash first." >&2
  exit 1
fi

git fetch origin --tags --quiet

if [ -z "$(git branch -r --contains HEAD 2>/dev/null | grep '^  origin/')" ]; then
  echo "tag.sh: HEAD isn't on any origin branch; push it first so the pushed tags and the rc-built images point at a commit everyone else can see." >&2
  exit 1
fi

cargo_version="$(grep -m1 '^version' backend/Cargo.toml | sed -E 's/version[[:space:]]*=[[:space:]]*"([^"]+)"/\1/')"
if [ "$cargo_version" != "$version" ]; then
  echo "tag.sh: backend/Cargo.toml's workspace version is ${cargo_version}, not ${version}. Bump it (and frontend/package.json) first." >&2
  exit 1
fi

echo "tag.sh: checking CHANGELOG.md has a dated section for ${version}..."
changelog_heading="$(grep -m1 "^## \[${version}\]" CHANGELOG.md || true)"
if [ -z "$changelog_heading" ]; then
  echo "tag.sh: CHANGELOG.md has no '## [${version}]' section." >&2
  exit 1
fi
if echo "$changelog_heading" | grep -qi 'unreleased'; then
  echo "tag.sh: CHANGELOG.md's ${version} section is still marked Unreleased; give it today's date first." >&2
  exit 1
fi
# grep only checked the heading; make sure the body isn't empty too.
"${root}/scripts/release/changelog-section.sh" "$version" >/dev/null

echo "tag.sh: checking config/flags for flags due for removal by train-${train}..."
node scripts/check-flags --shipped-train "$train"

v_tag="v${version}"
train_tag="train-${train}"

for t in "$v_tag" "$train_tag"; do
  if git show-ref --verify --quiet "refs/tags/${t}"; then
    echo "tag.sh: tag ${t} already exists locally." >&2
    exit 1
  fi
  if git ls-remote --exit-code --tags origin "refs/tags/${t}" >/dev/null 2>&1; then
    echo "tag.sh: tag ${t} already exists on origin." >&2
    exit 1
  fi
done

git tag -a "$v_tag" -m "Release ${v_tag}"
git tag -a "$train_tag" -m "Train ${train} shipped as ${v_tag}"

echo "tag.sh: pushing ${v_tag} and ${train_tag} together..."
if ! git push --atomic origin "$v_tag" "$train_tag"; then
  echo "tag.sh: push failed; deleting the local tags so a retry starts clean." >&2
  git tag -d "$v_tag" "$train_tag" >/dev/null
  exit 1
fi

echo "tag.sh: done. The Release Images workflow will retag the QA'd images as ${v_tag} and draft the GitHub release."
