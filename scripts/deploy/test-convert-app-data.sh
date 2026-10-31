#!/usr/bin/env bash
# The stubs are written as quoted heredocs and run on their own.
# shellcheck disable=SC2016
# Tests for scripts/deploy/convert-app-data.sh against stub kubectl, docker, kind and
# Python; no cluster, no database, no container:
#   - --cluster is required; --production must match the cluster's production marker;
#   - --dry-run checks the server and client major versions and prints every step, but
#     backs up, scales, port-forwards, reads Secrets and runs the conversion not at all;
#     a server/client version mismatch or a running Job stops it;
#   - a real run takes the full cluster backup before stopping anything, stops the
#     backend, the crawler and the backup CronJob, runs split-app-data.py's dry run and
#     copy (and the removal with --drop-source-app) with a 0600 service and password
#     file whose password is never printed, then removes those files, stops the
#     port-forwards and puts every writer back;
#   - after a removal or a rollback the backend stays at 0; after a failure it is back;
#   - a failed cluster backup stops before any writer is touched;
#   - every kubectl call goes to the kind cluster's own kubectl with --context kind-NAME
#     (backup-kind-databases.sh's calls run on that cluster's own kubeconfig).
#
# Everything runs in a scratch directory: HOME, XDG_CACHE_HOME, TMPDIR, the backup root
# and the Docker host point into it.
#
# Usage: scripts/deploy/test-convert-app-data.sh (from anywhere)
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$repo_root"
failures=0
fail() {
  echo "FAIL $1"
  failures=$((failures + 1))
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
export STUB_LOG="$tmp/log" STUB_KUBECTL="$tmp/kubectl"
mkdir -p "$tmp/home" "$tmp/tmpdir" "$tmp/bin"
export HOME="$tmp/home" XDG_CACHE_HOME="$tmp/cache" TMPDIR="$tmp/tmpdir"
export ECONGRAPH_BACKUP_ROOT="$tmp/econgraph-backups"
export ECONGRAPH_DOCKER_HOST="unix://$tmp/no-docker.sock"
unset KUBECONFIG KUBECTL KIND_CLUSTER NAMESPACE ECONGRAPH_SCRATCH_STORAGE ECONGRAPH_KUBE_DIR PG_IMAGE
export PYTHON="$tmp/bin/python-stub"
export PATH="$tmp/bin:$PATH"
PASSWORD='se:cr\et-value'
export STUB_PASSWORD_B64
STUB_PASSWORD_B64="$(printf '%s' "$PASSWORD" | base64)"

cat >"$tmp/bin/docker" <<'STUB'
#!/usr/bin/env bash
echo "docker $*" >>"$STUB_LOG"
case "$*" in
  "info --format "*) echo "29.0.0 /var/lib/docker [name=seccomp]" ;;
  "cp -q -L "*"-control-plane:/usr/bin/kubectl "*) cp "$STUB_KUBECTL" "${*: -1}" ;;
  "image inspect postgres:18") ;;
  "run --rm postgres:18 pg_dump --version") echo "pg_dump (PostgreSQL) ${STUB_CLIENT:-18.1}" ;;
  *) echo "stub docker: unexpected call: $*" >&2; exit 1 ;;
esac
STUB
cat >"$tmp/bin/kind" <<'STUB'
#!/usr/bin/env bash
echo "kind $*" >>"$STUB_LOG"
case "$*" in
  "get clusters") printf 'econ-graph\nkg-test\n' ;;
  "get kubeconfig --name "*) echo "# stub kubeconfig" ;;
  *) echo "stub kind: unexpected call: $*" >&2; exit 1 ;;
esac
STUB

