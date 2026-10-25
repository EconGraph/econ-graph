#!/usr/bin/env bash
#
# Runs the postgres-backup CronJob's scripts (backup.sh, sync.sh and restore.sh
# from k8s/manifests/postgres-backup-configmap.yaml) in the CronJob's images,
# against two scratch postgres:18 containers standing in for postgres-service
# (econ_graph) and keycloak-postgres-service (keycloak), with an S3 mock as the
# off-site destination. The pg-dump container's environment is read from
# k8s/manifests/postgres-backup-cronjob.yaml, with test passwords in place of
# the Secrets. Never touches a cluster.
#
# Checks:
#   - both dumps are written, read back, restore into scratch databases with
#     every table's row count equal to the source, and are synced to S3
#     without the latest*.dump links;
#   - restore.sh puts econ_graph back, and refuses a Keycloak dump;
#   - when either database's dump fails (server down, wrong password, no
#     tables) the other dump is still written and synced, and sync.sh fails
#     the Job; without BACKUP_STATUS_FILE, backup.sh itself exits 1;
#   - without the Keycloak password (no Keycloak deployed) econ_graph alone
#     is backed up and the run succeeds;
#   - no run deletes a dump, and no output shows either password.
#
# Needs docker (DOCKER_HOST is honoured) and python3 with PyYAML.
# Usage: scripts/deploy/test-postgres-backup-scripts.sh

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CONFIGMAP="$ROOT/k8s/manifests/postgres-backup-configmap.yaml"
CRONJOB="$ROOT/k8s/manifests/postgres-backup-cronjob.yaml"

PG_IMAGE="postgres:18"
AWS_IMAGE="amazon/aws-cli:2.38.5"
S3MOCK_IMAGE="adobe/s3mock:4.7.0"

ID="pgbackup-test-$$"
NET="$ID"
ECON="$ID-econ"
KC="$ID-keycloak"
S3="$ID-s3"
ECON_PW="econ-pw-$RANDOM$RANDOM"
KC_PW="keycloak-pw-$RANDOM$RANDOM"

tmp="$(mktemp -d)"
LOG="$tmp/all-output.log"
: >"$LOG"

cleanup() {
    docker rm -f "$ECON" "$KC" "$S3" >/dev/null 2>&1 || true
    docker network rm "$NET" >/dev/null 2>&1 || true
    rm -rf "$tmp" 2>/dev/null || true
}
trap cleanup EXIT

failures=0
pass() { echo "ok - $*"; }
fail() { echo "FAIL - $*"; failures=$((failures + 1)); }
check() { # check DESCRIPTION COMMAND...
    local desc="$1"
    shift
    if "$@"; then pass "$desc"; else fail "$desc"; fi
}

# --- scripts and environment from the manifests ---------------------------

mkdir -p "$tmp/scripts" "$tmp/backups" "$tmp/aws"
python3 - "$CONFIGMAP" "$tmp/scripts" <<'PY'
import os, sys, yaml
for key, text in yaml.safe_load(open(sys.argv[1]))["data"].items():
    path = os.path.join(sys.argv[2], key)
    with open(path, "w") as f:
        f.write(text)
    os.chmod(path, 0o555)
PY

# KEY=VALUE lines for docker --env-file, from the pg-dump init container.
python3 - "$CRONJOB" "$ECON_PW" "$KC_PW" >"$tmp/dump.env" <<'PY'
import sys, yaml
doc = yaml.safe_load(open(sys.argv[1]))
secrets = {
    ("econ-graph-postgres", "postgres-password"): sys.argv[2],
    ("econ-graph-keycloak", "db-password"): sys.argv[3],
}
spec = doc["spec"]["jobTemplate"]["spec"]["template"]["spec"]
(container,) = [c for c in spec["initContainers"] if c["name"] == "pg-dump"]
for env in container["env"]:
    if "value" in env:
        print(f"{env['name']}={env['value']}")
    else:
        ref = env["valueFrom"]["secretKeyRef"]
        print(f"{env['name']}={secrets[(ref['name'], ref['key'])]}")
PY

# The S3 mock wants path-style requests; the bucket host name wouldn't resolve.
printf '[default]\ns3 =\n    addressing_style = path\n' >"$tmp/aws/config"

# Owned by the containers' uid 999 (or its rootless mapping) as on the PVC.
chmod 0777 "$tmp/backups"

