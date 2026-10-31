#!/usr/bin/env bash
# Tests for the checks in the backup scripts that need no root:
# scripts/host/backup-snapshots.sh, scripts/host/restore-snapshot.sh,
# scripts/backup/check-freshness.sh and scripts/backup/restore-dump.sh.
#
# Runs as any user with fake btrfs, stat, mountpoint, findmnt, id, docker
# and pg_restore (scripts/backup/test-stubs.sh), and mount, umount and a
# mktemp that refuses /mnt, first on PATH, and with
# ECONGRAPH_TEST_ROOT pointing the host scripts' /srv, /var/lib and /run
# paths into a temporary directory. Nothing outside it is read or written.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=scripts/backup/test-stubs.sh
source "${repo_root}/scripts/backup/test-stubs.sh"

failures=0
tmp="$(mktemp -d)"
trap 'rm -rf "${tmp}"' EXIT
make_stubs "${tmp}/bin"
# restore-snapshot.sh mounts the filesystem top level on a mktemp directory
# under /mnt once its checks pass. The checks run here are meant to stop it
# first; should one not, these record the call and fail instead of mounting
# anything or creating a directory outside ${tmp}.
# shellcheck disable=SC2016 # the stubs' $ expand when they run, not here
for cmd in mount umount; do
    printf '#!/bin/bash\necho "%s $*" >>"${STUB_LOG:-/dev/null}"\necho "fake %s: refused in tests" >&2\nexit 1\n' \
        "${cmd}" "${cmd}" >"${tmp}/bin/${cmd}"
done
real_mktemp="$(command -v mktemp)"
# shellcheck disable=SC2016
printf '#!/bin/bash\ncase " $* " in *" /mnt/"*) echo "mktemp $*" >>"${STUB_LOG:-/dev/null}"; echo "fake mktemp: refused in tests" >&2; exit 1 ;; esac\nexec %q "$@"\n' \
    "${real_mktemp}" >"${tmp}/bin/mktemp"
chmod +x "${tmp}/bin/mount" "${tmp}/bin/umount" "${tmp}/bin/mktemp"
export PATH="${tmp}/bin:${PATH}"
export STUB_LOG="${tmp}/calls.log"
# Never read the operator's backup.env.
export ECONGRAPH_CONFIG_DIR="${tmp}/config"
mkdir -p "${ECONGRAPH_CONFIG_DIR}"

pass() { echo "ok   $*"; }
fail() { echo "FAIL $*"; failures=$((failures + 1)); }

# run_script <name> <want exit> <text the output must contain> <command...>
# Leaves the output in $out.
out=""
run_script() {
    local name="$1" want="$2" text="$3" status=0
    shift 3
    : >"${STUB_LOG}"
    rm -f "${STUB_LOG}.get-pod"
    out="$("$@" </dev/null 2>&1)" || status=$?
    if [[ "${status}" -ne "${want}" ]] || ! grep -qF -- "${text}" <<<"${out}"; then
        fail "${name}: want exit ${want} and output containing '${text}', got exit ${status}:"
        while IFS= read -r line; do echo "    ${line}"; done <<<"${out}"
    else
        pass "${name}"
    fi
}
# check <name> <test command...>
check() {
    local name="$1"
    shift
    if "$@"; then pass "${name}"; else fail "${name}"; fi
}
# Every path under a directory with its fake btrfs metadata, to show that a
# dry run changed nothing.
tree_state() { (cd "$1" && find . | sort && find . -type f | sort | xargs cat); }

L1=11111111-0000-0000-0000-000000000001
L2=11111111-0000-0000-0000-000000000002
L3=11111111-0000-0000-0000-000000000003
STALE=99999999-0000-0000-0000-000000000009
FS_UUID="03ee0fcb-5f19-4f44-8596-e227f1fb647c"

