#!/usr/bin/env bash
# Tests for the checks in the backup scripts that need no root:
# scripts/host/backup-snapshots.sh, scripts/host/restore-snapshot.sh,
# scripts/backup/check-freshness.sh and scripts/backup/restore-dump.sh.
#
# Runs as any user with fake btrfs, stat, mountpoint, findmnt, id, docker
# and pg_restore (scripts/backup/test-stubs.sh) first on PATH, and with
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
FAKE_POD_NAME=pod/postgresql-0 FAKE_RECEIVE_UUID="${STALE}" run_script "restore-snapshot --from-pool sets a mismatched copy back aside" 1 \
    "set aside as ${snaps}/data-plane.20261023T030000Z.mismatch-" \
    "${restore}" --from-pool data-plane.20261023T030000Z
check "... kept, not deleted" bash -c "ls -d '${snaps}'/data-plane.20261023T030000Z.mismatch-* >/dev/null"
FAKE_POD_NAME=pod/postgresql-0 run_script "restore-snapshot --from-pool checks a good copy back and goes on" 1 \
    "postgresql-0 still exists" "${restore}" --from-pool data-plane.20261023T030000Z
check "... the copy back has the pool copy's Received UUID" \
    [ "$(fake_meta "${snaps}/data-plane.20261023T030000Z" received)" == "${L2}" ]
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