# The stub cluster.
#   STUB_MARKER=production   the cluster carries the production marker
#   STUB_SERVER=17.4         the servers' version (default 18.1)
#   STUB_ACTIVE_JOBS=NAME    a Job is running
#   STUB_BACKUP_FAILS=1      pg_dump in backup-kind-databases.sh fails
#   STUB_SCALE_FAILS=NAME    scaling deployment NAME to 0 fails
#   STUB_SOURCE_USERS=f      econ_graph no longer holds public.users (fails, junk, hang:
#                            the check fails, answers nonsense, or never returns)
#   STUB_RESTORE_PATCH_FAILS=1  un-suspending the CronJob fails
cat >"$STUB_KUBECTL" <<'STUB'
#!/usr/bin/env bash
# Under create-secrets.sh's cut-down environment the settings come from the file convert() writes.
[ -n "${STUB_LOG:-}" ] || { set -a; . "${0%/*}/stub.env"; set +a; }
echo "kubectl $*" >>"$STUB_LOG"
args=("$@")
[ "${args[0]}" = --context ] && args=("${args[@]:2}")
[ "${args[0]}" = -n ] && args=("${args[@]:2}")
set -- "${args[@]}"
case "$*" in
  "version --request-timeout=15s") ;;
  "get namespace kube-system "*) printf 'uid-this-cluster' ;;
  # The marker (lib/cluster-role.sh): exists, then each value with a trailing "#".
  "get configmap econgraph-cluster-role "*)
    [ "${STUB_MARKER:-none}" = production ] || exit 0
    case "$*" in
      *" -o name") echo configmap/econgraph-cluster-role ;;
      *"{.data.role}"*) printf 'production#' ;;
      *) printf 'uid-this-cluster#' ;;
    esac ;;
  "get pod "*" -o jsonpath={.status.phase} --ignore-not-found") printf Running ;;
  "exec postgresql-0 -c postgresql -- postgres --version" | "exec app-postgres-0 -c postgres -- postgres --version")
    echo "postgres (PostgreSQL) ${STUB_SERVER:-18.1}" ;;
  "get jobs -o jsonpath="*) printf '%s' "${STUB_ACTIVE_JOBS:+${STUB_ACTIVE_JOBS} }" ;;
  "get deployment econ-graph-backend -o jsonpath={.spec.replicas} --ignore-not-found") printf 2 ;;
  "get deployment crawler-worker -o jsonpath={.spec.replicas} --ignore-not-found") printf 1 ;;
  "get cronjob postgres-backup -o name --ignore-not-found") echo cronjob.batch/postgres-backup ;;
  "get cronjob postgres-backup -o jsonpath={.spec.suspend}") ;;
  "scale deployment ${STUB_SCALE_FAILS:-none} --replicas=0") exit 1 ;;
  "scale deployment "*" --replicas="*) ;;
  "patch cronjob postgres-backup --type=merge -p "*'"suspend":false'*) [ "${STUB_RESTORE_PATCH_FAILS:-0}" != 1 ] || exit 1 ;;
  "patch cronjob postgres-backup --type=merge -p "*) ;;
  "get pods -l app="*" -o name") ;;
  "port-forward --address 127.0.0.1 pod/postgresql-0 :5432") echo "Forwarding from 127.0.0.1:15432 -> 5432"; exec sleep 300 ;;
  "port-forward --address 127.0.0.1 pod/app-postgres-0 :5432") echo "Forwarding from 127.0.0.1:15433 -> 5432"; exec sleep 300 ;;
  "get secret econ-graph-postgres -o jsonpath={.data.postgres-password}" | \
  "get secret econ-graph-app-postgres -o jsonpath={.data.postgres-password}") printf '%s' "$STUB_PASSWORD_B64" ;;
  # backup-kind-databases.sh
  "exec "*" -- pg_dump -Fc "*) [ "${STUB_BACKUP_FAILS:-0}" != 1 ] || exit 1; printf 'PGDMP stub' ;;
  "exec -i "*" -- pg_restore -f /dev/null") cat >/dev/null ;;
  "exec postgresql-0 -c postgresql -- psql "*"to_regclass"*)
    case "${STUB_SOURCE_USERS:-t}" in fails) exit 1 ;; hang) exec sleep 300 ;; *) echo "${STUB_SOURCE_USERS:-t}" ;; esac ;;
  "exec "*" -- psql "*"to_regclass"*) echo t ;;
  "exec "*" -- psql "*) echo 3 ;;
  "get secret "*" -o name --ignore-not-found") ;;
  # create-secrets.sh, run by the wrapper to rewrite app-database-url. The URL it writes is
  # recorded without its password (host, port and database) in $STUB_LOG.app_url.
  "config get-contexts -o name"*) printf 'kind-econ-graph\nkind-kg-test\n' ;;
  "config current-context") echo kind-kg-test ;;
  "get namespace econ-graph") echo namespace/econ-graph ;;
  "config view --minify -o jsonpath={.clusters[0].cluster.server}") printf 'https://127.0.0.1:6443' ;;
  "config use-context "*) ;;
  "get secret econ-graph-secrets --ignore-not-found -o jsonpath={.data.app-database-url}")
    printf '%s' "${STUB_APP_URL:-postgresql://econgraph:x@data-postgres-service:5432/econ_graph}" | base64 -w0 ;;
  "get secret econ-graph-secrets --ignore-not-found -o name") echo secret/econ-graph-secrets ;;
  "get secret econ-graph-postgres --ignore-not-found -o jsonpath={.data.app-password}") printf 'data-pw' | base64 -w0 ;;
  "get secret econ-graph-app-postgres --ignore-not-found -o jsonpath={.data.app-password}") printf 'app-pw' | base64 -w0 ;;
  "get secret "*" --ignore-not-found -o jsonpath="*) ;;
  "get secret "*" --ignore-not-found -o name") ;;
  "get persistentvolumeclaim"* | "get pvc "*) ;;
  "get pod postgresql-0 --ignore-not-found -o name"*) echo pod/postgresql-0 ;;
  "get pod app-postgres-0 --ignore-not-found -o name"*) echo pod/app-postgres-0 ;;
  # --app-database-only writes app-database-url with a merge patch from a file.
  "patch secret econ-graph-secrets --type=merge --patch-file="*)
    last="${*: -1}"
    python3 -c 'import base64, json, sys; print(base64.b64decode(json.load(open(sys.argv[1]))["data"]["app-database-url"]).decode().split("@")[1])' \
      "${last#--patch-file=}" >>"$STUB_LOG.app_url" ;;
  "create secret generic "*) echo "kind: Secret" ;;
  "apply --server-side "*) cat >/dev/null ;;
  "annotate secret "*) ;;
  *) echo "stub kubectl: unexpected call: $*" >&2; exit 1 ;;
