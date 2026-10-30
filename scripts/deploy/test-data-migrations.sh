#!/usr/bin/env bash
# Tests for scripts/deploy/lib/data-migrations.sh (ECO-802), offline: a stub kubectl first on
# PATH, a stub precheck-db-migration.sh in a scratch project root, no cluster.
#   - with data migrations pending, crawler-worker is stopped first
#     (precheck-db-migration.sh --stop-crawler), and the versions are named;
#   - with none pending, nothing is stopped;
#   - a check Job that fails, or a precheck that refuses, fails the step, so deploy.sh
#     doesn't run the migrations;
#   - under DRY_RUN the step is reported and nothing runs;
#   - the check Job is crawler-migrate-job.yaml with only its name, labels, TTL and
#     `--dry-run` changed;
#   - deploy.sh runs the step before it applies the crawler-migrate Job.
#
# Needs python3 with PyYAML. Usage: scripts/deploy/test-data-migrations.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
W="$(mktemp -d)"
trap 'rm -rf "$W"' EXIT
mkdir -p "$W/bin" "$W/root/k8s/manifests" "$W/root/scripts/deploy"
cp "$ROOT/k8s/manifests/crawler-migrate-job.yaml" "$W/root/k8s/manifests/"

# The stub kubectl: logs each call; `apply -f -` keeps the manifest; the check Job's
# conditions are STUB_CHECK_STATE (default Complete) and its log STUB_CHECK_LOGS.
cat >"$W/bin/kubectl" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"$STUB_DIR/calls"
case "$*" in
    "apply -f -") cat >"$STUB_DIR/applied.yaml" ;;
    "-n econ-graph get job crawler-migrate-check "*) printf '%s' "${STUB_CHECK_STATE-Complete}" ;;
    "-n econ-graph logs job/crawler-migrate-check "*) printf '%b' "${STUB_CHECK_LOGS:-}" ;;
esac
STUB
cat >"$W/root/scripts/deploy/precheck-db-migration.sh" <<'STUB'
#!/usr/bin/env bash
printf 'precheck %s\n' "$*" >>"$STUB_DIR/calls"
exit "${STUB_PRECHECK_RC:-0}"
STUB
chmod +x "$W/bin/kubectl" "$W/root/scripts/deploy/precheck-db-migration.sh"

failures=0
ok() { echo "ok   $1"; }
bad() {
    echo "FAIL $1"
    printf '     %s\n' "$out"
    failures=$((failures + 1))
}

# step [VAR=VALUE...]: stop_crawler_for_pending_migrations with the stubs; output in $out,
# status in $rc, the calls in $W/calls.
step() {
    : >"$W/calls"
    rm -f "$W/applied.yaml"
    set +e
    # shellcheck disable=SC2016 # $1 is the inner shell's, given after the script
    out="$(env "$@" STUB_DIR="$W" PATH="$W/bin:$PATH" PROJECT_ROOT="$W/root" \
        DATA_MIGRATION_CHECK_POLL=0 DATA_MIGRATION_CHECK_LOOKS=3 bash -c '
            set -e
            source "$1/scripts/deploy/lib/data-migrations.sh"
            stop_crawler_for_pending_migrations' _ "$ROOT" 2>&1)"
    rc=$?
    set -e
}
called() { grep -qF -- "$1" "$W/calls"; }
has() { grep -qF -- "$1" <<<"$out"; }

# Pending: two versions among the job's logging.
step STUB_CHECK_LOGS='2026-10-30T10:00:00Z INFO 2 data migration(s) pending\n20261029000100\n20261030000100\n'
if [ "$rc" -eq 0 ] && called "precheck --stop-crawler" &&
    has "2 data migration(s) pending: 20261029000100 20261030000100"; then
    ok "pending migrations: crawler-worker is stopped first, and the versions are named"
else
    bad "pending migrations: exit $rc, calls $(tr '\n' '|' <"$W/calls")"
fi

step STUB_CHECK_LOGS='2026-10-30T10:00:00Z INFO 0 data migration(s) pending\n'
if [ "$rc" -eq 0 ] && ! called "precheck" && has "No data migrations pending; crawler-worker keeps running"; then
    ok "nothing pending: crawler-worker is left running"
else
    bad "nothing pending: exit $rc, calls $(tr '\n' '|' <"$W/calls")"
