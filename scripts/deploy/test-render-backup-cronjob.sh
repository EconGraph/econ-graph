#!/usr/bin/env bash
# Tests for scripts/deploy/render-backup-cronjob.py against the committed
# k8s/manifests/postgres-backup-cronjob.yaml, and for the part of deploy.sh that uses it:
#   - the committed manifest leaves BACKUP_AGE_RECIPIENT empty, so the render is needed;
#   - the render sets it on the encrypt container only, and changes nothing else;
#   - a malformed recipient, a private AGE-SECRET-KEY identity and an empty one are refused
#     and print nothing to apply;
#   - a manifest without the variable is refused;
#   - scripts/backup/encryption.sh's backup_recipient_load reads the recipient from the
#     environment first, then from backup.env, and refuses a missing or malformed one;
#   - deploy.sh applies the rendered CronJob when there is a recipient, and otherwise does
#     not apply it and removes an existing one.
#
# Needs python3 with PyYAML. Usage: scripts/deploy/test-render-backup-cronjob.sh
# ok and fail both return 0, so `test && ok || fail` runs exactly one of them; the
# single-quoted programs expand their variables when they run, not here.
# shellcheck disable=SC2015,SC2016
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
render="${repo_root}/scripts/deploy/render-backup-cronjob.py"
manifest="${repo_root}/k8s/manifests/postgres-backup-cronjob.yaml"
recipient="age1qyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqs3290gq"
failures=0
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

fail() {
  echo "FAIL $1"
  failures=$((failures + 1))
}
ok() { echo "ok   $1"; }

# recipient_of FILE: the encrypt container's BACKUP_AGE_RECIPIENT in FILE's CronJob.
recipient_of() {
  python3 -c '
import sys, yaml
for d in yaml.safe_load_all(open(sys.argv[1])):
    if d and d["kind"] == "CronJob":
        spec = d["spec"]["jobTemplate"]["spec"]["template"]["spec"]
        for c in spec.get("initContainers", []) + spec.get("containers", []):
            for e in c.get("env", []):
                if e["name"] == "BACKUP_AGE_RECIPIENT":
                    print(c["name"], repr(e.get("value")))
' "$1"
}

[ "$(recipient_of "$manifest")" = "encrypt ''" ] &&
  ok "the committed CronJob leaves the recipient empty, on the encrypt container only" ||
  fail "the committed CronJob's recipient: $(recipient_of "$manifest")"

"$render" "$recipient" "$manifest" >"$tmp/rendered.yaml"
[ "$(recipient_of "$tmp/rendered.yaml")" = "encrypt '${recipient}'" ] &&
  ok "the render sets the recipient on the encrypt container" ||
  fail "the rendered recipient: $(recipient_of "$tmp/rendered.yaml")"

# Nothing but that value changes.
python3 -c '
import sys, yaml
a = [d for d in yaml.safe_load_all(open(sys.argv[1])) if d]
b = [d for d in yaml.safe_load_all(open(sys.argv[2])) if d]
for d in b:
    spec = d["spec"]["jobTemplate"]["spec"]["template"]["spec"]
    for c in spec.get("initContainers", []) + spec.get("containers", []):
        for e in c.get("env", []):
            if e["name"] == "BACKUP_AGE_RECIPIENT":
                e["value"] = ""
sys.exit(0 if a == b else 1)
' "$manifest" "$tmp/rendered.yaml" && ok "the render changes nothing else" || fail "the render changed more than the recipient"

# The default manifest is the committed one.
(cd "$tmp" && "$render" "$recipient") | cmp -s - "$tmp/rendered.yaml" &&
  ok "the default manifest is k8s/manifests/postgres-backup-cronjob.yaml" ||
  fail "the render without a manifest differs from the committed one's"

before="$failures"
for bad in "" "age1" "AGE-SECRET-KEY-1QQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQ" \
  "age1ABC" "ssh-ed25519 AAAA" "${recipient} extra" "${recipient}"$'\nage1other'; do
  if out="$("$render" "$bad" "$manifest" 2>/dev/null)" || [ -n "$out" ]; then
    fail "recipient '${bad}' was accepted"
  fi
done
[ "$failures" = "$before" ] && ok "malformed, empty and private-key recipients are refused, with nothing to apply"

grep -v BACKUP_AGE_RECIPIENT "$manifest" >"$tmp/no-var.yaml"
if "$render" "$recipient" "$tmp/no-var.yaml" >/dev/null 2>&1; then
  fail "a manifest without BACKUP_AGE_RECIPIENT was accepted"
else
  ok "a manifest without BACKUP_AGE_RECIPIENT is refused"
fi

# backup_recipient_load: the environment, then backup.env.
mkdir -p "$tmp/config"
load() { # load [ENV-RECIPIENT]: prints the recipient loaded, or "refused"
  env -i PATH="$PATH" HOME="$tmp/home" ECONGRAPH_CONFIG_DIR="$tmp/config" ${1:+BACKUP_AGE_RECIPIENT="$1"} \
    bash -c 'source "$1"; if backup_recipient_load; then printf "%s" "$BACKUP_AGE_RECIPIENT"; else echo refused; fi' \
    _ "${repo_root}/scripts/backup/encryption.sh"
}
[ "$(load)" = refused ] && ok "no recipient anywhere is refused" || fail "no recipient: $(load)"
printf 'BACKUP_AGE_RECIPIENT=%s\nOTHER=1\n' "$recipient" >"$tmp/config/backup.env"
[ "$(load)" = "$recipient" ] && ok "the recipient comes from backup.env" || fail "from backup.env: $(load)"
[ "$(load age1fromenvironment)" = age1fromenvironment ] &&
  ok "the environment wins over backup.env" || fail "from the environment: $(load age1fromenvironment)"
