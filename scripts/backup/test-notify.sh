#!/usr/bin/env bash
# Tests for the path from a failed backup unit to a person:
#   - notify.sh failure UNIT posts one message, naming the unit, the host and the
#     check's STALE/MISSING/LOWSPACE lines from the unit's journal, to the Slack webhook
#     and to NOTIFY_COMMAND; the webhook URL is not on curl's command line;
#   - with no channel configured, or none that delivers, it exits 1 and says nobody was
#     told; a notify.env readable by others is refused;
#   - notify.sh heartbeat pings NOTIFY_HEARTBEAT_URL, and does nothing without one;
#   - every backup service in systemd/ names econgraph-notify@%N.service in OnFailure=,
#     the template runs notify.sh failure %i, the freshness check pings the heartbeat
#     only after it passes, and the off-site retries end in a failed unit;
#   - install-user-units.sh installs notify.sh and the template, enables lingering when
#     it is off, stops before installing anything when lingering stays off (unless
#     --allow-no-linger), and warns when notify.env is missing.
# Uses stub curl, journalctl, loginctl and systemctl: no network, no systemd needed.
#
# Usage: scripts/backup/test-notify.sh
# pass and fail both return 0, so `test && pass || fail` runs exactly one of them.
# shellcheck disable=SC2015
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
notify="${repo_root}/scripts/backup/notify.sh"
units="${repo_root}/scripts/backup/systemd"
failures=0
tmp="$(mktemp -d)"
trap 'rm -rf "${tmp}"' EXIT

pass() { echo "ok   $*"; }
fail() { echo "FAIL $*"; failures=$((failures + 1)); }

mkdir -p "${tmp}/bin" "${tmp}/config" "${tmp}/home"
export STUB_LOG="${tmp}/calls.log"
# curl: logs its arguments, the URL from its -K config and its stdin; FAKE_CURL_RC fails it.
cat >"${tmp}/bin/curl" <<'EOF'
#!/usr/bin/env bash
url="" prev=""
for a in "$@"; do
    if [[ "${prev}" == -K ]]; then url="$(sed -n 's/^url = "\(.*\)"$/\1/p' "${a}")"; fi
    prev="${a}"
done
{ echo "curl args: $*"; echo "curl url: ${url}"; echo "curl body: $(cat)"; } >>"${STUB_LOG}"
exit "${FAKE_CURL_RC:-0}"
EOF
# journalctl: by unit (-u), the lines of a failed freshness run, or FAKE_JOURNAL_FILE's;
# by invocation ID, FAKE_INVOCATION_FILE's (nothing without one).
cat >"${tmp}/bin/journalctl" <<'EOF'
#!/usr/bin/env bash
echo "journalctl $*" >>"${STUB_LOG}"
if [[ " $* " == *" _SYSTEMD_INVOCATION_ID="* ]]; then
    [[ -z "${FAKE_INVOCATION_FILE:-}" ]] || cat "${FAKE_INVOCATION_FILE}"
    exit 0
fi
if [[ -n "${FAKE_JOURNAL_FILE:-}" ]]; then cat "${FAKE_JOURNAL_FILE}"; exit 0; fi
printf '%s\n' "OK      dump: newest verified dump is 3h old (limit 26h)" \
    "STALE   offsite: newest off-site copy is 30h old (limit 26h)" \
    "LOWSPACE dumps: 1.0GiB free of 100GiB (1%, minimum 15%)" "BACKUP CHECK FAIL"
EOF
chmod +x "${tmp}/bin/curl" "${tmp}/bin/journalctl"
export PATH="${tmp}/bin:${PATH}"
# Never read the operator's backup.env or notify.env, or write its state.
export ECONGRAPH_CONFIG_DIR="${tmp}/config" HOME="${tmp}/home" ECONGRAPH_STATE_DIR="${tmp}/state"
unset MONITOR_INVOCATION_ID
webhook="https://hooks.slack.test/services/T000/B000/secret-token"

# run_notify <name> <want exit> <text the output must contain> <args...>
# Forgets earlier reports first, unless KEEP_STATE=1.
out=""
run_notify() {
    local name="$1" want="$2" text="$3" status=0
    shift 3
    : >"${STUB_LOG}"
    [[ "${KEEP_STATE:-}" == 1 ]] || rm -rf "${ECONGRAPH_STATE_DIR}"
    out="$("${notify}" "$@" </dev/null 2>&1)" || status=$?
    if [[ "${status}" -ne "${want}" ]] || ! grep -qF -- "${text}" <<<"${out}"; then
        fail "${name}: want exit ${want} and output containing '${text}', got exit ${status}: ${out}"
    else
        pass "${name}"
    fi
}

