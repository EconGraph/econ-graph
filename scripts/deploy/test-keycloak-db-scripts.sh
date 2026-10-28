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
#     image and the cluster's kube-system UID, encrypted and decrypt-checked with a
#     throwaway key; a count mismatch fails without VERIFIED or plaintext and still drops
#     the scratch database; ciphertext that does not decrypt is never published; a failure
#     right after the tmpfs staging is made leaves none behind;
#   - a backup waits for (then refuses) another plaintext-staging writer before stopping
#     Keycloak; a restore without a public recipient or with disk staging stops before any kubectl
#     call; an encrypted bundle restores in full at a (pseudo) terminal;
#   - an unknown context or a pod that isn't ready stops before anything is changed;
#   - a restore needs a VERIFIED dump from the same cluster (or --allow-other-cluster),
#     refuses without a terminal unless --yes, and with --yes takes a verified dump first,
#     restores into keycloak_restore_*, compares counts, then renames in one transaction;
#   - a restore whose safety dump fails, or whose restored counts differ, exits non-zero
#     and never drops or renames `keycloak`;
#   - every kubectl call goes to $KUBECTL on the given context, and execs in the postgres
#     container (the pod also has an init container);
#   - without KUBECTL, the kind cluster's own kubectl and kubeconfig are used
#     (lib/kind-env.sh), with nothing cached by a dry run; the default context follows
#     KIND_CLUSTER;
#   - the restore says PRODUCTION only on a cluster with the production marker
#     (lib/cluster-role.sh), read on the context it restores to.
#
# Everything runs in a scratch directory: HOME, XDG_CACHE_HOME, TMPDIR, the backup roots
# and the Docker host are pointed into it, and docker and kind are stubs too.
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
ram="$(mktemp -d /dev/shm/econgraph-keycloak-test.XXXXXXXX)"
trap 'rm -rf "$tmp" "$ram"' EXIT
age-keygen -o "$tmp/identity" 2>/dev/null
BACKUP_AGE_RECIPIENT="$(age-keygen -y "$tmp/identity")"
export BACKUP_AGE_RECIPIENT
export BACKUP_PLAINTEXT_ROOT="$ram"
# On-disk plaintext staging for the refusal cases: it must really not be tmpfs.
disk_staging="$tmp/disk-staging"
mkdir "$disk_staging"
if [ "$(stat -f -c %T "$disk_staging")" = tmpfs ]; then
  echo "run with TMPDIR on a disk filesystem, not tmpfs" >&2
  exit 1
fi
# These are synthetic test identities and dumps, never operator credentials.
open_test_bundle() {
    local out
    out="$(mktemp -d "$ram/opened.XXXXXXXX")"
    age -d -i "$tmp/identity" "$1/backup.tar.age" |
        python3 scripts/backup/unpack-backup.py "$out"
    printf '%s\n' "$out"
}
stub="$tmp/kubectl"
export STUB_LOG="$tmp/log"

mkdir -p "$tmp/home" "$tmp/tmpdir" "$tmp/bin"
export HOME="$tmp/home" XDG_CACHE_HOME="$tmp/cache" TMPDIR="$tmp/tmpdir"
export ECONGRAPH_CONFIG_DIR="$tmp/no-config"
export ECONGRAPH_BACKUP_ROOT="$tmp/econgraph-backups" KEYCLOAK_BACKUP_ROOT="$tmp/backups"
export ECONGRAPH_DOCKER_HOST="unix://$tmp/no-docker.sock"
unset KUBECONFIG KUBECTL KIND_CLUSTER NAMESPACE ECONGRAPH_SCRATCH_STORAGE ECONGRAPH_KUBE_DIR
export STUB_KUBECTL="$stub"
export PATH="$tmp/bin:$PATH"

# Stub docker and kind for lib/kind-env.sh: `docker cp` of the node's kubectl copies the
# stub kubectl, `kind get kubeconfig` prints a placeholder; anything else fails.
cat >"$tmp/bin/docker" <<'STUB'
#!/usr/bin/env bash
echo "docker $*" >>"$STUB_LOG"
case "$*" in
  "cp -q -L "*"-control-plane:/usr/bin/kubectl "*) cp "$STUB_KUBECTL" "${*: -1}" ;;
  *) echo "stub docker: unexpected call: $*" >&2; exit 1 ;;
