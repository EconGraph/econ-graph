#!/usr/bin/env bash
# The stubs are quoted heredocs and run on their own.
# shellcheck disable=SC2016
#
# Check scripts/release/tag.sh's two paths:
#
# - X.Y.0: tags vX.Y.0 and train-N and pushes both together.
# - X.Y.Z with Z > 0 (a patch release): needs train-N on origin, with its commit
#   an ancestor of HEAD, and tags and pushes only vX.Y.Z.
#
# Also that a patch is refused without train-N on origin (including a train-N
# that exists only locally) or on a branch that doesn't descend from it, and
# that a failed push deletes the local tag.
#
# Runs offline. tag.sh and changelog-section.sh are copied into a scratch git
# repository whose origin is a local bare repository, next to minimal version
# manifests, a CHANGELOG.md and a stub scripts/check-flags. A `git` first on
# PATH records every `git push` and fails or succeeds as asked without pushing;
# every other git command is the real git. HOME and the git config are in the
# scratch directory.
#
# Usage: scripts/release/test-tag.sh

set -uo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
REAL_GIT="$(command -v git)" || { echo "test-tag.sh: git not found" >&2; exit 1; }
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
REPO="$WORK/repo"
ORIGIN="$WORK/origin.git"
LOGS="$WORK/logs"
mkdir -p "$WORK/bin" "$WORK/home" "$LOGS" "$REPO" || exit 1

export HOME="$WORK/home"
export GIT_CONFIG_GLOBAL="$WORK/gitconfig"
export GIT_CONFIG_NOSYSTEM=1
export TEST_LOGS="$LOGS"
export REAL_GIT
cat >"$GIT_CONFIG_GLOBAL" <<'EOF'
[user]
	name = tag.sh test
	email = test@example.invalid
[init]
	defaultBranch = main
[commit]
	gpgsign = false
[tag]
	gpgsign = false
EOF

# git push is recorded and never run; STUB_PUSH_EXIT makes it fail.
cat >"$WORK/bin/git" <<'STUB'
#!/usr/bin/env bash
if [ "${1:-}" = push ]; then
    echo "push ${*:2}" >>"$TEST_LOGS/push.log"
    exit "${STUB_PUSH_EXIT:-0}"
fi
exec "$REAL_GIT" "$@"
STUB
chmod +x "$WORK/bin/git"

failures=0
fail() { echo "FAIL: $*" >&2; failures=$((failures + 1)); }
pass() { echo "ok: $*"; }

# The test's own git: the real one, in the scratch repository.
rgit() { "$REAL_GIT" -C "$REPO" "$@"; }

"$REAL_GIT" init -q --bare "$ORIGIN" || exit 1
rgit init -q || exit 1
rgit remote add origin "$ORIGIN"
mkdir -p "$REPO/scripts/release" "$REPO/scripts/check-flags" "$REPO/backend" "$REPO/frontend"
cp "$PROJECT_ROOT/scripts/release/tag.sh" "$PROJECT_ROOT/scripts/release/changelog-section.sh" "$REPO/scripts/release/"
cat >"$REPO/scripts/check-flags/index.js" <<'EOF'
require('fs').appendFileSync(process.env.TEST_LOGS + '/check-flags.log', process.argv.slice(2).join(' ') + '\n');
EOF

# Write the version manifests and a CHANGELOG.md section for version $1.
set_version() {
    local v="$1"
    printf '[workspace]\nmembers = []\n\n[workspace.package]\nversion = "%s"\n' "$v" >"$REPO/backend/Cargo.toml"
    printf '{\n  "name": "fe",\n  "version": "%s"\n}\n' "$v" >"$REPO/frontend/package.json"
    printf '{\n  "name": "fe",\n  "version": "%s",\n  "lockfileVersion": 3,\n  "packages": { "": { "version": "%s" } }\n}\n' "$v" "$v" >"$REPO/frontend/package-lock.json"
    {
        printf '# Changelog\n\n## [%s] - 2026-10-30\n\n### Fixed\n\n- A fix in %s.\n\n' "$v" "$v"
        [ -f "$REPO/CHANGELOG.md" ] && sed -n '/^## \[/,$p' "$REPO/CHANGELOG.md"
    } >"$REPO/CHANGELOG.md.new"
    mv "$REPO/CHANGELOG.md.new" "$REPO/CHANGELOG.md"
}

commit_all() {
    rgit add -A && rgit commit -q -m "$1"
}

# Run tag.sh from the scratch repository with the stub git first on PATH.
# Output goes to $LOGS/out; push.log is reset first.
run_tag() {
    rm -f "$LOGS/push.log"
    (cd "$REPO" && PATH="$WORK/bin:$PATH" scripts/release/tag.sh "$@") >"$LOGS/out" 2>&1
}

pushes() { cat "$LOGS/push.log" 2>/dev/null; }
has_tag() { rgit show-ref --verify --quiet "refs/tags/$1"; }

# History: base (on main) -> v4.0.0 (train-1) -> v4.0.1 (release branch).
# A side branch from base, which doesn't contain train-1, also has 4.0.1.
printf 'base\n' >"$REPO/README"
set_version 3.9.0
commit_all base
base_commit="$(rgit rev-parse HEAD)"
set_version 4.0.0
commit_all "Release 4.0.0"
rgit push -q origin HEAD:refs/heads/release/v4.0 || exit 1

