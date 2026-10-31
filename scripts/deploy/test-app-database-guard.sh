#!/usr/bin/env bash
# The stub kubectl is a quoted heredoc; the extracted decision runs under eval.
# shellcheck disable=SC2016,SC2030,SC2031,SC2034,SC2119,SC2120,SC2154,SC2329
# Tests for scripts/deploy/lib/app-database-guard.sh and the app database decision of
# scripts/deploy/create-secrets.sh, against a stub kubectl, no cluster:
#   - before a change to separate: econ_graph holding public.users is refused, naming the
#     runbook and APP_DATABASE=shared; on a Secret without app-database-url (strict) an absent
#     econ_graph is refused too;
#   - before shared, on every run: econ_graph without public.users while econ_graph_app has it
#     is refused, naming APP_DATABASE=separate and the rollback; when econ_graph can't be
#     asked, econ_graph_app answering f, or not existing, lets it through;
#   - before a kept separate: econ_graph holding public.users is refused, naming the
#     --app-database-only command; a database that can't be asked does not block it;
#   - a database that can't be asked (lookup or query failing, an unexpected answer, a volume
#     without its pod, an exec that hangs past APP_DB_GUARD_TIMEOUT) refuses a change with
#     advice to wait and rerun or set ALLOW_UNCHECKED_APP_DATABASE=1, which lets it through
#     with a warning; a positive answer is refused even then;
#   - the guards only read;
#   - the decision of create-secrets.sh (resolve_app_database), alone and with the real
#     guards against database states;
#   - end to end, the real create-secrets.sh: a kept shared Secret after a removal is refused
#     before anything is written, in a full run and with --app-database-only;
#     --app-database-only writes only app-database-url, with a merge patch from a file, built
#     from the app-password already in the cluster.
#
# Usage: scripts/deploy/test-app-database-guard.sh (from anywhere)
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
export STUB_LOG="$tmp/log"

# The stub cluster. Per database (DATA_* for postgresql-0/econ_graph, APP_* for
# app-postgres-0/econ_graph_app): *_POD running|missing, *_PVC present|missing,
# *_USERS t|f|junk|fails|hang. STUB_LOOKUP=fails: pod lookups fail. For create-secrets.sh:
# STUB_APP_URL (app-database-url; empty: none), STUB_SECRET_EXISTS=yes|no. Anything else
# answers nothing and succeeds; every call is logged.
cat >"$tmp/kubectl" <<'STUB'
#!/usr/bin/env bash
echo "kubectl $*" >>"$STUB_LOG"
args=("$@")
[ "${args[0]:-}" != --context ] || args=("${args[@]:2}")
[ "${args[0]:-}" != -n ] || args=("${args[@]:2}")
set -- "${args[@]}"
users() { case "$1" in fails) exit 1 ;; hang) exec sleep 300 ;; *) echo "$1" ;; esac; }
case "$*" in
  "get pod postgresql-0 "*)
    [ "${STUB_LOOKUP:-ok}" != fails ] || exit 1
    [ "${DATA_POD:-running}" != running ] || echo pod/postgresql-0 ;;
  "get pod app-postgres-0 "*)
    [ "${STUB_LOOKUP:-ok}" != fails ] || exit 1
    [ "${APP_POD:-running}" != running ] || echo pod/app-postgres-0 ;;
  "get persistentvolumeclaim"* | "get pvc"*)
    [ "${DATA_PVC:-present}" != present ] || echo persistentvolumeclaim/postgresql-data-postgresql-0
    [ "${APP_PVC:-present}" != present ] || echo persistentvolumeclaim/app-postgres-data-app-postgres-0 ;;
  "exec postgresql-0 -c postgresql -- psql -U postgres -d econ_graph "*) users "${DATA_USERS:-t}" ;;
  "exec app-postgres-0 -c postgres -- psql -U postgres -d econ_graph_app "*) users "${APP_USERS:-f}" ;;
  "config view --minify -o jsonpath={.clusters[0].cluster.server}") printf 'https://127.0.0.1:6443' ;;
  "get secret econ-graph-secrets --ignore-not-found -o jsonpath={.data.app-database-url}")
    [ -z "${STUB_APP_URL:-}" ] || printf '%s' "$STUB_APP_URL" | base64 -w0 ;;
  "get secret econ-graph-secrets --ignore-not-found -o name")
    [ "${STUB_SECRET_EXISTS:-yes}" != yes ] || echo secret/econ-graph-secrets ;;
  "get secret econ-graph-postgres --ignore-not-found -o jsonpath={.data.app-password}") printf 'data:pw' | base64 -w0 ;;
  "get secret econ-graph-app-postgres --ignore-not-found -o jsonpath={.data.app-password}") printf 'app:pw' | base64 -w0 ;;
  "patch secret econ-graph-secrets --type=merge --patch-file="*) last="${*: -1}"; cp "${last#--patch-file=}" "$STUB_LOG.patch" ;;
  "get secret econ-graph-postgres --ignore-not-found -o name") echo secret/econ-graph-postgres ;;
  "get secret econ-graph-app-postgres --ignore-not-found -o name") echo secret/econ-graph-app-postgres ;;