esac
STUB

# The stub conversion: records its arguments and checks the credentials it was given.
#   STUB_PY_FAIL=WORD  exit 1 when an argument equals WORD (killing STUB_KILL_READER first)
#   STUB_PY_SLEEP_ON=WORD  sleep 2s when an argument equals WORD (touching $STUB_LOG.sleeping)
cat >"$PYTHON" <<'STUB'
#!/usr/bin/env bash
[ "${1:-}" != -c ] || exit 0
echo "python ${*:2}" >>"$STUB_LOG"
for f in "$PGSERVICEFILE" "$PGPASSFILE"; do
  [ "$(stat -c %a "$f")" = 600 ] || echo "BAD mode of $f" >>"$STUB_LOG"
done
echo "PGSERVICEFILE=$PGSERVICEFILE" >>"$STUB_LOG"
grep -q '^\[conversion_source\]$' "$PGSERVICEFILE" && grep -q '^port=15432$' "$PGSERVICEFILE" &&
  grep -q '^\[conversion_app\]$' "$PGSERVICEFILE" && grep -q '^port=15433$' "$PGSERVICEFILE" ||
  echo "BAD service file" >>"$STUB_LOG"
grep -qxF '127.0.0.1:15432:econ_graph:postgres:se\:cr\\et-value' "$PGPASSFILE" ||
  echo "BAD pgpass" >>"$STUB_LOG"
[ "$(command -v pg_dump)" = "$(dirname "$PGSERVICEFILE")/bin/pg_dump" ] && grep -qF '"postgres:18" pg_dump' "$(command -v pg_dump)" ||
  echo "BAD pg_dump shim" >>"$STUB_LOG"
for a in "$@"; do
  [ "$a" != "${STUB_PY_SLEEP_ON:-}" ] || { : >"$STUB_LOG.sleeping"; sleep 2; }
  if [ "$a" = "${STUB_PY_FAIL:-}" ]; then
    # Close the wrapper's output before it cleans up: its reader goes away.
    [ -z "${STUB_KILL_READER:-}" ] || { kill "$STUB_KILL_READER"; sleep 0.3; }
    exit 1
  fi
