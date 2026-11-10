#!/bin/bash
# Install the backup scripts and their systemd --user timers for the
# current user (no sudo).
#
# Copies scripts/backup/*.sh to ~/.local/lib/econgraph-backup/ (so the
# timers run a fixed, reviewed copy, not whatever branch the checkout is
# on), the units in scripts/backup/systemd/ to ~/.config/systemd/user/, and
# enables:
#   econgraph-dump.timer              nightly 10:00 UTC
#   econgraph-restore-drill.timer     disabled: encrypted restore is interactive
#   econgraph-backup-freshness.timer  hourly
#   econgraph-offsite.timer           daily 10:45 UTC, only if
#                                     ~/.config/econgraph/restic.env exists
# Every backup service has OnFailure=econgraph-notify@%N.service, which runs
# notify.sh: it tells a person through the channels in
# ~/.config/econgraph/notify.env (docs/deployment/BACKUPS.md, "Notifications").
# Without that file a failure reaches nobody; the installer says so.
#
# User timers keep running after logout only with lingering enabled
# (`loginctl show-user "$USER" -p Linger` must say Linger=yes). If it is off,
# the installer runs `loginctl --no-ask-password enable-linger` for this user
# (polkit usually lets a user do that for themselves); if lingering is still
# off it stops with an error, before installing anything. --allow-no-linger
# skips both: lingering is left as it is and the units are installed anyway.
# With restic.env but no recorded off-site run yet, it starts the off-site
# copy once, so the freshness check has an age to read.
#
# Re-run after merging changes to these scripts. Safe to re-run.
#
# Usage: install-user-units.sh [--dry-run] [--no-enable] [--allow-no-linger]

set -euo pipefail

SRC_DIR="$(dirname "$(readlink -f "$0")")"
LIB_DIR="${HOME}/.local/lib/econgraph-backup"
UNIT_DIR="${XDG_CONFIG_HOME:-${HOME}/.config}/systemd/user"

CONFIG_DIR="${ECONGRAPH_CONFIG_DIR:-${HOME}/.config/econgraph}"

DRY_RUN="false"
ENABLE="true"
ALLOW_NO_LINGER="false"
for arg in "$@"; do
    case "$arg" in
        --dry-run|-n) DRY_RUN="true" ;;
        --no-enable) ENABLE="false" ;;
        --allow-no-linger) ALLOW_NO_LINGER="true" ;;
        *) echo "usage: $0 [--dry-run] [--no-enable] [--allow-no-linger]" >&2; exit 2 ;;
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

user="$(id -un)"
linger_state() { loginctl show-user "$user" -p Linger --value 2>/dev/null || echo unknown; }
linger="$(linger_state)"
if [ "$linger" != "yes" ] && [ "$ALLOW_NO_LINGER" = "false" ]; then
    echo "Linger=$linger: the timers would stop whenever $user logs out; enabling lingering"
    # --no-ask-password: polkit either allows it or refuses; never a prompt.
    run loginctl --no-ask-password enable-linger "$user" || true
    [ "$DRY_RUN" = "true" ] || linger="$(linger_state)"
fi
if [ "$linger" != "yes" ] && [ "$DRY_RUN" = "false" ]; then
    if [ "$ALLOW_NO_LINGER" = "true" ]; then
        echo "warning: Linger=$linger; the timers only run while $user is logged in (--allow-no-linger)." >&2
    else
        echo "error: Linger=$linger, and loginctl enable-linger $user did not turn it on: the backup timers" >&2
        echo "       would stop whenever $user logs out. Run 'sudo loginctl enable-linger $user' and re-run," >&2
        echo "       or pass --allow-no-linger to install anyway. Nothing was installed." >&2
        exit 1
    fi
fi
if [ ! -f "$CONFIG_DIR/notify.env" ]; then
    echo "warning: no $CONFIG_DIR/notify.env: a failed or stale backup will reach nobody." >&2
    echo "         Set NOTIFY_SLACK_WEBHOOK_URL or NOTIFY_COMMAND (and NOTIFY_HEARTBEAT_URL) there," >&2
    echo "         mode 600, then run $LIB_DIR/notify.sh test (docs/deployment/BACKUPS.md, \"Notifications\")." >&2
fi

run mkdir -p "$LIB_DIR" "$UNIT_DIR"
for f in "$SRC_DIR"/*.sh "$SRC_DIR"/unpack-backup.py; do
    case "$(basename "$f")" in install-user-units.sh) continue ;; esac
    run install -m 0755 "$f" "$LIB_DIR/"
done
# The nightly writer shares the deploy writer's verified payload and helpers.
run mkdir -p "$LIB_DIR/kind-lib"
for helper in kind-env.sh cluster-role.sh kind-version.sh agent-access.sh; do
    [[ -f "$SRC_DIR/../deploy/lib/$helper" ]] || continue
    run install -m 0755 "$SRC_DIR/../deploy/lib/$helper" "$LIB_DIR/kind-lib/"
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
    # Private identities are never installed for a timer. A restore drill is
    # interactive after public-key encryption, rather than a false PASS.
    run systemctl --user disable --now econgraph-restore-drill.timer
    timers=(econgraph-dump.timer econgraph-backup-freshness.timer)
    if [ -f "${HOME}/.config/econgraph/restic.env" ]; then
        timers+=(econgraph-offsite.timer)
    else
        echo "= off-site not configured (no ~/.config/econgraph/restic.env); econgraph-offsite.timer left disabled"
    fi
    run systemctl --user enable --now "${timers[@]}"
    # check-freshness.sh reads the off-site copy's age from offsite.last, which only
    # offsite-restic.sh backup writes. Without one yet (an off-site copy set up before
    # it recorded its runs), run it once now rather than leave the check failing until
    # the next 10:45 UTC run.
    state_dir="${ECONGRAPH_STATE_DIR:-${XDG_STATE_HOME:-${HOME}/.local/state}/econgraph}"
    if [ -f "${HOME}/.config/econgraph/restic.env" ] && [ ! -f "$state_dir/offsite.last" ]; then
        echo "= no off-site run recorded yet ($state_dir/offsite.last); starting econgraph-offsite.service once"
        run systemctl --user start --no-block econgraph-offsite.service
    fi
    echo "= Run $LIB_DIR/check-freshness.sh now: whatever it reports as STALE or MISSING (a restore"
    echo "  drill older than 8 days, say) is reported to a person from the next hourly check on."
    if [ "$DRY_RUN" = "false" ]; then
        systemctl --user list-timers 'econgraph-*' --no-pager
    fi
fi