echo "== notify.sh"
run_notify "failure with no notify.env tells nobody, and fails" 1 "nobody was told" failure econgraph-dump
(umask 077 && : >"${tmp}/config/notify.env")
run_notify "failure with no channel in notify.env fails" 1 "no notification channel" failure econgraph-dump
run_notify "heartbeat without a URL does nothing and succeeds" 0 "" heartbeat
[[ ! -s "${STUB_LOG}" ]] && pass "... and calls nothing" || fail "heartbeat without a URL called: $(cat "${STUB_LOG}")"

printf 'NOTIFY_SLACK_WEBHOOK_URL=%s\nNOTIFY_COMMAND=%q\nNOTIFY_HEARTBEAT_URL=https://hc-ping.test/uuid\n' \
    "${webhook}" "cat >>'${tmp}/command.out'" >"${tmp}/config/notify.env"
chmod 644 "${tmp}/config/notify.env"
run_notify "a notify.env readable by others is refused" 1 "chmod 600" failure econgraph-dump
chmod 600 "${tmp}/config/notify.env"

: >"${tmp}/command.out"
run_notify "failure notifies" 0 "notified: econgraph backup: econgraph-backup-freshness.service failed" \
    failure econgraph-backup-freshness
grep -qF "curl url: ${webhook}" "${STUB_LOG}" && pass "... through the Slack webhook" || fail "the webhook was not called: $(cat "${STUB_LOG}")"
grep -q "^curl args: .*hooks.slack.test" "${STUB_LOG}" && fail "the webhook URL is on curl's command line" ||
    pass "... with the URL off curl's command line"
body="$(sed -n 's/^curl body: //p' "${STUB_LOG}")"
if python3 -c '
import json, sys
text = json.loads(sys.argv[1])["text"]
want = ["econgraph-backup-freshness.service failed on ", "STALE   offsite: newest off-site copy is 30h old",
        "LOWSPACE dumps:", "journalctl --user -u econgraph-backup-freshness.service"]
sys.exit(0 if all(w in text for w in want) and "OK      dump" not in text else 1)
' "${body}"; then
    pass "... naming the unit, the host and the failing checks, not the passing ones"
else
    fail "the Slack message: ${body}"
fi
grep -q "journalctl --user -u econgraph-backup-freshness.service" "${STUB_LOG}" &&
    pass "... read from that unit's journal" || fail "journalctl was not asked for the unit: $(cat "${STUB_LOG}")"
grep -qF "STALE   offsite" "${tmp}/command.out" && pass "... and through NOTIFY_COMMAND" ||
    fail "NOTIFY_COMMAND got: $(cat "${tmp}/command.out")"

echo "== notify.sh: one message per change, and a daily reminder"
sent() { grep -c '^curl body: ' "${STUB_LOG}" || true; }
run_notify "a first failure is sent" 0 "notified:" failure econgraph-backup-freshness
KEEP_STATE=1 run_notify "the same failure an hour later is not sent again" 0 "already reported" failure econgraph-backup-freshness
[[ "$(sent)" == 0 ]] && pass "... nothing posted" || fail "a repeated failure was posted: $(cat "${STUB_LOG}")"
printf '%s\n' "STALE   offsite: newest off-site copy is 31h old (limit 26h)" \
    "LOWSPACE dumps: 0.9GiB free of 100GiB (0%, minimum 15%)" "BACKUP CHECK FAIL" >"${tmp}/journal.ages"
FAKE_JOURNAL_FILE="${tmp}/journal.ages" KEEP_STATE=1 run_notify "... nor when only ages and sizes changed" 0 \
    "already reported" failure econgraph-backup-freshness
printf '%s\n' "STALE   offsite: newest off-site copy is 31h old (limit 26h)" \
    "MISSING restore_drill: newest passing restore drill" >"${tmp}/journal.changed"
FAKE_JOURNAL_FILE="${tmp}/journal.changed" KEEP_STATE=1 run_notify "a change in what fails is sent at once" 0 \
    "notified:" failure econgraph-backup-freshness
