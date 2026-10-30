#!/usr/bin/env bash
# Tests for scripts/deploy/backup-postgres-now.sh against a stub kubectl (KUBECTL), no
# cluster (ECO-752):
#   - a Job that completes: its logs, exit 0;
#   - a Job that fails: its logs and exit 1 at once, not after the timeout;
#   - a Job still running at --timeout: not stopped, and the "still running" line
#     release-deploy.sh reads; a Job that is gone, or can't be read five times in a
#     row, ends the wait at once without that line;
#   - an unfinished Job of the CronJob (scheduled or ad hoc, also between a failed pod
#     and its retry): no second Job is created; finished and other Jobs don't count;
#   - --timeout and BACKUP_WAIT_TIMEOUT: positive whole seconds only;
#   - --dry-run creates nothing.
#
# Usage: scripts/deploy/test-backup-postgres-now.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="${ROOT}/scripts/deploy/backup-postgres-now.sh"
W="$(mktemp -d)"
trap 'rm -rf "$W"' EXIT

# The stub: context kind-econ-graph, the Jobs in STUB_JOBS (lines of name<TAB>owner<TAB>
# true conditions, each followed by a space), and the new Job's conditions as STUB_STATE
# says: complete, failed, running, gone (NotFound), error (any other failure), or
# "complete-after N" (running for N looks). Every call is logged.
cat >"$W/kubectl" <<'STUB'
#!/usr/bin/env bash
echo "$*" >>"$STUB_LOG"
args=("$@")
if [ "${args[0]:-}" = --context ]; then args=("${args[@]:2}"); fi
set -- "${args[@]}"
case "$*" in
    "config get-contexts -o name"*) echo kind-econ-graph ;;
    "config view --minify"*) printf 'https://127.0.0.1:6443' ;;
    "get nodes"*) printf 'econ-graph-control-plane' ;;
    "get namespace kube-system"*) printf '11111111-2222-3333-4444-555555555555' ;;
    "get jobs -n econ-graph "*) printf '%b' "${STUB_JOBS:-}" ;;
    "create job "*) ;;
    "get job/"*)
        looks="$(grep -c '^--context kind-econ-graph get job/' "$STUB_LOG")"
        case "${STUB_STATE}" in
            complete) echo Complete ;;
            failed) echo Failed ;;
            running) ;;
            gone) echo 'Error from server (NotFound): jobs.batch "x" not found' >&2; exit 1 ;;
            error) echo 'The connection to the server 127.0.0.1:6443 was refused' >&2; exit 1 ;;
            complete-after*) [ "$looks" -le "${STUB_STATE#complete-after }" ] || echo Complete ;;
        esac ;;
    "logs "*) echo "[backup] Backup OK: 3 tables, 1MB in /backups/x.dump" ;;
    *) ;;
esac
STUB
chmod +x "$W/kubectl"

failures=0
ok() { echo "ok   $1"; }
bad() {
    echo "FAIL $1"
    printf '     %s\n' "$out"
    failures=$((failures + 1))
}

# run [VAR=VALUE...] -- ARGS...: the script against the stub; output in $out, status in $rc.
run() {
    local vars=()
    while [ "$1" != -- ]; do vars+=("$1"); shift; done
    shift
    : >"$W/log"
    set +e
    out="$(env STUB_LOG="$W/log" KUBECTL="$W/kubectl" KUBECONFIG="$W/kubeconfig" \
        BACKUP_POLL_SECONDS=0 STUB_STATE=complete "${vars[@]}" "$SCRIPT" "$@" 2>&1)"
    rc=$?
    set -e
}
created() { grep -q '^--context kind-econ-graph create job ' "$W/log"; }
looks() { grep -c '^--context kind-econ-graph get job/' "$W/log" || true; }
has() { grep -qF -- "$1" <<<"$out"; }

run -- --timeout 5
if [ "$rc" -eq 0 ] && created && has "Backup complete" && has "Backup OK"; then
    ok "a Job that completes: created, its logs, exit 0"
else
    bad "a Job that completes: exit $rc"
fi

run STUB_STATE="complete-after 3" -- --timeout 60
if [ "$rc" -eq 0 ] && [ "$(looks)" -eq 4 ]; then
    ok "it looks at the Job until it completes"
else
    bad "a Job that completes on the 4th look: exit $rc, $(looks) looks"
fi

run STUB_STATE=failed -- --timeout 600
if [ "$rc" -eq 1 ] && has "did not complete successfully" && has "Backup OK" && [ "$(looks)" -eq 1 ] &&
    ! has "still running"; then
    ok "a Job that fails: its logs and exit 1 at the first look, not after the timeout"
