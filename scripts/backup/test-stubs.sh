#!/bin/bash
# Stub commands for the backup script tests (test-branch-clone.sh,
# scripts/host/test-backup-scripts.sh). Sourced, not executed.
#
# make_stubs <dir> writes fake btrfs, stat, mountpoint, findmnt, id, docker
# and pg_restore into <dir>; put it first on PATH. They need no root and
# touch nothing outside the test's temporary directory:
#
#   - A directory is a fake btrfs subvolume when it holds a .fake-btrfs file
#     of KEY=value lines: ro (true/false), uuid, received ("-" when unset),
#     and optionally unreadable=true (`btrfs subvolume show` then fails).
#     `btrfs subvolume show` and `btrfs property get <p> ro` read it,
#     `stat -c %i` reports inode 256 for it, `btrfs subvolume snapshot`
#     creates one, and `btrfs send | btrfs receive` copies one, recording the
#     sent UUID as the Received UUID and (like btrfs) failing an incremental
#     receive whose parent is not on the receiving side.
#     FAKE_RECEIVE_FAIL=1 makes a receive leave a writable half-copy and
#     fail; FAKE_RECEIVE_UUID overrides the Received UUID it records.
#   - A directory is a mount point when it holds .fake-mount; findmnt
#     --mountpoint prints that file's content ("<fs uuid> <subvolume>"), and
#     findmnt -T prints the same filesystem UUID for every path.
#   - id -u prints 0 when FAKE_ROOT=1.
#   - docker answers the kubectl calls the scripts make: get pod <name>
#     prints pod/<name> when <name> is in FAKE_PODS (space-separated; with
#     FAKE_PODS_FROM_CALL=N, only from the Nth get pod call on) and fails
#     when it is FAKE_POD_FAILS; get pv -o jsonpath prints FAKE_PVS (printf
#     %b; default the three host-storage PersistentVolumes as
#     "<path>\t<namespace>/<claim>" lines) and get pods -A prints
#     FAKE_POD_LIST (printf %b; "<namespace>/<pod>\t<volume>=<claim or host
#     path>..." lines; default none), failing with FAKE_PV_FAILS=1 and
#     FAKE_POD_LIST_FAILS=1; crictl ps -q on econ-graph-worker, for a pod in
#     econ-graph (any other crictl call is unsupported), prints a container
#     ID when the io.kubernetes.pod.name label is in FAKE_RUNNING and fails
#     with FAKE_CRICTL_FAILS=1; wait exits FAKE_WAIT_RC (default 0), psql prints
#     nothing, and pg_restore copies its stdin to FAKE_RESTORE_INPUT.
#   - pg_restore --list prints "; dbname: <second word of the file>" and,
#     when FAKE_SWAP_LINK is set, then re-points that symlink at
#     FAKE_SWAP_TO (a symlink changed between the check and the restore).
#
# Every btrfs and docker call is appended to STUB_LOG.

