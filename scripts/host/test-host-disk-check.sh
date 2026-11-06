#!/usr/bin/env bash
# Tests for scripts/host/host-disk-check.sh and the checks in
# scripts/host/install-host-disk-exporter.sh that run before it changes
# anything (its --dry-run path).
#
# Runs as any user with fake virsh, df, mountpoint, id and ip first on PATH.
# Nothing outside a temporary directory is read or written. When promtool is
# on PATH (or PROMTOOL names it), the written file is also checked with
# `promtool check metrics`.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
check_script="${repo_root}/scripts/host/host-disk-check.sh"
installer="${repo_root}/scripts/host/install-host-disk-exporter.sh"

failures=0
tmp="$(mktemp -d)"
trap 'rm -rf "${tmp}"' EXIT
mkdir -p "${tmp}/bin"
export STUB_LOG="${tmp}/calls.log"

# df: GNU-style output for the path (last argument) from FAKE_DF_<n> lines
# "<path> <source> <fstype> <size> <avail> <target>"; fails for other paths.
# shellcheck disable=SC2016 # the stubs' $ expand when they run, not here
cat >"${tmp}/bin/df" <<'EOF'
#!/bin/bash
echo "df $*" >>"${STUB_LOG}"
path="${!#}"
while read -r p rest; do
    if [ "$p" = "$path" ]; then
        echo "Filesystem Type 1B-blocks Avail Mounted on"
        echo "$rest"
        exit 0
    fi
done <<<"${FAKE_DF:-}"
echo "df: $path: No such file or directory" >&2
exit 1
EOF
# mountpoint -q PATH succeeds for paths listed in FAKE_MOUNTS.
cat >"${tmp}/bin/mountpoint" <<'EOF'
#!/bin/bash
echo "mountpoint $*" >>"${STUB_LOG}"
for m in ${FAKE_MOUNTS:-}; do [ "$m" = "${!#}" ] && exit 0; done
exit 1
EOF
# virsh: one pool "eg" with two qcow2 volumes and an ISO.
# FAKE_VIRSH_DOWN=1 fails everything; FAKE_VOLINFO_FAILS=1 fails vol-info.
cat >"${tmp}/bin/virsh" <<'EOF'
#!/bin/bash
echo "virsh $*" >>"${STUB_LOG}"
[ "${FAKE_VIRSH_DOWN:-0}" = 1 ] && { echo "error: failed to connect" >&2; exit 1; }
[ "$1 $2" = "-c qemu:///system" ] || { echo "unexpected uri" >&2; exit 1; }
shift 2
case "$1" in
    pool-list) echo "eg"; echo ;;
    pool-refresh) echo "Pool $2 refreshed" ;;
    pool-info)
        printf 'Name:           eg\nState:          running\nCapacity:       1000000000000\nAllocation:     300000000000\nAvailable:      700000000000\n' ;;
    vol-list)
        printf ' Name              Path\n-----------------------------------------\n eg-cp-1-os.qcow2  /pool/eg-cp-1-os.qcow2\n eg-w-1-pg.qcow2   /pool/eg-w-1-pg.qcow2\n talos.iso         /pool/talos.iso\n\n' ;;
    vol-info)
        [ "${FAKE_VOLINFO_FAILS:-0}" = 1 ] && exit 1
        case "${!#}" in
            /pool/eg-cp-1-os.qcow2) c=42949672960 a=5000000000 ;;
            /pool/eg-w-1-pg.qcow2) c=214748364800 a=20000000000 ;;
            /pool/talos.iso) c=100000000 a=100000000 ;;
            *) exit 1 ;;
        esac
        printf 'Name:           x\nType:           file\nCapacity:       %s bytes\nAllocation:     %s bytes\n' "$c" "$a" ;;
    *) echo "unexpected virsh call: $*" >&2; exit 1 ;;
esac
EOF
real_id="$(command -v id)"
cat >"${tmp}/bin/id" <<EOF
#!/bin/bash
if [ "\$1" = -u ] && [ "\${FAKE_ROOT:-0}" = 1 ]; then echo 0; else exec ${real_id} "\$@"; fi
EOF
cat >"${tmp}/bin/ip" <<'EOF'
#!/bin/bash
echo "2: eg0    inet 192.168.250.1/24 brd 192.168.250.255 scope global eg0"
EOF
# Anything that would change the host fails loudly instead.
# shellcheck disable=SC2016 # the stubs' $ expand when they run, not here
for cmd in systemctl curl install chown; do
    printf '#!/bin/bash\necho "%s $*" >>"${STUB_LOG}"\necho "fake %s: refused in tests" >&2\nexit 1\n' \
        "${cmd}" "${cmd}" >"${tmp}/bin/${cmd}"
