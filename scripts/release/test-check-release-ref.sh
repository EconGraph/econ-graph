#!/usr/bin/env bash
#
# Tests for scripts/release/check-release-ref.sh against a local origin with
# main, release/v4.2, a feature branch and look-alike branch names:
#
#   - a commit on main, an older commit on main, a commit only on a release/*
#     branch, and a tag on a main commit pass;
#   - a commit only on a feature branch, on release-x, mainline or
#     release/nested/x (not release/<name>), a tag on a feature commit, a dangling commit and a name that
#     is not a commit are refused;
#   - a shallow clone and a clone without origin/main are refused;
#   - REMOTE selects another remote.
#
# Runs offline; HOME and the git config are in a scratch directory.
#
# Usage: scripts/release/test-check-release-ref.sh

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
check="${SCRIPT_DIR}/check-release-ref.sh"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
failed=0

export HOME="$WORK/home"
export GIT_CONFIG_GLOBAL="$WORK/gitconfig"
export GIT_CONFIG_NOSYSTEM=1
mkdir -p "$HOME"
cat >"$GIT_CONFIG_GLOBAL" <<'EOF'
[user]
	name = check-release-ref test
	email = test@example.invalid
[init]
	defaultBranch = main
[commit]
	gpgsign = false
[tag]
	gpgsign = false
[protocol "file"]
	allow = always
EOF

ok() { echo "ok   $1"; }
fail() {
    echo "FAIL $1"
    [ -z "${2:-}" ] || printf '     %s\n' "$2"
    failed=1
}

# The origin: main (two commits), release/v4.2 (one commit past main),
# feature/x, release-x and mainline (one commit each past main).
src="$WORK/src"
git init -q "$src"
commit() { git -C "$src" commit -q --allow-empty -m "$1" && git -C "$src" rev-parse HEAD; }
main_old="$(commit "main 1")"
main_head="$(commit "main 2")"
git -C "$src" checkout -q -b release/v4.2
release_only="$(commit "release fix")"
for b in feature/x release-x mainline release/nested/x; do
    git -C "$src" checkout -q -b "$b" main
    commit "$b work" >/dev/null
done
feature_only="$(git -C "$src" rev-parse feature/x)"
release_x="$(git -C "$src" rev-parse release-x)"
mainline="$(git -C "$src" rev-parse mainline)"
nested="$(git -C "$src" rev-parse release/nested/x)"
git -C "$src" checkout -q main
git -C "$src" tag -a v4.2.0-rc.1 -m rc "$main_head"
git -C "$src" tag -a v9.9.9-rc.1 -m rc "$feature_only"
git clone -q --bare "$src" "$WORK/origin.git"

# A full clone, as actions/checkout with fetch-depth: 0 leaves it.
clone="$WORK/clone"
git clone -q "file://$WORK/origin.git" "$clone"
# A commit no branch contains.
git -C "$clone" checkout -q --detach main
dangling="$(git -C "$clone" commit -q --allow-empty -m dangling && git -C "$clone" rev-parse HEAD)"
git -C "$clone" checkout -q main

# run DIR ARGS...: sets $out and $rc.
run() {
    local dir="$1"
    shift
    rc=0
    out="$(cd "$dir" && "$check" "$@" 2>&1)" || rc=$?
}
expect_pass() {
    local name="$1" want="$2"
    shift 2
    run "$clone" "$@"
    if [ "$rc" -eq 0 ] && grep -qF "is on ${want}" <<<"$out"; then ok "$name"; else fail "$name (exit $rc)" "$out"; fi
}
expect_refusal() {
    local name="$1" dir="$2" message="$3"
    shift 3
    run "$dir" "$@"
    if [ "$rc" -ne 0 ] && grep -qF -- "$message" <<<"$out"; then ok "$name"; else fail "$name (exit $rc)" "$out"; fi
}

expect_pass "main's head passes" origin/main "$main_head"
expect_pass "an older main commit passes" origin/main "$main_old"
expect_pass "a commit only on release/v4.2 passes" origin/release/v4.2 "$release_only"
expect_pass "a branch name resolving to a main commit passes" origin/main origin/main
expect_pass "an rc tag on a main commit passes" origin/main v4.2.0-rc.1

not_reviewed="is not on origin/main or any origin/release/* branch"
expect_refusal "a feature-branch commit is refused" "$clone" "$not_reviewed" "$feature_only"
expect_refusal "a feature branch by name is refused" "$clone" "$not_reviewed" origin/feature/x
expect_refusal "release-x is not a release branch" "$clone" "$not_reviewed" "$release_x"
expect_refusal "mainline is not main" "$clone" "$not_reviewed" "$mainline"
expect_refusal "release/nested/x is outside the release ruleset" "$clone" "$not_reviewed" "$nested"
expect_refusal "a tag on a feature commit is refused" "$clone" "$not_reviewed" v9.9.9-rc.1
expect_refusal "a commit on no branch is refused" "$clone" "$not_reviewed" "$dangling"
expect_refusal "a name that is not a commit is refused" "$clone" "is not a commit" no-such-ref
expect_refusal "no arguments is a usage error" "$clone" "usage:"

shallow="$WORK/shallow"
git clone -q --depth 1 --no-single-branch "file://$WORK/origin.git" "$shallow"
expect_refusal "a shallow clone is refused" "$shallow" "the clone is shallow" "$main_head"

no_main="$WORK/no-main"
git clone -q --single-branch --branch feature/x "file://$WORK/origin.git" "$no_main"
expect_refusal "a clone without origin/main is refused" "$no_main" "refs/remotes/origin/main is missing" "$feature_only"

git -C "$clone" remote rename origin upstream
run "$clone" "$release_only" upstream
if [ "$rc" -eq 0 ] && grep -qF "is on upstream/release/v4.2" <<<"$out"; then
    ok "REMOTE selects another remote"
else
    fail "REMOTE (exit $rc)" "$out"
fi
expect_refusal "the default remote is origin" "$clone" "refs/remotes/origin/main is missing" "$main_head"

exit "$failed"