esac
STUB
cat >"$tmp/bin/kind" <<'STUB'
#!/usr/bin/env bash
echo "kind $*" >>"$STUB_LOG"
case "$*" in
  "get clusters") echo "${KIND_CLUSTER:-econ-graph}" ;;
  "get kubeconfig --name "*) echo "# stub kubeconfig" ;;
  *) echo "stub kind: unexpected call: $*" >&2; exit 1 ;;
esac
STUB
chmod +x "$tmp/bin/docker" "$tmp/bin/kind"

# The stub cluster. Every call is appended to $STUB_LOG.
#   STUB_POD_READY=false     keycloak-postgres-0 is not ready
#   STUB_MISMATCH=1          a keycloak_verify_* database has different row counts
#   STUB_RESTORE_MISMATCH=1  a keycloak_restore_* database has different row counts
#   STUB_PGDUMP_FAILS=1      pg_dump fails
#   STUB_UID                 the kube-system UID (default uid-this-cluster)
#   STUB_MARKER=production   the cluster carries a production marker for its UID
#   STUB_ENV_LOG=FILE        append the kubectl path and KUBECONFIG of every call to FILE
cat >"$stub" <<'STUB'
#!/usr/bin/env bash
echo "kubectl $*" >>"$STUB_LOG"
[ -z "${STUB_ENV_LOG:-}" ] || echo "$0 KUBECONFIG=${KUBECONFIG:-}" >>"$STUB_ENV_LOG"
args=("$@")
[ "${args[0]}" = --context ] && args=("${args[@]:2}")
[ "${args[0]}" = -n ] && args=("${args[@]:2}")
set -- "${args[@]}"
counts() { if [ "$1" = 1 ]; then printf 'realm\t2\nuser_entity\t6\n'; else printf 'realm\t2\nuser_entity\t7\n'; fi; }
case "$*" in
  "version --request-timeout=15s") ;;
  "config get-contexts -o name") printf 'kind-econ-graph\nkind-other\nkind-kg-test\n' ;;
  "get namespace kube-system "*) printf '%s' "${STUB_UID:-uid-this-cluster}" ;;
  "get configmap econgraph-cluster-role "*)
    [ "${STUB_MARKER:-none}" != production ] || printf 'production %s' "${STUB_UID:-uid-this-cluster}" ;;
  "get pod keycloak-postgres-0 "*'@.name=="postgres"'*) printf '%s' "${STUB_POD_READY:-true}" ;;
  *"jsonpath={.spec.replicas}") printf 1 ;;
  *"jsonpath={.spec.template.spec.containers[0].image}") printf 'quay.io/keycloak/keycloak:26.7.4' ;;
  "scale deployment/keycloak --replicas=0") ;;
  "get pod -l app=keycloak -o name") echo pod/keycloak-1 ;;
  "wait --for=delete pod -l app=keycloak --timeout=300s") ;;
  "exec -i keycloak-postgres-0 -c postgres -- pg_dump "*)
    [ "${STUB_PGDUMP_FAILS:-0}" != 1 ] || exit 1
    printf 'PGDMP fake dump' ;;
  "exec -i keycloak-postgres-0 -c postgres -- pg_restore "*) cat >/dev/null ;;
  "exec -i keycloak-postgres-0 -c postgres -- psql -U keycloak -d keycloak "*) counts 0 ;;
  "exec -i keycloak-postgres-0 -c postgres -- psql -U keycloak -d keycloak_verify_"*) counts "${STUB_MISMATCH:-0}" ;;
  "exec -i keycloak-postgres-0 -c postgres -- psql -U keycloak -d keycloak_restore_"*) counts "${STUB_RESTORE_MISMATCH:-0}" ;;
  "exec -i keycloak-postgres-0 -c postgres -- psql -U keycloak -d postgres "*) ;;
  "exec -i keycloak-postgres-0 -c postgres -- createdb "*) ;;
  "exec -i keycloak-postgres-0 -c postgres -- dropdb "*)
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
  env KUBECTL="$stub" "$@" >"$tmp/out" 2>&1 </dev/null || status=$?
}
# run, without KUBECTL: the scripts find the kind cluster's kubectl themselves.
run_kind() {
  : >"$STUB_LOG"
  status=0
  "$@" >"$tmp/out" 2>&1 </dev/null || status=$?
}
writes() { grep -E -- ' scale | exec | createdb | dropdb ' "$STUB_LOG" || true; }
unpinned() { grep -v -e '--context kind-econ-graph ' -e 'config get-contexts' -e '^REFUSED' -e '^docker ' -e '^kind ' -e '^kubectl version ' "$STUB_LOG" || true; }
# Anything that removed, renamed or replaced the database `keycloak`.
touched_keycloak() { grep -E 'RENAME|DROP DATABASE|^REFUSED|-- dropdb .* keycloak$' "$STUB_LOG" || true; }