fi

step STUB_CHECK_STATE=Failed STUB_CHECK_LOGS='Error: connecting to the data database\n'
if [ "$rc" -ne 0 ] && ! called "precheck" && has "did not complete (status: Failed)" &&
    has "connecting to the data database"; then
    ok "a check Job that fails fails the step, with its log, and stops nothing"
else
    bad "a failed check Job: exit $rc"
fi

step STUB_CHECK_STATE=''
if [ "$rc" -ne 0 ] && ! called "precheck" && has "did not complete (status: none)"; then
    ok "a check Job that never finishes fails the step"
else
    bad "a check Job that never finishes: exit $rc"
fi

step STUB_CHECK_LOGS='20261030000100\n' STUB_PRECHECK_RC=1
if [ "$rc" -ne 0 ] && called "precheck --stop-crawler" && has "the data migrations were not applied"; then
    ok "a precheck that refuses fails the step"
else
    bad "a refusing precheck: exit $rc"
fi

step DRY_RUN=true
if [ "$rc" -eq 0 ] && [ ! -s "$W/calls" ] && has "[dry-run] would run crawler migrate --dry-run" &&
    has "precheck-db-migration.sh --stop-crawler before crawler-migrate"; then
    ok "dry run: the step is reported, and nothing runs"
else
    bad "dry run: exit $rc, calls $(tr '\n' '|' <"$W/calls")"
fi

# The check Job: the migrate Job, renamed, with --dry-run.
step STUB_CHECK_LOGS=''
if called "-n econ-graph delete job crawler-migrate-check --ignore-not-found" &&
    python3 - "$W/applied.yaml" "$ROOT/k8s/manifests/crawler-migrate-job.yaml" <<'PY'
import sys
import yaml
check = yaml.safe_load(open(sys.argv[1]))
base = yaml.safe_load(open(sys.argv[2]))
assert check["metadata"]["name"] == "crawler-migrate-check"
assert check["metadata"]["labels"]["app"] == "crawler-migrate-check"
assert check["spec"]["template"]["metadata"]["labels"]["app"] == "crawler-migrate-check"
assert check["spec"]["ttlSecondsAfterFinished"] == 3600
c, b = check["spec"]["template"]["spec"]["containers"][0], base["spec"]["template"]["spec"]["containers"][0]
assert c["command"] == ["/app/crawler", "migrate", "--dry-run"], c["command"]
# Everything else as crawler-migrate's: image, env (the data database Secret), resources.
c["command"] = b["command"]
assert c == b
for key in ("metadata", "spec"):
    check[key].pop("ttlSecondsAfterFinished", None)
    base[key].pop("ttlSecondsAfterFinished", None)
check["metadata"]["name"] = base["metadata"]["name"]
check["metadata"]["labels"]["app"] = base["metadata"]["labels"]["app"]
check["spec"]["template"]["metadata"]["labels"]["app"] = base["spec"]["template"]["metadata"]["labels"]["app"]
check["spec"]["template"]["spec"]["containers"][0]["command"] = b["command"]
assert check == base, "the check Job differs from crawler-migrate's beyond its name, labels, TTL and --dry-run"
PY
then
    ok "the check Job is crawler-migrate's with only its name, labels, TTL and --dry-run changed"
else
    bad "the check Job: $(cat "$W/applied.yaml" 2>/dev/null | head -5)"
fi

# deploy.sh: the step runs before the crawler-migrate Job is applied.
step_line="$(grep -n 'stop_crawler_for_pending_migrations || exit 1' "$ROOT/scripts/deploy/deploy.sh" | cut -d: -f1)"
apply_line="$(grep -n 'kubectl apply -f k8s/manifests/crawler-migrate-job.yaml' "$ROOT/scripts/deploy/deploy.sh" | cut -d: -f1)"
if [ -n "$step_line" ] && [ -n "$apply_line" ] && [ "$step_line" -lt "$apply_line" ]; then
    ok "deploy.sh stops the crawler for pending migrations before it applies crawler-migrate"
else
    out="step at line '${step_line}', apply at '${apply_line}'"
    bad "deploy.sh: the step is not before the crawler-migrate Job"
fi

if [ "$failures" -eq 0 ]; then
    echo "data-migrations.sh: all checks passed"
else
    echo "${failures} failed"
    exit 1
fi
