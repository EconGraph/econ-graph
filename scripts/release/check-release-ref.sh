#!/usr/bin/env bash
#
# Refuse to release a commit that is not on a reviewed branch.
#
# Usage: scripts/release/check-release-ref.sh COMMIT [REMOTE]
#
# Succeeds, and prints the branch, when COMMIT is reachable from
# REMOTE/main or a REMOTE/release/<name> branch, <name> without a slash
# (REMOTE defaults to origin). Both
# are protected by rulesets that require a reviewed pull request
# (scripts/github/apply-rulesets.sh), so any commit on them was reviewed.
# Anything else fails: a commit only on a feature branch, a commit no branch
# contains, or a ref that is not a commit.
#
# It reads the remote-tracking refs that are already in the repository and
# does not fetch. The caller fetches full history first (release.yml checks out
# with fetch-depth: 0), and the script refuses a shallow clone or one without
# REMOTE/main, where a missing ancestor would look the same as an unreviewed
# commit.

set -euo pipefail

die() {
    echo "check-release-ref: $*" >&2
    exit 1
}

[ $# -ge 1 ] && [ $# -le 2 ] || die "usage: $0 COMMIT [REMOTE]"
target="$1"
remote="${2:-origin}"

[ "$(git rev-parse --is-shallow-repository)" = false ] ||
    die "the clone is shallow; fetch full history (fetch-depth: 0) so ancestry can be checked"
git rev-parse --verify --quiet "refs/remotes/${remote}/main^{commit}" >/dev/null ||
    die "refs/remotes/${remote}/main is missing; fetch every branch of ${remote} first"

commit="$(git rev-parse --verify --quiet "${target}^{commit}")" ||
    die "'${target}' is not a commit in this repository"

# Only main and release/<name>: not release-foo, mainline, or release/a/b,
# which the release ruleset's refs/heads/release/* does not cover (GitHub's
# `*` stops at a slash).
branches="$(git for-each-ref --format='%(refname)' "refs/remotes/${remote}/main" "refs/remotes/${remote}/release/")"
while IFS= read -r ref; do
    case "$ref" in
        "refs/remotes/${remote}/release/"*/*) continue ;;
        "refs/remotes/${remote}/main" | "refs/remotes/${remote}/release/"?*) ;;
        *) continue ;;
    esac
    if git merge-base --is-ancestor "$commit" "$ref"; then
        echo "check-release-ref: ${commit} is on ${ref#refs/remotes/}"
        exit 0
    fi
done <<<"$branches"

die "${commit} ('${target}') is not on ${remote}/main or any ${remote}/release/* branch; only reviewed commits are released (merge it first)"
