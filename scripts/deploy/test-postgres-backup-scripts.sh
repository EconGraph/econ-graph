#!/usr/bin/env bash
#
# Runs the postgres-backup CronJob's scripts (backup.sh, sync.sh and restore.sh
# from k8s/manifests/postgres-backup-configmap.yaml) in the CronJob's images,
# against three scratch postgres:18 containers standing in for
# data-postgres-service (econ_graph, also aliased postgres-service),
# app-postgres-service (econ_graph_app) and keycloak-postgres-service
# (keycloak), with an S3 mock as the off-site destination. The pg-dump
# container's environment is read from
# k8s/manifests/postgres-backup-cronjob.yaml, with test passwords in place of
# the Secrets. Never touches a cluster.
#
# Checks:
#   - all three dumps are written, read back, restore into scratch databases
#     with every table's row count equal to the source, and are synced to S3
#     without the latest*.dump links;
#   - the app dump, restored as the runbook says (--role=econgraph into a
#     database owned by econgraph), leaves econgraph owning its tables and
#     able to write them;
#   - restore.sh puts econ_graph back, and refuses an app or Keycloak dump;
#   - when any database's dump fails (server down, wrong password, no tables)
#     the others are still written and synced, and sync.sh fails the Job;
#     without BACKUP_STATUS_FILE, backup.sh itself exits 1;
#   - without the app or Keycloak password, that database is a failed dump
#     when APP_REQUIRED / KEYCLOAK_REQUIRED is true (the manifest's value) and
#     skipped when it is false, with the others backed up either way;
#   - a table locked past PG_DUMP_LOCK_WAIT_TIMEOUT fails that dump promptly
#     and keeps the others;
#   - the Job has a deadline and does not retry a pod whose container failed;
#   - no run deletes a dump, and no output shows any password.
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
APP="$ID-app"
KC="$ID-keycloak"
S3="$ID-s3"
ECON_PW="econ-pw-$RANDOM$RANDOM"
APP_PW="app-pw-$RANDOM$RANDOM"
APP_ROLE_PW="app-role-pw-$RANDOM$RANDOM"
KC_PW="keycloak-pw-$RANDOM$RANDOM"

tmp="$(mktemp -d)"
LOG="$tmp/all-output.log"
: >"$LOG"

