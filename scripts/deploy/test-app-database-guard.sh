#!/usr/bin/env bash
# The stub kc is called by the sourced library; the extracted decision runs under eval.
# shellcheck disable=SC2016,SC2034,SC2154,SC2329
# Tests for scripts/deploy/lib/app-database-guard.sh and the app database decision of
# scripts/deploy/create-secrets.sh, against a stub kc, no cluster:
#   - before separate: econ_graph still holding public.users stops it, naming the runbook
#     and APP_DATABASE=shared; no app tables, or no data database yet, passes;
#   - before shared: econ_graph without public.users while econ_graph_app has it stops it,
#     naming APP_DATABASE=separate and the rollback; app tables in econ_graph, or in
#     neither database, pass;
#   - a volume without a running pod, a failed lookup, a failed query or an unexpected
#     answer stops either guard, saying to wait for the databases and rerun or to leave
#     APP_DATABASE unset (fail closed);
#   - the guards only read;
#   - create-secrets.sh keeps the current setting (no check) when APP_DATABASE is unset or
#     equals it; a cluster without econ-graph-secrets gets separate; an existing Secret
#     without app-database-url needs APP_DATABASE, naming the runbook; any change runs the
#     guard for its direction, before the first Secret is written.
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

# Per database (DATA_* for postgresql-0/econ_graph, APP_* for app-postgres-0/econ_graph_app):
#   *_POD running|missing   *_PVC present|missing   *_USERS t|f|junk|fails
# STUB_LOOKUP=fails: pod lookups fail.
STUB_LOG=""
kc() {
  STUB_LOG+="kc $*"$'\n'
  case "$*" in
    *"get pod postgresql-0"*)
      [ "${STUB_LOOKUP:-ok}" != fails ] || return 1
      [ "$DATA_POD" != running ] || echo pod/postgresql-0 ;;
    *"get pod app-postgres-0"*)
      [ "${STUB_LOOKUP:-ok}" != fails ] || return 1
      [ "$APP_POD" != running ] || echo pod/app-postgres-0 ;;
    *"get persistentvolumeclaim"*)
      [ "$DATA_PVC" != present ] || echo persistentvolumeclaim/postgresql-data-postgresql-0
      [ "$APP_PVC" != present ] || echo persistentvolumeclaim/app-postgres-data-app-postgres-0 ;;
    *"exec postgresql-0 -c postgresql -- psql -U postgres -d econ_graph "*)
      case "$DATA_USERS" in fails) return 1 ;; *) echo "$DATA_USERS" ;; esac ;;
    *"exec app-postgres-0 -c postgres -- psql -U postgres -d econ_graph_app "*)
      case "$APP_USERS" in fails) return 1 ;; *) echo "$APP_USERS" ;; esac ;;
    *) echo "unexpected: $*" >&2; return 1 ;;
  esac
}
die() {
  echo "create-secrets: error: $*" >&2
  exit 1
}
NAMESPACE=econ-graph
# shellcheck source=scripts/deploy/lib/app-database-guard.sh
source scripts/deploy/lib/app-database-guard.sh

# check GUARD NAME "DATA_POD DATA_PVC DATA_USERS" "APP_POD APP_PVC APP_USERS" STATUS [MESSAGE] [LOOKUP]
check() {
  local guard="$1" name="$2" status=0 output
  read -r DATA_POD DATA_PVC DATA_USERS <<<"$3"
  read -r APP_POD APP_PVC APP_USERS <<<"$4"
  STUB_LOOKUP="${7:-ok}" STUB_LOG=""
  output="$( ("$guard") 2>&1)" || status=$?
  [ "$status" -eq "$5" ] || fail "$guard: $name: status $status, expected $5 ($output)"
  if [ -n "${6:-}" ] && [[ "$output" != *"$6"* ]]; then
    fail "$guard: $name: output lacks '$6': $output"
  fi
}
up="running present"
none="missing missing -"

# Before separate.
g=require_app_tables_moved
check $g "app tables in econ_graph stop it" "$up t" "$up f" 1 "Keep APP_DATABASE=shared"
check $g "the refusal names the runbook" "$up t" "$up f" 1 "docs/deployment/APP_DATA_CONVERSION.md"
check $g "no app tables pass" "$up f" "$up t" 0
check $g "no data database yet passes" "$none" "$none" 0
check $g "a volume without postgresql-0 stops it" "missing present -" "$none" 1 "is not running"
check $g "a failed query stops it" "$up fails" "$up f" 1 "wait until postgresql-0"
check $g "an unexpected answer stops it" "$up junk" "$up f" 1 "unexpected answer"
check $g "a failed lookup stops it" "$up t" "$up f" 1 "leave APP_DATABASE unset" fails

