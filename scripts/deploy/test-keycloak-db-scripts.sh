#!/usr/bin/env bash
# The stub kubectl is written as a quoted heredoc and runs on its own.
# shellcheck disable=SC2016
# Tests for scripts/deploy/backup-keycloak-db.sh and restore-keycloak-db.sh against a stub
# kubectl, no cluster. The stub refuses (and logs REFUSED) any dropdb of a database that is
# not a scratch one (keycloak_verify_*, keycloak_restore_*).
#   - --dry-run scales nothing, runs nothing in the pod and writes nothing;
#   - a backup scales Keycloak to 0, dumps, restores into a scratch database in one
#     transaction stopping at the first error, compares row counts, drops the scratch
#     database, and leaves a VERIFIED directory (0700, files 0600) that names the Keycloak
#     image and the cluster's kube-system UID; a count mismatch fails, keeps <time>.failed
#     without VERIFIED, and still drops the scratch database;
#   - an unknown context or a pod that isn't ready stops before anything is changed;
#   - a restore needs a VERIFIED dump from the same cluster (or --allow-other-cluster),
#     refuses without a terminal unless --yes, and with --yes takes a verified dump first,
#     restores into keycloak_restore_*, compares counts, then renames in one transaction;
#   - a restore whose safety dump fails, or whose restored counts differ, exits non-zero
#     and never drops or renames `keycloak`;
#   - every kubectl call goes to $KUBECTL on the given context.
#
# Usage: scripts/deploy/test-keycloak-db-scripts.sh (from anywhere)
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
stub="$tmp/kubectl"
export STUB_LOG="$tmp/log"

# The stub cluster. Every call is appended to $STUB_LOG.
#   STUB_POD_READY=false     keycloak-postgres-0 is not ready
#   STUB_MISMATCH=1          a keycloak_verify_* database has different row counts
#   STUB_RESTORE_MISMATCH=1  a keycloak_restore_* database has different row counts
#   STUB_PGDUMP_FAILS=1      pg_dump fails
#   STUB_UID                 the kube-system UID (default uid-this-cluster)
#   STUB_MARKER=production   the cluster carries a production marker for its UID
cat >"$stub" <<'STUB'
#!/usr/bin/env bash
echo "kubectl $*" >>"$STUB_LOG"
args=("$@")
[ "${args[0]}" = --context ] && args=("${args[@]:2}")
[ "${args[0]}" = -n ] && args=("${args[@]:2}")
set -- "${args[@]}"
counts() { if [ "$1" = 1 ]; then printf 'realm\t2\nuser_entity\t6\n'; else printf 'realm\t2\nuser_entity\t7\n'; fi; }
case "$*" in
  "config get-contexts -o name") printf 'kind-econ-graph\nkind-other\n' ;;
  "config current-context") echo kind-econ-graph ;;
  "get namespace kube-system "*) printf '%s' "${STUB_UID:-uid-this-cluster}" ;;
  "get configmap econgraph-cluster-role "*)
    [ "${STUB_MARKER:-none}" != production ] || printf 'production %s' "${STUB_UID:-uid-this-cluster}" ;;
  "get pod keycloak-postgres-0 "*) printf '%s' "${STUB_POD_READY:-true}" ;;
  *"jsonpath={.spec.replicas}") printf 1 ;;
  *"jsonpath={.spec.template.spec.containers[0].image}") printf 'quay.io/keycloak/keycloak:26.7.4' ;;
  "scale deployment/keycloak --replicas=0") ;;
  "get pod -l app=keycloak -o name") echo pod/keycloak-1 ;;
  "wait --for=delete pod -l app=keycloak --timeout=300s") ;;
  "exec -i keycloak-postgres-0 -- pg_dump "*)
    [ "${STUB_PGDUMP_FAILS:-0}" != 1 ] || exit 1
    printf 'PGDMP fake dump' ;;
  "exec -i keycloak-postgres-0 -- pg_restore "*) cat >/dev/null ;;
  "exec -i keycloak-postgres-0 -- psql -U keycloak -d keycloak "*) counts 0 ;;
  "exec -i keycloak-postgres-0 -- psql -U keycloak -d keycloak_verify_"*) counts "${STUB_MISMATCH:-0}" ;;
  "exec -i keycloak-postgres-0 -- psql -U keycloak -d keycloak_restore_"*) counts "${STUB_RESTORE_MISMATCH:-0}" ;;
  "exec -i keycloak-postgres-0 -- psql -U keycloak -d postgres "*) ;;
  "exec -i keycloak-postgres-0 -- createdb "*) ;;
  "exec -i keycloak-postgres-0 -- dropdb "*)
    case "${*: -1}" in
      keycloak_verify_* | keycloak_restore_*) ;;
      *) echo "REFUSED dropdb ${*: -1}" >>"$STUB_LOG"; exit 1 ;;
    esac ;;
  *) echo "stub: unexpected call: $*" >&2; exit 1 ;;
esac
STUB
chmod +x "$stub"

