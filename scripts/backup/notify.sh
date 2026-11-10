#!/bin/bash
# Tell a person that a backup unit failed, or ping the dead-man's switch.
#
# Usage: notify.sh failure UNIT   a unit failed: every backup unit's OnFailure=
#                                 starts econgraph-notify@<unit>.service, which runs this;
#                                 a repeat of the same failure is sent once a day
#        notify.sh ok UNIT        UNIT succeeded (its ExecStartPost=): its next failure
#                                 is reported at once
#        notify.sh heartbeat      ping NOTIFY_HEARTBEAT_URL, if set (after each passing
#                                 freshness check: econgraph-backup-freshness.service)
#        notify.sh test           send a test message through every configured channel
#
# There is no Alertmanager on the kind host, so this is the only path from a
# failed or stale backup to a person. The channels come from
# ~/.config/econgraph/notify.env (mode 600: it holds webhook URLs, which are
# credentials; never commit it). Set at least one of the first two:
#   NOTIFY_SLACK_WEBHOOK_URL=<Slack incoming webhook URL>
#       a Slack incoming webhook (anything that takes Slack's {"text": ...} JSON)
#   NOTIFY_COMMAND='mail -s "econgraph backup" someone@example.com'
#       run by bash, with the message on stdin
# and, so that a host that is down, asleep or whose timers stopped is noticed too:
#   NOTIFY_HEARTBEAT_URL=<the ping URL of a dead-man switch check>
#       a dead-man's switch (healthchecks.io or similar) that alerts when the
#       hourly pings stop; set its period to 1 hour and its grace to about 2.
#
# failure and test exit 1 when no channel is configured or none delivered, so
# the notify unit itself shows as failed (journalctl --user -u 'econgraph-notify@*').
# heartbeat without a URL does nothing and succeeds.
# Tests: scripts/backup/test-notify.sh. See docs/deployment/BACKUPS.md, "Notifications".

set -euo pipefail

# shellcheck source=scripts/backup/lib.sh
source "$(dirname "$(readlink -f "$0")")/lib.sh"

NOTIFY_ENV="${NOTIFY_ENV:-${ECONGRAPH_CONFIG_DIR}/notify.env}"
NOTIFY_SLACK_WEBHOOK_URL=""
NOTIFY_COMMAND=""
NOTIFY_HEARTBEAT_URL=""
if [[ -f "${NOTIFY_ENV}" ]]; then
    perms="$(stat -c %a "${NOTIFY_ENV}")"
    [[ "${perms}" =~ ^[0-7]00$ ]] || die "${NOTIFY_ENV} is mode ${perms}; chmod 600 it"
    # shellcheck disable=SC1090
    source "${NOTIFY_ENV}"
fi

# post URL: POST stdin to URL. The URL goes to curl through a config on a file
# descriptor, not the command line, so other users can't read it with ps.
post() {
    curl -fsS --max-time 20 --retry 3 -o /dev/null -H 'Content-Type: application/json' \
        --data-binary @- -K <(printf 'url = "%s"\n' "$1")
}

# send MESSAGE: through every configured channel; fails unless one delivered.
send() {
    local message="$1" sent=0 tried=0
    if [[ -n "${NOTIFY_SLACK_WEBHOOK_URL}" ]]; then
        tried=1
        if python3 -c 'import json, sys; print(json.dumps({"text": sys.argv[1]}))' "${message}" |
            post "${NOTIFY_SLACK_WEBHOOK_URL}"; then
            sent=1
        else
            log "the Slack webhook did not accept the message"
        fi
    fi
    if [[ -n "${NOTIFY_COMMAND}" ]]; then
        tried=1
        # Operator-trusted configuration: notify.env is sourced as shell code
        # anyway, and only when no other user can write it (mode checked above).
        if printf '%s\n' "${message}" | bash -c "${NOTIFY_COMMAND}"; then
            sent=1
        else
            log "NOTIFY_COMMAND failed"
        fi
    fi
    (( tried )) || die "no notification channel: set NOTIFY_SLACK_WEBHOOK_URL or NOTIFY_COMMAND in ${NOTIFY_ENV}; nobody was told: ${message%%$'\n'*}"
    (( sent )) || die "no notification channel delivered; nobody was told: ${message%%$'\n'*}"
    log "notified: ${message%%$'\n'*}"
}

# unit_arg NAME: NAME checked for unit-name characters, with .service added if it
# has no suffix (OnFailure= passes %N).
unit_arg() {
    local unit="$1"
    [[ "${unit}" =~ ^[A-Za-z0-9@._:-]+$ ]] || die "usage: $(basename "$0") failure|ok UNIT"
    [[ "${unit}" == *.* ]] || unit="${unit}.service"
    printf '%s' "${unit}"
}

