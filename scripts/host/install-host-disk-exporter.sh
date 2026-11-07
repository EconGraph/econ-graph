#!/bin/bash

# Install host disk monitoring (docs/deployment/HOST_DISK_MONITORING.md):
#
#   /usr/local/sbin/econgraph-node-exporter     <- official node_exporter, SHA-256 pinned below
#   /usr/local/sbin/econgraph-host-disk-check   <- scripts/host/host-disk-check.sh
#   /var/lib/econgraph/node-exporter-textfile/  <- econgraph_host_disk.prom
#   /etc/systemd/system/econgraph-node-exporter.service
#   /etc/systemd/system/econgraph-host-disk-check.service
#   /etc/systemd/system/econgraph-host-disk-check.timer   (every minute)
#
# node_exporter listens only on --listen-address, the host's address on the
# bridge the Talos VMs use, so in-cluster Prometheus can scrape it; it never
# binds all interfaces. Only the collectors the disk alerts need are on:
# filesystem, meminfo and vmstat (swap), btrfs, and textfile.
#
# The disk check reports the host root (/), the object store mount (unless
# --without-objectstore) and every active libvirt storage pool (unless
# --without-libvirt). Files are copied root-owned rather than run from the
# checkout, so root never runs a file the operator account can change; re-run
# this after merging changes to host-disk-check.sh. Safe to re-run. Nothing
# it installs writes anywhere but its textfile directory, and nothing deletes
# anything. It does not change any firewall: see the doc.
#
# Usage: sudo scripts/host/install-host-disk-exporter.sh --listen-address IP:PORT
#            [--objectstore PATH | --without-objectstore] [--without-libvirt]
#            [--archive FILE] [--dry-run]
#
# --archive uses an already-downloaded node_exporter release archive instead
# of fetching it; either way its SHA-256 must match.

set -euo pipefail

NE_VERSION="1.12.1"
NE_ARCHIVE="node_exporter-${NE_VERSION}.linux-amd64.tar.gz"
NE_URL="https://github.com/prometheus/node_exporter/releases/download/v${NE_VERSION}/${NE_ARCHIVE}"
NE_ARCHIVE_SHA256="b51d8a76aa2a9156a55d501aca6276fae09e262259a5e4e831d2c2222f084e63"
NE_BINARY_SHA256="1108f7453ecfe4a72f131c73b69537c171840ad4f1713a2402328ad56cf12e09"

EXPORTER_UNIT="econgraph-node-exporter"
CHECK_UNIT="econgraph-host-disk-check"
SBIN="/usr/local/sbin"
UNIT_DIR="/etc/systemd/system"
TEXTFILE_DIR="/var/lib/econgraph/node-exporter-textfile"
SRC="$(cd "$(dirname "$(readlink -f "$0")")" && pwd)/host-disk-check.sh"

usage() {
    echo "usage: $0 --listen-address IP:PORT [--objectstore PATH | --without-objectstore] [--without-libvirt] [--archive FILE] [--dry-run]" >&2
    exit 2
}

DRY_RUN="false"
listen=""
objectstore="/srv/econgraph/objectstore"
libvirt="true"
archive=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --dry-run | -n) DRY_RUN="true"; shift ;;
        --listen-address) [ "$#" -ge 2 ] || usage; listen="$2"; shift 2 ;;
        --objectstore) [ "$#" -ge 2 ] || usage; objectstore="$2"; shift 2 ;;
        --without-objectstore) objectstore=""; shift ;;
        --without-libvirt) libvirt="false"; shift ;;
        --archive) [ "$#" -ge 2 ] || usage; archive="$2"; shift 2 ;;
        *) usage ;;
    esac
done

# Only an explicit IPv4 address and port: never all interfaces.
if ! [[ "$listen" =~ ^([0-9]{1,3}(\.[0-9]{1,3}){3}):([0-9]{1,5})$ ]]; then
    echo "error: --listen-address must be the host's bridge IPv4 address and a port, e.g. 192.168.250.1:9100" >&2
    exit 2