grep -qF "MISSING restore_drill" "${STUB_LOG}" && pass "... with the new failure" || fail "the change: $(cat "${STUB_LOG}")"
state="${ECONGRAPH_STATE_DIR}/notify-econgraph-backup-freshness.service.last"
sed -i.bak "s/^sent_epoch=.*/sent_epoch=$(( $(date +%s) - 22 * 3600 ))/" "${state}"
FAKE_JOURNAL_FILE="${tmp}/journal.changed" KEEP_STATE=1 run_notify "the same failure 22h later is not sent yet" 0 \
    "already reported" failure econgraph-backup-freshness
# A daily unit whose timer jitters fails again a little under 24h later: still a reminder.
sed -i.bak "s/^sent_epoch=.*/sent_epoch=$(( $(date +%s) - 23 * 3600 - 50 * 60 ))/" "${state}"
FAKE_JOURNAL_FILE="${tmp}/journal.changed" KEEP_STATE=1 run_notify "the same failure 23h50m later is sent as a reminder" 0 \
    "failed (still failing; reminder)" failure econgraph-backup-freshness
# Without details, every failure of a unit has the same signature: always sent.
: >"${tmp}/journal.empty"
FAKE_JOURNAL_FILE="${tmp}/journal.empty" run_notify "a failure without details is sent" 0 "notified:" failure econgraph-dump
FAKE_JOURNAL_FILE="${tmp}/journal.empty" KEEP_STATE=1 run_notify "... and so is the next one, at once" 0 "notified:" \
    failure econgraph-dump
KEEP_STATE=1 run_notify "ok forgets the unit's last failure" 0 "" ok econgraph-backup-freshness
[[ ! -e "${state}" ]] && pass "... so its next failure is sent at once" || fail "ok left ${state}"
KEEP_STATE=1 run_notify "the next failure after a success is sent" 0 "notified:" failure econgraph-backup-freshness
run_notify "ok refuses a unit name with shell characters" 1 "usage" ok 'x;y'

echo "== notify.sh: the failed run's own journal lines"
printf '%s\n' "MISSING dump: newest verified dump under /backups" >"${tmp}/journal.invocation"
MONITOR_INVOCATION_ID=0123456789abcdef0123456789abcdef FAKE_INVOCATION_FILE="${tmp}/journal.invocation" \
    run_notify "with MONITOR_INVOCATION_ID, the failed run's lines are quoted" 0 "notified:" failure econgraph-dump
if grep -qF "MISSING dump" "${STUB_LOG}" && ! grep -qF "STALE   offsite" "${STUB_LOG}" &&
    grep -qF "_SYSTEMD_INVOCATION_ID=0123456789abcdef0123456789abcdef" "${STUB_LOG}"; then
    pass "... and not an earlier run's"
else
    fail "the invocation's lines: $(cat "${STUB_LOG}")"
fi
MONITOR_INVOCATION_ID=0123456789abcdef0123456789abcdef \
    run_notify "with nothing under the invocation ID, the unit's last 3 hours are read" 0 "notified:" failure econgraph-dump
grep -q "journalctl --user -u econgraph-dump.service --since -3h" "${STUB_LOG}" && grep -qF "STALE   offsite" "${STUB_LOG}" &&
    pass "... as the fallback" || fail "the fallback: $(cat "${STUB_LOG}")"
MONITOR_INVOCATION_ID='bad; id' run_notify "a malformed MONITOR_INVOCATION_ID is not used" 0 "notified:" failure econgraph-dump
grep -q "INVOCATION_ID=" "${STUB_LOG}" && fail "a malformed invocation ID reached journalctl" || pass "... in the journal query"

FAKE_CURL_RC=22 run_notify "failure still notifies when only NOTIFY_COMMAND delivers" 0 "the Slack webhook did not accept" \
    failure econgraph-dump
printf 'NOTIFY_SLACK_WEBHOOK_URL=%s\n' "${webhook}" >"${tmp}/config/notify.env"
FAKE_CURL_RC=22 run_notify "failure fails when no channel delivers" 1 "no notification channel delivered" \
    failure econgraph-dump
run_notify "a unit name with shell characters is refused" 1 "usage" failure 'x;rm -rf /'
run_notify "test sends a test message" 0 "notified: econgraph backup: test message" test

