#!/usr/bin/env bash
# Tests for the safety checks in scripts/backup/branch-clone.sh. Needs no
# btrfs, Docker or root: every case must be refused before anything is
# touched, and the test checks that nothing was. (The successful create and
# delete paths need real btrfs subvolumes; see BACKUPS.md, "Branch clones".)
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
script="${repo_root}/scripts/backup/branch-clone.sh"
failures=0

tmp="$(mktemp -d)"
trap 'rm -rf "${tmp}"' EXIT

# BRANCH_CLONE_DIR one level under a directory holding data that must
# survive, like a subvolume root.
mkdir -p "${tmp}/root/precious" "${tmp}/root/branches" "${tmp}/config" "${tmp}/snapshots"
echo keep >"${tmp}/root/precious/file"
branches="${tmp}/root/branches"
mkdir "${branches}/unmarked" "${branches}/marked" "${branches}/.hidden"
echo keep >"${branches}/unmarked/file"
echo keep >"${branches}/marked/file"
echo "${tmp}/snapshots/data-plane.20261024T040000Z" >"${branches}/marked.source"
ln -s "${tmp}/root" "${branches}/escape"
ln -s "${tmp}/root/precious" "${branches}/escape2"
echo "x" >"${branches}/escape2.source"

export ECONGRAPH_CONFIG_DIR="${tmp}/config"
export SNAPSHOT_DIR="${tmp}/snapshots"
export BRANCH_CLONE_DIR="${branches}"

# expect <exit code> <text the output must contain> <args...>
expect() {
    local want="$1" text="$2" output status=0
    shift 2
    output="$("${script}" "$@" </dev/null 2>&1)" || status=$?
    if [[ "${status}" -ne "${want}" ]] || ! grep -qF -- "${text}" <<<"${output}"; then
        echo "FAIL branch-clone.sh $*: want exit ${want} and output containing '${text}', got exit ${status}:"
        echo "${output}"
        failures=$((failures + 1))
    else
        echo "ok   branch-clone.sh $*"
    fi
}

expect 1 "starts with '.'" delete ..
expect 1 "starts with '.'" --dry-run delete ..
expect 1 "starts with '.'" --yes delete ..
expect 1 "starts with '.'" delete .
expect 1 "starts with '.'" delete .hidden
expect 1 "contains '/'" delete ../precious
expect 1 "contains '/'" delete unmarked/file
expect 1 "empty clone name" delete ""
expect 1 "no clone named" delete missing
expect 1 "is a symlink" --yes delete escape
expect 1 "is a symlink" --yes delete escape2
expect 1 ".source is missing" --yes delete unmarked
expect 1 "not a btrfs subvolume" --yes delete marked
expect 1 "not a btrfs subvolume" --dry-run delete marked
expect 1 "usage:" delete
expect 1 "usage:" delete a b
expect 1 "usage:" --force delete marked
expect 1 "starts with '.'" --dry-run create ..
expect 1 "no data-plane snapshots" --dry-run create my-branch
expect 1 "is not a snapshot" --dry-run create my-branch data-plane.missing
expect 0 "marked  <- " list

for f in "${tmp}/root/precious/file" "${branches}/unmarked/file" "${branches}/marked/file" \
    "${branches}/marked.source"; do
    if [[ ! -f "${f}" ]]; then
        echo "FAIL: ${f} was removed"
        failures=$((failures + 1))
    fi
done
if [[ -e "${branches}/my-branch" ]]; then
    echo "FAIL: --dry-run create made ${branches}/my-branch"
    failures=$((failures + 1))
fi

if [[ "${failures}" -ne 0 ]]; then
    echo "${failures} branch-clone test(s) failed"
    exit 1
fi
echo "all branch-clone tests passed"