# --- X.Y.0: both tags, pushed together ---
if run_tag 4.0.0 1; then
    if [ "$(pushes)" = "push --atomic origin v4.0.0 train-1" ]; then
        pass "4.0.0 pushes v4.0.0 and train-1 together"
    else
        fail "4.0.0 pushed '$(pushes)', not 'push --atomic origin v4.0.0 train-1'"
    fi
    { has_tag v4.0.0 && has_tag train-1; } || fail "4.0.0 did not create v4.0.0 and train-1 locally"
    grep -qx -- '--shipped-train 1' "$LOGS/check-flags.log" || fail "4.0.0 did not run check-flags --shipped-train 1"
else
    fail "4.0.0 failed: $(cat "$LOGS/out")"
fi
train1_commit="$(rgit rev-parse 'train-1^{commit}' 2>/dev/null)"

# Put the tags on origin, as the real push would have.
rgit push -q origin v4.0.0 train-1 || exit 1

# --- X.Y.0 again: refused, its tags exist ---
if run_tag 4.0.0 1; then
    fail "4.0.0 was tagged a second time"
elif [ -n "$(pushes)" ]; then
    fail "the refused second 4.0.0 pushed '$(pushes)'"
else
    pass "4.0.0 is refused once v4.0.0 exists"
fi

# --- patch on the release branch: only the version tag ---
set_version 4.0.1
commit_all "Release 4.0.1"
rgit push -q origin HEAD:refs/heads/release/v4.0 || exit 1
release_head="$(rgit rev-parse HEAD)"

# Without the train's tag on origin: train-2 exists nowhere, train-3 only locally.
if run_tag 4.0.1 2; then
    fail "4.0.1 was tagged as a patch of train 2, which has no tag"
elif ! grep -q "train-2 doesn't exist on origin" "$LOGS/out"; then
    fail "4.0.1 for train 2 failed, but not on the missing train-2: $(cat "$LOGS/out")"
else
    pass "a patch of a train with no tag is refused"
fi
has_tag v4.0.1 && fail "a refused patch left a local v4.0.1"

rgit tag -a train-3 -m local-only "$base_commit"
if run_tag 4.0.1 3; then
    fail "4.0.1 was tagged as a patch of train 3, whose tag is only local"
elif ! grep -q "train-3 doesn't exist on origin" "$LOGS/out"; then
    fail "4.0.1 for train 3 failed, but not on train-3 missing from origin: $(cat "$LOGS/out")"
else
    pass "a patch of a train tagged only locally is refused"
fi
rgit tag -d train-3 >/dev/null

# A failed push deletes the local tag and leaves train-1 alone.
if STUB_PUSH_EXIT=1 run_tag 4.0.1 1; then
    fail "4.0.1 succeeded although its push failed"
elif has_tag v4.0.1; then
    fail "a failed push left the local v4.0.1"
elif [ "$(rgit rev-parse 'train-1^{commit}')" != "$train1_commit" ]; then
    fail "a failed patch push changed train-1"
else
    pass "a failed patch push deletes the local v4.0.1"
fi

if run_tag 4.0.1 1; then
    if [ "$(pushes)" = "push --atomic origin v4.0.1" ]; then
        pass "4.0.1 pushes only v4.0.1"
    else
        fail "4.0.1 pushed '$(pushes)', not 'push --atomic origin v4.0.1'"
    fi
    [ "$(rgit rev-parse 'v4.0.1^{commit}' 2>/dev/null)" = "$release_head" ] || fail "v4.0.1 is not on HEAD"
    [ "$(rgit rev-parse 'train-1^{commit}')" = "$train1_commit" ] || fail "4.0.1 moved train-1"
    grep -q "patch release of train 1" "$LOGS/out" || fail "4.0.1 did not say it is a patch release"
else
    fail "4.0.1 failed: $(cat "$LOGS/out")"
fi
rgit tag -d v4.0.1 >/dev/null 2>&1

# --- patch on a branch that doesn't contain train-1: refused ---
rgit checkout -q -b side "$base_commit"
set_version 4.0.1
commit_all "4.0.1 off the wrong base"
rgit push -q origin HEAD:refs/heads/side || exit 1
if run_tag 4.0.1 1; then
    fail "4.0.1 was tagged on a branch that doesn't contain train-1"
elif ! grep -q "is not an ancestor of HEAD" "$LOGS/out"; then
    fail "4.0.1 off train-1 failed, but not on the ancestry: $(cat "$LOGS/out")"
elif [ -n "$(pushes)" ]; then
    fail "4.0.1 off train-1 pushed '$(pushes)'"
else
    pass "a patch on a branch that doesn't contain train-N is refused"
fi
has_tag v4.0.1 && fail "a refused patch left a local v4.0.1"

if [ "$failures" -gt 0 ]; then
    echo "test-tag.sh: ${failures} failure(s)" >&2
    exit 1
fi
echo "test-tag.sh: all checks passed"
