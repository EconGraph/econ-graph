#!/bin/bash

# Put production data back to a btrfs snapshot (docs/deployment/BACKUPS.md,
# "Restore from a snapshot").
#
#   sudo scripts/host/restore-snapshot.sh [--dry-run] [--yes] [--from-pool] [--lock-wait=SECONDS] <snapshot>
#
# <snapshot> is a name (or path) in /srv/econgraph/snapshots, e.g.
# data-plane.20261024T040000Z or app-prod.pre-20261024T044210Z-recrawl.
# With --from-pool it is a name in /srv/econgraph-backups/snapshots (the
# copy on the SATA backup pool): it is first copied back into
# /srv/econgraph/snapshots with btrfs send/receive, then restored as usual.
# Its prefix says which subvolume it replaces, and the Postgres pods whose
# data it holds:
#   data-plane.*  -> @econgraph-data-plane at /srv/econgraph/data-plane
#                    (pod postgresql-0)
#   app-prod.*    -> @econgraph-app-prod   at /srv/econgraph/app-prod
#                    (pods app-postgres-0 and keycloak-postgres-0)
#
# Steps: refuse unless every pod using that subvolume is gone: those Postgres
# pods (in the API, and with no container still running on the storage
# node), and any other pod, in any namespace, that the API shows mounting it
# (a hostPath volume at, under or above /srv/econgraph/<name>, compared as
# text after resolving //, . and .., links not followed, or anywhere under
# /proc or relative; or a claim, generic ephemeral ones included,
# bound to a PersistentVolume there). The refusal names each one; scale
# their StatefulSets to 0 first. If the cluster can't be asked, refuse too.
# The check runs again after the confirmation. Containers started outside
# Kubernetes are not seen. This
# check does not replace stopping the writers (backend, crawler, Keycloak)
# first, nor restarting the storage node after the restore (below). Then ask
# you to type the subvolume name (e.g.
# @econgraph-data-plane; refused without a terminal unless --yes); unmount the subvolume; rename it to
# @econgraph-<name>.replaced-<UTC stamp> (kept, so the restore can itself
# be undone; delete it later with scripts/host/delete-snapshots.sh); create
# a writable snapshot of <snapshot> under the original name; mount it.
#
# Postgres runs on the kind node econ-graph-worker (labelled
# econgraph.io/host-storage=true), which bind-mounts /srv/econgraph/data-plane
# and /srv/econgraph/app-prod. That bind mount keeps pointing at the OLD
# subvolume until the node container restarts, so the script ends by telling
# you to restart econ-graph-worker before scaling Postgres back up.
#
# Nothing is deleted. Safe to re-run with a different snapshot.
#
# Before its first check it takes the lock the hourly backup-snapshots.sh
# holds (/run/econgraph/backup-snapshots.lock), and keeps it until it exits,
# after the swap and remount: no backup run snapshots, sends or prunes
# meanwhile, and no second restore runs. If the lock is held it stops,
# changing nothing; --lock-wait=SECONDS (up to 86400) waits up to that long
# for it first. While it waits at the typed confirmation it still holds the
# lock, so an hourly backup run that starts then fails and is skipped.
# The storage node restart that follows (below) is yours to run after the
# script exits, so the lock does not cover it.
#
# --dry-run (-n) writes nothing, mounts nothing and takes no lock (opening
# the lock file would create it): it runs the read-only
# checks (the snapshot is a read-only subvolume, no pod uses the subvolume,
# and, through findmnt on the existing mounts, that the target and the
# snapshot directory are the expected subvolumes of this filesystem), then
# prints each step. With --from-pool it stops before the copy back.
#
# --from-pool uses the pool copy only if it has a Received UUID (it was made
# by btrfs receive), and refuses a local snapshot of the same name that is
# not the one the pool copy was received from. After the copy back, the new
# local snapshot's Received UUID must match too; if not, it is set aside as
# <snapshot>.mismatch-<stamp> (not deleted).
#
# ECONGRAPH_TEST_ROOT, if set, is prefixed to the /srv paths below, so
# scripts/host/test-backup-scripts.sh can run this against stubs in a
# temporary directory.