fi
listen_ip="${BASH_REMATCH[1]}"
case "$listen_ip" in
    0.0.0.0 | 127.*)
        echo "error: --listen-address $listen_ip would not be reachable only from the VM bridge; use the host's bridge address" >&2
        exit 2
        ;;
esac
if [ -n "$objectstore" ] && ! [[ "$objectstore" =~ ^/[A-Za-z0-9._/-]*$ ]]; then
    echo "error: --objectstore must be an absolute path without spaces" >&2
    exit 2
fi

if [ "$(id -u)" -ne 0 ]; then
    echo "error: run with sudo" >&2
    exit 1
fi
[ -f "$SRC" ] || { echo "error: $SRC not found" >&2; exit 1; }
# The address must already be on this host, or node_exporter cannot bind it.
if ! ip -o -4 addr show | awk '{ split($4, a, "/"); print a[1] }' | grep -qxF "$listen_ip"; then
    echo "error: $listen_ip is not an address on this host (ip -4 addr show)" >&2
    exit 1
fi
if [ "$libvirt" = true ] && ! command -v virsh >/dev/null; then
    echo "error: virsh not found; install libvirt clients or pass --without-libvirt" >&2
    exit 1
fi
if [ -n "$objectstore" ] && ! mountpoint -q -- "$objectstore"; then
    echo "warning: $objectstore is not mounted yet; the check reports it missing (and EconGraphHostDiskPathMissing fires) until it is, or re-run with --without-objectstore" >&2
fi

run() {
    echo "+ $*"
    if [ "$DRY_RUN" = "false" ]; then
        "$@"
    fi
}

# True when <path> is a regular file (not a symlink) owned by root:root with
# exactly <mode>.
root_owned() {
    local path="$1" mode="$2"
    [ -f "$path" ] && [ ! -L "$path" ] \
        && [ "$(stat -c '%u:%g %a' "$path")" = "0:0 ${mode#0}" ]
}

# install_file <content> <path> <mode>
install_file() {
    local content="$1" path="$2" mode="$3"
    if root_owned "$path" "$mode" && [ "$(cat "$path")" = "$content" ]; then
        echo "= $path already up to date"
        return
    fi
    echo "+ write $path (mode $mode)"
    if [ "$DRY_RUN" = "false" ]; then
        rm -f "$path.tmp"
        (umask 077 && printf '%s\n' "$content" >"$path.tmp")
        chown root:root "$path.tmp"
        chmod "$mode" "$path.tmp"
        mv -f "$path.tmp" "$path"
    fi
}

sha256() { sha256sum "$1" | awk '{ print $1 }'; }

changed="false"

# node_exporter: the exact release binary, checked before and after extraction.
exporter="$SBIN/$EXPORTER_UNIT"
if root_owned "$exporter" 0755 && [ "$(sha256 "$exporter")" = "$NE_BINARY_SHA256" ]; then
    echo "= $exporter already node_exporter $NE_VERSION"
else
    echo "+ install node_exporter $NE_VERSION to $exporter"
    if [ "$DRY_RUN" = "false" ]; then
        work="$(mktemp -d)"
        trap 'rm -rf "$work"' EXIT
        if [ -n "$archive" ]; then
            cp -- "$archive" "$work/$NE_ARCHIVE"
        else
            curl -fsSL --max-filesize 67108864 -o "$work/$NE_ARCHIVE" "$NE_URL"
        fi
        if [ "$(sha256 "$work/$NE_ARCHIVE")" != "$NE_ARCHIVE_SHA256" ]; then
            echo "error: $NE_ARCHIVE SHA-256 mismatch; not installing" >&2
            exit 1
        fi
        member="node_exporter-${NE_VERSION}.linux-amd64/node_exporter"
        tar -xzf "$work/$NE_ARCHIVE" -C "$work" --no-same-owner -- "$member"
        if [ -L "$work/$member" ] || [ "$(sha256 "$work/$member")" != "$NE_BINARY_SHA256" ]; then
            echo "error: extracted node_exporter SHA-256 mismatch; not installing" >&2
            exit 1
        fi
        install -o root -g root -m 0755 "$work/$member" "$exporter.tmp"
        mv -f "$exporter.tmp" "$exporter"
        rm -rf "$work"
        trap - EXIT
    fi
    changed="true"