done
chmod +x "${tmp}/bin/"*
export PATH="${tmp}/bin:${PATH}"

pass() { echo "ok   $*"; }
fail() { echo "FAIL $*"; failures=$((failures + 1)); }

# run_script <name> <want exit> <text the output must contain> <command...>
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
# has <name> <file> <exact line>
has() {
    if grep -qxF -- "$3" "$2"; then pass "$1"; else fail "$1: no line '$3' in $2:"; sed 's/^/    /' "$2"; fi
}
lacks() {
    if grep -qF -- "$3" "$2"; then fail "$1: unexpected '$3' in $2"; else pass "$1"; fi
}

promtool_bin="${PROMTOOL:-$(command -v promtool || true)}"
lint() {
    if [ -n "${promtool_bin}" ]; then
        if "${promtool_bin}" check metrics <"$2" >/dev/null 2>"${tmp}/lint"; then
            pass "$1: promtool check metrics"
        else
            fail "$1: promtool check metrics:"; sed 's/^/    /' "${tmp}/lint"
        fi
    fi
}

export FAKE_DF="/ /dev/nvme0n1p2 ext4 500000000000 100000000000 /
/srv/econgraph/objectstore /dev/md0 btrfs 4000000000000 3000000000000 /srv/econgraph/objectstore"
export FAKE_MOUNTS="/srv/econgraph/objectstore"
prom="${tmp}/out.prom"

# --- host-disk-check.sh ---------------------------------------------------

run_script "check: all sources" 0 "" \
    bash "${check_script}" --output "${prom}" --path host_root=/ --mount objectstore=/srv/econgraph/objectstore --libvirt
has "check: root size" "${prom}" 'econgraph_host_filesystem_size_bytes{role="host_root",mountpoint="/",device="/dev/nvme0n1p2",fstype="ext4"} 500000000000'
has "check: root avail" "${prom}" 'econgraph_host_filesystem_avail_bytes{role="host_root",mountpoint="/",device="/dev/nvme0n1p2",fstype="ext4"} 100000000000'
has "check: objectstore present" "${prom}" 'econgraph_host_filesystem_present{role="objectstore",path="/srv/econgraph/objectstore"} 1'
has "check: objectstore avail" "${prom}" 'econgraph_host_filesystem_avail_bytes{role="objectstore",mountpoint="/srv/econgraph/objectstore",device="/dev/md0",fstype="btrfs"} 3000000000000'
has "check: pool size from pool-info" "${prom}" 'econgraph_host_filesystem_size_bytes{role="libvirt_pool",pool="eg"} 1000000000000'
has "check: pool avail from pool-info" "${prom}" 'econgraph_host_filesystem_avail_bytes{role="libvirt_pool",pool="eg"} 700000000000'
has "check: pool volume count" "${prom}" 'econgraph_libvirt_pool_volumes{pool="eg"} 3'
has "check: pool virtual sizes summed" "${prom}" 'econgraph_libvirt_pool_volumes_capacity_bytes{pool="eg"} 257798037760'
has "check: pool allocations summed" "${prom}" 'econgraph_libvirt_pool_volumes_allocation_bytes{pool="eg"} 25100000000'
has "check: libvirt up" "${prom}" 'econgraph_libvirt_up 1'
check_ts="$(grep '^econgraph_host_disk_check_timestamp_seconds ' "${prom}" | awk '{ print $2 }')"
if [[ "${check_ts}" =~ ^[0-9]+$ ]]; then pass "check: timestamp written"; else fail "check: timestamp written"; fi
if grep -q "pool-refresh eg" "${STUB_LOG}" && ! grep -qE "virsh .* (pool-(destroy|delete|undefine)|vol-(delete|wipe|resize))" "${STUB_LOG}"; then
    pass "check: virsh only reads"
else
    fail "check: virsh only reads:"; sed 's/^/    /' "${STUB_LOG}"