esac
exit 0
STUB
chmod +x "$tmp/kubectl"
export KUBECTL="$tmp/kubectl"
unset CLUSTER_ROLE_CONTEXT
kc() { "$KUBECTL" "$@"; }
die() {
  echo "create-secrets: error: $*" >&2
  exit 1
}
log() { echo "create-secrets: $*"; }
NAMESPACE=econ-graph
# shellcheck source=scripts/deploy/lib/app-database-guard.sh
source scripts/deploy/lib/app-database-guard.sh

# state "DATA_POD DATA_PVC DATA_USERS" "APP_POD APP_PVC APP_USERS": export a database state.
state() {
  read -r DATA_POD DATA_PVC DATA_USERS <<<"$1"
  read -r APP_POD APP_PVC APP_USERS <<<"$2"
  export DATA_POD DATA_PVC DATA_USERS APP_POD APP_PVC APP_USERS
}
up="running present"
none="missing missing -"

# check GUARD NAME DATA APP STATUS [MESSAGE] [ENV...]
check() {
  local guard="$1" name="$2" status=0 output
  state "$3" "$4"
  : >"$STUB_LOG"
  output="$(env "${@:7}" bash -c "$(declare -f kc die log app_db_guard_users app_db_guard_unchecked "$guard"); \
    APP_DB_GUARD_RETRY=\"$APP_DB_GUARD_RETRY\" NAMESPACE=$NAMESPACE $guard ${GUARD_ARG:-}" 2>&1)" || status=$?
  [ "$status" -eq "$5" ] || fail "$guard: $name: status $status, expected $5 ($output)"
  if [ -n "${6:-}" ] && [[ "$output" != *"$6"* ]]; then
    fail "$guard: $name: output lacks '$6': $output"
  fi
}

# Before a change to separate.
g=require_app_tables_moved
check $g "app tables in econ_graph are refused" "$up t" "$up f" 1 "Keep APP_DATABASE=shared"
check $g "the refusal names the runbook" "$up t" "$up f" 1 "docs/deployment/APP_DATA_CONVERSION.md"
check $g "no app tables pass" "$up f" "$up t" 0
check $g "no data database yet passes" "$none" "$none" 0
check $g "a volume without postgresql-0 is refused" "missing present -" "$none" 1 "is not running"
check $g "a failed query is refused" "$up fails" "$up f" 1 "ALLOW_UNCHECKED_APP_DATABASE=1"
check $g "an unexpected answer is refused" "$up junk" "$up f" 1 "unexpected answer"
check $g "a failed query passes with the override" "$up fails" "$up f" 0 "not checked" ALLOW_UNCHECKED_APP_DATABASE=1
check $g "app tables are refused even with the override" "$up t" "$up f" 1 "Keep APP_DATABASE=shared" ALLOW_UNCHECKED_APP_DATABASE=1
check $g "a failed lookup is refused" "$up t" "$up f" 1 "could not look up" STUB_LOOKUP=fails
start=$SECONDS
check $g "a hanging query is refused within the timeout" "$up hang" "$up f" 1 "could not check econ_graph" APP_DB_GUARD_TIMEOUT=2
[ $((SECONDS - start)) -lt 30 ] || fail "$g: a hanging query took $((SECONDS - start))s"
GUARD_ARG=strict
check $g "strict: no data database is refused" "$none" "$none" 1 "postgresql-0 is not there to confirm"
check $g "strict: no app tables pass" "$up f" "$up t" 0
GUARD_ARG=""

