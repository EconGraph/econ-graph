#!/usr/bin/env bash
# Exercise opt-in store coverage with existing backup stubs, never real host storage.
set -euo pipefail
repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=scripts/backup/test-stubs.sh
source "$repo_root/scripts/backup/test-stubs.sh"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
make_stubs "$scratch/bin"
# The production lock checks real ownership even while id is stubbed below.
BACKUP_LOCK_OWNER="$(id -u):$(id -g)"
export BACKUP_LOCK_OWNER
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
# An invalid opt-in must still run the independent backup pipeline and refresh
# monitoring, not merely return a failure before doing any backup work.
for invalid in contents directory symlink; do
    root="$scratch/invalid-$invalid"
    export ECONGRAPH_TEST_ROOT="$root"
    srv="$root/srv/econgraph"
    pool="$root/srv/econgraph-backups/snapshots"
    state="$root/var/lib/econgraph"
    marker="$root/etc/econgraph/objectstore-snapshots.enabled"
    mkdir -p "$pool" "$state" "$root/etc/econgraph" "$root/run"
    for name in data-plane app-prod objectstore snapshots; do
        mkdir -p "$srv/$name"
        touch "$srv/$name/.fake-mount"
    done
    touch "$root/srv/econgraph-backups/.fake-mount"
    # Even an old objectstore snapshot must not be copied when coverage is invalid.
    fake_subvol "$srv/snapshots/objectstore.20261024T010000Z" true \
        11111111-0000-0000-0000-000000000001
    case "$invalid" in
        contents) printf 'foreign\n' >"$marker" ;;
        directory) mkdir "$marker" ;;
        symlink)
            printf 'objectstore-v1\n' >"$root/foreign-marker"
            ln -s "$root/foreign-marker" "$marker"
            ;;
    esac
    printf 'run_epoch=1\nfailures=0\n' >"$state/backup-snapshots.status"
    : >"$STUB_LOG"
    status=0
    bash "$repo_root/scripts/host/backup-snapshots.sh" >"$scratch/$invalid.log" 2>&1 || status=$?
    [[ "$status" == 1 ]]
    grep -q 'invalid objectstore snapshot coverage marker' "$scratch/$invalid.log"
    grep -q 'backup-snapshots: 1 failure(s)' "$scratch/$invalid.log"
    for name in data-plane app-prod; do
        grep -q "^btrfs subvolume snapshot -r $srv/$name " "$STUB_LOG"
        # A received copy proves that the pool phase also continued.
        find "$pool" -mindepth 1 -maxdepth 1 -type d -name "$name.*" | grep -q .
    done
    if grep -q objectstore "$STUB_LOG"; then
        echo "invalid $invalid marker allowed objectstore backup operations" >&2
        exit 1
    fi
    grep -qx 'failures=1' "$state/backup-snapshots.status"
    grep -qx 'pool_status=ok' "$state/backup-snapshots.status"
    awk -F= '$1 == "run_epoch" && $2 > 1 { found=1 } END { exit !found }' \
        "$state/backup-snapshots.status"
    grep -qE '^local_newest_epoch=[1-9][0-9]*$' "$state/backup-snapshots.status"
    grep -qE '^pool_newest_epoch=[1-9][0-9]*$' "$state/backup-snapshots.status"
    test -f "$state/backup-sizes.env"
    if grep -q '^btrfs subvolume delete' "$STUB_LOG"; then
        echo 'invalid-marker backup attempted deletion' >&2
        exit 1
    fi
done
echo 'objectstore opt-in and invalid-marker isolation/status checks passed'