# shellcheck disable=SC2016  # the stubs' $ expand when they run, not here
make_stubs() {
    local dir="$1" real_stat real_id
    real_stat="$(command -v stat)"
    real_id="$(command -v id)"
    mkdir -p "${dir}"

    cat >"${dir}/btrfs" <<'EOF'
#!/bin/bash
set -euo pipefail
echo "btrfs $*" >>"${STUB_LOG:-/dev/null}"
meta() { sed -n "s/^$2=//p" "$1/.fake-btrfs" 2>/dev/null | tail -n 1; }
send_uuid() {
    local r
    r="$(meta "$1" received)"
    if [[ -n "$r" && "$r" != "-" ]]; then echo "$r"; else meta "$1" uuid; fi
}
new_uuid() { cat /proc/sys/kernel/random/uuid; }
case "$1 ${2:-}" in
    "property get")
        [[ -f "$3/.fake-btrfs" ]] || { echo "ERROR: not a btrfs object: $3" >&2; exit 1; }
        echo "ro=$(meta "$3" ro)" ;;
    "subvolume show")
        [[ -f "$3/.fake-btrfs" ]] || { echo "ERROR: not a subvolume: $3" >&2; exit 1; }
        [[ "$(meta "$3" unreadable)" != true ]] || { echo "ERROR: cannot read $3" >&2; exit 1; }
        printf '%s\n\tName: \t\t\t%s\n\tUUID: \t\t\t%s\n\tParent UUID: \t\t-\n\tReceived UUID: \t\t%s\n' \
            "$3" "$(basename "$3")" "$(meta "$3" uuid)" "$(meta "$3" received)" ;;
    "subvolume snapshot")
        ro=false
        if [[ "$3" == -r ]]; then ro=true; shift; fi
        [[ ! -e "$4" ]] || { echo "ERROR: $4 exists" >&2; exit 1; }
        mkdir "$4"
        printf 'ro=%s\nuuid=%s\nreceived=-\n' "$ro" "$(new_uuid)" >"$4/.fake-btrfs" ;;
    "filesystem du")
        echo "     Total   Exclusive  Set shared  Filename"
        echo "         0           0           0  ${*: -1}" ;;
    send*)
        shift
        parent="-"
        while [[ $# -gt 1 ]]; do
            case "$1" in
                -q) shift ;;
                -p) parent="$(send_uuid "$2")"; shift 2 ;;
                *) echo "ERROR: unexpected send argument $1" >&2; exit 1 ;;
            esac
        done
        [[ -f "$1/.fake-btrfs" ]] || { echo "ERROR: not a subvolume: $1" >&2; exit 1; }
        echo "FAKESEND name=$(basename "$1") uuid=$(send_uuid "$1") parent=${parent}" ;;
    receive*)
        read -r _ name uuid parent
        name="${name#name=}"; uuid="${uuid#uuid=}"; parent="${parent#parent=}"
        dest="$2/${name}"
        [[ ! -e "${dest}" ]] || { echo "ERROR: ${dest} exists" >&2; exit 1; }
        if [[ "${parent}" != "-" ]] && ! grep -lqx "received=${parent}" "$2"/*/.fake-btrfs 2>/dev/null \
            && ! grep -lqx "uuid=${parent}" "$2"/*/.fake-btrfs 2>/dev/null; then
            echo "ERROR: cannot find parent subvolume ${parent}" >&2
            exit 1
        fi
        mkdir "${dest}"
        if [[ "${FAKE_RECEIVE_FAIL:-0}" == 1 ]]; then
            printf 'ro=false\nuuid=%s\nreceived=-\n' "$(new_uuid)" >"${dest}/.fake-btrfs"
            echo "ERROR: fake receive failure" >&2
            exit 1
        fi
        printf 'ro=true\nuuid=%s\nreceived=%s\n' "$(new_uuid)" "${FAKE_RECEIVE_UUID:-${uuid}}" >"${dest}/.fake-btrfs" ;;
    *)
        echo "fake btrfs: unsupported: $*" >&2
        exit 1 ;;
esac
EOF

    cat >"${dir}/stat" <<EOF
#!/bin/bash
if [[ "\$1" == -c && "\$2" == %i && -f "\$3/.fake-btrfs" ]]; then
    echo 256
    exit 0
fi
exec ${real_stat} "\$@"
EOF

    cat >"${dir}/mountpoint" <<'EOF'
#!/bin/bash
[[ "$1" == -q ]] && shift
[[ -f "$1/.fake-mount" ]]
EOF

    cat >"${dir}/findmnt" <<'EOF'
#!/bin/bash
while [[ $# -gt 0 ]]; do
    case "$1" in
        --mountpoint) [[ -s "$2/.fake-mount" ]] || exit 1; cat "$2/.fake-mount"; exit 0 ;;
        -T) echo "fake-fs-uuid"; exit 0 ;;
        *) shift ;;
    esac
done
exit 1
EOF

    cat >"${dir}/id" <<EOF
#!/bin/bash
if [[ "\$1" == -u && "\${FAKE_ROOT:-0}" == 1 ]]; then echo 0; exit 0; fi
exec ${real_id} "\$@"
EOF

    cat >"${dir}/docker" <<'EOF'