echo "== backup-snapshots.sh: the pool copy must have the local snapshot's UUID"
root="${tmp}/root"
srv="${root}/srv/econgraph"
snaps="${srv}/snapshots"
pool="${root}/srv/econgraph-backups/snapshots"
mkdir -p "${srv}/data-plane" "${srv}/app-prod" "${snaps}" "${pool}" "${root}/run" "${root}/var/lib"
for m in "${srv}/data-plane" "${srv}/app-prod" "${snaps}" "${root}/srv/econgraph-backups"; do
    touch "${m}/.fake-mount"
done
fake_subvol "${snaps}/data-plane.20261024T010000Z" true "${L1}"
fake_subvol "${snaps}/data-plane.20261024T020000Z" true "${L2}"
fake_subvol "${snaps}/app-prod.20261024T010000Z" true "${L3}"
# 01: a good copy. 02: a read-only copy of something else (stale). app-prod
# 01: a read-only subvolume that was never received.
fake_subvol "${pool}/data-plane.20261024T010000Z" true aaaaaaaa-0000-0000-0000-000000000001 "${L1}"
fake_subvol "${pool}/data-plane.20261024T020000Z" true aaaaaaaa-0000-0000-0000-000000000002 "${STALE}"
fake_subvol "${pool}/app-prod.20261024T010000Z" true aaaaaaaa-0000-0000-0000-000000000003 -
export ECONGRAPH_TEST_ROOT="${root}"
export FAKE_ROOT=1

before="$(tree_state "${root}")"
run_script "backup-snapshots --dry-run plans to set the mismatched copies aside" 0 \
    "+ mv ${pool}/data-plane.20261024T020000Z ${pool}/data-plane.20261024T020000Z.mismatch-" \
    "${repo_root}/scripts/host/backup-snapshots.sh" --dry-run
check "backup-snapshots --dry-run plans to set a never-received copy aside" \
    grep -qF "+ mv ${pool}/app-prod.20261024T010000Z ${pool}/app-prod.20261024T010000Z.mismatch-" <<<"${out}"
check "backup-snapshots --dry-run plans an incremental resend from the good copy" \
    grep -qF "+ btrfs send -p ${snaps}/data-plane.20261024T010000Z ${snaps}/data-plane.20261024T020000Z" <<<"${out}"
check "backup-snapshots --dry-run changes nothing" [ "$(tree_state "${root}")" == "${before}" ]
check "backup-snapshots --dry-run makes no btrfs writes" \
    bash -c "! grep -qE '^btrfs (send|receive|subvolume snapshot)' '${STUB_LOG}'"

run_script "backup-snapshots receives again over a mismatched copy" 0 "backup-snapshots: OK" \
    "${repo_root}/scripts/host/backup-snapshots.sh"
