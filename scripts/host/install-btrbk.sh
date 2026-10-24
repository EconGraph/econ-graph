#!/bin/bash

# Install btrbk, install the repo's btrbk config and run it hourly
# (docs/deployment/BACKUPS.md).
#
#   1. apt-get install btrbk (skipped if already installed)
#   2. /etc/btrbk/btrbk.conf <- scripts/backup/btrbk/btrbk.conf
#      (an existing different file is kept as btrbk.conf.<timestamp>)
#   3. /etc/systemd/system/btrbk.timer.d/econgraph-hourly.conf: the packaged
#      timer runs daily; this makes it hourly
#   4. btrbk dryrun with the new config, enable --now btrbk.timer and
#      start one run immediately
#
# Send/receive to the Samsung 870 SATA pool is switched on automatically
# when /srv/econgraph-backups is mounted with its btrbk/ directory (see
# scripts/host/setup-backup-pool.sh); --no-backup-pool leaves it off.
#
# Requires the subvolumes from scripts/host/setup-subvolumes.sh to be
# mounted. Safe to re-run.
#
# Usage: sudo scripts/host/install-btrbk.sh [--dry-run] [--no-backup-pool]

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$(readlink -f "$0")")/../.." && pwd)"
SRC_CONF="$REPO_ROOT/scripts/backup/btrbk/btrbk.conf"
DEST_CONF="/etc/btrbk/btrbk.conf"
DROPIN_DIR="/etc/systemd/system/btrbk.timer.d"
DROPIN="$DROPIN_DIR/econgraph-hourly.conf"
POOL_MOUNT="/srv/econgraph-backups"
SATA_TARGET="$POOL_MOUNT/btrbk"
REQUIRED_MOUNTS=(/srv/econgraph/data-plane /srv/econgraph/app-prod /srv/econgraph/snapshots)

DRY_RUN="false"
ENABLE_SATA="auto"
for arg in "$@"; do
    case "$arg" in
        --dry-run|-n) DRY_RUN="true" ;;
        --no-backup-pool) ENABLE_SATA="false" ;;
        *) echo "usage: $0 [--dry-run] [--no-backup-pool]" >&2; exit 2 ;;
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

[ -f "$SRC_CONF" ] || { echo "error: $SRC_CONF not found" >&2; exit 1; }

for m in "${REQUIRED_MOUNTS[@]}"; do
    if ! mountpoint -q "$m"; then
        echo "error: $m is not mounted; run scripts/host/setup-subvolumes.sh first" >&2
        exit 1
    fi
done

if [ "$ENABLE_SATA" = "auto" ]; then
    if mountpoint -q "$POOL_MOUNT" && [ -d "$SATA_TARGET" ]; then
        ENABLE_SATA="true"
    else
        ENABLE_SATA="false"
        echo "= $SATA_TARGET not available (scripts/host/setup-backup-pool.sh not run yet)"
    fi
fi

# 1. package
if dpkg-query -W -f='${Status}' btrbk 2>/dev/null | grep -q "install ok installed"; then
    echo "= btrbk already installed ($(dpkg-query -W -f='${Version}' btrbk))"
else
    run apt-get install -y btrbk
fi

# 2. config (rendered to a temp file, so enabling the target never edits the repo)
rendered="$(mktemp)"
trap 'rm -f "$rendered"' EXIT
if [ "$ENABLE_SATA" = "true" ]; then
    sed 's/^#SATA# //' "$SRC_CONF" >"$rendered"
    echo "= SATA send/receive target enabled: $SATA_TARGET"
else
    cp "$SRC_CONF" "$rendered"
    echo "= SATA send/receive target left disabled"
fi

if [ -f "$DEST_CONF" ] && cmp -s "$rendered" "$DEST_CONF"; then
    echo "= $DEST_CONF already up to date"
else
    if [ -f "$DEST_CONF" ]; then
        run cp -a "$DEST_CONF" "$DEST_CONF.$(date +%Y%m%dT%H%M%S)"
    fi
    run install -D -m 0644 "$rendered" "$DEST_CONF"
fi

# 3. hourly timer
dropin_content="# Installed by econ-graph scripts/host/install-btrbk.sh: run btrbk hourly.
[Timer]
OnCalendar=
OnCalendar=hourly
RandomizedDelaySec=0
AccuracySec=1min"
if [ -f "$DROPIN" ] && [ "$(cat "$DROPIN")" = "$dropin_content" ]; then
    echo "= $DROPIN already up to date"
else
    run mkdir -p "$DROPIN_DIR"
    echo "+ write $DROPIN"
    if [ "$DRY_RUN" = "false" ]; then
        printf '%s\n' "$dropin_content" >"$DROPIN"
    fi
fi

# 4. validate, then enable
if command -v btrbk >/dev/null; then
    echo "+ btrbk -c $rendered dryrun"
    btrbk -c "$rendered" dryrun
else
    echo "= btrbk not installed yet (dry run); skipping btrbk dryrun"
fi
run systemctl daemon-reload
run systemctl enable --now btrbk.timer
# Take the first snapshots now rather than at the top of the next hour.
run systemctl start btrbk.service

if [ "$DRY_RUN" = "false" ]; then
    systemctl list-timers btrbk.timer --no-pager
fi