set -euo pipefail

TEST_ROOT="${ECONGRAPH_TEST_ROOT:-}"
FS_UUID="03ee0fcb-5f19-4f44-8596-e227f1fb647c"
SNAPSHOT_DIR="$TEST_ROOT/srv/econgraph/snapshots"
SRV_ROOT="$TEST_ROOT/srv/econgraph"
KIND_NODE="econ-graph-control-plane"      # where kubectl runs
STORAGE_NODE="econ-graph-worker"          # the node that bind-mounts /srv/econgraph/*
NAMESPACE="econ-graph"
# The rootful daemon the kind cluster runs on (as scripts/deploy/lib/kind-env.sh;
# pass ECONGRAPH_DOCKER_HOST through sudo to override).
DOCKER_SOCK="${ECONGRAPH_DOCKER_HOST:-unix:///var/run/docker.sock}"

DRY_RUN="false"
YES="false"
FROM_POOL="false"
POOL_MOUNT="$TEST_ROOT/srv/econgraph-backups"
POOL_SNAPSHOT_DIR="$POOL_MOUNT/snapshots"
LOCK_FILE="$TEST_ROOT/run/econgraph/backup-snapshots.lock"
LOCK_WAIT=0
usage="usage: $0 [--dry-run] [--yes] [--from-pool] [--lock-wait=SECONDS] <snapshot>"
snap=""
for arg in "$@"; do
    case "$arg" in
        --dry-run|-n) DRY_RUN="true" ;;
        --yes|-y) YES="true" ;;
        --from-pool) FROM_POOL="true" ;;
        --lock-wait=*)
            LOCK_WAIT="${arg#*=}"
            # At most a day: flock can't set up a timer for some larger values.
            [[ "$LOCK_WAIT" =~ ^[0-9]{1,5}$ ]] && [ "$((10#$LOCK_WAIT))" -le 86400 ] ||
                { echo "$usage (SECONDS: 0 to 86400)" >&2; exit 2; } ;;
        -*) echo "$usage" >&2; exit 2 ;;
        *) snap="$arg" ;;
    esac
done
[ -n "$snap" ] || { echo "$usage" >&2; exit 2; }

run() {
    echo "+ $*"
    if [ "$DRY_RUN" = "false" ]; then
        "$@"
    fi
}

if [ "$(id -u)" -ne 0 ]; then
    echo "error: run with sudo" >&2
    exit 1
fi
OWNER="${SUDO_USER:-}"
if [ -z "$OWNER" ] || [ "$OWNER" = "root" ]; then
    echo "error: run with sudo from the account that runs the kind cluster (SUDO_USER is unset)" >&2
    exit 1
fi