# --- scratch servers ---------------------------------------------------------

docker network create "$NET" >/dev/null
docker run -d --name "$ECON" --network "$NET" --network-alias postgres-service \
    -e POSTGRES_PASSWORD="$ECON_PW" -e POSTGRES_DB=econ_graph "$PG_IMAGE" >/dev/null
docker run -d --name "$KC" --network "$NET" --network-alias keycloak-postgres-service \
    -e POSTGRES_USER=keycloak -e POSTGRES_PASSWORD="$KC_PW" -e POSTGRES_DB=keycloak "$PG_IMAGE" >/dev/null
docker run -d --name "$S3" --network "$NET" --network-alias s3mock \
    -e initialBuckets=backups "$S3MOCK_IMAGE" >/dev/null

wait_pg() { # wait_pg CONTAINER USER DB
    for _ in $(seq 1 60); do
        if docker exec "$1" pg_isready -h 127.0.0.1 -U "$2" -d "$3" >/dev/null 2>&1; then
            return 0
        fi
        sleep 1
    done
    echo "$1 did not become ready" >&2
    docker logs "$1" >&2
    return 1
}
wait_pg "$ECON" postgres econ_graph
wait_pg "$KC" keycloak keycloak

psql_in() { # psql_in CONTAINER USER DB SQL
    docker exec -i -e PGOPTIONS="-c client_min_messages=warning" "$1" psql -X -q -v ON_ERROR_STOP=1 -U "$2" -d "$3" -At -c "$4"
}

psql_in "$ECON" postgres econ_graph "
CREATE TABLE economic_series (id serial PRIMARY KEY, title text NOT NULL);
CREATE TABLE data_points (id bigserial PRIMARY KEY, series_id int REFERENCES economic_series(id), value numeric);
CREATE SCHEMA audit;
CREATE TABLE audit.events (id serial PRIMARY KEY, note text);
INSERT INTO economic_series (title) SELECT 'series ' || g FROM generate_series(1, 25) g;
INSERT INTO data_points (series_id, value) SELECT 1 + g % 25, g * 1.5 FROM generate_series(1, 5000) g;
INSERT INTO audit.events (note) VALUES ('a'), ('b'), ('c');"

psql_in "$KC" keycloak keycloak "
CREATE TABLE user_entity (id text PRIMARY KEY, username text NOT NULL, email text);
CREATE TABLE federated_identity (identity_provider text, user_id text REFERENCES user_entity(id), federated_user_id text);
CREATE TABLE credential (id text PRIMARY KEY, user_id text REFERENCES user_entity(id), secret_data text);
INSERT INTO user_entity SELECT 'u' || g, 'user' || g, 'user' || g || '@example.com' FROM generate_series(1, 40) g;
INSERT INTO federated_identity SELECT 'google', 'u' || g, 'g-' || g FROM generate_series(1, 30) g;
INSERT INTO credential SELECT 'c' || g, 'u' || g, 'hash' || g FROM generate_series(1, 12) g;"

# Exact row count of every table: "schema.table<TAB>count" lines.
COUNTS_SQL="SELECT table_schema || '.' || table_name || chr(9) ||
  (xpath('/row/c/text()', query_to_xml(format('SELECT count(*) AS c FROM %I.%I', table_schema, table_name), false, true, '')))[1]::text
  FROM information_schema.tables
  WHERE table_type = 'BASE TABLE' AND table_schema NOT IN ('pg_catalog', 'information_schema')
  ORDER BY 1"
counts() { psql_in "$1" "$2" "$3" "$COUNTS_SQL"; }

counts "$ECON" postgres econ_graph >"$tmp/econ.counts"
counts "$KC" keycloak keycloak >"$tmp/kc.counts"

# --- running the CronJob's containers -----------------------------------------

# A fresh status emptyDir for each run, like each Job pod gets.
new_status() {
    rm -rf "$tmp/status"
    mkdir "$tmp/status"
    chmod 0777 "$tmp/status"
}

# run_backup [docker run options...]: backup.sh as the pg-dump init container
# runs it (uid 999, read-only root, no capabilities). Prints its exit status.
run_backup() {
    local rc=0
    docker run --rm --network "$NET" --user 999:999 --read-only --cap-drop ALL \
        --security-opt no-new-privileges --env-file "$tmp/dump.env" "$@" \
        -v "$tmp/scripts:/scripts:ro" -v "$tmp/backups:/backups" -v "$tmp/status:/status" \
        "$PG_IMAGE" /scripts/backup.sh >>"$LOG" 2>&1 || rc=$?
    echo "$rc"
}