done
STUB
chmod +x "$tmp/bin/docker" "$tmp/bin/kind" "$STUB_KUBECTL" "$PYTHON"

map="$tmp/source-map.json"
echo '{}' >"$map"
# stub_env: the STUB_ settings of this call, for the stubs that run without them.
stub_env() {
  local v
  for v in ${!STUB_@}; do printf '%s=%q\n' "$v" "${!v}"; done >"$tmp/stub.env.new"
  for d in "$tmp" "$XDG_CACHE_HOME/econgraph/bin"; do
    mkdir -p "$d"; cp "$tmp/stub.env.new" "$d/stub.env"
  done
}
convert() {
  stub_env
  ECONGRAPH_SCRATCH_STORAGE="$tmp/scratch-storage" scripts/deploy/convert-app-data.sh "$@" >"$tmp/out" 2>&1
}
reset() {
  sleep 1; : >"$STUB_LOG"; rm -f "$STUB_LOG.sleeping" "$STUB_LOG.app_url"
  unset STUB_MARKER STUB_SERVER STUB_ACTIVE_JOBS STUB_BACKUP_FAILS STUB_PY_FAIL STUB_CLIENT STUB_SCALE_FAILS \
    STUB_SOURCE_USERS STUB_KILL_READER STUB_PY_SLEEP_ON STUB_RESTORE_PATCH_FAILS
}
never() { # never PATTERN WHAT: no log line matches PATTERN
  if grep -qE "$1" "$STUB_LOG"; then fail "$2"; grep -E "$1" "$STUB_LOG" | head -3; fi
}
has() { grep -qE "$1" "$STUB_LOG" || fail "$2"; }
leftovers() { # no credentials or port-forwards left behind
  [ -z "$(ls -A "$TMPDIR")" ] || fail "$1: temporary files left in TMPDIR: $(ls -A "$TMPDIR")"
  ! pgrep -f "kubectl.*port-forward" -u "$(id -u)" -a | grep -q "$STUB_KUBECTL\|$tmp" || fail "$1: port-forward left running"
  ! grep -qF "$PASSWORD" "$tmp/out" || fail "$1: password printed"
}

# --cluster is required.
reset
if convert --source-map "$map" --dry-run; then fail "ran without --cluster"; fi
grep -q -- "--cluster NAME is required" "$tmp/out" || fail "no message for a missing --cluster"

# Dry run: checks, prints, changes nothing.
reset
convert --cluster kg-test --source-map "$map" --dry-run || { fail "dry run failed"; cat "$tmp/out"; }
grep -q "PostgreSQL 18 on both servers; client tools from postgres:18" "$tmp/out" || fail "dry run: no version check"
for step in backup-kind-databases.sh "scale deployment/econ-graph-backend --replicas=0" \
  "patch cronjob/postgres-backup suspend=true" "split-app-data.py .* --dry-run" "--writers-stopped"; do
  grep -qE -- "$step" "$tmp/out" || fail "dry run does not print: $step"
done
never "scale |patch |port-forward|get secret|pg_dump -Fc|^python " "dry run changed or read something"
[ ! -e "$ECONGRAPH_BACKUP_ROOT" ] || fail "dry run wrote under the backup root"
never "^kubectl (version|get namespace|get configmap)" "" # (identity calls carry --context; checked below)
grep -E "^kubectl " "$STUB_LOG" | grep -vqE "^kubectl --context kind-kg-test " &&
  fail "dry run: a kubectl call without --context kind-kg-test: $(grep -E '^kubectl ' "$STUB_LOG" | grep -vE '^kubectl --context kind-kg-test ' | head -1)"
leftovers "dry run"

# Production marker must match --production, both ways.
reset
export STUB_MARKER=production
if convert --cluster kg-test --source-map "$map" --dry-run; then fail "production cluster without --production"; fi
grep -q "pass --production" "$tmp/out" || fail "no message for a production cluster"
convert --cluster kg-test --production --source-map "$map" --dry-run || fail "--production on a production cluster refused"
unset STUB_MARKER
if convert --cluster kg-test --production --source-map "$map" --dry-run; then fail "--production on a non-production cluster"; fi