# >>> backup lock: the same in backup-snapshots.sh, restore-snapshot.sh,
# delete-snapshots.sh and snapshot-now.sh (test-backup-scripts.sh checks).
# take_backup_lock WAIT: take the backup lock, /run/econgraph/backup-snapshots.lock,
# on fd 9, waiting up to WAIT seconds, and keep it until the script exits.
# Then the lock older copies take, /run/econgraph-backup-snapshots.lock, on
# fd 7: any user can open that file, so it counts as held only while a root
# process holds it (an older checkout's restore-snapshot.sh, say); held by
# anyone else it is skipped.
# /run/econgraph is root's, mode 0700, so no other user can open the file to
# hold the lock; a directory or file that is not as this function makes them
# is refused, never used. The hourly job installed by install-backup-timer.sh
# must take this same lock: an installed copy without the line below is
# refused until the installer is re-run. Returns 1 when the lock is held, 2
# on any other failure.
BACKUP_LOCK_VERSION=2
# backup_lock_root_holds FILE: whether a process of root's has FILE open, so
# may hold its lock. (/proc/locks can't say: it names the process that took
# the lock, often a flock(1) that has since exited.) Call with FILE closed.
backup_lock_root_holds() {
    local file="$1" root_uid="0" fd pid
    # Under the test: the test user, unless BACKUP_LOCK_ROOT_UID says otherwise.
    [ -z "${ECONGRAPH_TEST_ROOT:-}" ] || root_uid="${BACKUP_LOCK_ROOT_UID:-${BACKUP_LOCK_OWNER%%:*}}"
    while IFS= read -r fd; do
        pid="${fd#/proc/}"
        pid="${pid%%/*}"
        [ "$(stat -c %u "/proc/$pid" 2>/dev/null)" != "$root_uid" ] || return 0
    done < <(find /proc/[0-9]*/fd -maxdepth 1 -lname "$file" 2>/dev/null)
    return 1
}
take_backup_lock() {
    local wait="$1" root="${ECONGRAPH_TEST_ROOT:-}" owner="0:0" old_umask status
    local dir file installed legacy
    dir="$root/run/econgraph"
    file="$dir/backup-snapshots.lock"
    installed="$root/usr/local/sbin/econgraph-backup-snapshots"
    legacy="$root/run/econgraph-backup-snapshots.lock"
    if [ -n "$root" ]; then
        # Test runs only (scripts/host/test-backup-scripts.sh, never root):
        # the test user's own files.
        if [ "$EUID" -eq 0 ]; then
            echo "error: ECONGRAPH_TEST_ROOT is set; refusing to run as root with it" >&2
            return 2
        fi
        owner="${BACKUP_LOCK_OWNER:-0:0}"
    fi
    if [ -e "$installed" ] && ! grep -qx "BACKUP_LOCK_VERSION=$BACKUP_LOCK_VERSION" "$installed"; then
        echo "error: the installed hourly job $installed takes an older lock, so it would not" >&2
        echo "  wait for this run. Re-run sudo scripts/host/install-backup-timer.sh first." >&2
        return 2
    fi
    if [ ! -e "$dir" ] && ! mkdir -m 0700 "$dir" 2>/dev/null && [ ! -d "$dir" ]; then
        echo "error: could not create $dir" >&2
        return 2
    fi
    if [ -L "$dir" ] || [ ! -d "$dir" ] || [ "$(stat -c '%u:%g %a' "$dir")" != "$owner 700" ]; then
        echo "error: $dir is not a directory owned by $owner with mode 0700; refusing to lock in it." >&2
        echo "  Check what made it; if no backup, restore or delete is running, remove it and re-run." >&2
        return 2
    fi
    if [ -L "$file" ] || { [ -e "$file" ] && [ ! -f "$file" ]; }; then
        echo "error: $file is not a regular file; refusing to use it as the lock" >&2
        return 2
    fi
    old_umask="$(umask)"
    umask 077
    if ! exec 9>>"$file"; then
        umask "$old_umask"
        echo "error: could not open $file" >&2
        return 2
    fi
    umask "$old_umask"
    status=0
    flock -w "$wait" 9 || status=$?
    if [ "$status" -eq 1 ]; then
        return 1
    elif [ "$status" -ne 0 ]; then
        echo "error: could not lock $file (flock exited $status)" >&2
        return 2
    fi
    if [ -f "$legacy" ] && [ ! -L "$legacy" ]; then
        if ! exec 7<"$legacy"; then
            echo "error: could not open $legacy" >&2
            return 2
        fi
        if ! flock -n 7; then
            exec 7<&-
            if backup_lock_root_holds "$legacy"; then
                if ! exec 7<"$legacy"; then
                    echo "error: could not open $legacy" >&2
                    return 2
                fi
                status=0
                flock -w "$wait" 7 || status=$?
                if [ "$status" -eq 1 ]; then
                    return 1
                elif [ "$status" -ne 0 ]; then
                    echo "error: could not lock $legacy (flock exited $status)" >&2
                    return 2
                fi
            fi
        fi
    fi
}
# <<< backup lock