# run_sync PREFIX: sync.sh as the backup-sync container runs it, to the mock.
run_sync() {
    local rc=0
    docker run --rm --network "$NET" --user 999:999 --read-only --cap-drop ALL \
        --security-opt no-new-privileges --entrypoint /scripts/sync.sh \
        -e BACKUP_DIR=/backups -e BACKUP_DESTINATION=s3 -e BACKUP_S3_BUCKET=backups \
        -e BACKUP_S3_PREFIX="$1" -e BACKUP_S3_ENDPOINT=http://s3mock:9090 \
        -e BACKUP_STATUS_FILE=/status/failed \
        -e AWS_ACCESS_KEY_ID=test -e AWS_SECRET_ACCESS_KEY=test -e AWS_DEFAULT_REGION=us-east-1 \
        -e AWS_CONFIG_FILE=/aws/config \
        -v "$tmp/scripts:/scripts:ro" -v "$tmp/backups:/backups:ro" -v "$tmp/status:/status:ro" \
        -v "$tmp/aws:/aws:ro" \
        "$AWS_IMAGE" >>"$LOG" 2>&1 || rc=$?
    echo "$rc"
}

s3_keys() { # s3_keys PREFIX: object keys under the prefix, one per line
    docker run --rm --network "$NET" -e AWS_ACCESS_KEY_ID=test -e AWS_SECRET_ACCESS_KEY=test \
        -e AWS_DEFAULT_REGION=us-east-1 -e AWS_CONFIG_FILE=/aws/config -v "$tmp/aws:/aws:ro" \
        "$AWS_IMAGE" s3 ls --recursive --endpoint-url http://s3mock:9090 "s3://backups/$1/" |
        awk '{ print $4 }' | sed "s|^$1/||" | sort
}

# Dump files (not the latest*.dump links), and the newest dump of one database.
dumps() { find "$tmp/backups" -maxdepth 1 -type f -name '*-2*.dump' -printf '%f\n' | sort; }
newest() { find "$tmp/backups" -maxdepth 1 -type f -name "$1-2*.dump" -printf '%f\n' | sort | tail -n 1; }
partials() { (cd "$tmp/backups" && ls -1 -- *.partial 2>/dev/null) || true; }
status() { cat "$tmp/status/failed" 2>/dev/null || echo "<missing>"; }
link_target() { basename "$(readlink "$tmp/backups/$1")"; }

# Every dump earlier runs wrote is still there.
SEEN="$tmp/seen"
: >"$SEEN"
nothing_deleted() {
    local now="$tmp/now"
    dumps >"$now"
    [ -z "$(comm -23 "$SEEN" "$now")" ] || return 1
    cp "$now" "$SEEN"
}

# Restore a dump into a new database on SERVER and compare every table's row
# count with EXPECTED. Keycloak's dump goes through stdin, as in the runbook.
restore_matches() { # restore_matches CONTAINER USER DUMP EXPECTED SCRATCH_DB
    psql_in "$1" "$2" postgres "DROP DATABASE IF EXISTS $5" &&
        psql_in "$1" "$2" postgres "CREATE DATABASE $5" &&
        docker exec -i "$1" pg_restore -U "$2" -d "$5" --no-owner --no-privileges \
            --exit-on-error --single-transaction <"$tmp/backups/$3" &&
        diff <(counts "$1" "$2" "$5") "$4"
}

for _ in $(seq 1 60); do
    if s3_keys ready >/dev/null 2>&1; then break; fi
    sleep 1
done

# --- 1. both databases up -------------------------------------------------------

echo "# both databases up"
new_status
rc="$(run_backup)"
check "backup.sh exits 0" [ "$rc" = 0 ]
check "no dump recorded as failed" [ "$(status)" = "" ]
econ_dump="$(newest econ_graph)"
kc_dump="$(newest keycloak)"
check "econ_graph dump written ($econ_dump)" [ -n "$econ_dump" ]
check "keycloak dump written ($kc_dump)" [ -n "$kc_dump" ]
check "both dumps carry the run's one timestamp" [ "${econ_dump#econ_graph-}" = "${kc_dump#keycloak-}" ]
check "latest.dump -> econ_graph dump" [ "$(link_target latest.dump)" = "$econ_dump" ]
check "latest-keycloak.dump -> keycloak dump" [ "$(link_target latest-keycloak.dump)" = "$kc_dump" ]
check "no .partial left behind" [ -z "$(partials)" ]
check "econ_graph dump restores with equal row counts" \
    restore_matches "$ECON" postgres "$econ_dump" "$tmp/econ.counts" econ_restore