# Version mismatches and running Jobs stop the dry run.
reset
STUB_SERVER=17.4 convert --cluster kg-test --source-map "$map" --dry-run && fail "server 17 with client 18 accepted"
grep -q "pg_dump 18, the servers run PostgreSQL 17" "$tmp/out" || fail "no message for a version mismatch"
STUB_ACTIVE_JOBS=crawler-migrate convert --cluster kg-test --source-map "$map" --dry-run && fail "running Job accepted"
grep -q "Jobs are running (crawler-migrate)" "$tmp/out" || fail "no message for a running Job"

# A real copy run.
reset
convert --cluster kg-test --source-map "$map" || { fail "copy run failed"; cat "$tmp/out"; }
first() { grep -nE "$1" "$STUB_LOG" | head -1 | cut -d: -f1; }
backup="$(first 'pg_dump -Fc')" stop="$(first 'scale deployment econ-graph-backend --replicas=0')"
[ -n "$backup" ] && [ -n "$stop" ] && [ "$backup" -lt "$stop" ] || fail "the cluster backup did not come before stopping writers"
has "patch cronjob postgres-backup --type=merge -p \{\"spec\":\{\"suspend\":true\}\}" "backup CronJob not suspended"
has "scale deployment crawler-worker --replicas=0" "crawler not stopped"
[ "$(grep -c '^python ' "$STUB_LOG")" = 2 ] || fail "copy run: expected the dry run and the copy"
has "^python .*--source-map $map --init-app-schema econgraph --dry-run$" "no conversion dry run"
has "^python .*--source-map $map --init-app-schema econgraph --writers-stopped$" "no conversion copy"
never "drop-source-app" "copy run removed app tables"
never "^BAD" "credentials or tools wrong: $(grep '^BAD' "$STUB_LOG" | head -1)"
has "scale deployment econ-graph-backend --replicas=2" "backend not put back"
has "scale deployment crawler-worker --replicas=1" "crawler not put back"
has "patch cronjob postgres-backup --type=merge -p \{\"spec\":\{\"suspend\":false\}\}" "CronJob not put back"
servicefile="$(sed -n 's/^PGSERVICEFILE=//p' "$STUB_LOG" | head -1)"
[ ! -e "$servicefile" ] || fail "service file left behind"
grep -E "^kubectl " "$STUB_LOG" | grep -vE "^kubectl (--context kind-kg-test |-n econ-graph )" | head -1 | grep -q . &&
  fail "a kubectl call outside the cluster's context"
leftovers "copy run"

# --drop-source-app: the removal runs last and the backend stays stopped.
reset
STUB_SOURCE_USERS=f POSTGRES_PASSWORD=operator-shell-credential GRAFANA_ADMIN_PASSWORD=operator-shell-credential \
  convert --cluster kg-test --source-map "$map" --drop-source-app || { fail "drop run failed"; cat "$tmp/out"; }
[ "$(grep -c '^python ' "$STUB_LOG")" = 3 ] || fail "drop run: expected dry run, copy, removal"
[ "$(tail -n1 "$STUB_LOG.app_url" 2>/dev/null)" = "app-postgres-service:5432/econ_graph_app" ] ||
  fail "drop run: app-database-url not pointed at the app database: $(cat "$STUB_LOG.app_url" 2>/dev/null)"
removal="$(grep -n -- '--drop-source-app$' "$STUB_LOG" | tail -1 | cut -d: -f1)"
switch="$(grep -n 'patch secret econ-graph-secrets' "$STUB_LOG" | tail -1 | cut -d: -f1 || true)"
[ -n "$switch" ] && [ "$switch" -gt "$removal" ] || fail "drop run: app-database-url not rewritten after the removal"
never "create secret|apply --server-side" "drop run: wrote more than app-database-url"
grep -q "app-database-url names it" "$tmp/out" || fail "drop run: the next step does not say the Secret was switched"
grep -q "^python .*--writers-stopped --drop-source-app$" "$STUB_LOG" || fail "no removal"
never "scale deployment econ-graph-backend --replicas=2" "backend restarted after the removal"
has "scale deployment crawler-worker --replicas=1" "crawler not put back after the removal"
grep -q "KIND_CLUSTER=kg-test .*scripts/deploy/deploy.sh" "$tmp/out" || fail "no next step after the removal"
leftovers "drop run"

