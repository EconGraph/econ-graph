#!/bin/bash

# Install the hourly btrfs backup job (docs/deployment/BACKUPS.md).
#
#   /usr/local/sbin/econgraph-backup-snapshots          <- scripts/host/backup-snapshots.sh
#   /etc/systemd/system/econgraph-backup-snapshots.service
#   /etc/systemd/system/econgraph-backup-snapshots.timer  (ON_CALENDAR below)
#
# then enables the timer. The script is copied (root-owned, 0755) rather
# than run from the checkout, so root never executes a file that the
# jmalicki account can change; re-run this installer after merging changes
# to backup-snapshots.sh. Safe to re-run. Nothing it installs deletes
# anything.
#
# --run-now also starts one run immediately instead of waiting for the timer.
#
# Usage: sudo scripts/host/install-backup-timer.sh [--dry-run] [--run-now]

set -euo pipefail

ON_CALENDAR="hourly"
UNIT="econgraph-backup-snapshots"
BIN="/usr/local/sbin/$UNIT"
UNIT_DIR="/etc/systemd/system"
SRC="$(cd "$(dirname "$(readlink -f "$0")")" && pwd)/backup-snapshots.sh"

DRY_RUN="false"
RUN_NOW="false"
for arg in "$@"; do
    case "$arg" in
        --dry-run|-n) DRY_RUN="true" ;;
        --run-now) RUN_NOW="true" ;;
        *) echo "usage: $0 [--dry-run] [--run-now]" >&2; exit 2 ;;
    esac
done

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
[ -f "$SRC" ] || { echo "error: $SRC not found" >&2; exit 1; }

# True when <path> is a regular file (not a symlink) owned by root:root with
# exactly <mode>: content alone is not enough, since a file the jmalicki
# account could write would let it change what root runs.
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

if root_owned "$BIN" 0755 && cmp -s "$SRC" "$BIN"; then
    echo "= $BIN already up to date"
else
    run install -o root -g root -m 0755 "$SRC" "$BIN"
fi

install_file "[Unit]
Description=econ-graph btrfs snapshots of the production data, copied to the backup pool
Documentation=file://$(dirname "$(dirname "$(dirname "$SRC")")")/docs/deployment/BACKUPS.md
RequiresMountsFor=/srv/econgraph/data-plane /srv/econgraph/app-prod /srv/econgraph/snapshots

[Service]
Type=oneshot
ExecStart=$BIN
Nice=10
IOSchedulingClass=idle" "$UNIT_DIR/$UNIT.service" 0644

install_file "[Unit]
Description=Run $UNIT.service $ON_CALENDAR

[Timer]
OnCalendar=$ON_CALENDAR
Persistent=true
AccuracySec=1min

[Install]
WantedBy=timers.target" "$UNIT_DIR/$UNIT.timer" 0644

run systemctl daemon-reload
run systemctl enable --now "$UNIT.timer"
if [ "$RUN_NOW" = "true" ]; then
    run systemctl start "$UNIT.service"
fi

if [ "$DRY_RUN" = "false" ]; then
    systemctl list-timers "$UNIT.timer" --no-pager
fi