printf 'NOTIFY_HEARTBEAT_URL=https://hc-ping.test/uuid\n' >"${tmp}/config/notify.env"
run_notify "heartbeat pings the URL" 0 "" heartbeat
grep -qF "curl url: https://hc-ping.test/uuid" "${STUB_LOG}" && pass "... the configured one" || fail "heartbeat called: $(cat "${STUB_LOG}")"
FAKE_CURL_RC=7 run_notify "a failed heartbeat ping fails" 1 "heartbeat ping failed" heartbeat
rm -f "${tmp}/config/notify.env"

echo "== systemd units"
# in_section FILE SECTION LINE: LINE is a whole line of FILE's [SECTION]. systemd ignores
# (with a warning) a [Unit] setting written under [Service] and the other way round.
in_section() {
    awk -v want="[$2]" -v line="$3" '
        /^\[.*\]$/ { section = $0; next }
        section == want && $0 == line { found = 1 }
        END { exit !found }' "$1"
}
for service in "${units}"/econgraph-*.service; do
    name="$(basename "${service}")"
    [[ "${name}" == econgraph-notify@.service ]] && continue
    in_section "${service}" Unit 'OnFailure=econgraph-notify@%N.service' &&
        pass "${name} reports its failure ([Unit] OnFailure=)" || fail "${name} has no [Unit] OnFailure=econgraph-notify@%N.service"
    in_section "${service}" Service 'ExecStartPost=-%h/.local/lib/econgraph-backup/notify.sh ok %N' &&
        pass "${name} forgets its last failure after a success" || fail "${name} has no [Service] ExecStartPost=... notify.sh ok %N"
done
in_section "${units}/econgraph-notify@.service" Service 'ExecStart=%h/.local/lib/econgraph-backup/notify.sh failure %i' &&
    pass "econgraph-notify@.service runs notify.sh failure with the escaped instance" ||
    fail "econgraph-notify@.service does not run notify.sh failure %i"
grep -q '^OnFailure=' "${units}/econgraph-notify@.service" &&
    fail "econgraph-notify@.service reports its own failure (a loop)" || pass "econgraph-notify@.service does not report itself"
in_section "${units}/econgraph-backup-freshness.service" Service 'ExecStartPost=-%h/.local/lib/econgraph-backup/notify.sh heartbeat' &&
    pass "the freshness check pings the heartbeat after it passes" || fail "the freshness check does not ping the heartbeat"
offsite="${units}/econgraph-offsite.service"
in_section "${offsite}" Unit 'StartLimitBurst=4' && in_section "${offsite}" Unit 'StartLimitIntervalSec=3h' &&
    in_section "${offsite}" Service 'Restart=on-failure' && in_section "${offsite}" Service 'RestartMode=direct' &&
    pass "the off-site retries end in one failed unit ([Unit] StartLimit*, [Service] RestartMode=direct)" ||
    fail "econgraph-offsite.service: StartLimit* must be in [Unit], Restart= and RestartMode=direct in [Service]"
in_section "${offsite}" Unit 'ConditionPathExists=%h/.config/econgraph/restic.env' &&
    pass "without restic.env the off-site unit is skipped, not failed" ||
    fail "econgraph-offsite.service has no [Unit] ConditionPathExists= on restic.env"
# The checks themselves must fail when a setting is in the wrong section.
printf '[Unit]\nDescription=x\n[Service]\nOnFailure=econgraph-notify@%%N.service\n' >"${tmp}/misplaced.service"
in_section "${tmp}/misplaced.service" Unit 'OnFailure=econgraph-notify@%N.service' &&
    fail "in_section accepted OnFailure= under [Service]" || pass "in_section refuses a setting in the wrong section"

echo "== install-user-units.sh"
# systemctl and loginctl stubs. FAKE_LINGER is the Linger value; enable-linger sets it to
# yes unless FAKE_ENABLE_LINGER_RC fails it.
cat >"${tmp}/bin/systemctl" <<'EOF'
#!/usr/bin/env bash
echo "systemctl $*" >>"${STUB_LOG}"
EOF
cat >"${tmp}/bin/loginctl" <<'EOF'
#!/usr/bin/env bash
echo "loginctl $*" >>"${STUB_LOG}"
case " $* " in
    *" show-user "*) cat "${FAKE_LINGER_FILE}" ;;
    *" enable-linger "*) [[ "${FAKE_ENABLE_LINGER_RC:-0}" == 0 ]] && echo yes >"${FAKE_LINGER_FILE}"; exit "${FAKE_ENABLE_LINGER_RC:-0}" ;;