# A switch that create-secrets.sh refuses (econ_graph still answers it has the app tables):
# the run fails, the backend stays stopped, and the message says what to run.
reset
STUB_SOURCE_USERS=t convert --cluster kg-test --source-map "$map" --drop-source-app &&
  fail "refused switch reported success"
never "scale deployment econ-graph-backend --replicas=2" "refused switch: backend restarted"
grep -q "APP_DATABASE=separate KUBECTL=.* scripts/deploy/create-secrets.sh --app-database-only --context=kind-kg-test" "$tmp/out" ||
  fail "refused switch: no instruction naming the cluster"
leftovers "refused switch"

# A failed conversion puts every writer back.
reset
STUB_PY_FAIL=--drop-source-app convert --cluster kg-test --source-map "$map" --drop-source-app && fail "failed removal reported success"
has "scale deployment econ-graph-backend --replicas=2" "backend not put back after a failure"
has "patch cronjob postgres-backup --type=merge -p \{\"spec\":\{\"suspend\":false\}\}" "CronJob not put back after a failure"
leftovers "failed run"

# A failed cluster backup stops before any writer is touched.
reset
STUB_BACKUP_FAILS=1 convert --cluster kg-test --source-map "$map" && fail "failed backup reported success"
never "scale |patch |port-forward|get secret .* jsonpath|^python " "acted after a failed backup"

# Rollback: needs a directory under the backup root; the backend stays stopped.
reset
outside="$tmp/elsewhere"
mkdir -p "$outside"
: >"$outside/combined.dump"; echo x >"$outside/combined.dump"; echo '{}' >"$outside/evidence.json"
convert --cluster kg-test --restore-source-app "$outside" && fail "rollback from outside the backup root accepted"
grep -q "not under the backup root" "$tmp/out" || fail "no message for a rollback directory outside the root"
dir="$ECONGRAPH_BACKUP_ROOT/20990101T000000Z"
mkdir -p "$dir"; echo x >"$dir/combined.dump"; echo '{}' >"$dir/evidence.json"
convert --cluster kg-test --restore-source-app "$dir" --source-map "$map" && fail "rollback with --source-map accepted"
convert --cluster kg-test --restore-source-app "$dir" || { fail "rollback failed"; cat "$tmp/out"; }
[ "$(grep -c '^python ' "$STUB_LOG")" = 1 ] || fail "rollback: expected one conversion call"
[ "$(tail -n1 "$STUB_LOG.app_url" 2>/dev/null)" = "data-postgres-service:5432/econ_graph" ] ||
  fail "rollback: app-database-url not pointed back at econ_graph: $(cat "$STUB_LOG.app_url" 2>/dev/null)"
has "^python .*--restore-source-app $dir --writers-stopped$" "no rollback call"
never "scale deployment econ-graph-backend --replicas=2" "backend restarted after the rollback"
grep -q "app-database-url names it; start the backend with KIND_CLUSTER=kg-test" "$tmp/out" || fail "no next step after the rollback"
leftovers "rollback"

# A failure after the source lost its app tables: the backend stays stopped, the rest comes back.
reset
STUB_PY_FAIL=--drop-source-app STUB_SOURCE_USERS=f convert --cluster kg-test --source-map "$map" --drop-source-app &&
  fail "failed removal reported success"
never "scale deployment econ-graph-backend --replicas=2" "backend restarted on a source without app tables"
has "scale deployment crawler-worker --replicas=1" "crawler not put back"
grep -q "no longer holds the app tables" "$tmp/out" || fail "no explanation for the stopped backend"
grep -q "KIND_CLUSTER=kg-test APP_DATABASE=separate" "$tmp/out" || fail "the hint does not name the cluster"