# Before shared.
g=require_app_tables_not_moved
check $g "tables only in econ_graph_app are refused" "$up f" "$up t" 1 "Use APP_DATABASE=separate"
check $g "the refusal names the rollback" "$up f" "$up t" 1 "--restore-source-app"
check $g "...even with the override" "$up f" "$up t" 1 "Use APP_DATABASE=separate" ALLOW_UNCHECKED_APP_DATABASE=1
check $g "app tables in econ_graph pass" "$up t" "$up t" 0
check $g "app tables in neither pass (a new install)" "$up f" "$up f" 0
check $g "no app database yet passes" "$up f" "$none" 0
check $g "no databases yet pass" "$none" "$none" 0
check $g "econ_graph down, econ_graph_app without app tables passes" "$up fails" "$up f" 0
check $g "econ_graph down, no econ_graph_app passes" "missing present -" "$none" 0
check $g "econ_graph down, app tables in econ_graph_app are refused" "$up fails" "$up t" 1 "ALLOW_UNCHECKED_APP_DATABASE=1"
check $g "...and pass with the override" "$up fails" "$up t" 0 "not checked" ALLOW_UNCHECKED_APP_DATABASE=1
check $g "econ_graph without tables, econ_graph_app down is refused" "$up f" "$up fails" 1 "could not check econ_graph_app"
check $g "an app volume without app-postgres-0 is refused" "$up f" "missing present -" 1 "is not running"
check $g "an unexpected app answer is refused" "$up f" "$up junk" 1 "unexpected answer"

# Before a kept separate.
g=require_app_tables_not_left_behind
check $g "app tables left in econ_graph are refused" "$up t" "$up t" 1 "--app-database-only"
check $g "no app tables in econ_graph pass" "$up f" "$up t" 0
check $g "econ_graph down does not block a kept setting" "$up fails" "$up t" 0
check $g "no data database passes" "$none" "$up t" 0

# The guards only read.
state "$up f" "$up f"
: >"$STUB_LOG"
(require_app_tables_moved; require_app_tables_not_moved; require_app_tables_not_left_behind) >/dev/null
if grep -vE "^kubectl -n econ-graph (get pod (postgresql-0|app-postgres-0) |get persistentvolumeclaim |exec (postgresql-0 -c postgresql|app-postgres-0 -c postgres) -- psql -U postgres -d econ_graph(_app)? -Atc select to_regclass)" "$STUB_LOG" | grep -q .; then
  fail "the guards did more than read: $(cat "$STUB_LOG")"
fi

# --- create-secrets.sh: resolve_app_database --------------------------------------------
script=scripts/deploy/create-secrets.sh
resolver="$(awk '$0 == "resolve_app_database() {" { on = 1 } on { print } on && $0 == "}" { exit }' "$script")"
[[ "$resolver" == *$'\n}' ]] || { fail "cannot find resolve_app_database in create-secrets.sh"; resolver="resolve_app_database() { false; }"; }
APP_DB_HOST="app-postgres-service"
# decide APP_DATABASE URL SECRET_EXISTS: "<app_database> <how> <guard>" or "refused: ..."
decide() {
  (
    existing() { printf '%s' "$CURRENT_URL"; }
    secret_exists() { [ "$SECRET_EXISTS" = yes ]; }
    source() { :; }
    require_app_tables_moved() { GUARD="separate${1:+-$1}"; }
    require_app_tables_not_moved() { GUARD=shared; }
    require_app_tables_not_left_behind() { GUARD=kept-separate; }
    die() { echo "refused: $*"; exit 0; }
    APP_DATABASE="$1" CURRENT_URL="$2" SECRET_EXISTS="$3" GUARD=none
    eval "$resolver"
    resolve_app_database
    echo "$app_database $app_database_how $GUARD"
  )
}
data_url="postgresql://u:p@data-postgres-service:5432/econ_graph"
app_url="postgresql://u:p@app-postgres-service:5432/econ_graph_app"
while IFS='|' read -r env url exists expected; do
  [ -n "$expected" ] || continue
  got="$(decide "$env" "$url" "$exists")"
  # shellcheck disable=SC2053 # expected may be a glob
  [[ "$got" == $expected ]] || fail "APP_DATABASE='$env', URL '$url', Secret=$exists: got '$got', expected '$expected'"