# The backup lock, held until this script exits (fd 9 stays open).
if [ "$DRY_RUN" = "false" ]; then
    # Computed on its own line: a failed expansion stops the script here
    # (set -e) instead of skipping the lock.
    lock_wait_seconds=$((10#$LOCK_WAIT))
    lock_status=0
    take_backup_lock "$lock_wait_seconds" || lock_status=$?
    if [ "$lock_status" -eq 2 ]; then
        echo "error: could not take the lock $LOCK_FILE; nothing changed" >&2
        exit 1
    fi
    if [ "$lock_status" -eq 1 ]; then
        echo "error: $LOCK_FILE is held: a backup-snapshots run (econgraph-backup-snapshots.service)" >&2
        echo "  or another restore is in progress. Nothing changed. Wait for it to finish" >&2
        echo "  (systemctl status econgraph-backup-snapshots.service), then re-run, or pass --lock-wait=SECONDS." >&2
        exit 1
    fi
fi

snap="$(basename "$snap")"
case "$snap" in
    data-plane.*) name="data-plane"; pods=(postgresql-0) ;;
    app-prod.*) name="app-prod"; pods=(app-postgres-0 keycloak-postgres-0) ;;
    *) echo "error: $snap is not a data-plane.* or app-prod.* snapshot" >&2; exit 1 ;;
esac
target="$SRV_ROOT/$name"
subvol="@econgraph-$name"
# The subvolume's path on the storage node, as pods and PersistentVolumes
# name it (the node bind-mounts it at the same path).
node_path="/srv/econgraph/$name"

# mounted_subvol <mountpoint>: "<uuid> <subvolume path>" of the mount there.
mounted_subvol() {
    findmnt -n -o UUID,FSROOT --mountpoint "$1" | tail -n 1
}
# subvol_field <path> <field>: a field of `btrfs subvolume show <path>`
# ("UUID" or "Received UUID"); "-" when unset, empty when unreadable.
subvol_field() {
    # || true: an unreadable subvolume must reach the callers' error
    # messages, not abort the script under pipefail.
    { btrfs subvolume show "$1" 2>/dev/null \
        | sed -nE "s/^[[:space:]]*$2:[[:space:]]+([^[:space:]]+).*/\1/p" | head -n 1; } || true
}
# send_uuid <path>: the UUID a send of <path> carries (its Received UUID if
# it was itself received, otherwise its UUID). Empty when unreadable.
send_uuid() {
    local received
    received="$(subvol_field "$1" "Received UUID")"
    if [ -n "$received" ] && [ "$received" != "-" ]; then
        echo "$received"
    else
        subvol_field "$1" "UUID"
    fi
}
# mount_target: mount $target from /etc/fstab (its options), then check it
# really is $subvol on this filesystem; if not, unmount it again and fail.
mount_target() {
    mount "$target" || return 1
    if [ "$(mounted_subvol "$target")" != "$FS_UUID /$subvol" ]; then
        echo "error: /etc/fstab mounted $target from $(mounted_subvol "$target"), not UUID=$FS_UUID /$subvol" >&2
        umount "$target" || true
        return 1
    fi
}
# Dry run or not, before anything else: the checks below read
# $SNAPSHOT_DIR/$snap and the restore uses @econgraph-snapshots/$snap at the
# top level, so $SNAPSHOT_DIR must be that subvolume (and $target, if
# mounted, the one being replaced).
if [ "$(mounted_subvol "$SNAPSHOT_DIR")" != "$FS_UUID /@econgraph-snapshots" ]; then
    echo "error: $SNAPSHOT_DIR is not @econgraph-snapshots on UUID=$FS_UUID" >&2
    exit 1