#!/bin/bash
echo "docker $*" >>"${STUB_LOG:-/dev/null}"
case " $* " in
    " info "*) echo "27.0.0 [name=seccomp]" ;;
    *" pg_restore "*) cat >"${FAKE_RESTORE_INPUT:-/dev/null}" ;;
    *" psql "*) : ;;
    *" wait "*) exit "${FAKE_WAIT_RC:-0}" ;;
    *" exec econ-graph-worker crictl ps -q --label io.kubernetes.pod.namespace=econ-graph --label io.kubernetes.pod.name="*)
        [[ -z "${FAKE_CRICTL_FAILS:-}" ]] || { echo "error: connect: no such file or directory" >&2; exit 1; }
        for a in "$@"; do
            case "${a}" in io.kubernetes.pod.name=*)
                [[ " ${FAKE_RUNNING:-} " != *" ${a#*=} "* ]] || echo 0123456789ab ;;
            esac
        done ;;
    *" get pv -o jsonpath="*)
        [[ -z "${FAKE_PV_FAILS:-}" ]] || { echo "error: the server is unavailable" >&2; exit 1; }
        printf '%b' "${FAKE_PVS-/srv/econgraph/data-plane/pg\tecon-graph/postgresql-data-postgresql-0\n/srv/econgraph/app-prod/keycloak-pg\tecon-graph/keycloak-postgres-data-keycloak-postgres-0\n/srv/econgraph/app-prod/app-pg\tecon-graph/app-postgres-data-app-postgres-0\n}" ;;
    *" get pods -A -o jsonpath="*)
        [[ -z "${FAKE_POD_LIST_FAILS:-}" ]] || { echo "error: the server is unavailable" >&2; exit 1; }
        printf '%b' "${FAKE_POD_LIST-}" ;;
    *" -o jsonpath="*) echo Running ;;
    *" get pod "*)
        args=("$@"); name=""
        for i in "${!args[@]}"; do [[ "${args[$i]}" != pod ]] || name="${args[$((i + 1))]}"; done
        [[ "${name}" != "${FAKE_POD_FAILS:-}" ]] || { echo "error: the server is unavailable" >&2; exit 1; }
        calls="$(($(cat "${STUB_LOG:-/dev/null}.get-pod" 2>/dev/null || echo 0) + 1))"
        [[ -z "${STUB_LOG:-}" ]] || echo "${calls}" >"${STUB_LOG}.get-pod"
        [[ "${calls}" -ge "${FAKE_PODS_FROM_CALL:-1}" ]] || exit 0
        [[ " ${FAKE_PODS:-} " != *" ${name} "* ]] || echo "pod/${name}" ;;
    *) echo "fake docker: unsupported: $*" >&2; exit 1 ;;
esac
EOF

    cat >"${dir}/pg_restore" <<'EOF'
#!/bin/bash
case "$1" in
    --version) echo "pg_restore (PostgreSQL) 18.1" ;;
    --list)
        read -r _ db _ <"$2"
        echo ";     dbname: ${db}"
        if [[ -n "${FAKE_SWAP_LINK:-}" ]]; then ln -sfn "${FAKE_SWAP_TO}" "${FAKE_SWAP_LINK}"; fi ;;
    *) echo "fake pg_restore: unsupported: $*" >&2; exit 1 ;;
esac
EOF

    chmod +x "${dir}"/*
}

# fake_subvol <path> <ro true|false> <uuid> [received UUID]: make <path> a
# fake btrfs subvolume.
fake_subvol() {
    mkdir -p "$1"
    printf 'ro=%s\nuuid=%s\nreceived=%s\n' "$2" "$3" "${4:--}" >"$1/.fake-btrfs"
}

# fake_meta <path> <key>: one field of a fake subvolume.
fake_meta() { sed -n "s/^$2=//p" "$1/.fake-btrfs" 2>/dev/null | tail -n 1; }