else
    bad "a Job that fails: exit $rc, $(looks) looks"
fi

run STUB_STATE=running -- --timeout 1
if [ "$rc" -eq 1 ] && has "still running after 1s; it was not stopped" &&
    grep -qE '^backup-postgres-now: still running: job/postgres-backup-manual-[0-9]+$' <<<"$out" &&
    ! grep -q ' delete ' "$W/log"; then
    ok "a Job still running at --timeout: not stopped, and the line release-deploy.sh reads"
else
    bad "a Job still running at the timeout: exit $rc"
fi

run STUB_STATE=gone -- --timeout 600
if [ "$rc" -eq 1 ] && has "is gone" && ! has "still running" && [ "$(looks)" -eq 1 ]; then
    ok "a Job deleted while it ran ends the wait at once, not as still running"
else
    bad "a Job that is gone: exit $rc, $(looks) looks"
fi

run STUB_STATE=error -- --timeout 600
if [ "$rc" -eq 1 ] && has "could not read" && has "connection to the server" && ! has "still running" &&
    [ "$(looks)" -eq 5 ]; then
    ok "a Job that can't be read five times in a row ends the wait, not as still running"
else
    bad "a Job that can't be read: exit $rc, $(looks) looks"
fi

run STUB_STATE=running BACKUP_WAIT_TIMEOUT=1 --
if [ "$rc" -eq 1 ] && has "still running after 1s"; then
    ok "BACKUP_WAIT_TIMEOUT sets the wait"
else
    bad "BACKUP_WAIT_TIMEOUT=1: exit $rc"
fi

# Scheduled (owned by the CronJob), ad hoc by name, and one between a failed pod and its
# retry, or one that will fail but hasn't yet (FailureTarget only).
for jobs in 'postgres-backup-29384756\tpostgres-backup\t\n' 'postgres-backup-manual-17\t\t\n' \
    'postgres-backup-29384757\tpostgres-backup\tFailureTarget \n'; do
    run STUB_JOBS="$jobs" --
    name="$(printf '%b' "$jobs" | cut -f1)"
    if [ "$rc" -eq 1 ] && ! created && has "job/${name} from cronjob/postgres-backup is running" &&
        has "backup-postgres-now: still running: job/${name}"; then
        ok "an unfinished backup Job (${name}): no second Job is created"
    else
        bad "a running backup Job (${name}): exit $rc, created: $(created && echo yes || echo no)"
    fi
done

run STUB_JOBS='postgres-backup-29384756\tpostgres-backup\tComplete \npostgres-backup-manual-3\t\tFailureTarget Failed \ncrawler-migrate-1\t\t\nkeycloak-db-backup-1\tkeycloak-db-backup\t\n' --
if [ "$rc" -eq 0 ] && created; then
    ok "completed or failed backup Jobs and other unfinished Jobs don't stop a backup"
else
    bad "finished or unrelated Jobs: exit $rc"
fi

run STUB_JOBS='postgres-backup-1\tpostgres-backup\t\n' -- --dry-run
if [ "$rc" -eq 0 ] && ! created && has "job/postgres-backup-1 is running, so no backup would be started"; then
    ok "--dry-run with a backup running: says so, creates nothing"
else
    bad "--dry-run with a backup running: exit $rc"
fi
run -- --dry-run --timeout 90
if [ "$rc" -eq 0 ] && ! created && has "wait up to 90s"; then
    ok "--dry-run: names the wait, creates nothing"
else
    bad "--dry-run: exit $rc"
fi

for bad_timeout in 0 -5 1.5 abc ''; do
    run -- --timeout "$bad_timeout"
    if [ "$rc" -eq 2 ] && ! created && [ ! -s "$W/log" ]; then
        ok "--timeout '${bad_timeout}' is refused before kubectl"
    else
        bad "--timeout '${bad_timeout}': exit $rc"
    fi
done
run BACKUP_WAIT_TIMEOUT=soon --
if [ "$rc" -eq 2 ] && [ ! -s "$W/log" ]; then
    ok "a BACKUP_WAIT_TIMEOUT that isn't a number is refused"
else
    bad "BACKUP_WAIT_TIMEOUT=soon: exit $rc"
fi

if [ "$failures" -eq 0 ]; then
    echo "backup-postgres-now.sh: all checks passed"
else
    echo "${failures} failed"
    exit 1
fi