fi

check="$SBIN/$CHECK_UNIT"
if root_owned "$check" 0755 && cmp -s "$SRC" "$check"; then
    echo "= $check already up to date"
else
    run install -o root -g root -m 0755 "$SRC" "$check.tmp"
    run mv -f "$check.tmp" "$check"
fi

run mkdir -p "$(dirname "$TEXTFILE_DIR")"
run install -d -o root -g root -m 0755 "$TEXTFILE_DIR"

check_args="--output $TEXTFILE_DIR/econgraph_host_disk.prom --path host_root=/"
if [ -n "$objectstore" ]; then
    check_args+=" --mount objectstore=$objectstore"
fi
if [ "$libvirt" = true ]; then
    check_args+=" --libvirt"
fi
doc="file://$(dirname "$(dirname "$(dirname "$SRC")")")/docs/deployment/HOST_DISK_MONITORING.md"

before="$(cat "$UNIT_DIR/$EXPORTER_UNIT.service" 2>/dev/null || true)"
install_file "[Unit]
Description=econ-graph host node_exporter (disk, swap and textfile metrics for in-cluster Prometheus)
Documentation=$doc
After=network-online.target
Wants=network-online.target

[Service]
ExecStart=$exporter --web.listen-address=$listen --web.disable-exporter-metrics --collector.disable-defaults --collector.filesystem --collector.meminfo --collector.vmstat --collector.btrfs --collector.textfile --collector.textfile.directory=$TEXTFILE_DIR
DynamicUser=yes
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=yes
# The bridge address may come up after boot; keep trying until it does.
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target" "$UNIT_DIR/$EXPORTER_UNIT.service" 0644
if [ "$before" != "$(cat "$UNIT_DIR/$EXPORTER_UNIT.service" 2>/dev/null || true)" ]; then
    changed="true"
fi

install_file "[Unit]
Description=econ-graph host disk check (writes $TEXTFILE_DIR/econgraph_host_disk.prom)
Documentation=$doc

[Service]
Type=oneshot
ExecStart=$check $check_args
TimeoutStartSec=120
# virsh may want a writable home for its cache; give it a private one.
RuntimeDirectory=$CHECK_UNIT
Environment=HOME=%t/$CHECK_UNIT XDG_CACHE_HOME=%t/$CHECK_UNIT
Nice=10
IOSchedulingClass=idle
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=read-only
ReadWritePaths=$TEXTFILE_DIR
PrivateTmp=yes" "$UNIT_DIR/$CHECK_UNIT.service" 0644

install_file "[Unit]
Description=Run $CHECK_UNIT.service every minute

[Timer]
OnBootSec=1min
OnUnitActiveSec=1min
AccuracySec=10s

[Install]
WantedBy=timers.target" "$UNIT_DIR/$CHECK_UNIT.timer" 0644

run systemctl daemon-reload
run systemctl start "$CHECK_UNIT.service"
run systemctl enable --now "$CHECK_UNIT.timer"
run systemctl enable "$EXPORTER_UNIT.service"
if [ "$changed" = true ]; then
    run systemctl restart "$EXPORTER_UNIT.service"
else
    run systemctl start "$EXPORTER_UNIT.service"
fi

if [ "$DRY_RUN" = "false" ]; then
    systemctl --no-pager status "$EXPORTER_UNIT.service" | head -n 5 || true
    echo "check: curl -s http://$listen/metrics | grep '^econgraph_'"
fi