# Output that breaks during cleanup (a closed pipe) does not stop the writers coming back.
reset
mkfifo "$tmp/fifo"
cat "$tmp/fifo" >"$tmp/out" &
reader=$!
status=0
STUB_KILL_READER="$reader" STUB_PY_FAIL=--drop-source-app ECONGRAPH_SCRATCH_STORAGE="$tmp/scratch-storage" \
  scripts/deploy/convert-app-data.sh --cluster kg-test --source-map "$map" --drop-source-app >"$tmp/fifo" 2>&1 || status=$?
wait "$reader" 2>/dev/null || true
rm -f "$tmp/fifo"
[ "$status" != 0 ] || fail "closed output: reported success"
has "scale deployment econ-graph-backend --replicas=2" "closed output: backend not put back"
has "scale deployment crawler-worker --replicas=1" "closed output: crawler not put back"
has "patch cronjob postgres-backup --type=merge -p \{\"spec\":\{\"suspend\":false\}\}" "closed output: CronJob not put back"

# A failure while stopping the writers puts back what was already stopped.
reset
STUB_SCALE_FAILS=crawler-worker convert --cluster kg-test --source-map "$map" && fail "failed scale reported success"
has "scale deployment econ-graph-backend --replicas=0" "the backend was not stopped first"
has "scale deployment econ-graph-backend --replicas=2" "failed stop: backend not put back"
has "patch cronjob postgres-backup --type=merge -p \{\"spec\":\{\"suspend\":false\}\}" "failed stop: CronJob not put back"
never "^python " "converted after a failed stop"
leftovers "failed stop"

# A hangup during the copy: writers come back, port-forwards and credentials go.
reset
status=0
STUB_PY_SLEEP_ON=--writers-stopped ECONGRAPH_SCRATCH_STORAGE="$tmp/scratch-storage" \
  scripts/deploy/convert-app-data.sh --cluster kg-test --source-map "$map" >"$tmp/out" 2>&1 &
pid=$!
for _ in $(seq 1 100); do [ -e "$STUB_LOG.sleeping" ] && break; sleep 0.1; done
[ -e "$STUB_LOG.sleeping" ] || fail "hangup: the copy never started"
kill -HUP "$pid"
wait "$pid" || status=$?
[ "$status" != 0 ] || fail "hangup: reported success"
has "scale deployment econ-graph-backend --replicas=2" "hangup: backend not put back"
has "patch cronjob postgres-backup --type=merge -p \{\"spec\":\{\"suspend\":false\}\}" "hangup: CronJob not put back"
leftovers "hangup"

# One writer that cannot be put back does not keep the others stopped.
reset
STUB_RESTORE_PATCH_FAILS=1 STUB_PY_FAIL=--writers-stopped convert --cluster kg-test --source-map "$map" &&
  fail "failed copy reported success"
has "scale deployment econ-graph-backend --replicas=2" "failed CronJob restore: backend not put back"
has "scale deployment crawler-worker --replicas=1" "failed CronJob restore: crawler not put back"
grep -q "could not restore cronjob/postgres-backup" "$tmp/out" || fail "failed CronJob restore not reported"

# The exit-time check of econ_graph failing, answering nonsense or hanging keeps the backend
# stopped and does not hold up the rest of the cleanup.
for answer in fails junk hang; do
  reset
  start=$SECONDS
  DB_CHECK_TIMEOUT=2 STUB_SOURCE_USERS=$answer STUB_PY_FAIL=--writers-stopped \
    convert --cluster kg-test --source-map "$map" && fail "$answer check: failed copy reported success"
  [ $((SECONDS - start)) -lt 30 ] || fail "$answer check: cleanup took $((SECONDS - start))s"
  never "scale deployment econ-graph-backend --replicas=2" "$answer check: backend restarted without a clear answer"
  has "scale deployment crawler-worker --replicas=1" "$answer check: crawler not put back"
  has "patch cronjob postgres-backup --type=merge -p \{\"spec\":\{\"suspend\":false\}\}" "$answer check: CronJob not put back"
  grep -q "could not be checked within 2s" "$tmp/out" || fail "$answer check: no explanation"
done

if [ "$failures" -gt 0 ]; then
  echo "$failures failure(s)"
  exit 1
fi
echo "convert-app-data.sh: all checks passed"