fi
if mountpoint -q "$target"; then
    if [ "$(mounted_subvol "$target")" != "$FS_UUID /$subvol" ]; then
        echo "error: $target is not $subvol on UUID=$FS_UUID" >&2
        exit 1
    fi
    echo "= $target is $subvol on UUID=$FS_UUID"
elif [ "$DRY_RUN" = "true" ]; then
    echo "= $target is not mounted, so a dry run cannot confirm $subvol exists"
fi

if [ "$FROM_POOL" = "true" ]; then
    if ! mountpoint -q "$POOL_MOUNT"; then
        echo "error: the backup pool is not mounted at $POOL_MOUNT" >&2
        exit 1
    fi
    if [ "$(btrfs property get "$POOL_SNAPSHOT_DIR/$snap" ro 2>/dev/null)" != "ro=true" ]; then
        echo "error: $POOL_SNAPSHOT_DIR/$snap is not a complete read-only snapshot" >&2
        exit 1
    fi
    pool_received="$(subvol_field "$POOL_SNAPSHOT_DIR/$snap" "Received UUID")"
    if [ -z "$pool_received" ] || [ "$pool_received" = "-" ]; then
        echo "error: $POOL_SNAPSHOT_DIR/$snap has no Received UUID, so it is not a copy made by btrfs receive; refusing" >&2
        exit 1
    fi
    if [ -e "$SNAPSHOT_DIR/$snap" ]; then
        local_uuid="$(send_uuid "$SNAPSHOT_DIR/$snap")"
        if [ "$local_uuid" != "$pool_received" ]; then
            echo "error: $SNAPSHOT_DIR/$snap (UUID ${local_uuid:-unreadable}) is not the snapshot" >&2
            echo "  $POOL_SNAPSHOT_DIR/$snap was received from (Received UUID $pool_received); refusing." >&2
            echo "  Restore the local one without --from-pool, or move it aside by hand to use the pool copy." >&2
            exit 1
        fi
        echo "= $SNAPSHOT_DIR/$snap already exists locally and is the snapshot the pool copy was received from; using it"
    else
        echo "+ btrfs send $POOL_SNAPSHOT_DIR/$snap | btrfs receive $SNAPSHOT_DIR"
        if [ "$DRY_RUN" = "true" ]; then
            echo "(dry run: stopping here, the remaining checks need the local copy)"
            exit 0
        fi
        if ! btrfs send -q "$POOL_SNAPSHOT_DIR/$snap" | btrfs receive "$SNAPSHOT_DIR"; then
            echo "error: copying $snap back from the backup pool failed" >&2
            if [ -e "$SNAPSHOT_DIR/$snap" ]; then
                # Set the partial copy aside (never delete it), so a rerun
                # receives again instead of finding a writable leftover.
                leftover="$SNAPSHOT_DIR/$snap.incomplete-$(date -u +%Y%m%dT%H%M%SZ)"
                echo "  the partial copy is set aside as $leftover (not deleted)" >&2
                mv "$SNAPSHOT_DIR/$snap" "$leftover" || echo "  could not rename it; move it aside by hand" >&2
            fi
            exit 1
        fi
        got="$(subvol_field "$SNAPSHOT_DIR/$snap" "Received UUID")"
        if [ "$got" != "$pool_received" ]; then
            mismatch="$SNAPSHOT_DIR/$snap.mismatch-$(date -u +%Y%m%dT%H%M%SZ)"
            echo "error: the copy back has Received UUID ${got:-(unreadable)}, not $pool_received; set aside as $mismatch (not deleted)" >&2
            mv "$SNAPSHOT_DIR/$snap" "$mismatch" || echo "  could not rename it; move it aside by hand" >&2
            exit 1
        fi
    fi
fi

if ! btrfs subvolume show "$SNAPSHOT_DIR/$snap" >/dev/null 2>&1; then
    echo "error: $SNAPSHOT_DIR/$snap is not a subvolume" >&2
    exit 1