run() { # run SCRIPT ARGS..., with the stub; output in $tmp/out, status in $status
  : >"$STUB_LOG"
  status=0
  env KUBECTL="$stub" KEYCLOAK_BACKUP_ROOT="$tmp/backups" "$@" >"$tmp/out" 2>&1 </dev/null || status=$?
}
writes() { grep -E -- ' scale | exec | createdb | dropdb ' "$STUB_LOG" || true; }
unpinned() { grep -v -e '--context kind-econ-graph ' -e 'config get-contexts' -e '^REFUSED' "$STUB_LOG" || true; }
# Anything that removed, renamed or replaced the database `keycloak`.
touched_keycloak() { grep -E 'RENAME|DROP DATABASE|^REFUSED|-- dropdb .* keycloak$' "$STUB_LOG" || true; }

backup=scripts/deploy/backup-keycloak-db.sh

# The scratch-drop guard: only keycloak_verify_* and keycloak_restore_* may be dropped.
for db in keycloak keycloak_pre_restore_x postgres; do
  run bash -c 'source scripts/deploy/lib/cluster-role.sh; source scripts/deploy/lib/keycloak-db.sh
    CLUSTER_ROLE_CONTEXT=kind-econ-graph KDB_SCRIPT=test kdb_drop_scratch "$1"' _ "$db"
  [ "$status" != 0 ] || fail "kdb_drop_scratch $db succeeded"
  if grep -q -- '-- dropdb' "$STUB_LOG"; then fail "kdb_drop_scratch $db ran dropdb"; fi
done
restore=scripts/deploy/restore-keycloak-db.sh

# Dry run.
run "$backup" --dry-run
[ "$status" = 0 ] || fail "backup --dry-run exits $status: $(cat "$tmp/out")"
[ -z "$(writes)" ] || fail "backup --dry-run changed something: $(writes)"
[ ! -e "$tmp/backups" ] || fail "backup --dry-run wrote $tmp/backups"

# Unknown context, pod not ready.
run "$backup" --context kind-missing
[ "$status" != 0 ] || fail "backup on an unknown context succeeded"
[ "$(wc -l <"$STUB_LOG")" = 1 ] || fail "backup on an unknown context made calls: $(cat "$STUB_LOG")"
STUB_POD_READY=false run "$backup"
[ "$status" != 0 ] || fail "backup with the pod not ready succeeded"
[ -z "$(writes)" ] || fail "backup with the pod not ready changed something: $(writes)"

# A good backup.
run "$backup"
[ "$status" = 0 ] || fail "backup exits $status: $(cat "$tmp/out")"
dir="$(find "$tmp/backups" -mindepth 1 -maxdepth 1 -type d -name '*Z' | head -n 1)"
if [ -z "$dir" ]; then
  fail "backup left no directory: $(ls -la "$tmp/backups" 2>&1)"