backup=scripts/deploy/backup-keycloak-db.sh

# The scratch-drop guard: only keycloak_verify_* and keycloak_restore_* may be dropped.
for db in keycloak keycloak_pre_restore_x postgres; do
  run bash -c 'source scripts/deploy/lib/keycloak-db.sh
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

# Encryption setup fails before any context access or downtime.
run env -u BACKUP_AGE_RECIPIENT "$backup"
[ "$status" != 0 ] || fail "backup without a public recipient succeeded"
[ ! -s "$STUB_LOG" ] || fail "backup without encryption setup contacted the cluster"

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
  [ -f "$dir/backup.tar.age" ] || fail "backup is not encrypted"
  # Encrypted to the recipient and to the throwaway key it was decrypt-checked with.
  [ "$(grep -ac '^-> X25519 ' "$dir/backup.tar.age")" = 2 ] || fail "backup was not decrypt-checked with a throwaway key"
  [ ! -f "$dir/keycloak.dump" ] || fail "backup persisted plaintext"
  dir="$(open_test_bundle "$dir")"
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
[ -z "$(find "$tmp/backups" -name '*.dump')" ] || fail "a mismatched backup persisted plaintext"
grep -q -- '-- dropdb -U keycloak --if-exists keycloak_verify_' "$STUB_LOG" || fail "a failed backup left the scratch database"
[ -z "$(touched_keycloak)" ] || fail "a failed backup touched keycloak: $(touched_keycloak)"

# Ciphertext that age wrote "successfully" but that does not decrypt is never
# published as VERIFIED.
rm -rf "$tmp/backups"
mkdir -p "$tmp/corrupt-age"
cat >"$tmp/corrupt-age/age" <<STUB
#!/usr/bin/env bash
set -euo pipefail
case " \$* " in
  *' -o '*)
    $(command -v age) "\$@"
    [ "\${*: -2:1}" = -o ]
    printf '\377' | dd of="\${*: -1}" bs=1 seek=300 conv=notrunc status=none ;;
  *) exec $(command -v age) "\$@" ;;
esac
STUB
chmod +x "$tmp/corrupt-age/age"
run env PATH="$tmp/corrupt-age:$PATH" "$backup"
[ "$status" != 0 ] || fail "backup with ciphertext that does not decrypt succeeded"
grep -q 'does not decrypt' "$tmp/out" || fail "backup with bad ciphertext did not say why: $(cat "$tmp/out")"
[ -z "$(find "$tmp/backups" -name VERIFIED)" ] || fail "ciphertext that does not decrypt was marked VERIFIED"
[ -z "$(find "$tmp/backups" -mindepth 1 -maxdepth 1 -name '*Z')" ] || fail "ciphertext that does not decrypt was published"

# Restore.
rm -rf "$tmp/backups"
run "$backup"
dir="$(find "$tmp/backups" -mindepth 1 -maxdepth 1 -type d -name '*Z' | head -n 1)"
encrypted_dir="$dir"
run "$restore" "$encrypted_dir" --yes
[ "$status" != 0 ] || fail "encrypted restore without operator terminal succeeded"
[ ! -s "$STUB_LOG" ] || fail "encrypted unattended restore contacted cluster"
dir="$(open_test_bundle "$dir")"
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
if grep -q PRODUCTION "$tmp/out"; then fail "restore on an unmarked cluster says it is production"; fi
STUB_MARKER=production run "$restore" "$dir" --dry-run
grep -q PRODUCTION "$tmp/out" || fail "restore on a production cluster does not say so: $(cat "$tmp/out")"
grep -q -- '--context kind-econ-graph -n kube-system get configmap econgraph-cluster-role' "$STUB_LOG" ||
  fail "restore did not read the production marker on its context"
STUB_UID=uid-other-cluster run "$restore" "$dir" --dry-run --allow-other-cluster --context kind-other
[ "$status" = 0 ] || fail "restore --context kind-other --dry-run exits $status: $(cat "$tmp/out")"
grep -q -- '--context kind-other ' "$STUB_LOG" || fail "restore --context kind-other did not use that context"
if grep -q -- '--context kind-econ-graph ' "$STUB_LOG"; then fail "restore --context kind-other also used kind-econ-graph"; fi

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