fi
if [ "$(btrfs property get "$SNAPSHOT_DIR/$snap" ro)" != "ro=true" ]; then
    echo "error: $SNAPSHOT_DIR/$snap is not read-only; refusing to restore from a snapshot that may have been modified" >&2
    exit 1
fi

# Nothing may use the subvolume: ask the cluster, on the rootful daemon.
# Any failure to ask refuses the restore.
k() {
    env -u DOCKER_CONTEXT DOCKER_HOST="$DOCKER_SOCK" docker exec "$KIND_NODE" kubectl "$@"
}
cannot_ask() {
    echo "error: could not ask the cluster $1, so it may still be in use; nothing changed: $2" >&2
    exit 1
}
# normalize <absolute path>: the path with empty, "." and ".." components
# resolved as text (no symlinks followed), e.g. /srv//econgraph/./x/ -> /srv/econgraph/x.
normalize() {
    local part parts out=()
    IFS=/ read -r -a parts <<<"$1"
    for part in ${parts[@]+"${parts[@]}"}; do
        case "$part" in
            "" | .) ;;
            ..) [ "${#out[@]}" -eq 0 ] || out=("${out[@]:0:${#out[@]}-1}") ;;
            *) out+=("$part") ;;
        esac
    done
    (IFS=/; echo "/${out[*]}")
}
# path_overlaps <path>: whether <path> is $node_path, inside it or above it.
# Compared as text, links not followed. A path that is not absolute, or one
# under /proc (/proc/<pid>/root leads to the node's whole filesystem), counts
# as overlapping: it can't be checked.
path_overlaps() {
    local p
    case "$1" in /*) ;; *) return 0 ;; esac
    p="$(normalize "$1")"
    [ "$p" = "$node_path" ] || [ "${p#"$node_path"/}" != "$p" ] ||
        [ "$p" = "/" ] || [ "${node_path#"$p"/}" != "$node_path" ] ||
        [ "$p" = /proc ] || [ "${p#/proc/}" != "$p" ]
}
# check_unused: exit 1, naming them, if any pod uses the subvolume:
#   - the plane's Postgres pods, as the API server reports them, or with a
#     container still running on the storage node (a pod force-deleted from
#     the API can leave its container running there);
#   - any pod, in any namespace, with a hostPath volume at, under or above
#     $node_path, or a claim (named, or a generic ephemeral volume's
#     <pod>-<volume>) bound to a PersistentVolume there.
check_unused() {
    local p state pvs line path claim all_pods fields id vol name rest
    local in_use=() others=() stuck=() claims=()
    for p in "${pods[@]}"; do
        state="$(k -n "$NAMESPACE" get pod "$p" --ignore-not-found -o name 2>&1)" ||
            cannot_ask "($KIND_NODE on $DOCKER_SOCK) whether $p exists" "$state"
        [ -z "$state" ] || in_use+=("$p")
        state="$(env -u DOCKER_CONTEXT DOCKER_HOST="$DOCKER_SOCK" docker exec "$STORAGE_NODE" \
            crictl ps -q --label "io.kubernetes.pod.namespace=$NAMESPACE" --label "io.kubernetes.pod.name=$p" 2>&1)" ||
            cannot_ask "($STORAGE_NODE on $DOCKER_SOCK) whether a container of $p still runs" "$state"
        if [ -n "$state" ] && [[ " ${in_use[*]} " != *" $p "* ]]; then
            in_use+=("$p")
            stuck+=("$p")
        fi
    done
    # Claims bound to a PersistentVolume on the subvolume, as "<namespace>/<name>".
    pvs="$(k get pv -o 'jsonpath={range .items[*]}{.spec.hostPath.path}{.spec.local.path}{"\t"}{.spec.claimRef.namespace}/{.spec.claimRef.name}{"\n"}{end}' 2>&1)" ||
        cannot_ask "($KIND_NODE on $DOCKER_SOCK) which PersistentVolumes are on $node_path" "$pvs"
    while IFS= read -r line; do
        path="${line%%$'\t'*}"
        claim="${line#*$'\t'}"
        if [ -n "$path" ] && path_overlaps "$path" && [ "$claim" != "/" ]; then
            claims+=("$claim")
        fi
    done <<<"$pvs"
    # Every pod, as "<namespace>/<name>", then a tab and
    # "<volume>=<claim>=<host path>" for each volume (names and claims never
    # contain "=", so the host path is everything after the second one).
    all_pods="$(k get pods -A -o 'jsonpath={range .items[*]}{.metadata.namespace}/{.metadata.name}{range .spec.volumes[*]}{"\t"}{.name}={.persistentVolumeClaim.claimName}={.hostPath.path}{end}{"\n"}{end}' 2>&1)" ||
        cannot_ask "($KIND_NODE on $DOCKER_SOCK) which pods use $node_path" "$all_pods"
    while IFS=$'\t' read -r -a fields; do
        [ "${#fields[@]}" -gt 0 ] || continue
        id="${fields[0]}"
        for vol in "${fields[@]:1}"; do
            name="${vol%%=*}"
            rest="${vol#*=}"
            claim="${rest%%=*}"
            path="${rest#*=}"
            # A host path; a claim bound there; or a generic ephemeral
            # volume's claim, <pod>-<volume>, bound there.
            if ! { [ -n "$path" ] && path_overlaps "$path"; } &&
                ! { [ -n "$claim" ] && [[ " ${claims[*]} " == *" ${id%%/*}/$claim "* ]]; } &&
                [[ " ${claims[*]} " != *" $id-$name "* ]]; then
                continue
            fi
            if [[ " ${in_use[*]} " != *" ${id#"$NAMESPACE"/} "* ]] && [[ " ${others[*]} " != *" $id "* ]]; then
                others+=("$id")
            fi
            break
        done
    done <<<"$all_pods"
    if [ "${#in_use[@]}" -gt 0 ] || [ "${#others[@]}" -gt 0 ]; then
        local named=()
        for p in ${in_use[@]+"${in_use[@]}"}; do named+=("$NAMESPACE/$p"); done
        named+=(${others[@]+"${others[@]}"})
        echo "error: $subvol is still in use by: ${named[*]}." >&2
        echo "Nothing changed. After stopping the writers (docs/deployment/BACKUPS.md, \"Restore from a snapshot\", step 2):" >&2
        if [ "${#in_use[@]}" -gt 0 ]; then
            echo "  docker -H $DOCKER_SOCK exec $KIND_NODE kubectl -n $NAMESPACE scale$(printf ' statefulset/%s' "${in_use[@]%-0}") --replicas=0" >&2
        fi
        if [ "${#stuck[@]}" -gt 0 ]; then
            echo "  ${stuck[*]}: gone from the API but a container still runs on $STORAGE_NODE; wait for it to stop" >&2
            echo "  (docker -H $DOCKER_SOCK exec $STORAGE_NODE crictl ps --label io.kubernetes.pod.name=<pod>)" >&2
        fi
        if [ "${#others[@]}" -gt 0 ]; then
            echo "  stop or delete ${others[*]}, which mount $node_path" >&2
        fi
        exit 1
    fi
}
check_unused
echo "= no pod uses $subvol (${pods[*]} are gone)"

stamp="$(date -u +%Y%m%dT%H%M%SZ)"
echo "Restore plan:"
echo "  $subvol (mounted at $target) -> renamed $subvol.replaced-$stamp (kept)"
echo "  new writable $subvol <- snapshot of $SNAPSHOT_DIR/$snap"
if [ "$YES" = "false" ] && [ "$DRY_RUN" = "false" ]; then
    if [ ! -t 0 ]; then
        echo "error: no terminal to confirm on; refusing (pass --yes only if you mean it)" >&2
        exit 1
    fi
    read -r -p "Type '$subvol' to replace it: " answer
    if [ "$answer" != "$subvol" ]; then
        echo "aborted; nothing changed"
        exit 1
    fi
fi

# Again, after the confirmation, which may have waited a long time.
check_unused
echo "= still no pod uses $subvol"

if [ "$DRY_RUN" = "true" ]; then
    # No temporary directory and no top-level mount in a dry run.
    echo "+ mount -o subvolid=5 UUID=$FS_UUID <temporary directory under /mnt>   (not in a dry run)"
    top="<top>"
else
    top="$(mktemp -d /mnt/econgraph-top.XXXXXX)"
    cleanup() {
        if mountpoint -q "$top"; then umount "$top"; fi
        rmdir "$top"
    }
    trap cleanup EXIT
    mount -o subvolid=5 "UUID=$FS_UUID" "$top"

    for p in "$top/$subvol" "$top/@econgraph-snapshots/$snap"; do
        if ! btrfs subvolume show "$p" >/dev/null 2>&1; then
            echo "error: $p not found at the filesystem top level" >&2
            exit 1
        fi
    done
fi

if mountpoint -q "$target"; then
    run umount "$target"
fi
run mv "$top/$subvol" "$top/$subvol.replaced-$stamp"
# put_back: rename $subvol.replaced-$stamp back to $subvol, remount $target,
# say that nothing was restored, exit 1. Only renames; nothing is deleted.
put_back() {
    echo "  renaming $subvol.replaced-$stamp back to $subvol" >&2
    if [ -e "$top/$subvol" ] || ! mv "$top/$subvol.replaced-$stamp" "$top/$subvol"; then
        echo "error: could not rename it back. The data is intact in $subvol.replaced-$stamp" >&2
        echo "  at the top level of UUID=$FS_UUID. To put it back by hand:" >&2
        echo "    sudo mount -o subvolid=5 UUID=$FS_UUID /mnt" >&2
        echo "    sudo mv /mnt/$subvol.replaced-$stamp /mnt/$subvol   (after checking /mnt/$subvol does not exist)" >&2
        echo "    sudo umount /mnt && sudo mount $target" >&2
        exit 1
    fi
    if ! mount_target; then
        echo "error: $subvol is back under its name but mounting $target failed; check /etc/fstab, then: sudo mount $target" >&2
    fi
    echo "Nothing was restored; $target holds the data it had before." >&2
    exit 1
}
if ! run btrfs subvolume snapshot "$top/@econgraph-snapshots/$snap" "$top/$subvol"; then
    echo "error: creating $subvol from $snap failed" >&2
    put_back
fi
if [ "$DRY_RUN" = "true" ]; then
    echo "+ mount $target   (from /etc/fstab, then checked to be UUID=$FS_UUID /$subvol)"
elif ! mount_target; then
    # Set the new copy aside (kept, not deleted) and put the original back.
    echo "error: mounting the restored $subvol at $target failed" >&2
    if ! mv "$top/$subvol" "$top/$subvol.failed-restore-$stamp"; then
        echo "error: could not set the restored copy aside; $subvol.replaced-$stamp still holds the previous data" >&2
        exit 1
    fi
    echo "  the restored copy is kept as $subvol.failed-restore-$stamp" >&2
    put_back
fi

if [ "$DRY_RUN" = "true" ]; then
    echo "(dry run: nothing restored)"
    exit 0
fi
findmnt "$target"
cat <<EOF

Restored $target from $snap. The previous data is kept as $subvol.replaced-$stamp.
Keep Postgres stopped. Its kind node, $STORAGE_NODE, still bind-mounts the
old subvolume and only picks up the new one when its container restarts.
As $OWNER, no sudo (the rootful daemon, through the docker group):
  docker -H $DOCKER_SOCK restart $STORAGE_NODE
wait for the node to be Ready, then scale Postgres (and the backend/crawler)
back up and check the data.
EOF