received_of() { fake_meta "$1" received; }
one_match() { local m=("$1"*); [[ ${#m[@]} -eq 1 && -d "${m[0]}" ]] && echo "${m[0]}"; }
check "the stale copy is kept (renamed .mismatch-*, not deleted)" \
    [ "$(received_of "$(one_match "${pool}/data-plane.20261024T020000Z.mismatch-")")" == "${STALE}" ]
check "the new pool copy of data-plane 02 was received from the local one" \
    [ "$(received_of "${pool}/data-plane.20261024T020000Z")" == "${L2}" ]
check "the never-received app-prod copy is kept (renamed .mismatch-*)" \
    [ "$(received_of "$(one_match "${pool}/app-prod.20261024T010000Z.mismatch-")")" == "-" ]
check "the new pool copy of app-prod 01 was received from the local one" \
    [ "$(received_of "${pool}/app-prod.20261024T010000Z")" == "${L3}" ]
check "the stale copy was never used as a send parent" \
    bash -c "! grep -qF -- '-p ${snaps}/data-plane.20261024T020000Z ${snaps}/data-plane.20261024T020000Z' '${STUB_LOG}'"
check "the status file records no failures" grep -qx "failures=0" "${root}/var/lib/econgraph/backup-snapshots.status"
newest_dp="$(find "${snaps}" -maxdepth 1 -name 'data-plane.2*Z' -printf '%f\n' | sort | tail -n 1)"
check "the new hourly snapshot reached the pool" [ -d "${pool}/${newest_dp}" ]

# Each pool copy as "<name> <its metadata>", to show a rerun leaves them be.
pool_copies() { for d in "${pool}"/*/; do echo "$(basename "${d}") $(tr '\n' ' ' <"${d}.fake-btrfs")"; done; }
pool_before="$(pool_copies)"
run_script "backup-snapshots runs again cleanly" 0 "backup-snapshots: OK" \
    "${repo_root}/scripts/host/backup-snapshots.sh"
check "second run: nothing more set aside" \
    [ "$(find "${pool}" -maxdepth 1 -name '*.mismatch-*' | wc -l)" -eq 2 ]
check "second run: every earlier pool copy is unchanged" \
    [ -z "$(comm -23 <(sort <<<"${pool_before}") <(pool_copies | sort))" ]
check "second run: no earlier snapshot is sent again" \
    bash -c "! grep -qE '^btrfs send .*(20261024T0[12]0000Z)\$' '${STUB_LOG}'"

# A receive that fails leaves the half-copy set aside and the run failed.
fake_subvol "${snaps}/data-plane.20261024T030000Z" true 11111111-0000-0000-0000-00000000000a
FAKE_RECEIVE_FAIL=1 run_script "backup-snapshots reports a failed receive" 1 "copying data-plane.20261024T030000Z to the backup pool failed" \
    "${repo_root}/scripts/host/backup-snapshots.sh"
check "the failed receive is kept as .incomplete-*" \
    bash -c "ls -d '${pool}'/data-plane.20261024T030000Z.incomplete-* >/dev/null"
check "the status file records the failure" grep -qx "failures=[1-9][0-9]*" "${root}/var/lib/econgraph/backup-snapshots.status"

# A local snapshot whose UUID cannot be read is a counted failure, not an
# abort: the other dataset is still copied and the status file written.
fake_subvol "${snaps}/app-prod.20261024T005000Z" true 11111111-0000-0000-0000-00000000000d
echo "unreadable=true" >>"${snaps}/app-prod.20261024T005000Z/.fake-btrfs"
fake_subvol "${pool}/app-prod.20261024T005000Z" true bbbbbbbb-0000-0000-0000-00000000000d 11111111-0000-0000-0000-00000000000d
rm -f "${root}/var/lib/econgraph/backup-snapshots.status"
run_script "backup-snapshots reports an unreadable local UUID and carries on" 1 \
    "cannot read the UUID of ${snaps}/app-prod.20261024T005000Z" "${repo_root}/scripts/host/backup-snapshots.sh"
check "... the other dataset is still copied" [ -d "${pool}/data-plane.20261024T030000Z" ]
check "... the status file records the failure" grep -qx "failures=[1-9][0-9]*" "${root}/var/lib/econgraph/backup-snapshots.status"
check "... the unreadable copy is not set aside" [ -z "$(find "${pool}" -maxdepth 1 -name 'app-prod.20261024T005000Z.*')" ]
rm -rf "${snaps}/app-prod.20261024T005000Z" "${pool}/app-prod.20261024T005000Z"

echo "== restore-snapshot.sh: --from-pool checks the Received UUID; --dry-run restores nothing"
export SUDO_USER=tester
echo "${FS_UUID} /@econgraph-snapshots" >"${snaps}/.fake-mount"
echo "${FS_UUID} /@econgraph-data-plane" >"${srv}/data-plane/.fake-mount"
restore="${repo_root}/scripts/host/restore-snapshot.sh"

before="$(tree_state "${root}")"
run_script "restore-snapshot --dry-run says nothing was restored" 0 "(dry run: nothing restored)" \
    "${restore}" --dry-run data-plane.20261024T010000Z
check "restore-snapshot --dry-run does not claim a restore" bash -c "! grep -q '^Restored' <<<\"\$1\"" _ "${out}"
check "restore-snapshot --dry-run changes nothing" [ "$(tree_state "${root}")" == "${before}" ]

fake_subvol "${pool}/data-plane.20261023T000000Z" true bbbbbbbb-0000-0000-0000-000000000001 -
run_script "restore-snapshot --from-pool refuses a never-received pool copy" 1 "has no Received UUID" \
    "${restore}" --dry-run --from-pool data-plane.20261023T000000Z
fake_subvol "${pool}/data-plane.20261023T005000Z" true bbbbbbbb-0000-0000-0000-00000000000e "${L1}"
echo "unreadable=true" >>"${pool}/data-plane.20261023T005000Z/.fake-btrfs"
run_script "restore-snapshot --from-pool reports an unreadable pool copy" 1 "has no Received UUID" \
    "${restore}" --dry-run --from-pool data-plane.20261023T005000Z
# A local snapshot of the same name that the pool copy was not received from.
fake_subvol "${pool}/data-plane.20261023T010000Z" true bbbbbbbb-0000-0000-0000-000000000002 "${STALE}"
fake_subvol "${snaps}/data-plane.20261023T010000Z" true 11111111-0000-0000-0000-00000000000b
run_script "restore-snapshot --from-pool refuses a local snapshot the pool copy is not of" 1 "is not the snapshot" \
    "${restore}" --dry-run --from-pool data-plane.20261023T010000Z
run_script "restore-snapshot --from-pool uses a matching local snapshot" 0 "is the snapshot the pool copy was received from" \
    "${restore}" --dry-run --from-pool data-plane.20261024T020000Z
check "... and the dry run still restores nothing" grep -qF "(dry run: nothing restored)" <<<"${out}"
# A local copy that was itself received back from the pool matches by its
# Received UUID.
fake_subvol "${pool}/data-plane.20261023T020000Z" true bbbbbbbb-0000-0000-0000-000000000003 "${L1}"
fake_subvol "${snaps}/data-plane.20261023T020000Z" true 11111111-0000-0000-0000-00000000000c "${L1}"
run_script "restore-snapshot --from-pool matches a local copy received earlier" 0 "is the snapshot the pool copy was received from" \
    "${restore}" --dry-run --from-pool data-plane.20261023T020000Z
fake_subvol "${pool}/data-plane.20261023T030000Z" true bbbbbbbb-0000-0000-0000-000000000004 "${L2}"
run_script "restore-snapshot --from-pool --dry-run stops before the copy back" 0 "dry run: stopping here" \
    "${restore}" --dry-run --from-pool data-plane.20261023T030000Z
check "... and receives nothing" [ ! -e "${snaps}/data-plane.20261023T030000Z" ]
# The copy back itself (not a dry run) stops at the pod check: postgresql-0
# still exists, so nothing is unmounted or renamed.
FAKE_PODS=postgresql-0 FAKE_RECEIVE_UUID="${STALE}" run_script "restore-snapshot --from-pool sets a mismatched copy back aside" 1 \
    "set aside as ${snaps}/data-plane.20261023T030000Z.mismatch-" \
    "${restore}" --from-pool data-plane.20261023T030000Z
check "... kept, not deleted" bash -c "ls -d '${snaps}'/data-plane.20261023T030000Z.mismatch-* >/dev/null"
FAKE_PODS=postgresql-0 run_script "restore-snapshot --from-pool checks a good copy back and goes on" 1 \
    "still in use by: econ-graph/postgresql-0" "${restore}" --from-pool data-plane.20261023T030000Z
check "... the copy back has the pool copy's Received UUID" \
    [ "$(fake_meta "${snaps}/data-plane.20261023T030000Z" received)" == "${L2}" ]

echo "== restore-snapshot.sh: every pod using the subvolume must be gone, or nothing changes"
# Refusals run without --dry-run (they must stop before anything changes);
# acceptances only as dry runs.
echo "${FS_UUID} /@econgraph-app-prod" >"${srv}/app-prod/.fake-mount"
app_snap=app-prod.20261024T010000Z
# refuse NAME TEXT [VAR=value...]: restoring app_snap (or RESTORE_SNAP) with
# the stubbed cluster in that state exits 1 with TEXT and changes nothing.
refuse() {
    local name="$1" text="$2" before
    shift 2
    before="$(tree_state "${root}")"
    run_script "restore-snapshot refuses: ${name}" 1 "${text}" \
        env "$@" "${restore}" --yes "${RESTORE_SNAP:-${app_snap}}"
    check "... and changes nothing" [ "$(tree_state "${root}")" == "${before}" ]
    check "... and touches no subvolume and mounts nothing" \
        bash -c "! grep -qE '^(btrfs (subvolume snapshot|send|receive)|mount|umount|mktemp)' '${STUB_LOG}'"
}
# accept NAME [VAR=value...]: a dry run of the same restore gets past the check.
accept() {
    local name="$1"
    shift
    run_script "restore-snapshot accepts: ${name}" 0 "(dry run: nothing restored)" \
        env "$@" "${restore}" --dry-run "${RESTORE_SNAP:-${app_snap}}"
    check "... and says no pod uses it" grep -qF "= no pod uses @econgraph-${RESTORE_SNAP%%.*}" <<<"${out}"
}
RESTORE_SNAP=""
refuse "app-prod while app-postgres-0 exists (keycloak-postgres-0 gone)" \
    "still in use by: econ-graph/app-postgres-0." FAKE_PODS=app-postgres-0
check "... and names its StatefulSet to scale down" \
    grep -qF "scale statefulset/app-postgres --replicas=0" <<<"${out}"
refuse "app-prod while keycloak-postgres-0 exists" \
    "still in use by: econ-graph/keycloak-postgres-0." FAKE_PODS=keycloak-postgres-0
refuse "app-prod while both exist" \
    "still in use by: econ-graph/app-postgres-0 econ-graph/keycloak-postgres-0." \
    "FAKE_PODS=app-postgres-0 keycloak-postgres-0"
check "... and names both StatefulSets to scale down" \
    grep -qF "scale statefulset/app-postgres statefulset/keycloak-postgres --replicas=0" <<<"${out}"
refuse "app-prod when the cluster can't say whether app-postgres-0 exists" \
    "could not ask the cluster (econ-graph-control-plane on unix:///var/run/docker.sock) whether app-postgres-0 exists" \
    FAKE_POD_FAILS=app-postgres-0
refuse "app-prod when the PersistentVolumes can't be listed" \
    "could not ask the cluster (econ-graph-control-plane on unix:///var/run/docker.sock) which PersistentVolumes" \
    FAKE_PV_FAILS=1
refuse "app-prod when the pods can't be listed" \
    "could not ask the cluster (econ-graph-control-plane on unix:///var/run/docker.sock) which pods use /srv/econgraph/app-prod" \
    FAKE_POD_LIST_FAILS=1
refuse "app-prod while another pod mounts app-postgres's claim" "still in use by: econ-graph/pg-debug." \
    'FAKE_POD_LIST=kube-system/coredns-1\tconfig-volume=\necon-graph/pg-debug\tscratch=\tdata=app-postgres-data-app-postgres-0\n'
# shellcheck disable=SC2016 # $1 expands in the inner bash
check "... and asks to stop it, not to scale Postgres" \
    bash -c 'grep -qF "stop or delete econ-graph/pg-debug" <<<"$1" && ! grep -qF "scale statefulset" <<<"$1"' _ "${out}"
refuse "app-prod while a pod mounts a hand-made PersistentVolume on the subvolume" "still in use by: other/copy." \
    'FAKE_PVS=/srv/econgraph/app-prod/\tother/hand-made\n' 'FAKE_POD_LIST=other/copy\tdata=hand-made\n'
for path in /srv/econgraph/app-prod /srv/econgraph/app-prod/app-pg/18 /srv/econgraph /srv/econgraph/ /srv / \
    /srv//econgraph/app-prod /srv/econgraph/./app-prod/app-pg /srv/econgraph// /srv/x/../econgraph/app-prod \
    srv/econgraph/app-prod; do
    refuse "app-prod while a pod has hostPath ${path}" "still in use by: econ-graph/shell." \
        "FAKE_POD_LIST=econ-graph/shell\thost=${path}\n"
done
refuse "app-prod while a pod mounts a PersistentVolume spelled with // on the subvolume" "still in use by: econ-graph/pg-debug." \
    'FAKE_PVS=/srv/econgraph//app-prod/app-pg\tecon-graph/side\n' 'FAKE_POD_LIST=econ-graph/pg-debug\tdata=side\n'
refuse "app-prod while a generic ephemeral volume's claim is bound on the subvolume" "still in use by: econ-graph/dbg." \
    'FAKE_PVS=/srv/econgraph/app-prod/scratch\tecon-graph/dbg-scratch\n' 'FAKE_POD_LIST=econ-graph/dbg\tscratch=\n'
refuse "app-prod while app-postgres-0 is gone from the API but its container still runs" \
    "still in use by: econ-graph/app-postgres-0." FAKE_RUNNING=app-postgres-0
check "... and says a container still runs on the storage node" \
    grep -qF "app-postgres-0: gone from the API but a container still runs on econ-graph-worker" <<<"${out}"
refuse "app-prod when the storage node can't say what runs on it" \
    "could not ask the cluster (econ-graph-worker on unix:///var/run/docker.sock) whether a container of app-postgres-0 still runs" \
    FAKE_CRICTL_FAILS=1
refuse "app-prod when a Postgres pod comes back while the confirmation waits" \
    "still in use by: econ-graph/app-postgres-0." FAKE_PODS=app-postgres-0 FAKE_PODS_FROM_CALL=3
check "... after the first check passed" grep -qF "= no pod uses @econgraph-app-prod" <<<"${out}"
accept "app-prod with both Postgres pods gone and nothing else on it" \
    'FAKE_POD_LIST=kube-system/coredns-1\tconfig-volume=\necon-graph/econ-graph-backend-1\ttmp=\tcerts=\n'
accept "app-prod while postgresql-0 and the data plane's users exist" FAKE_PODS=postgresql-0 \
    'FAKE_POD_LIST=econ-graph/postgresql-0\tdata=postgresql-data-postgresql-0\necon-graph/shell\thost=/srv/econgraph/data-plane\n'
accept "app-prod while a pod has a look-alike path or a same-named claim elsewhere" \
    'FAKE_POD_LIST=econ-graph/a\thost=/srv/econgraph/app-prod-old\nstaging/b\tdata=app-postgres-data-app-postgres-0\nstaging/app-postgres\t0=\n'
RESTORE_SNAP=data-plane.20261024T010000Z
refuse "data-plane while postgresql-0 exists" "still in use by: econ-graph/postgresql-0." FAKE_PODS=postgresql-0
check "... and names its StatefulSet to scale down" \
    grep -qF "scale statefulset/postgresql --replicas=0" <<<"${out}"
refuse "data-plane while a pod mounts postgresql's claim" "still in use by: econ-graph/x." \
    'FAKE_POD_LIST=econ-graph/x\tdata=postgresql-data-postgresql-0\n'
accept "data-plane while the app plane's Postgres pods exist" "FAKE_PODS=app-postgres-0 keycloak-postgres-0"
unset RESTORE_SNAP
unset ECONGRAPH_TEST_ROOT FAKE_ROOT SUDO_USER

echo "== check-freshness.sh: the status file's failures and run_epoch"
export SNAPSHOT_DIR="${tmp}/fresh/snapshots" BACKUP_ROOT="${tmp}/fresh/dumps" \
    ECONGRAPH_STATE_DIR="${tmp}/fresh/state" HOST_STATE_DIR="${tmp}/fresh/host-state" \
    POOL_MOUNT="${tmp}/fresh/pool" HOME_MIN_FREE_PCT=0 POOL_MIN_FREE_PCT=0
now_stamp="$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "${SNAPSHOT_DIR}/data-plane.${now_stamp}" "${SNAPSHOT_DIR}/app-prod.${now_stamp}" \
    "${BACKUP_ROOT}/${now_stamp}" "${ECONGRAPH_STATE_DIR}" "${HOST_STATE_DIR}" "${POOL_MOUNT}"
date -u +%Y-%m-%dT%H:%M:%SZ >"${BACKUP_ROOT}/${now_stamp}/VERIFIED"
echo "passed_epoch=$(date +%s)" >"${ECONGRAPH_STATE_DIR}/restore-drill.last"
status="${HOST_STATE_DIR}/backup-snapshots.status"
write_status() { printf 'run_epoch=%s\nfailures=%s\npool_status=not-mounted\npool_newest_epoch=0\n' "$1" "$2" >"${status}"; }
freshness="${repo_root}/scripts/backup/check-freshness.sh"

write_status "$(date +%s)" 0
run_script "check-freshness passes a recent run with failures=0" 0 "BACKUP CHECK OK" "${freshness}"
write_status "$(date +%s)" 1
run_script "check-freshness fails a recent run with failures=1" 1 "failures=1" "${freshness}"
write_status "$(( $(date +%s) - 4 * 3600 ))" 0
run_script "check-freshness fails a status file that stopped updating" 1 "STALE   snapshot_run" "${freshness}"
rm -f "${status}"
run_script "check-freshness fails a missing status file" 1 "MISSING snapshot_run" "${freshness}"
run_script "check-freshness --skip-snapshots skips the run check" 0 "BACKUP CHECK OK" "${freshness}" --skip-snapshots
unset SNAPSHOT_DIR BACKUP_ROOT ECONGRAPH_STATE_DIR HOST_STATE_DIR POOL_MOUNT HOME_MIN_FREE_PCT POOL_MIN_FREE_PCT

echo "== restore-dump.sh: one resolved path, and a Ready pod"
dumps="${tmp}/dumps"
mkdir -p "${dumps}"
echo "dump-of econ_graph CHECKED" >"${dumps}/a.dump"
echo "dump-of econ_graph SWAPPED-IN" >"${dumps}/b.dump"
ln -s "${dumps}/a.dump" "${dumps}/latest.dump"
export FAKE_RESTORE_INPUT="${tmp}/restored"
restore_dump="${repo_root}/scripts/backup/restore-dump.sh"

: >"${FAKE_RESTORE_INPUT}"
FAKE_SWAP_LINK="${dumps}/latest.dump" FAKE_SWAP_TO="${dumps}/b.dump" \
    run_script "restore-dump restores the archive it checked" 0 "restore of econ_graph complete" \
    "${restore_dump}" --yes econ_graph "${dumps}/latest.dump"
check "... even though the symlink was re-pointed after the check" \
    grep -qx "dump-of econ_graph CHECKED" "${FAKE_RESTORE_INPUT}"
line_of() { { grep -nF -- "$1" "${STUB_LOG}" || true; } | head -n 1 | cut -d: -f1; }
wait_line="$(line_of "wait --for=condition=Ready pod/postgresql-0")"
restore_line="$(line_of " pg_restore ")"
before_line() { [[ -n "$1" && -n "$2" && "$1" -lt "$2" ]]; }
check "... after waiting for the pod to be Ready" before_line "${wait_line}" "${restore_line}"

: >"${FAKE_RESTORE_INPUT}"
FAKE_WAIT_RC=1 run_script "restore-dump stops when the pod is not Ready" 1 "is not Ready" \
    "${restore_dump}" --yes econ_graph "${dumps}/a.dump"
check "... and runs no pg_restore" bash -c "! grep -q ' pg_restore ' '${STUB_LOG}'"
run_script "restore-dump refuses a dangling dump path" 1 "dump not found" \
    "${restore_dump}" --yes econ_graph "${dumps}/missing.dump"

if [[ "${failures}" -ne 0 ]]; then
    echo "${failures} backup script test(s) failed"
    exit 1
fi
echo "all backup script tests passed"
