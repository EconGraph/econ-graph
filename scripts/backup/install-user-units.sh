#!/bin/bash
# Install the backup scripts and their systemd --user timers for the
# current user (no sudo).
#
# Copies scripts/backup/*.sh to ~/.local/lib/econgraph-backup/ (so the
# timers run a fixed, reviewed copy, not whatever branch the checkout is
# on), the units in scripts/backup/systemd/ to ~/.config/systemd/user/, and
# enables:
#   econgraph-dump.timer              nightly 10:00 UTC
#   econgraph-restore-drill.timer     Sundays 11:00 UTC
#   econgraph-backup-freshness.timer  hourly
#   econgraph-offsite.timer           daily 10:45 UTC, only if
#                                     ~/.config/econgraph/restic.env exists
#
# User timers keep running after logout only with lingering enabled
# (`loginctl show-user "$USER" -p Linger` must say Linger=yes; it does on
# threadripper).
#
# Re-run after merging changes to these scripts. Safe to re-run.
#
# Usage: install-user-units.sh [--dry-run] [--no-enable]

set -euo pipefail

SRC_DIR="$(dirname "$(readlink -f "$0")")"
LIB_DIR="${HOME}/.local/lib/econgraph-backup"
UNIT_DIR="${XDG_CONFIG_HOME:-${HOME}/.config}/systemd/user"

DRY_RUN="false"
ENABLE="true"
for arg in "$@"; do
    case "$arg" in
        --dry-run|-n) DRY_RUN="true" ;;
        --no-enable) ENABLE="false" ;;
        *) echo "usage: $0 [--dry-run] [--no-enable]" >&2; exit 2 ;;
    esac
done

run() {
    echo "+ $*"
    if [ "$DRY_RUN" = "false" ]; then
        "$@"
    fi
}

if [ "$(id -u)" -eq 0 ]; then
    echo "error: run as the user that owns the backups, not root" >&2
    exit 1
fi

linger="$(loginctl show-user "$(id -un)" -p Linger --value 2>/dev/null || echo unknown)"
if [ "$linger" != "yes" ]; then
    echo "warning: Linger=$linger; the timers only run while $(id -un) is logged in." >&2
    echo "         Enabling lingering needs root (loginctl enable-linger $(id -un))." >&2
fi

run mkdir -p "$LIB_DIR" "$UNIT_DIR"
for f in "$SRC_DIR"/*.sh; do
    case "$(basename "$f")" in install-user-units.sh) continue ;; esac
    run install -m 0755 "$f" "$LIB_DIR/"
done
rev="$(git -C "$SRC_DIR" rev-parse --short HEAD 2>/dev/null || echo unknown)"
echo "+ record installed revision $rev in $LIB_DIR/INSTALLED_FROM"
if [ "$DRY_RUN" = "false" ]; then
    printf '%s %s\n' "$rev" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >"$LIB_DIR/INSTALLED_FROM"
fi
for u in "$SRC_DIR"/systemd/*.service "$SRC_DIR"/systemd/*.timer; do
    run install -m 0644 "$u" "$UNIT_DIR/"
done
run systemctl --user daemon-reload

if [ "$ENABLE" = "true" ]; then
    timers=(econgraph-dump.timer econgraph-restore-drill.timer econgraph-backup-freshness.timer)
    if [ -f "${HOME}/.config/econgraph/restic.env" ]; then
        timers+=(econgraph-offsite.timer)
    else
        echo "= off-site not configured (no ~/.config/econgraph/restic.env); econgraph-offsite.timer left disabled"
    fi
    run systemctl --user enable --now "${timers[@]}"
    if [ "$DRY_RUN" = "false" ]; then
        systemctl --user list-timers 'econgraph-*' --no-pager
    fi
fi