# A restore checks that its safety backup can be encrypted before it reaches the
# cluster, so it never stops Keycloak and then dies.
run env -u BACKUP_AGE_RECIPIENT "$restore" "$dir" --yes
[ "$status" != 0 ] || fail "restore without a public recipient succeeded"
grep -q 'set BACKUP_AGE_RECIPIENT' "$tmp/out" || fail "restore without a recipient did not say why: $(cat "$tmp/out")"
[ ! -s "$STUB_LOG" ] || fail "restore without a recipient contacted the cluster: $(cat "$STUB_LOG")"
run env BACKUP_PLAINTEXT_ROOT="$disk_staging" "$restore" "$dir" --yes
[ "$status" != 0 ] || fail "restore with disk staging succeeded"
grep -q 'must be tmpfs' "$tmp/out" || fail "restore with disk staging did not say why: $(cat "$tmp/out")"
[ ! -s "$STUB_LOG" ] || fail "restore with disk staging contacted the cluster: $(cat "$STUB_LOG")"

# Another writer is staging plaintext in the same tmpfs: the backup says whom it
# waits for and, after BACKUP_LOCK_TIMEOUT, refuses before stopping Keycloak.
staging_lock="$ram/.econgraph-backup-$(id -u).lock"
printf 'pid=1 command=dump-databases.sh\n' >"$staging_lock"
flock -o "$staging_lock" sleep 30 &
holder=$!
until ! flock -n "$staging_lock" true; do sleep 0.1; done
run env BACKUP_LOCK_TIMEOUT=1 "$backup"
kill "$holder"
wait "$holder" 2>/dev/null || true
[ "$status" != 0 ] || fail "backup went ahead while another writer held the staging lock"
grep -q 'Waiting up to 1s for another backup .*command=dump-databases.sh' "$tmp/out" ||
  fail "backup did not say whom it waits for: $(cat "$tmp/out")"
if grep -q 'scale deployment/keycloak' "$STUB_LOG"; then fail "backup stopped Keycloak before it had the staging lock"; fi

# The staging trap is in place before the staging exists: a failure right after
# it is created (here: making its payload directory) leaves no plaintext behind.
mkdir -p "$tmp/mkdir-fails"
cat >"$tmp/mkdir-fails/mkdir" <<STUB
#!/usr/bin/env bash
case "\${*: -1}" in */payload) echo "stub mkdir: refusing \$*" >&2; exit 1 ;; esac
exec $(command -v mkdir) "\$@"
STUB
chmod +x "$tmp/mkdir-fails/mkdir"
run env PATH="$tmp/mkdir-fails:$PATH" "$backup"
[ "$status" != 0 ] || fail "backup succeeded although its payload directory could not be made"
grep -q 'stub mkdir: refusing' "$tmp/out" || fail "the payload mkdir was never reached: $(cat "$tmp/out")"
if compgen -G "$ram/econgraph-backup.*" >/dev/null; then
  fail "a failed backup left plaintext staging: $(ls "$ram")"
fi

# A full restore of an encrypted bundle at an operator terminal: open-backup.sh
# asks for the identity, the restore runs on the opened copy (taking its own
# encrypted safety backup first), and the opened plaintext is removed after.
run_operator() { # run_operator SCRIPT ARGS..., on a pseudo terminal that gives the test identity
  : >"$STUB_LOG"
  status=0
  KUBECTL="$stub" python3 - "$tmp/identity" "$tmp/out" "$@" <<'PYOPEN' || status=$?
import errno, os, pty, select, sys, time
identity, log, *command = sys.argv[1:]
pid, fd = pty.fork()
if pid == 0:
    os.execv(command[0], command)
output = bytearray()
sent = False
deadline = time.monotonic() + 60
while time.monotonic() < deadline:
    if select.select([fd], [], [], 1)[0]:
        try:
            chunk = os.read(fd, 4096)
        except OSError as error:
            if error.errno == errno.EIO:
                break
            raise
        if not chunk:
            break
        output.extend(chunk)
        if not sent and b'identity' in output:
            os.write(fd, (identity + '\n').encode())
            sent = True
else:
    os.kill(pid, 9)
    raise AssertionError('operator restore timed out')
os.close(fd)
_, wait_status = os.waitpid(pid, 0)
open(log, 'wb').write(output)
sys.exit(os.waitstatus_to_exitcode(wait_status))
PYOPEN
}
verified_before="$(find "$tmp/backups" -name VERIFIED | wc -l)"
run_operator "$restore" "$encrypted_dir" --yes
[ "$status" = 0 ] || fail "encrypted restore at a terminal exits $status: $(cat "$tmp/out")"
grep -q "restored from $ram/econgraph-backup\." "$tmp/out" || fail "the encrypted restore did not run on the opened copy: $(cat "$tmp/out")"
order="$(grep -oE 'scale deployment|-- pg_dump|-- createdb -U keycloak -O keycloak keycloak_restore_[^ ]*|-d keycloak_restore_[^ ]* --no-owner --exit-on-error --single-transaction$|RENAME TO keycloak_pre_restore_' "$STUB_LOG" |
  sed 's/keycloak_restore_[^ ]*/R/' | uniq | tr '\n' ,)"
