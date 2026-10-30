#!/usr/bin/env bash
# The stub kc is called by the sourced library.
# shellcheck disable=SC2016,SC2034,SC2154,SC2329
# Tests for scripts/deploy/lib/app-database-guard.sh against a stub kc, no cluster:
#   - the data database still holding public.users stops it, naming the conversion runbook
#     and APP_DATABASE=shared;
#   - no app tables, or a new install (no postgresql-0 and no data volume), passes;
#   - a data volume without a running postgresql-0, a failed lookup, a failed query or an
#     unexpected answer stops it (fail closed);
#   - the guard only reads;
#   - create-secrets.sh runs it, before it writes any Secret, whenever app-database-url
#     would move to the app database (APP_DATABASE=separate, or a Secret without the key),
#     and not when the URL already names the app database or APP_DATABASE=shared.
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

# STUB_POD: running or missing; STUB_PVC: present or missing; STUB_USERS: t, f, junk or
# fails; STUB_LOOKUP=fails makes the pod lookup fail.
STUB_LOG=""
kc() {
  STUB_LOG+="kc $*"$'\n'
  case "$*" in
    *"get pod postgresql-0"*)
      [ "${STUB_LOOKUP:-ok}" != fails ] || return 1
      [ "$STUB_POD" != running ] || echo pod/postgresql-0 ;;
    *"get persistentvolumeclaim"*)
      [ "$STUB_PVC" != present ] || echo persistentvolumeclaim/postgresql-data-postgresql-0 ;;
    *"exec postgresql-0 -c postgresql -- psql"*)
      case "$STUB_USERS" in fails) return 1 ;; *) echo "$STUB_USERS" ;; esac ;;
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

# check NAME POD PVC USERS EXPECTED_STATUS [EXPECTED_MESSAGE] [LOOKUP]
check() {
  local name="$1" status=0 output
  STUB_POD="$2" STUB_PVC="$3" STUB_USERS="$4" STUB_LOOKUP="${7:-ok}" STUB_LOG=""
  output="$( (require_app_tables_moved) 2>&1)" || status=$?
  [ "$status" -eq "$5" ] || fail "$name: status $status, expected $5 ($output)"
  if [ -n "${6:-}" ] && [[ "$output" != *"$6"* ]]; then
    fail "$name: output lacks '$6': $output"
  fi
}

check "app tables in the data database stop it" running present t 1 "docs/deployment/APP_DATA_CONVERSION.md"
check "the refusal says to keep shared" running present t 1 "APP_DATABASE=shared"
check "no app tables pass" running present f 0
check "a new install passes" missing missing - 0
check "a volume without postgresql-0 stops it" missing present - 1 "is not running"
check "a failed query stops it" running present fails 1 "could not check"
check "an unexpected answer stops it" running present junk 1 "unexpected answer"
check "a failed lookup stops it" running present t 1 "could not look up" fails

# Only gets and the read-only query, run directly so the log is kept.
STUB_POD=running STUB_PVC=present STUB_USERS=f STUB_LOOKUP=ok STUB_LOG=""
require_app_tables_moved
if grep -vE '^kc -n econ-graph (get pod postgresql-0 |exec postgresql-0 -c postgresql -- psql -U postgres -d econ_graph -Atc select to_regclass)' <<<"$STUB_LOG" | grep -q .; then
  fail "the guard did more than read: $STUB_LOG"
fi

# create-secrets.sh: the guard runs before the first Secret is written, only when the URL
# would move to the app database.
script=scripts/deploy/create-secrets.sh
guard_line="$(grep -n '^    require_app_tables_moved$' "$script" | cut -d: -f1 || true)"
condition_line="$(grep -n '^if \[\[ "\$app_database" == separate && "\$app_database_how" != kept \]\]; then$' "$script" | cut -d: -f1 || true)"
first_write="$(grep -n '^apply_secret$' "$script" | head -1 | cut -d: -f1)"
if [ -z "$guard_line" ] || [ -z "$condition_line" ]; then
  fail "create-secrets.sh does not run require_app_tables_moved when the URL moves to the app database"
elif [ "$guard_line" -ge "$first_write" ]; then
  fail "create-secrets.sh runs the guard at line $guard_line, after its first write at line $first_write"
elif [ "$((guard_line - condition_line))" -gt 3 ]; then
  fail "require_app_tables_moved is not directly under the separate/not-kept condition"
fi

# The decision itself, extracted from create-secrets.sh and run with a stub `existing`.
decision="$(sed -n '/^app_database="\${APP_DATABASE-}"$/,/^fi$/p' "$script")"
[ -n "$decision" ] || fail "cannot find the app database decision in create-secrets.sh"
APP_DB_HOST=app-postgres-service
# decide APP_DATABASE CURRENT_URL: prints "<app_database> <how>"
decide() {
  existing() { printf '%s' "$CURRENT_URL"; }
  APP_DATABASE="$1" CURRENT_URL="$2"
  eval "$decision"
  echo "$app_database $app_database_how"
}
guarded() { [[ "$1" == separate* && "$1" != *kept ]]; }
for case_ in "|postgresql://u:p@data-postgres-service:5432/econ_graph|shared kept|no" \
  "||separate new|yes" \
  "|postgresql://u:p@app-postgres-service:5432/econ_graph_app|separate kept|no" \
  "separate|postgresql://u:p@data-postgres-service:5432/econ_graph|separate from APP_DATABASE|yes" \
  "shared||shared from APP_DATABASE|no"; do
  IFS='|' read -r env url expected guard <<<"$case_"
  got="$(decide "$env" "$url")"
  [ "$got" = "$expected" ] || fail "APP_DATABASE='$env', current URL '$url': decided '$got', expected '$expected'"
  if guarded "$got"; then is=yes; else is=no; fi
  [ "$is" = "$guard" ] || fail "APP_DATABASE='$env', current URL '$url': guard runs=$is, expected $guard"
done

if [ "$failures" -gt 0 ]; then
  echo "$failures failure(s)"
  exit 1
fi
echo "app database guard tests passed"