check "keycloak dump restores with equal row counts" \
    restore_matches "$KC" keycloak "$kc_dump" "$tmp/kc.counts" keycloak_restore
rc="$(run_sync run1)"
check "sync.sh exits 0" [ "$rc" = 0 ]
check "S3 holds exactly the two dumps, no latest links" \
    [ "$(s3_keys run1 | tr '\n' ' ')" = "$econ_dump $kc_dump " ]
nothing_deleted

# --- 2. restore.sh ------------------------------------------------------------

echo "# restore.sh"
psql_in "$ECON" postgres econ_graph "DELETE FROM data_points WHERE id > 100; DELETE FROM audit.events;"
rc=0
docker run --rm --network "$NET" --user 999:999 --read-only --cap-drop ALL \
    -e PGHOST=postgres-service -e PGPORT=5432 -e PGUSER=postgres -e PGPASSWORD="$ECON_PW" \
    -e PGDATABASE=econ_graph -e BACKUP_DIR=/backups \
    -v "$tmp/scripts:/scripts:ro" -v "$tmp/backups:/backups:ro" \
    "$PG_IMAGE" /scripts/restore.sh >>"$LOG" 2>&1 || rc=$?
check "restore.sh of latest.dump exits 0" [ "$rc" = 0 ]
check "econ_graph row counts are back" diff <(counts "$ECON" postgres econ_graph) "$tmp/econ.counts"
rc=0
docker run --rm --network "$NET" --user 999:999 --read-only --cap-drop ALL \
    -e PGHOST=postgres-service -e PGPORT=5432 -e PGUSER=postgres -e PGPASSWORD="$ECON_PW" \
    -e PGDATABASE=econ_graph -e BACKUP_DIR=/backups -e BACKUP_FILE="/backups/$kc_dump" \
    -v "$tmp/scripts:/scripts:ro" -v "$tmp/backups:/backups:ro" \
    "$PG_IMAGE" /scripts/restore.sh >"$tmp/out" 2>&1 || rc=$?
cat "$tmp/out" >>"$LOG"
check "restore.sh refuses a keycloak dump for econ_graph" [ "$rc" = 1 ]
check "  and says why" grep -q "is not a econ_graph dump" "$tmp/out"
check "  and econ_graph is untouched" \
    [ "$(psql_in "$ECON" postgres econ_graph "SELECT count(*) FROM information_schema.tables WHERE table_name = 'user_entity'")" = 0 ]

# --- 3. Keycloak's server down ----------------------------------------------------

echo "# keycloak-postgres down"
sleep 1
docker stop "$KC" >/dev/null
new_status
rc="$(run_backup)"
check "backup.sh exits 0 with a status file" [ "$rc" = 0 ]
check "keycloak recorded as failed" [ "$(status)" = "keycloak" ]
new_econ="$(newest econ_graph)"
check "a new econ_graph dump is kept" [ "$new_econ" != "$econ_dump" ]
check "no new keycloak dump" [ "$(newest keycloak)" = "$kc_dump" ]
check "latest-keycloak.dump still -> the last good keycloak dump" \
    [ "$(link_target latest-keycloak.dump)" = "$kc_dump" ]
check "no .partial left behind" [ -z "$(partials)" ]
rc="$(run_sync run3)"
check "sync.sh fails the Job" [ "$rc" = 1 ]
check "  after syncing the new econ_graph dump" grep -qx "$new_econ" <(s3_keys run3)
check "  and says which dump failed" grep -q "Backup FAILED for: keycloak" "$LOG"
check "the new econ_graph dump restores with equal row counts" \
    restore_matches "$ECON" postgres "$new_econ" "$tmp/econ.counts" econ_restore
check "no dump deleted" nothing_deleted