# The last failure reported for a unit: its signature and when it was sent. A
# failure that says the same as the last one is not sent again until
# NOTIFY_REMIND_HOURS have passed, so a check that stays failing (the hourly
# freshness check) sends one message when it starts failing, one whenever what
# fails changes, and a daily reminder, not one every hour. `ok UNIT` (each unit's
# ExecStartPost=, after a success) forgets it.
NOTIFY_REMIND_HOURS="${NOTIFY_REMIND_HOURS:-24}"
state_file() { printf '%s/notify-%s.last' "${ECONGRAPH_STATE_DIR}" "$1"; }

cmd="${1:-}"
case "${cmd}" in
    failure)
        unit="$(unit_arg "${2:-}")"
        # The lines of the failed run that say what is wrong (the freshness check's
        # STALE, MISSING and LOWSPACE lines; the scripts' ERROR lines), not its whole log.
        # systemd gives an OnFailure= unit the failed run's invocation ID; without
        # one, or when the journal has no lines under it, the last 3 hours of the
        # unit's journal are read, which may include an earlier run.
        details=""
        if command -v journalctl >/dev/null; then
            journal=""
            if [[ "${MONITOR_INVOCATION_ID:-}" =~ ^[0-9a-f]{32}$ ]]; then
                journal="$(journalctl --user "_SYSTEMD_INVOCATION_ID=${MONITOR_INVOCATION_ID}" + \
                    "USER_INVOCATION_ID=${MONITOR_INVOCATION_ID}" -o cat --no-pager 2>/dev/null || true)"
            fi
            if [[ -z "${journal}" ]]; then
                journal="$(journalctl --user -u "${unit}" --since -3h -o cat --no-pager 2>/dev/null || true)"
            fi
            details="$(grep -E '^(STALE|MISSING|FAIL|LOWSPACE|NOTCONF) |ERROR|BACKUP CHECK FAIL' <<<"${journal}" |
                tail -n 15 || true)"
        fi
        # Ages, sizes and times change every run; what fails is what counts.
        signature="$(printf '%s\n%s' "${unit}" "${details}" | tr -d '0-9' | sha256sum | cut -d' ' -f1)"
        state="$(state_file "${unit}")"
        last_signature="" last_sent=0
        if [[ -f "${state}" ]]; then
            last_signature="$(sed -n 's/^signature=//p' "${state}")"
            last_sent="$(sed -n 's/^sent_epoch=//p' "${state}")"
            [[ "${last_sent}" =~ ^[0-9]+$ ]] || last_sent=0
        fi
        now="$(date +%s)"
        # Without details every failure of the unit looks the same, so it is always
        # sent. The reminder allows an hour of slack, so a daily unit whose timer
        # jitters still reminds daily, not every second day.
        if [[ -n "${details}" && "${signature}" == "${last_signature}" ]] &&
            (( now - last_sent < NOTIFY_REMIND_HOURS * 3600 - 3600 )); then
            log "${unit} failed as it did at $(date -u -d "@${last_sent}" +%Y-%m-%dT%H:%MZ), already reported; reminder after ${NOTIFY_REMIND_HOURS}h"
            exit 0
        fi
        reminder=""
        [[ -n "${details}" && "${signature}" == "${last_signature}" ]] && reminder=" (still failing; reminder)"
        send "econgraph backup: ${unit} failed${reminder} on $(hostname) at $(date -u +%Y-%m-%dT%H:%MZ).
${details:+${details}
}Check: journalctl --user -u ${unit} (docs/deployment/BACKUPS.md)"
        mkdir -p "${ECONGRAPH_STATE_DIR}"
        printf 'signature=%s\nsent_epoch=%s\n' "${signature}" "${now}" >"${state}.tmp"
        mv "${state}.tmp" "${state}"
        ;;
    ok)
        unit="$(unit_arg "${2:-}")"
        rm -f "$(state_file "${unit}")"
        ;;
    heartbeat)
        if [[ -z "${NOTIFY_HEARTBEAT_URL}" ]]; then
            exit 0
        fi
        : | post "${NOTIFY_HEARTBEAT_URL}" || die "the heartbeat ping failed"
        ;;
    test)
        send "econgraph backup: test message from $(hostname) at $(date -u +%Y-%m-%dT%H:%MZ); a failed backup unit is reported like this."
        ;;
    *)
        die "usage: $(basename "$0") failure UNIT | ok UNIT | heartbeat | test"
        ;;
esac
