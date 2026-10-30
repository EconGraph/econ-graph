#!/bin/bash
# Tags a release after checks that are cheap to run before a push but expensive
# to undo after one (a pushed tag is effectively permanent, see
# docs/development/RELEASE_PROCESS.md).
#
# - A train's first release (X.Y.0) gets the version tag (vX.Y.0) and the train
#   tag (train-N), pushed together.
# - A patch release (X.Y.Z with Z > 0) gets only the version tag. Train N must
#   already have shipped: its tag train-N must exist on origin, and the commit it
#   marks must be an ancestor of HEAD, so the patch is tagged on the branch that
#   train shipped from.
#
# - Working tree must be clean, and HEAD must already be pushed to origin.
# - backend/Cargo.toml's workspace version, frontend/package.json's version and
#   frontend/package-lock.json's version must all match the version given (the
#   frontend shows its package.json version on the About page).
# - CHANGELOG.md must have a non-empty section headed exactly
#   "## [X.Y.Z] - YYYY-MM-DD" for that version (not "Unreleased", and an
#   actual date, not any other trailing text).
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
#   e.g. scripts/release/tag.sh 4.0.0 1   (tags v4.0.0 and train-1)
#        scripts/release/tag.sh 4.0.1 1   (tags v4.0.1; train-1 must exist)

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

if ! git branch -r --contains HEAD 2>/dev/null | grep -q '^  origin/'; then
  echo "tag.sh: HEAD isn't on any origin branch; push it first so the pushed tags and the rc-built images point at a commit everyone else can see." >&2
  exit 1
fi

patch="${version##*.}"
v_tag="v${version}"
train_tag="train-${train}"

if [ "$patch" != 0 ]; then
  echo "tag.sh: ${version} is a patch release; checking that ${train_tag} has shipped and HEAD descends from it..."
  if ! git ls-remote --exit-code --tags origin "refs/tags/${train_tag}" >/dev/null 2>&1; then
    echo "tag.sh: ${version} is a patch release, but ${train_tag} doesn't exist on origin. A train is patched only after its X.Y.0 release is tagged." >&2
    exit 1
  fi
  if ! train_commit="$(git rev-parse --verify --quiet "refs/tags/${train_tag}^{commit}")"; then
    echo "tag.sh: ${train_tag} exists on origin but not locally after fetching tags." >&2
    exit 1
  fi
  if ! git merge-base --is-ancestor "$train_commit" HEAD; then
    echo "tag.sh: ${train_tag} (${train_commit}) is not an ancestor of HEAD; tag a patch for train ${train} on the release branch it shipped from." >&2
    exit 1
  fi
fi

cargo_version="$(grep -m1 '^version' backend/Cargo.toml | sed -E 's/version[[:space:]]*=[[:space:]]*"([^"]+)"/\1/')"
if [ "$cargo_version" != "$version" ]; then
  echo "tag.sh: backend/Cargo.toml's workspace version is ${cargo_version}, not ${version}. Bump it (and frontend/package.json) first." >&2
  exit 1
fi

for manifest in frontend/package.json frontend/package-lock.json; do
  manifest_version="$(node -p "require('./${manifest}').version")"
  if [ "$manifest_version" != "$version" ]; then
    echo "tag.sh: ${manifest}'s version is ${manifest_version}, not ${version}. Bump it with \`npm version --no-git-tag-version ${version}\` in frontend/ first." >&2
    exit 1
  fi
done

echo "tag.sh: checking CHANGELOG.md has a dated section for ${version}..."
version_re="${version//./\\.}"
# Accept the heading with its brackets either plain ("## [X.Y.Z] - ...") or
# backslash-escaped ("## \[X.Y.Z\] - ..."), matching changelog-section.sh.
changelog_heading="$(grep -m1 -E "^## \\\\?\\[${version_re}\\\\?\\]" CHANGELOG.md || true)"
if [ -z "$changelog_heading" ]; then
  echo "tag.sh: CHANGELOG.md has no '## [${version}]' section." >&2
  exit 1
fi
if ! [[ "$changelog_heading" =~ ^\#\#\ \\?\[${version_re}\\?\]\ -\ [0-9]{4}-[0-9]{2}-[0-9]{2}$ ]]; then
  echo "tag.sh: CHANGELOG.md's ${version} heading must be '## [${version}] - YYYY-MM-DD' (got: '${changelog_heading}')." >&2
  exit 1
fi
# The heading regex above only checked the heading line; make sure the body
# isn't empty too.
"${root}/scripts/release/changelog-section.sh" "$version" >/dev/null

echo "tag.sh: checking config/flags for flags due for removal by train-${train}..."
node scripts/check-flags --shipped-train "$train"

if [ "$patch" = 0 ]; then
  new_tags=("$v_tag" "$train_tag")
else
  new_tags=("$v_tag")
fi

for t in "${new_tags[@]}"; do
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
if [ "$patch" = 0 ]; then
  git tag -a "$train_tag" -m "Train ${train} shipped as ${v_tag}"
  echo "tag.sh: pushing ${v_tag} and ${train_tag} together..."
else
  echo "tag.sh: pushing ${v_tag}, a patch release of train ${train} (${train_tag} is unchanged)..."
fi
if ! git push --atomic origin "${new_tags[@]}"; then
  echo "tag.sh: push failed; deleting the local tags so a retry starts clean." >&2
  git tag -d "${new_tags[@]}" >/dev/null
  exit 1
fi

echo "tag.sh: done. The Release Images workflow will retag the QA'd images as ${v_tag} and draft the GitHub release."
