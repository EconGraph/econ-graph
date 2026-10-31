#!/usr/bin/env bash
# Exercise opt-in store coverage with existing backup stubs, never real host storage.
set -euo pipefail
repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=scripts/backup/test-stubs.sh
source "$repo_root/scripts/backup/test-stubs.sh"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
make_stubs "$scratch/bin"
export PATH="$scratch/bin:$PATH" ECONGRAPH_TEST_ROOT="$scratch/root" FAKE_ROOT=1 STUB_LOG="$scratch/calls"
root="$ECONGRAPH_TEST_ROOT"
: >"$STUB_LOG"
for name in data-plane app-prod objectstore snapshots; do
    mkdir -p "$root/srv/econgraph/$name"
    touch "$root/srv/econgraph/$name/.fake-mount"
done
mkdir -p "$root/etc/econgraph" "$root/run" "$root/var/lib"
bash "$repo_root/scripts/host/backup-snapshots.sh" --dry-run >"$scratch/without"
if grep -q 'snapshot -r .*objectstore' "$scratch/without"; then
    echo 'uninstalled host unexpectedly gained store snapshots' >&2
    exit 1
fi
printf 'objectstore-v1\n' >"$root/etc/econgraph/objectstore-snapshots.enabled"
bash "$repo_root/scripts/host/backup-snapshots.sh" --dry-run >"$scratch/with"
grep -q 'snapshot -r .*objectstore' "$scratch/with"
if grep -q '^btrfs subvolume delete' "$STUB_LOG"; then
    echo 'snapshot coverage attempted deletion' >&2
    exit 1
fi
printf 'foreign\n' >"$root/etc/econgraph/objectstore-snapshots.enabled"
if bash "$repo_root/scripts/host/backup-snapshots.sh" --dry-run >"$scratch/refused" 2>&1; then
    echo 'invalid store marker unexpectedly accepted' >&2
    exit 1
fi
grep -q 'invalid objectstore snapshot coverage marker' "$scratch/refused"
echo 'objectstore snapshot opt-in, no-deletion and refusal checks passed'