# Before shared.
g=require_app_tables_not_moved
check $g "tables only in econ_graph_app stop it" "$up f" "$up t" 1 "Use APP_DATABASE=separate"
check $g "the refusal names the rollback" "$up f" "$up t" 1 "--restore-source-app"
check $g "app tables in econ_graph pass" "$up t" "$up t" 0
check $g "app tables in neither pass (a new install)" "$up f" "$up f" 0
check $g "no app database yet passes" "$up f" "$none" 0
check $g "no databases yet pass" "$none" "$none" 0
check $g "an app volume without app-postgres-0 stops it" "$up f" "missing present -" 1 "is not running"
check $g "a failed app query stops it" "$up f" "$up fails" 1 "could not check econ_graph_app"
check $g "an unexpected app answer stops it" "$up f" "$up junk" 1 "unexpected answer"
check $g "a failed data query stops it" "$up fails" "$up t" 1 "could not check econ_graph"
check $g "a volume without postgresql-0 stops it" "missing present -" "$up t" 1 "is not running"

# Only gets and read-only queries, run in this shell so the log is kept.
read -r DATA_POD DATA_PVC DATA_USERS <<<"$up f"
read -r APP_POD APP_PVC APP_USERS <<<"$up f"
STUB_LOOKUP=ok STUB_LOG=""
require_app_tables_moved
require_app_tables_not_moved
if grep -vE "^kc -n econ-graph (get pod (postgresql-0|app-postgres-0) |exec (postgresql-0 -c postgresql|app-postgres-0 -c postgres) -- psql -U postgres -d econ_graph(_app)? -Atc select to_regclass)" <<<"$STUB_LOG" | grep -q .; then
  fail "the guards did more than read: $STUB_LOG"
fi

# create-secrets.sh: the decision, extracted and run with stub lookups, and the guard
# chosen for its direction.
script=scripts/deploy/create-secrets.sh
decision="$(sed -n '/^current_app_url="\$(existing econ-graph-secrets app-database-url)" || exit 1$/,/^# Moving app-database-url to a database/p' "$script")"
[ -n "$decision" ] || fail "cannot find the app database decision in create-secrets.sh"
guards="$(sed -n '/^if \[\[ "\$app_database_how" != kept \]\]; then$/,/^fi$/p' "$script")"
[ -n "$guards" ] || fail "cannot find the guard call in create-secrets.sh"
APP_DB_HOST="app-postgres-service"
# decide APP_DATABASE CURRENT_URL SECRET_EXISTS: prints "<app_database> <how> <guard>" or
# "refused: <message>".
decide() {
  (
    existing() { printf '%s' "$CURRENT_URL"; }
    secret_exists() { [ "$SECRET_EXISTS" = yes ]; }
    require_app_tables_moved() { GUARD=separate; }
    require_app_tables_not_moved() { GUARD=shared; }
    source() { :; }
    die() { echo "refused: $*"; exit 0; }
    APP_DATABASE="$1" CURRENT_URL="$2" SECRET_EXISTS="$3" GUARD=none
    eval "$decision"
    eval "$guards"
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
|$data_url|yes|shared kept none
|$app_url|yes|separate kept none
||no|separate new separate
||yes|refused: *APP_DATABASE=shared until docs/deployment/APP_DATA_CONVERSION.md*
shared||yes|shared from APP_DATABASE shared
separate||yes|separate from APP_DATABASE separate
shared|$data_url|yes|shared kept none
separate|$app_url|yes|separate kept none
separate|$data_url|yes|separate from APP_DATABASE separate
shared|$app_url|yes|shared from APP_DATABASE shared
shared||no|shared from APP_DATABASE shared
EOF

# The guards run before the first Secret is written.
guard_line="$(grep -n '^if \[\[ "\$app_database_how" != kept \]\]; then$' "$script" | cut -d: -f1 || true)"
first_write="$(grep -n '^apply_secret$' "$script" | head -1 | cut -d: -f1)"
if [ -z "$guard_line" ] || [ "$guard_line" -ge "$first_write" ]; then
  fail "create-secrets.sh runs the guards at line ${guard_line:-?}, after its first write at line $first_write"
fi

if [ "$failures" -gt 0 ]; then
  echo "$failures failure(s)"
  exit 1
fi
echo "app database guard tests passed"
