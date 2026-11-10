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
# journalctl: the lines of a failed freshness run.
cat >"${tmp}/bin/journalctl" <<'EOF'
#!/usr/bin/env bash
echo "journalctl $*" >>"${STUB_LOG}"
printf '%s\n' "OK      dump: newest verified dump is 3h old (limit 26h)" \
    "STALE   offsite: newest off-site copy is 30h old (limit 26h)" \
    "LOWSPACE dumps: 1.0GiB free of 100GiB (1%, minimum 15%)" "BACKUP CHECK FAIL"
EOF
chmod +x "${tmp}/bin/curl" "${tmp}/bin/journalctl"
export PATH="${tmp}/bin:${PATH}"
# Never read the operator's backup.env or notify.env.
export ECONGRAPH_CONFIG_DIR="${tmp}/config" HOME="${tmp}/home"
webhook="https://hooks.slack.test/services/T000/B000/secret-token"

# run_notify <name> <want exit> <text the output must contain> <args...>
out=""
run_notify() {
    local name="$1" want="$2" text="$3" status=0
    shift 3
    : >"${STUB_LOG}"
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
for service in "${units}"/econgraph-*.service; do
    name="$(basename "${service}")"
    [[ "${name}" == econgraph-notify@.service ]] && continue
    grep -qx 'OnFailure=econgraph-notify@%N.service' "${service}" &&
        pass "${name} reports its failure" || fail "${name} has no OnFailure=econgraph-notify@%N.service"
done
grep -qx 'ExecStart=%h/.local/lib/econgraph-backup/notify.sh failure %i' "${units}/econgraph-notify@.service" &&
    pass "econgraph-notify@.service runs notify.sh failure with the escaped instance" ||
    fail "econgraph-notify@.service does not run notify.sh failure %i"
grep -q '^OnFailure=' "${units}/econgraph-notify@.service" &&
    fail "econgraph-notify@.service reports its own failure (a loop)" || pass "econgraph-notify@.service does not report itself"
grep -qx 'ExecStartPost=-%h/.local/lib/econgraph-backup/notify.sh heartbeat' "${units}/econgraph-backup-freshness.service" &&
    pass "the freshness check pings the heartbeat after it passes" || fail "the freshness check does not ping the heartbeat"
grep -qx 'StartLimitBurst=4' "${units}/econgraph-offsite.service" && grep -qx 'StartLimitIntervalSec=3h' "${units}/econgraph-offsite.service" &&
    pass "the off-site retries end in a failed unit" || fail "econgraph-offsite.service retries without a start limit"

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
case "$1" in
    show-user) cat "${FAKE_LINGER_FILE}" ;;
    enable-linger) [[ "${FAKE_ENABLE_LINGER_RC:-0}" == 0 ]] && echo yes >"${FAKE_LINGER_FILE}"; exit "${FAKE_ENABLE_LINGER_RC:-0}" ;;
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
grep -qx "loginctl enable-linger $(id -un)" "${STUB_LOG}" && [[ -x "${lib}/notify.sh" ]] &&
    pass "... for this user" || fail "enable-linger: $(cat "${STUB_LOG}")"
FAKE_ENABLE_LINGER_RC=1 run_install "when lingering stays off, the installer stops" 1 "Nothing was installed" no
[[ ! -e "${lib}" ]] && ! grep -q '^systemctl' "${STUB_LOG}" && pass "... before installing or enabling anything" ||
    fail "the installer went on without lingering: $(cat "${STUB_LOG}")"
FAKE_ENABLE_LINGER_RC=1 run_install "--allow-no-linger installs anyway, with a warning" 0 "(--allow-no-linger)" no --allow-no-linger
(umask 077 && echo 'NOTIFY_COMMAND=true' >"${tmp}/config/notify.env")
run_install "with notify.env, the installer does not warn about it" 0 "" yes
grep -q "notify.env" <<<"${out}" && fail "the installer warned about an existing notify.env: ${out}" || pass "... and says nothing about it"

if [[ "${failures}" -ne 0 ]]; then
    echo "${failures} notification test(s) failed"
    exit 1
fi
echo "all notification tests passed"