printf 'BACKUP_AGE_RECIPIENT=AGE-SECRET-KEY-1QQQQ\n' >"$tmp/config/backup.env"
[ "$(load)" = refused ] && ok "a private identity in backup.env is refused" || fail "private identity: $(load)"

# deploy.sh's backup block, on its own, with a kubectl stub that records its calls.
block="$(sed -n '/^# Deploy PostgreSQL backups$/,/^run_script "\$PROJECT_ROOT\/scripts\/deploy\/protect-postgres-pv.sh"$/p' \
  "${repo_root}/scripts/deploy/deploy.sh")"
grep -q 'render-backup-cronjob.py' <<<"$block" || fail "deploy.sh's backup block was not found"
run_block() { # run_block [ENV-RECIPIENT [LIVE-RECIPIENT]]: the block's kubectl calls, one per line
  : >"$tmp/kubectl.log"
  # LIVE: what `kubectl get cronjob postgres-backup` reports as the live recipient
  # (empty: no CronJob, or one without a recipient).
  env -i PATH="$PATH" HOME="$tmp/home" ECONGRAPH_CONFIG_DIR="$tmp/config" ${1:+BACKUP_AGE_RECIPIENT="$1"} \
    PROJECT_ROOT="$repo_root" LOG="$tmp/kubectl.log" LIVE="${2:-}" bash -c '
set -e
cd "$PROJECT_ROOT"
kubectl() {
  echo "kubectl $*" >>"$LOG"
  case "$*" in
    "apply -f -") cat >>"$LOG" ;;
    "-n econ-graph get cronjob postgres-backup "*) printf "%s" "$LIVE" ;;
  esac
}
run_script() { :; }
eval "$1"
' _ "$block" >/dev/null
  cat "$tmp/kubectl.log"
}
rm -f "$tmp/config/backup.env"
calls="$(run_block "$recipient")"
if grep -qx 'kubectl apply -f -' <<<"$calls" && grep -qF "value: ${recipient}" <<<"$calls" &&
  ! grep -q 'apply -f k8s/manifests/postgres-backup-cronjob.yaml\|delete cronjob' <<<"$calls"; then
  ok "with a recipient, deploy.sh applies the rendered CronJob"
else
  fail "with a recipient, deploy.sh ran: ${calls}"
fi
calls="$(run_block)"
if grep -qx 'kubectl -n econ-graph delete cronjob postgres-backup --ignore-not-found' <<<"$calls" &&
  ! grep -q 'postgres-backup-cronjob\|apply -f -' <<<"$calls" &&
  grep -qx 'kubectl apply -f k8s/manifests/postgres-backup-pvc.yaml' <<<"$calls"; then
  ok "without a recipient, deploy.sh keeps the PVC, applies no CronJob and removes an existing one"
else
  fail "without a recipient, deploy.sh ran: ${calls}"
fi
# A shell without backup.env (another HOME, sudo) must not remove a working CronJob: its
# live recipient is reused.
live="age1livelivelivelivelivelivelivelivelivelivelivelivelivelivexyz"
calls="$(run_block "" "$live")"
if grep -qx 'kubectl apply -f -' <<<"$calls" && grep -qF "value: ${live}" <<<"$calls" &&
  ! grep -q 'delete cronjob' <<<"$calls"; then
  ok "without a recipient here, deploy.sh reapplies the CronJob with its live recipient"
else
  fail "without a recipient here and a live one, deploy.sh ran: ${calls}"
fi
calls="$(run_block "$recipient" "$live")"
if grep -qF "value: ${recipient}" <<<"$calls" && ! grep -q 'delete cronjob' <<<"$calls"; then
  ok "a recipient here wins over the live one"
else
  fail "with both recipients, deploy.sh ran: ${calls}"
fi
for bad_live in "AGE-SECRET-KEY-1QQQQ" "age1ABC" "not-a-recipient"; do
  calls="$(run_block "" "$bad_live")"
  grep -qx 'kubectl -n econ-graph delete cronjob postgres-backup --ignore-not-found' <<<"$calls" &&
    ! grep -q 'apply -f -' <<<"$calls" ||
    fail "a live CronJob with recipient '${bad_live}' and none here was not removed: ${calls}"
done
# The recipient must pass backup_recipient_proven, like the host dumps' (ECO-718's hook).
source_line='source "$PROJECT_ROOT/scripts/backup/encryption.sh"'
unproven_block="${block/"$source_line"/"$source_line"; backup_recipient_proven() { return 1; \}}"
grep -q 'backup_recipient_proven() { return 1; }$' <<<"$unproven_block" || fail "could not stub backup_recipient_proven"
saved_block="$block"
block="$unproven_block"
calls="$(run_block "$recipient" "$live")"
if ! grep -q 'apply -f -\|delete cronjob' <<<"$calls"; then
  ok "an unproven recipient applies nothing and leaves a working CronJob as it is"
else
  fail "with an unproven recipient, deploy.sh ran: ${calls}"
fi
block="$saved_block"

if [ "$failures" -gt 0 ]; then
  echo "$failures check(s) failed"
  exit 1
fi
echo "render-backup-cronjob: all checks passed"