done <<EOF
|$data_url|yes|shared kept shared
|$app_url|yes|separate kept kept-separate
||no|separate new separate
||yes|refused: *APP_DATABASE=shared until docs/deployment/APP_DATA_CONVERSION.md*
shared||yes|shared from APP_DATABASE shared
separate||yes|separate from APP_DATABASE separate-strict
shared|$data_url|yes|shared kept shared
separate|$app_url|yes|separate kept kept-separate
separate|$data_url|yes|separate from APP_DATABASE separate
shared|$app_url|yes|shared from APP_DATABASE shared
shared||no|shared from APP_DATABASE none
EOF

# The same decision with the real guards, against database states.
decide_real() {
  local output status=0
  output="$( (
    existing() { printf '%s' "$CURRENT_URL"; }
    secret_exists() { [ "$SECRET_EXISTS" = yes ]; }
    source() { :; }
    APP_DATABASE="$1" CURRENT_URL="$2" SECRET_EXISTS="$3"
    state "$4" "$5"
    eval "$resolver"
    resolve_app_database
  ) 2>&1)" || status=$?
  if [ "$status" = 0 ]; then echo ok; else echo "refused: $output"; fi
}
while IFS='|' read -r name env url exists data app expected; do
  [ -n "$expected" ] || continue
  got="$(decide_real "$env" "$url" "$exists" "$data" "$app")"
  # shellcheck disable=SC2053 # expected may be a glob
  [[ "$got" == $expected ]] || fail "$name: got '$got', expected '$expected'"
done <<CASES
kept shared after the removal is refused||$data_url|yes|$up f|$up t|refused: *Use APP_DATABASE=separate*
explicit shared after the removal is refused|shared|$data_url|yes|$up f|$up t|refused: *Use APP_DATABASE=separate*
kept shared before the removal passes||$data_url|yes|$up t|$up f|ok
kept shared on a cluster not deployed yet passes||$data_url|yes|$none|$none|ok
kept shared with econ_graph down and no app tables in econ_graph_app passes||$data_url|yes|$up fails|$up f|ok
separate on a Secret without the key and no data database is refused|separate||yes|$none|$none|refused: *postgresql-0 is not there to confirm*
separate on a Secret without the key and app tables in econ_graph is refused|separate||yes|$up t|$up f|refused: *Keep APP_DATABASE=shared*
separate on a Secret without the key after the removal passes|separate||yes|$up f|$up t|ok
separate on a new install passes|||no|$none|$none|ok
separate kept after the removal passes||$app_url|yes|$up f|$up t|ok
separate kept with app tables back in econ_graph is refused||$app_url|yes|$up t|$up t|refused: *--app-database-only*
CASES

# --- create-secrets.sh end to end ---------------------------------------------------------
# run_cs [VAR=value...] -- ARGS: the real create-secrets.sh against the stub; status in $rc.
run_cs() {
  local vars=()
  while [ "$1" != -- ]; do vars+=("$1"); shift; done
  shift
  : >"$STUB_LOG"; rm -f "$STUB_LOG.patch"
  rc=0
  out="$(env -u ECONGRAPH_AGENT_SESSION -u KUBECONFIG -u APP_DATABASE -u ALLOW_UNCHECKED_APP_DATABASE \
    "${vars[@]}" "$script" --context=kind-test "$@" 2>&1)" || rc=$?
}
writes() { grep -E "^kubectl .*(create |apply |patch |delete |annotate )" "$STUB_LOG" || true; }

# A kept shared Secret after the removal: refused before anything is written.
for mode in full --app-database-only; do
  args=(); [ "$mode" = full ] || args=("$mode")
  run_cs STUB_APP_URL="$data_url" DATA_POD=running DATA_PVC=present DATA_USERS=f \
    APP_POD=running APP_PVC=present APP_USERS=t -- "${args[@]}"
  [ "$rc" != 0 ] || fail "$mode: a kept shared Secret after the removal was accepted"
  [[ "$out" == *"Use APP_DATABASE=separate"* ]] || fail "$mode: no refusal message: $out"
  [ -z "$(writes)" ] || fail "$mode: wrote before refusing: $(writes)"