[ "$order" = "scale deployment,-- pg_dump,-- createdb -U keycloak -O keycloak R,-d R --no-owner --exit-on-error --single-transaction,RENAME TO keycloak_pre_restore_," ] ||
  fail "encrypted restore steps in the wrong order: $order"
[ "$(find "$tmp/backups" -name VERIFIED | wc -l)" -gt "$verified_before" ] || fail "the encrypted restore took no safety backup"
[ -z "$(find "$tmp/backups" -name '*.dump')" ] || fail "the encrypted restore persisted plaintext"
if compgen -G "$ram/econgraph-backup.*" >/dev/null; then
  fail "the encrypted restore left opened plaintext: $(ls "$ram")"
fi

# Without KUBECTL: the kind cluster's kubectl and kubeconfig (lib/kind-env.sh). A dry run
# caches nothing in XDG_CACHE_HOME and leaves no temporary copy behind.
cache="$XDG_CACHE_HOME/econgraph"
export STUB_ENV_LOG="$tmp/env-log"
: >"$STUB_ENV_LOG"
run_kind "$backup" --dry-run
[ "$status" = 0 ] || fail "backup --dry-run with the kind kubectl exits $status: $(cat "$tmp/out")"
[ ! -e "$cache" ] || fail "backup --dry-run cached the kind kubectl in $cache"
[ -z "$(ls -A "$TMPDIR")" ] || fail "backup --dry-run left $(ls -A "$TMPDIR") in TMPDIR"
grep -q 'docker cp -q -L econ-graph-control-plane:/usr/bin/kubectl' "$STUB_LOG" ||
  fail "backup --dry-run did not copy the kind node's kubectl: $(cat "$STUB_LOG")"
if grep -v "/bin/kubectl KUBECONFIG=$TMPDIR/econgraph-kube\.[^/]*/kubeconfig-econ-graph$" "$STUB_ENV_LOG"; then
  fail "backup --dry-run ran kubectl other than the kind cluster's"
fi
: >"$STUB_ENV_LOG"
run_kind "$backup"
[ "$status" = 0 ] || fail "backup with the kind kubectl exits $status: $(cat "$tmp/out")"
[ -x "$cache/bin/kubectl" ] || fail "backup did not use the cached kind kubectl"
calls="$(grep -vxF "$cache/bin/kubectl KUBECONFIG=$cache/kubeconfig-econ-graph" "$STUB_ENV_LOG" || true)"
[ -z "$calls" ] || fail "backup ran kubectl other than the kind cluster's: $calls"
[ -s "$STUB_ENV_LOG" ] || fail "backup with the kind kubectl made no kubectl calls"
[ -z "$(unpinned)" ] || fail "kubectl calls without the context: $(unpinned)"
unset STUB_ENV_LOG
# A throwaway cluster (lib/kind-env.sh): the default context follows KIND_CLUSTER.
KIND_CLUSTER=kg-test ECONGRAPH_SCRATCH_STORAGE="$tmp/storage" run "$backup" --dry-run
[ "$status" = 0 ] || fail "backup --dry-run with KIND_CLUSTER=kg-test exits $status: $(cat "$tmp/out")"
grep -q -- '--context kind-kg-test ' "$STUB_LOG" || fail "backup with KIND_CLUSTER=kg-test did not use kind-kg-test"
[ ! -e "$tmp/storage" ] || fail "backup --dry-run created $tmp/storage"
[ -z "$(ls -A "$HOME")" ] || fail "something was written to HOME: $(ls -A "$HOME")"
[ ! -e "$ECONGRAPH_BACKUP_ROOT" ] || fail "something was written to ECONGRAPH_BACKUP_ROOT"

if [ "$failures" -ne 0 ]; then
  echo "$failures failure(s)"
  exit 1
fi
echo "backup-keycloak-db.sh and restore-keycloak-db.sh: all checks passed"