fi
if [ ! -e "${prom}.tmp" ]; then pass "check: no temporary file left"; else fail "check: no temporary file left"; fi
lint "check: all sources" "${prom}"

run_script "check: unmounted object store is missing, not its parent" 0 "not a mount point" \
    env FAKE_MOUNTS="" bash "${check_script}" --output "${prom}" --path host_root=/ --mount objectstore=/srv/econgraph/objectstore
has "check: objectstore missing" "${prom}" 'econgraph_host_filesystem_present{role="objectstore",path="/srv/econgraph/objectstore"} 0'
lacks "check: no objectstore size when unmounted" "${prom}" 'role="objectstore",mountpoint'
lacks "check: no libvirt metrics without --libvirt" "${prom}" 'econgraph_libvirt_up'
lint "check: unmounted" "${prom}"

run_script "check: df failure reports missing" 0 "cannot stat" \
    bash "${check_script}" --output "${prom}" --path scratch=/nowhere
has "check: df failure present 0" "${prom}" 'econgraph_host_filesystem_present{role="scratch",path="/nowhere"} 0'

run_script "check: libvirt unreachable" 0 "pool-list failed" \
    env FAKE_VIRSH_DOWN=1 bash "${check_script}" --output "${prom}" --path host_root=/ --libvirt
has "check: libvirt down" "${prom}" 'econgraph_libvirt_up 0'
has "check: root still reported" "${prom}" 'econgraph_host_filesystem_avail_bytes{role="host_root",mountpoint="/",device="/dev/nvme0n1p2",fstype="ext4"} 100000000000'
lint "check: libvirt unreachable" "${prom}"

run_script "check: vol-info failure" 0 "vol-info failed" \
    env FAKE_VOLINFO_FAILS=1 bash "${check_script}" --output "${prom}" --libvirt
has "check: pool missing on vol-info failure" "${prom}" 'econgraph_host_filesystem_present{role="libvirt_pool",pool="eg"} 0'
has "check: libvirt not up on vol-info failure" "${prom}" 'econgraph_libvirt_up 0'
lacks "check: no partial pool sums" "${prom}" 'econgraph_libvirt_pool_volumes_capacity_bytes{'

run_script "check: no output" 2 "usage" bash "${check_script}" --path host_root=/
run_script "check: relative path" 2 "ROLE=/absolute/path" bash "${check_script}" --output "${prom}" --path host_root=relative
run_script "check: bad role" 2 "lower-case" bash "${check_script}" --output "${prom}" --path 'host-root=/'

# --- install-host-disk-exporter.sh (dry run only) ---------------------------

run_script "install: listen address required" 2 "--listen-address must be" \
    env FAKE_ROOT=1 bash "${installer}" --dry-run
run_script "install: refuses all interfaces" 2 "--listen-address must be" \
    env FAKE_ROOT=1 bash "${installer}" --dry-run --listen-address :9100
run_script "install: refuses 0.0.0.0" 2 "not be reachable only" \
    env FAKE_ROOT=1 bash "${installer}" --dry-run --listen-address 0.0.0.0:9100
run_script "install: refuses loopback" 2 "not be reachable only" \
    env FAKE_ROOT=1 bash "${installer}" --dry-run --listen-address 127.0.0.1:9100
run_script "install: refuses a relative object store" 2 "absolute path" \
    env FAKE_ROOT=1 bash "${installer}" --dry-run --listen-address 192.168.250.1:9100 --objectstore srv/os
run_script "install: needs root" 1 "run with sudo" \
    env FAKE_ROOT=0 bash "${installer}" --dry-run --listen-address 192.168.250.1:9100
run_script "install: address must be on the host" 1 "is not an address on this host" \
    env FAKE_ROOT=1 bash "${installer}" --dry-run --listen-address 192.168.250.9:9100
run_script "install: dry run plans the units" 0 "+ systemctl enable --now econgraph-host-disk-check.timer" \
    env FAKE_ROOT=1 bash "${installer}" --dry-run --listen-address 192.168.250.1:9100
if grep -qE "^(systemctl|curl|install|chown) " "${STUB_LOG}"; then
    fail "install: dry run changed nothing:"; sed 's/^/    /' "${STUB_LOG}"
else
    pass "install: dry run changed nothing"
fi

if [ "${failures}" -ne 0 ]; then
    echo "${failures} failure(s)"
    exit 1
fi
echo "all host disk check tests passed"