done

# --app-database-only switches only app-database-url, from the passwords in the cluster.
run_cs STUB_APP_URL="$data_url" APP_DATABASE=separate DATA_USERS=f APP_USERS=t \
  POSTGRES_PASSWORD=must-not-be-used -- --app-database-only
[ "$rc" = 0 ] || fail "--app-database-only separate: failed: $out"
if [ "$(writes | wc -l)" != 1 ] || ! grep -q "patch secret econ-graph-secrets --type=merge --patch-file=" "$STUB_LOG"; then
  fail "--app-database-only: wrote more than app-database-url: $(writes)"
fi
written="$(python3 -c 'import base64, json, sys; print(base64.b64decode(json.load(open(sys.argv[1]))["data"]["app-database-url"]).decode())' "$STUB_LOG.patch" 2>/dev/null || true)"
[ "$written" = "postgresql://econgraph:app%3Apw@app-postgres-service:5432/econ_graph_app" ] ||
  fail "--app-database-only: wrote '$written'"
[[ "$out" != *"app:pw"* && "$out" != *"must-not-be-used"* ]] || fail "--app-database-only printed a credential"
run_cs STUB_APP_URL="$app_url" APP_DATABASE=shared DATA_USERS=t APP_USERS=t -- --app-database-only
[ "$rc" = 0 ] || fail "--app-database-only shared after a rollback: failed: $out"
written="$(python3 -c 'import base64, json, sys; print(base64.b64decode(json.load(open(sys.argv[1]))["data"]["app-database-url"]).decode())' "$STUB_LOG.patch" 2>/dev/null || true)"
[ "$written" = "postgresql://econgraph:data%3Apw@data-postgres-service:5432/econ_graph" ] ||
  fail "--app-database-only shared: wrote '$written'"
# --dry-run writes nothing; a setting already in place is left alone.
run_cs STUB_APP_URL="$data_url" APP_DATABASE=separate DATA_USERS=f APP_USERS=t -- --app-database-only --dry-run
[ "$rc" = 0 ] && [ -z "$(writes)" ] && [[ "$out" == *"dry run: not written"* ]] ||
  fail "--app-database-only --dry-run: rc=$rc writes=$(writes) $out"
run_cs STUB_APP_URL="postgresql://econgraph:data%3Apw@data-postgres-service:5432/econ_graph" DATA_USERS=t -- --app-database-only
[ "$rc" = 0 ] && [ -z "$(writes)" ] && [[ "$out" == *"unchanged"* ]] ||
  fail "--app-database-only on an unchanged setting: rc=$rc writes=$(writes) $out"
# econ_graph down: a kept shared passes when econ_graph_app has no app tables; the override.
run_cs STUB_APP_URL="$data_url" DATA_USERS=fails APP_USERS=f -- --app-database-only --dry-run
[ "$rc" = 0 ] || fail "kept shared, econ_graph down, econ_graph_app empty: refused: $out"
run_cs STUB_APP_URL="$data_url" DATA_USERS=fails APP_USERS=t -- --app-database-only --dry-run
[ "$rc" != 0 ] && [[ "$out" == *"ALLOW_UNCHECKED_APP_DATABASE=1"* ]] ||
  fail "kept shared, econ_graph down, app tables in econ_graph_app: rc=$rc $out"
run_cs STUB_APP_URL="$data_url" DATA_USERS=fails APP_USERS=t ALLOW_UNCHECKED_APP_DATABASE=1 -- --app-database-only --dry-run
[ "$rc" = 0 ] && [[ "$out" == *"not checked"* ]] || fail "the override: rc=$rc $out"
run_cs -- --app-database-only --prompt-tunnel-token
[ "$rc" = 2 ] || fail "--app-database-only with another mode: rc=$rc"

if [ "$failures" -gt 0 ]; then
  echo "$failures failure(s)"
  exit 1
fi
echo "app database guard tests passed"