sleep 1
rc=0
docker run --rm --network "$NET" --user 999:999 --read-only --cap-drop ALL \
    --env-file "$tmp/dump.env" -e BACKUP_STATUS_FILE= \
    -v "$tmp/scripts:/scripts:ro" -v "$tmp/backups:/backups" \
    "$PG_IMAGE" /scripts/backup.sh >>"$LOG" 2>&1 || rc=$?
check "without BACKUP_STATUS_FILE, backup.sh itself exits 1" [ "$rc" = 1 ]
check "  and still keeps the econ_graph dump" [ "$(newest econ_graph)" != "$new_econ" ]
check "no dump deleted" nothing_deleted
docker start "$KC" >/dev/null
wait_pg "$KC" keycloak keycloak

# --- 4. econ_graph's server down ---------------------------------------------------

echo "# postgres-service down"
sleep 1
docker stop "$ECON" >/dev/null
new_status
rc="$(run_backup)"
check "backup.sh exits 0 with a status file" [ "$rc" = 0 ]
check "econ_graph recorded as failed" [ "$(status)" = "econ_graph" ]
new_kc="$(newest keycloak)"
check "a new keycloak dump is kept" [ "$new_kc" != "$kc_dump" ]
check "the new keycloak dump restores with equal row counts" \
    restore_matches "$KC" keycloak "$new_kc" "$tmp/kc.counts" keycloak_restore
rc="$(run_sync run4)"
check "sync.sh fails the Job" [ "$rc" = 1 ]
check "  after syncing the new keycloak dump" grep -qx "$new_kc" <(s3_keys run4)
check "no dump deleted" nothing_deleted
docker start "$ECON" >/dev/null
wait_pg "$ECON" postgres econ_graph

# --- 5. wrong Keycloak password, and a database without tables ---------------------

echo "# keycloak dump rejected"
sleep 1
new_status
rc="$(run_backup -e KEYCLOAK_PGPASSWORD=not-the-password)"
check "wrong password: backup.sh exits 0 with a status file" [ "$rc" = 0 ]
check "wrong password: keycloak recorded as failed" [ "$(status)" = "keycloak" ]
check "no dump deleted" nothing_deleted
sleep 1
new_status
rc="$(run_backup -e KEYCLOAK_PGDATABASE=postgres)"
check "database with no tables: keycloak recorded as failed" [ "$(status)" = "keycloak" ]
check "  as holding no table" grep -q "\[keycloak\] the dump holds no table" "$LOG"
check "  and no keycloak dump of it kept" [ "$(newest keycloak)" = "$new_kc" ]
check "no .partial left behind" [ -z "$(partials)" ]
check "no dump deleted" nothing_deleted

# --- 6. Keycloak not deployed --------------------------------------------------------

echo "# no Keycloak Secret"
sleep 1
new_status
grep -v '^KEYCLOAK_PGPASSWORD=' "$tmp/dump.env" >"$tmp/dump-no-kc.env"
rc=0
docker run --rm --network "$NET" --user 999:999 --read-only --cap-drop ALL \
    --env-file "$tmp/dump-no-kc.env" \
    -v "$tmp/scripts:/scripts:ro" -v "$tmp/backups:/backups" -v "$tmp/status:/status" \
    "$PG_IMAGE" /scripts/backup.sh >"$tmp/out" 2>&1 || rc=$?
cat "$tmp/out" >>"$LOG"
check "backup.sh exits 0" [ "$rc" = 0 ]
check "nothing recorded as failed" [ "$(status)" = "" ]
check "says Keycloak is skipped" grep -q "\[keycloak\] KEYCLOAK_PGPASSWORD is not set" "$tmp/out"
check "keycloak dump unchanged" [ "$(newest keycloak)" = "$new_kc" ]
rc="$(run_sync run6)"
check "sync.sh exits 0" [ "$rc" = 0 ]
check "no dump deleted" nothing_deleted

# --- 7. sync.sh without a status from backup.sh ---------------------------------------

new_status
rc="$(run_sync run7)"
check "sync.sh fails when backup.sh wrote no status" [ "$rc" = 1 ]

# --- credentials ---------------------------------------------------------------------

check "no output shows the econ_graph password" bash -c "! grep -qF -- '$ECON_PW' '$LOG'"
check "no output shows the keycloak password" bash -c "! grep -qF -- '$KC_PW' '$LOG'"

echo
if [ "$failures" -ne 0 ]; then
    echo "$failures check(s) failed; output of the scripts:"
    cat "$LOG"
    exit 1
fi
echo "All checks passed."