esac
EOF
chmod +x "${tmp}/bin/systemctl" "${tmp}/bin/loginctl"
export FAKE_LINGER_FILE="${tmp}/linger"
installer="${repo_root}/scripts/backup/install-user-units.sh"
lib="${tmp}/home/.local/lib/econgraph-backup"
unit_dir="${tmp}/home/.config/systemd/user"
# run_install <name> <want exit> <text> <linger> <args...>
run_install() {
    local name="$1" want="$2" text="$3" status=0
    echo "$4" >"${FAKE_LINGER_FILE}"
    shift 4
    : >"${STUB_LOG}"
    rm -rf "${tmp}/home/.local" "${tmp}/home/.config/systemd"
    out="$(env -u XDG_CONFIG_HOME "${installer}" "$@" </dev/null 2>&1)" || status=$?
    if [[ "${status}" -ne "${want}" ]] || ! grep -qF -- "${text}" <<<"${out}"; then
        fail "${name}: want exit ${want} and output containing '${text}', got exit ${status}: ${out}"
    else
        pass "${name}"
    fi
}
run_install "with lingering on, the installer installs" 0 "no ${tmp}/config/notify.env: a failed or stale backup will reach nobody" yes
[[ -x "${lib}/notify.sh" && -f "${unit_dir}/econgraph-notify@.service" ]] &&
    pass "... notify.sh and econgraph-notify@.service" || fail "notify.sh or the template was not installed"
grep -q "enable-linger" "${STUB_LOG}" && fail "lingering was already on, but enable-linger ran" || pass "... without touching lingering"
run_install "with lingering off, the installer enables it and installs" 0 "enabling lingering" no
grep -qx "loginctl --no-ask-password enable-linger $(id -un)" "${STUB_LOG}" && [[ -x "${lib}/notify.sh" ]] &&
    pass "... for this user, without a password prompt" || fail "enable-linger: $(cat "${STUB_LOG}")"
FAKE_ENABLE_LINGER_RC=1 run_install "when lingering stays off, the installer stops" 1 "Nothing was installed" no
[[ ! -e "${lib}" ]] && ! grep -q '^systemctl' "${STUB_LOG}" && pass "... before installing or enabling anything" ||
    fail "the installer went on without lingering: $(cat "${STUB_LOG}")"
run_install "--allow-no-linger installs anyway, with a warning" 0 "(--allow-no-linger)" no --allow-no-linger
grep -q "enable-linger" "${STUB_LOG}" && fail "--allow-no-linger still ran enable-linger" ||
    pass "... and leaves lingering alone"
grep -q "start --no-block econgraph-offsite.service" "${STUB_LOG}" &&
    fail "the off-site copy was started without restic.env" || pass "without restic.env, no off-site run is started"
mkdir -p "${tmp}/home/.config/econgraph"
: >"${tmp}/home/.config/econgraph/restic.env"
run_install "with restic.env and no recorded off-site run, the installer starts one" 0 \
    "starting econgraph-offsite.service once" yes
grep -qx "systemctl --user start --no-block econgraph-offsite.service" "${STUB_LOG}" &&
    pass "... without waiting for it" || fail "the first off-site run: $(cat "${STUB_LOG}")"
mkdir -p "${ECONGRAPH_STATE_DIR}"
echo "passed_epoch=$(date +%s)" >"${ECONGRAPH_STATE_DIR}/offsite.last"
run_install "with a recorded off-site run, the installer starts none" 0 "" yes
grep -q "start --no-block econgraph-offsite.service" "${STUB_LOG}" &&
    fail "an off-site run was started although one is recorded" || pass "... and leaves the timer to it"
rm -f "${tmp}/home/.config/econgraph/restic.env"
(umask 077 && echo 'NOTIFY_COMMAND=true' >"${tmp}/config/notify.env")
run_install "with notify.env, the installer does not warn about it" 0 "" yes
grep -q "notify.env" <<<"${out}" && fail "the installer warned about an existing notify.env: ${out}" || pass "... and says nothing about it"

if [[ "${failures}" -ne 0 ]]; then
    echo "${failures} notification test(s) failed"
    exit 1
fi
echo "all notification tests passed"