else
  [ -f "$dir/VERIFIED" ] || fail "backup has no VERIFIED marker"
  grep -qx 'keycloak_image=quay.io/keycloak/keycloak:26.7.4' "$dir/VERIFIED" || fail "VERIFIED does not name the image"
  grep -qx 'cluster_uid=uid-this-cluster' "$dir/VERIFIED" || fail "VERIFIED does not name the cluster"
  [ "$(stat -c %a "$dir")" = 700 ] || fail "dump directory mode is $(stat -c %a "$dir")"
  [ "$(stat -c %a "$tmp/backups")" = 700 ] || fail "backup root mode is $(stat -c %a "$tmp/backups")"
  for f in "$dir"/*; do
    [ "$(stat -c %a "$f")" = 600 ] || fail "$f mode is $(stat -c %a "$f")"
  done
  [ "$(cat "$dir/keycloak.dump")" = 'PGDMP fake dump' ] || fail "the dump file is not what pg_dump wrote"
fi
grep -q 'scale deployment/keycloak --replicas=0' "$STUB_LOG" || fail "backup did not scale Keycloak to 0"
order="$(grep -oE 'scale deployment|-- pg_dump|-- createdb|-- pg_restore|-- dropdb' "$STUB_LOG" | tr '\n' ,)"
[ "$order" = "scale deployment,-- pg_dump,-- createdb,-- pg_restore,-- dropdb," ] || fail "backup steps in the wrong order: $order"
grep -q -- '-- pg_restore -U keycloak -d keycloak_verify_[^ ]* --no-owner --exit-on-error --single-transaction$' "$STUB_LOG" ||
  fail "backup did not restore into a scratch database with --exit-on-error --single-transaction"
[ -z "$(touched_keycloak)" ] || fail "backup touched keycloak: $(touched_keycloak)"
[ -z "$(unpinned)" ] || fail "kubectl calls without the context: $(unpinned)"

# A backup whose restored row counts differ.
rm -rf "$tmp/backups"
STUB_MISMATCH=1 run "$backup"
[ "$status" != 0 ] || fail "backup with mismatched row counts succeeded"
[ -z "$(find "$tmp/backups" -name VERIFIED)" ] || fail "a mismatched backup was marked VERIFIED"
[ -n "$(find "$tmp/backups" -maxdepth 1 -name '*.failed')" ] || fail "a mismatched backup left no .failed directory"
grep -q -- '-- dropdb -U keycloak --if-exists keycloak_verify_' "$STUB_LOG" || fail "a failed backup left the scratch database"
[ -z "$(touched_keycloak)" ] || fail "a failed backup touched keycloak: $(touched_keycloak)"

# Restore.
rm -rf "$tmp/backups"
run "$backup"
dir="$(find "$tmp/backups" -mindepth 1 -maxdepth 1 -type d -name '*Z' | head -n 1)"
mkdir -p "$tmp/unverified"
cp "$dir/keycloak.dump" "$dir/counts.tsv" "$tmp/unverified/"
run "$restore" "$tmp/unverified" --yes
[ "$status" != 0 ] || fail "restore of a dump without VERIFIED succeeded"
[ ! -s "$STUB_LOG" ] || fail "restore of an unverified dump made calls: $(cat "$STUB_LOG")"
run "$restore" "$dir"
[ "$status" != 0 ] || fail "restore without a terminal or --yes succeeded"
[ -z "$(writes)" ] || fail "an unconfirmed restore changed something: $(writes)"
run "$restore" "$dir" --dry-run
[ "$status" = 0 ] || fail "restore --dry-run exits $status: $(cat "$tmp/out")"
[ -z "$(writes)" ] || fail "restore --dry-run changed something: $(writes)"
# A dump from another cluster.
STUB_UID=uid-other-cluster run "$restore" "$dir" --yes
[ "$status" != 0 ] || fail "restore of another cluster's dump succeeded"
[ -z "$(writes)" ] || fail "restore of another cluster's dump changed something: $(writes)"
STUB_UID=uid-other-cluster run "$restore" "$dir" --dry-run --allow-other-cluster
[ "$status" = 0 ] || fail "restore --allow-other-cluster --dry-run exits $status: $(cat "$tmp/out")"
grep -q 'WARNING: .*ANOTHER CLUSTER' "$tmp/out" || fail "restore --allow-other-cluster does not warn"
STUB_MARKER=production run "$restore" "$dir" --dry-run
grep -q PRODUCTION "$tmp/out" || fail "restore on a production cluster does not say so: $(cat "$tmp/out")"

# The safety dump inside a restore fails: nothing happens to keycloak.
STUB_PGDUMP_FAILS=1 run "$restore" "$dir" --yes
[ "$status" != 0 ] || fail "restore succeeded although its safety dump failed"
[ -z "$(touched_keycloak)" ] || fail "restore touched keycloak after a failed safety dump: $(touched_keycloak)"
if grep -q -- '-- createdb .*keycloak_restore_' "$STUB_LOG"; then fail "restore went on after a failed safety dump"; fi

# The restored database's counts differ: keycloak untouched, the restore database dropped.
STUB_RESTORE_MISMATCH=1 run "$restore" "$dir" --yes
[ "$status" != 0 ] || fail "restore with mismatched row counts succeeded"
[ -z "$(touched_keycloak)" ] || fail "restore with mismatched row counts touched keycloak: $(touched_keycloak)"
grep -q -- '-- dropdb -U keycloak --if-exists keycloak_restore_' "$STUB_LOG" ||
  fail "restore with mismatched row counts left its restore database"

# A good restore.
run "$restore" "$dir" --yes
[ "$status" = 0 ] || fail "restore --yes exits $status: $(cat "$tmp/out")"
# The stub always reports 1 replica, so the backup inside the restore scales again: uniq.
order="$(grep -oE 'scale deployment|-- pg_dump|-- createdb -U keycloak -O keycloak keycloak_restore_[^ ]*|-d keycloak_restore_[^ ]* --no-owner --exit-on-error --single-transaction$|RENAME TO keycloak_pre_restore_' "$STUB_LOG" |
  sed 's/keycloak_restore_[^ ]*/R/' | uniq | tr '\n' ,)"
[ "$order" = "scale deployment,-- pg_dump,-- createdb -U keycloak -O keycloak R,-d R --no-owner --exit-on-error --single-transaction,RENAME TO keycloak_pre_restore_," ] ||
  fail "restore steps in the wrong order: $order"
swap="$(grep -- '-d postgres' "$STUB_LOG" || true)"
[[ "$swap" == *"--single-transaction"*"pg_terminate_backend"*"ALTER DATABASE keycloak RENAME TO keycloak_pre_restore_"*"RENAME TO keycloak" ]] ||
  fail "the swap is not one transaction that ends connections and renames both: $swap"
if grep -q 'DROP DATABASE\|^REFUSED' "$STUB_LOG"; then fail "restore dropped a database: $(grep 'DROP DATABASE\|^REFUSED' "$STUB_LOG")"; fi
grep -q 'keycloak_pre_restore_' "$tmp/out" || fail "restore does not print the kept previous database"
[ -z "$(unpinned)" ] || fail "kubectl calls without the context: $(unpinned)"
[ "$(find "$tmp/backups" -name VERIFIED | wc -l)" -ge 2 ] || fail "restore did not take a verified dump first"

if [ "$failures" -ne 0 ]; then
  echo "$failures failure(s)"
  exit 1
fi
echo "backup-keycloak-db.sh and restore-keycloak-db.sh: all checks passed"