cleanup() {
    docker rm -f "$ECON" "$APP" "$KC" "$S3">/dev/null 2>&1 || true
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
python3 - "$CRONJOB" "$ECON_PW" "$APP_PW" "$KC_PW" >"$tmp/dump.env" <<'PY'
import sys, yaml
doc = yaml.safe_load(open(sys.argv[1]))
secrets = {
    ("econ-graph-postgres", "postgres-password"): sys.argv[2],
    ("econ-graph-app-postgres", "postgres-password"): sys.argv[3],
    ("econ-graph-keycloak", "db-password"): sys.argv[4],
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

# The Job settings that bound a run: a deadline, and no retry after a container
# exits non-zero (each retry would write another full dump).
check "the Job has an activeDeadlineSeconds" python3 - "$CRONJOB" <<'PY'
import sys, yaml
job = yaml.safe_load(open(sys.argv[1]))["spec"]["jobTemplate"]["spec"]
sys.exit(0 if int(job.get("activeDeadlineSeconds", 0)) > 0 else 1)
PY
check "the Job fails at once on any non-zero container exit" python3 - "$CRONJOB" <<'PY'
import sys, yaml
job = yaml.safe_load(open(sys.argv[1]))["spec"]["jobTemplate"]["spec"]
rules = job.get("podFailurePolicy", {}).get("rules", [])
ok = any(r.get("action") == "FailJob"
         and "containerName" not in r.get("onExitCodes", {})
         and r.get("onExitCodes", {}).get("operator") == "NotIn"
         and r["onExitCodes"].get("values") == [0] for r in rules)
sys.exit(0 if ok and job["template"]["spec"]["restartPolicy"] == "Never" else 1)
PY
check "a disrupted pod (eviction, drain, preemption) is retried, not failed" python3 - "$CRONJOB" <<'PY'
import sys, yaml
job = yaml.safe_load(open(sys.argv[1]))["spec"]["jobTemplate"]["spec"]
rules = job.get("podFailurePolicy", {}).get("rules", [])
def disruption(r):
    return r.get("action") in ("Count", "Ignore") and any(
        c.get("type") == "DisruptionTarget" for c in r.get("onPodConditions", []))
first_fail = next((i for i, r in enumerate(rules) if r.get("action") == "FailJob"), len(rules))
sys.exit(0 if any(disruption(r) for r in rules[:first_fail]) else 1)
PY

# The S3 mock wants path-style requests; the bucket host name wouldn't resolve.
printf '[default]\ns3 =\n    addressing_style = path\n' >"$tmp/aws/config"

# Owned by the containers' uid 999 (or its rootless mapping) as on the PVC.
chmod 0777 "$tmp/backups"

# --- scratch servers ---------------------------------------------------------

docker network create "$NET" >/dev/null
# Both names of the data database's Service (k8s/manifests/postgres.yaml).
docker run -d --name "$ECON" --network "$NET" --network-alias data-postgres-service \
    --network-alias postgres-service \
    -e POSTGRES_PASSWORD="$ECON_PW" -e POSTGRES_DB=econ_graph "$PG_IMAGE" >/dev/null
docker run -d --name "$APP" --network "$NET" --network-alias app-postgres-service \
    -e POSTGRES_PASSWORD="$APP_PW" -e POSTGRES_DB=econ_graph_app "$PG_IMAGE" >/dev/null
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
wait_pg "$APP" postgres econ_graph_app
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

# As in the cluster: app-postgres-init.yaml creates the role econgraph and gives
# it the public schema, and the backend's app migrations, run as econgraph,
# create (and so own) the extension and the tables.
psql_in "$APP" postgres econ_graph_app "
CREATE USER econgraph WITH ENCRYPTED PASSWORD '$APP_ROLE_PW';
GRANT ALL PRIVILEGES ON DATABASE econ_graph_app TO econgraph;
ALTER SCHEMA public OWNER TO econgraph;
GRANT ALL ON SCHEMA public TO econgraph;
SET ROLE econgraph;
CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE TABLE users (id uuid PRIMARY KEY DEFAULT gen_random_uuid(), email text NOT NULL);
CREATE TABLE chart_annotations (id serial PRIMARY KEY, user_id uuid REFERENCES users(id), note text);
INSERT INTO users (email) SELECT 'user' || g || '@example.com' FROM generate_series(1, 30) g;
INSERT INTO chart_annotations (user_id, note) SELECT id, 'note' FROM users;"

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
counts "$APP" postgres econ_graph_app >"$tmp/app.counts"
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

# The app database's restore, as docs/deployment/POSTGRES_BACKUP_RESTORE.md
# gives it: a new database owned by econgraph, and pg_restore --role=econgraph,
# so that econgraph (the backend's role) owns what the dump recreates. Then
# econgraph must own every table and be able to write to them.
restore_app_as_documented() { # restore_app_as_documented DUMP SCRATCH_DB
    psql_in "$APP" postgres postgres "DROP DATABASE IF EXISTS $2" &&
        docker exec "$APP" createdb -U postgres -O econgraph "$2" &&
        docker exec -i "$APP" pg_restore -U postgres -d "$2" --role=econgraph \
            --no-owner --no-privileges --exit-on-error --single-transaction \
            <"$tmp/backups/$1" &&
        diff <(counts "$APP" postgres "$2") "$tmp/app.counts" &&
        [ "$(psql_in "$APP" postgres "$2" "SELECT count(*) FROM pg_tables
              WHERE schemaname = 'public' AND tableowner <> 'econgraph'")" = 0 ] &&
        psql_in "$APP" econgraph "$2" "INSERT INTO users (email) VALUES ('new@example.com');
              INSERT INTO chart_annotations (user_id, note)
                SELECT id, 'new' FROM users WHERE email = 'new@example.com'" &&
        psql_in "$APP" postgres postgres "DROP DATABASE $2"
}

for _ in $(seq 1 60); do
    if s3_keys ready >/dev/null 2>&1; then break; fi
    sleep 1
done

# --- 1. both databases up -------------------------------------------------------

echo "# all three databases up"
new_status
rc="$(run_backup)"
check "backup.sh exits 0" [ "$rc" = 0 ]
check "no dump recorded as failed" [ "$(status)" = "" ]
econ_dump="$(newest econ_graph)"
app_dump="$(newest econ_graph_app)"
kc_dump="$(newest keycloak)"
check "econ_graph dump written ($econ_dump)" [ -n "$econ_dump" ]
check "econ_graph_app dump written ($app_dump)" [ -n "$app_dump" ]
check "keycloak dump written ($kc_dump)" [ -n "$kc_dump" ]
check "the dumps carry the run's one timestamp" \
    [ "${econ_dump#econ_graph-} ${app_dump#econ_graph_app-}" = "${kc_dump#keycloak-} ${kc_dump#keycloak-}" ]
check "latest.dump -> econ_graph dump" [ "$(link_target latest.dump)" = "$econ_dump" ]
check "latest-econ_graph_app.dump -> econ_graph_app dump" \
    [ "$(link_target latest-econ_graph_app.dump)" = "$app_dump" ]
check "latest-keycloak.dump -> keycloak dump" [ "$(link_target latest-keycloak.dump)" = "$kc_dump" ]
check "no .partial left behind" [ -z "$(partials)" ]
check "econ_graph dump restores with equal row counts" \
    restore_matches "$ECON" postgres "$econ_dump" "$tmp/econ.counts" econ_restore
check "econ_graph_app dump restores with equal row counts" \
    restore_matches "$APP" postgres "$app_dump" "$tmp/app.counts" app_restore
check "econ_graph_app dump restored as documented: econgraph owns and writes its tables" \
    restore_app_as_documented "$app_dump" app_restore_doc
check "keycloak dump restores with equal row counts" \
    restore_matches "$KC" keycloak "$kc_dump" "$tmp/kc.counts" keycloak_restore
rc="$(run_sync run1)"
check "sync.sh exits 0" [ "$rc" = 0 ]
check "S3 holds exactly the three dumps, no latest links" \
    [ "$(s3_keys run1 | tr '\n' ' ')" = "$(printf '%s\n' "$econ_dump" "$app_dump" "$kc_dump" | sort | tr '\n' ' ')" ]
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
rc=0
docker run --rm --network "$NET" --user 999:999 --read-only --cap-drop ALL \
    -e PGHOST=postgres-service -e PGPORT=5432 -e PGUSER=postgres -e PGPASSWORD="$ECON_PW" \
    -e PGDATABASE=econ_graph -e BACKUP_DIR=/backups -e BACKUP_FILE="/backups/latest-econ_graph_app.dump" \
    -v "$tmp/scripts:/scripts:ro" -v "$tmp/backups:/backups:ro" \
    "$PG_IMAGE" /scripts/restore.sh >"$tmp/out" 2>&1 || rc=$?
cat "$tmp/out" >>"$LOG"
check "restore.sh refuses an econ_graph_app dump (through its latest link) for econ_graph" [ "$rc" = 1 ]
check "  and says why" grep -q "is not a econ_graph dump" "$tmp/out"
check "  and econ_graph is untouched" \
    [ "$(psql_in "$ECON" postgres econ_graph "SELECT count(*) FROM information_schema.tables WHERE table_name = 'chart_annotations'")" = 0 ]

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
check "a new econ_graph_app dump is kept" [ "$(newest econ_graph_app)" != "$app_dump" ]
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

echo "# data-postgres-service down"
sleep 1
docker stop "$ECON" >/dev/null
new_status
before_app="$(newest econ_graph_app)"
rc="$(run_backup)"
check "backup.sh exits 0 with a status file" [ "$rc" = 0 ]
check "econ_graph recorded as failed" [ "$(status)" = "econ_graph" ]
new_kc="$(newest keycloak)"
check "a new econ_graph_app dump is kept" [ "$(newest econ_graph_app)" != "$before_app" ]
check "a new keycloak dump is kept" [ "$new_kc" != "$kc_dump" ]
check "the new keycloak dump restores with equal row counts" \
    restore_matches "$KC" keycloak "$new_kc" "$tmp/kc.counts" keycloak_restore
rc="$(run_sync run4)"
check "sync.sh fails the Job" [ "$rc" = 1 ]
check "  after syncing the new keycloak dump" grep -qx "$new_kc" <(s3_keys run4)
check "no dump deleted" nothing_deleted
docker start "$ECON" >/dev/null
wait_pg "$ECON" postgres econ_graph

# --- 4b. the app database's server down, and its dump rejected ---------------------

echo "# app-postgres-service down"
sleep 1
docker stop "$APP" >/dev/null
new_status
before_app="$(newest econ_graph_app)"
before_econ="$(newest econ_graph)"
before_kc="$(newest keycloak)"
rc="$(run_backup)"
check "backup.sh exits 0 with a status file" [ "$rc" = 0 ]
check "econ_graph_app recorded as failed" [ "$(status)" = "econ_graph_app" ]
check "no new econ_graph_app dump" [ "$(newest econ_graph_app)" = "$before_app" ]
check "latest-econ_graph_app.dump still -> the last good app dump" \
    [ "$(link_target latest-econ_graph_app.dump)" = "$before_app" ]
new_econ="$(newest econ_graph)"
new_kc="$(newest keycloak)"
check "a new econ_graph dump is kept" [ "$new_econ" != "$before_econ" ]
check "a new keycloak dump is kept" [ "$new_kc" != "$before_kc" ]
check "no .partial left behind" [ -z "$(partials)" ]
rc="$(run_sync run4b)"
check "sync.sh fails the Job" [ "$rc" = 1 ]
s3_keys run4b >"$tmp/keys"
check "  after syncing the new econ_graph dump" grep -qx "$new_econ" "$tmp/keys"
check "  and the new keycloak dump" grep -qx "$new_kc" "$tmp/keys"
check "  and says which dump failed" grep -q "Backup FAILED for: econ_graph_app" "$LOG"
check "no dump deleted" nothing_deleted
docker start "$APP" >/dev/null
wait_pg "$APP" postgres econ_graph_app

sleep 1
new_status
before_app="$(newest econ_graph_app)"
rc="$(run_backup -e APP_PGPASSWORD=not-the-password)"
check "wrong app password: econ_graph_app recorded as failed" [ "$(status)" = "econ_graph_app" ]
sleep 1
new_status
rc="$(run_backup -e APP_PGDATABASE=postgres)"
check "app database with no tables: econ_graph_app recorded as failed" [ "$(status)" = "econ_graph_app" ]
check "  as holding no table" grep -q "\[econ_graph_app\] the dump holds no table" "$LOG"
check "  and no app dump of it kept" [ "$(newest econ_graph_app)" = "$before_app" ]
check "no .partial left behind" [ -z "$(partials)" ]
check "no dump deleted" nothing_deleted

# --- 4c. no app database password (Secret or key missing) ------------------------------

echo "# no app database password, APP_REQUIRED=true (the manifest's value)"
check "the manifest sets APP_REQUIRED=true" grep -qx 'APP_REQUIRED=true' "$tmp/dump.env"
# Without the Secret, the optional secretKeyRef leaves the variable unset.
grep -v '^APP_PGPASSWORD=' "$tmp/dump.env" >"$tmp/dump-no-app.env"
sleep 1
new_status
before_econ="$(newest econ_graph)"
before_kc="$(newest keycloak)"
rc=0
docker run --rm --network "$NET" --user 999:999 --read-only --cap-drop ALL \
    --env-file "$tmp/dump-no-app.env" \
    -v "$tmp/scripts:/scripts:ro" -v "$tmp/backups:/backups" -v "$tmp/status:/status" \
    "$PG_IMAGE" /scripts/backup.sh >"$tmp/out" 2>&1 || rc=$?
cat "$tmp/out" >>"$LOG"
check "backup.sh exits 0 with a status file" [ "$rc" = 0 ]
check "econ_graph_app recorded as failed" [ "$(status)" = "econ_graph_app" ]
check "  and says the Secret is missing" grep -q "APP_REQUIRED=true (Secret econ-graph-app-postgres" "$tmp/out"
check "a new econ_graph dump is kept" [ "$(newest econ_graph)" != "$before_econ" ]
check "a new keycloak dump is kept" [ "$(newest keycloak)" != "$before_kc" ]
check "app dump unchanged" [ "$(newest econ_graph_app)" = "$before_app" ]
rc="$(run_sync run4c)"
check "sync.sh fails the Job" [ "$rc" = 1 ]
check "no dump deleted" nothing_deleted

echo "# no app database password, APP_REQUIRED=false (a cluster without the app database)"
sleep 1
new_status
rc=0
docker run --rm --network "$NET" --user 999:999 --read-only --cap-drop ALL \
    --env-file "$tmp/dump-no-app.env" -e APP_REQUIRED=false \
    -v "$tmp/scripts:/scripts:ro" -v "$tmp/backups:/backups" -v "$tmp/status:/status" \
    "$PG_IMAGE" /scripts/backup.sh >"$tmp/out" 2>&1 || rc=$?
cat "$tmp/out" >>"$LOG"
check "backup.sh exits 0" [ "$rc" = 0 ]
check "nothing recorded as failed" [ "$(status)" = "" ]
check "says the app database is skipped" \
    grep -q "\[econ_graph_app\] APP_PGPASSWORD is empty and APP_REQUIRED is not true" "$tmp/out"
check "app dump unchanged" [ "$(newest econ_graph_app)" = "$before_app" ]
rc="$(run_sync run4d)"
check "sync.sh exits 0" [ "$rc" = 0 ]
check "no dump deleted" nothing_deleted
new_kc="$(newest keycloak)"

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

# --- 6. no Keycloak password (Secret or key missing) -----------------------------------

echo "# no Keycloak password, KEYCLOAK_REQUIRED=true (the manifest's value)"
check "the manifest sets KEYCLOAK_REQUIRED=true" grep -qx 'KEYCLOAK_REQUIRED=true' "$tmp/dump.env"
# Without the Secret, the optional secretKeyRef leaves the variable unset.
grep -v '^KEYCLOAK_PGPASSWORD=' "$tmp/dump.env" >"$tmp/dump-no-kc.env"
sleep 1
new_status
before_econ="$(newest econ_graph)"
rc=0
docker run --rm --network "$NET" --user 999:999 --read-only --cap-drop ALL \
    --env-file "$tmp/dump-no-kc.env" \
    -v "$tmp/scripts:/scripts:ro" -v "$tmp/backups:/backups" -v "$tmp/status:/status" \
    "$PG_IMAGE" /scripts/backup.sh >"$tmp/out" 2>&1 || rc=$?
cat "$tmp/out" >>"$LOG"
check "backup.sh exits 0 with a status file" [ "$rc" = 0 ]
check "keycloak recorded as failed" [ "$(status)" = "keycloak" ]
check "  and says the Secret is missing" grep -q "KEYCLOAK_REQUIRED=true (Secret econ-graph-keycloak" "$tmp/out"
check "a new econ_graph dump is kept" [ "$(newest econ_graph)" != "$before_econ" ]
check "keycloak dump unchanged" [ "$(newest keycloak)" = "$new_kc" ]
rc="$(run_sync run6a)"
check "sync.sh fails the Job" [ "$rc" = 1 ]
check "no dump deleted" nothing_deleted

sleep 1
new_status
rc="$(run_backup -e KEYCLOAK_PGPASSWORD=)"
check "empty password with KEYCLOAK_REQUIRED=true: keycloak recorded as failed" [ "$(status)" = "keycloak" ]
check "no dump deleted" nothing_deleted

echo "# no Keycloak password, KEYCLOAK_REQUIRED=false (a cluster without Keycloak)"
sleep 1
new_status
rc=0
docker run --rm --network "$NET" --user 999:999 --read-only --cap-drop ALL \
    --env-file "$tmp/dump-no-kc.env" -e KEYCLOAK_REQUIRED=false \
    -v "$tmp/scripts:/scripts:ro" -v "$tmp/backups:/backups" -v "$tmp/status:/status" \
    "$PG_IMAGE" /scripts/backup.sh >"$tmp/out" 2>&1 || rc=$?
cat "$tmp/out" >>"$LOG"
check "backup.sh exits 0" [ "$rc" = 0 ]
check "nothing recorded as failed" [ "$(status)" = "" ]
check "says Keycloak is skipped" grep -q "\[keycloak\] KEYCLOAK_PGPASSWORD is empty and KEYCLOAK_REQUIRED is not true" "$tmp/out"
check "keycloak dump unchanged" [ "$(newest keycloak)" = "$new_kc" ]
rc="$(run_sync run6b)"
check "sync.sh exits 0" [ "$rc" = 0 ]
check "no dump deleted" nothing_deleted

# --- 6b. a lock held past the lock wait ------------------------------------------------

echo "# econ_graph table locked longer than PG_DUMP_LOCK_WAIT_TIMEOUT"
docker exec -d "$ECON" psql -U postgres -d econ_graph \
    -c "BEGIN; LOCK TABLE audit.events IN ACCESS EXCLUSIVE MODE; SELECT pg_sleep(60); COMMIT;"
for _ in $(seq 1 20); do
    locks="$(psql_in "$ECON" postgres econ_graph \
        "SELECT count(*) FROM pg_locks WHERE mode = 'AccessExclusiveLock' AND relation = 'audit.events'::regclass")"
    [ "$locks" = 1 ] && break
    sleep 0.5
done
sleep 1
new_status
before_econ="$(newest econ_graph)"
before_kc="$(newest keycloak)"
started="$(date +%s)"
rc="$(run_backup -e PG_DUMP_LOCK_WAIT_TIMEOUT=2s)"
took=$(($(date +%s) - started))
check "backup.sh returns well before the lock is released (${took}s)" [ "$took" -lt 40 ]
check "econ_graph recorded as failed" [ "$(status)" = "econ_graph" ]
check "  with pg_dump's lock timeout" grep -q "Query was: LOCK TABLE .* IN ACCESS SHARE MODE" "$LOG"
check "no new econ_graph dump" [ "$(newest econ_graph)" = "$before_econ" ]
check "a new keycloak dump is kept" [ "$(newest keycloak)" != "$before_kc" ]
check "no .partial left behind" [ -z "$(partials)" ]
check "no dump deleted" nothing_deleted
psql_in "$ECON" postgres econ_graph \
    "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE query LIKE '%pg_sleep(60)%' AND pid <> pg_backend_pid()" >/dev/null

# --- 7. sync.sh without a status from backup.sh ---------------------------------------

new_status
rc="$(run_sync run7)"
check "sync.sh fails when backup.sh wrote no status" [ "$rc" = 1 ]

# --- credentials ---------------------------------------------------------------------

check "no output shows the econ_graph password" bash -c "! grep -qF -- '$ECON_PW' '$LOG'"
check "no output shows the econ_graph_app password" bash -c "! grep -qF -- '$APP_PW' '$LOG'"
check "no output shows the keycloak password" bash -c "! grep -qF -- '$KC_PW' '$LOG'"

echo
if [ "$failures" -ne 0 ]; then
    echo "$failures check(s) failed; output of the scripts:"
    cat "$LOG"
    exit 1
fi
echo "All checks passed."
