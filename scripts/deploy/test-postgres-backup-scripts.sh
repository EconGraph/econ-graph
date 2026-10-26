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
# econ_graph is built as in production: the server runs the init script from
# k8s/manifests/postgres-init.yaml (the econgraph role, which owns the public
# schema, and the extensions), pgcrypto is created as the superuser, and the
# backend's migrations then run as econgraph: every extension is owned by the
# superuser and every other object by econgraph, as in production.
#
# Checks:
#   - both dumps are written, read back, restore into scratch databases with
#     every table's row count equal to the source, and are synced to S3
#     without the latest*.dump links;
#   - restore.sh puts econ_graph back, both over the existing database and on
#     a newly initialised server, with every object owned by econgraph, the
#     extensions in place, and econgraph able to read and write every table;
#     it refuses a Keycloak dump, a restore that fails at the last drop rolls
#     every table back, and latest.dump moving to another dump mid-restore
#     doesn't change which dump is restored;
#   - when either database's dump fails (server down, wrong password, no
#     tables) the other dump is still written and synced, and sync.sh fails
#     the Job; without BACKUP_STATUS_FILE, backup.sh itself exits 1;
#   - without the Keycloak password, Keycloak is a failed dump when
#     KEYCLOAK_REQUIRED=true (the manifest's value) and skipped when it is
#     false, with econ_graph backed up either way;
#   - a table locked past PG_DUMP_LOCK_WAIT_TIMEOUT fails that dump promptly
#     and keeps the other;
#   - the Job has a deadline and does not retry a pod whose container failed;
#   - no run deletes a dump, and no output shows a password.
#
# Needs docker (DOCKER_HOST is honoured) and python3 with PyYAML.
# Usage: scripts/deploy/test-postgres-backup-scripts.sh

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CONFIGMAP="$ROOT/k8s/manifests/postgres-backup-configmap.yaml"
CRONJOB="$ROOT/k8s/manifests/postgres-backup-cronjob.yaml"
PG_INIT="$ROOT/k8s/manifests/postgres-init.yaml"
MIGRATIONS="$ROOT/backend/migrations"

PG_IMAGE="postgres:18"
AWS_IMAGE="amazon/aws-cli:2.38.5"
S3MOCK_IMAGE="adobe/s3mock:4.7.0"

ID="pgbackup-test-$$"
NET="$ID"
ECON="$ID-econ"
FRESH="$ID-fresh"
KC="$ID-keycloak"
S3="$ID-s3"
ECON_PW="econ-pw-$RANDOM$RANDOM"
APP_PW="app-pw-$RANDOM$RANDOM"
KC_PW="keycloak-pw-$RANDOM$RANDOM"

tmp="$(mktemp -d)"
LOG="$tmp/all-output.log"
: >"$LOG"

cleanup() {
    docker rm -f "$ECON" "$FRESH" "$KC" "$S3" >/dev/null 2>&1 || true
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

mkdir -p "$tmp/scripts" "$tmp/backups" "$tmp/aws" "$tmp/initdb"
# A ConfigMap's scripts, as its volume mounts them.
extract_configmap() { # extract_configmap MANIFEST DIR
    python3 - "$1" "$2" <<'PY'
import os, sys, yaml
for key, text in yaml.safe_load(open(sys.argv[1]))["data"].items():
    path = os.path.join(sys.argv[2], key)
    with open(path, "w") as f:
        f.write(text)
    os.chmod(path, 0o555)
PY
}
extract_configmap "$CONFIGMAP" "$tmp/scripts"
extract_configmap "$PG_INIT" "$tmp/initdb"

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
# start_econ CONTAINER ALIAS: an econ_graph server as postgres-deployment.yaml
# runs it, with the init script that creates the econgraph role.
start_econ() {
    docker run -d --name "$1" --network "$NET" --network-alias "$2" \
        -e POSTGRES_PASSWORD="$ECON_PW" -e POSTGRES_DB=econ_graph -e APP_DB_PASSWORD="$APP_PW" \
        -v "$tmp/initdb:/docker-entrypoint-initdb.d:ro" "$PG_IMAGE" >/dev/null
}
start_econ "$ECON" postgres-service
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

# app_sql HOST [psql options...]: run the SQL on stdin as econgraph with its
# password, from another container over the network, as the backend and
# crawler connect.
app_sql() {
    local host="$1"
    shift
    docker run --rm -i --network "$NET" -e PGPASSWORD="$APP_PW" \
        -e PGOPTIONS="-c client_min_messages=warning" "$PG_IMAGE" \
        psql -X -q -v ON_ERROR_STOP=1 -h "$host" -U econgraph -d econ_graph -At "$@"
}

# In production every extension, pgcrypto included, is owned by the
# superuser; the baseline migration's CREATE EXTENSION IF NOT EXISTS pgcrypto
# then does nothing.
psql_in "$ECON" postgres econ_graph "CREATE EXTENSION IF NOT EXISTS pgcrypto"

# The backend runs the migrations as econgraph at startup, each in a
# transaction, and records them in Diesel's table.
for migration in "$MIGRATIONS"/*/up.sql; do
    app_sql postgres-service --single-transaction <"$migration" >/dev/null 2>"$tmp/out" || {
        echo "migration $migration failed:" >&2
        cat "$tmp/out" >&2
        exit 1
    }
done
{
    echo "CREATE TABLE __diesel_schema_migrations (version VARCHAR(50) PRIMARY KEY NOT NULL,
          run_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP);"
    for migration in "$MIGRATIONS"/*/; do
        version="$(basename "$migration" | cut -d_ -f1 | tr -d -)"
        echo "INSERT INTO __diesel_schema_migrations (version) VALUES ('$version');"
    done
} | app_sql postgres-service

app_sql postgres-service <<'SQL'
CREATE TABLE backup_test_series (id serial PRIMARY KEY, title text NOT NULL);
CREATE TABLE backup_test_points (id bigserial PRIMARY KEY, series_id int REFERENCES backup_test_series(id), value numeric);
CREATE SCHEMA audit;
CREATE TABLE audit.events (id serial PRIMARY KEY, note text);
INSERT INTO backup_test_series (title) SELECT 'series ' || g FROM generate_series(1, 25) g;
INSERT INTO backup_test_points (series_id, value) SELECT 1 + g % 25, g * 1.5 FROM generate_series(1, 5000) g;
INSERT INTO audit.events (note) VALUES ('a'), ('b'), ('c');
SQL

# Objects in econ_graph's own schemas not owned by econgraph, one per line:
# relations (tables, sequences, views, indexes), functions, types (not the
# array types, which follow their element) and schemas. Extensions' members
# don't count: the init script creates most extensions as the superuser.
FOREIGN_OWNED_SQL="SELECT kind || ' ' || name || ' ' || owner FROM (
    SELECT 'relation' AS kind, c.oid::regclass::text AS name, c.relowner::regrole::text AS owner,
           c.oid, 'pg_class'::regclass AS cls, c.relnamespace AS nsp
      FROM pg_class c
    UNION ALL
    SELECT 'function', p.oid::regprocedure::text, p.proowner::regrole::text, p.oid, 'pg_proc'::regclass, p.pronamespace
      FROM pg_proc p
    UNION ALL
    SELECT 'type', t.oid::regtype::text, t.typowner::regrole::text, t.oid, 'pg_type'::regclass, t.typnamespace
      FROM pg_type t
     WHERE NOT EXISTS (SELECT 1 FROM pg_type e WHERE e.typarray = t.oid)
    UNION ALL
    SELECT 'schema', n.nspname::text, n.nspowner::regrole::text, n.oid, 'pg_namespace'::regclass, n.oid
      FROM pg_namespace n
  ) o
  JOIN pg_namespace n ON n.oid = o.nsp
 WHERE n.nspname NOT IN ('pg_catalog', 'information_schema') AND n.nspname NOT LIKE 'pg\\_%'
   AND o.owner <> 'econgraph'
   AND NOT EXISTS (SELECT 1 FROM pg_depend d WHERE d.classid = o.cls AND d.objid = o.oid AND d.deptype = 'e')
 ORDER BY 1"
TABLE_OWNERS_SQL="SELECT tableowner || ' ' || count(*) FROM pg_tables
 WHERE schemaname NOT IN ('pg_catalog', 'information_schema') GROUP BY tableowner ORDER BY 1"
EXTENSIONS_SQL="SELECT string_agg(extname || ':' || extowner::regrole::text, ' ' ORDER BY extname) FROM pg_extension"
PRODUCTION_EXTENSIONS="pg_stat_statements:postgres pg_trgm:postgres pgcrypto:postgres plpgsql:postgres uuid-ossp:postgres"
# What the app does: read every table, write rows (sequences, defaults,
# triggers and the functions they call), record a migration.
APP_ACCESS_SQL="DO \$\$
DECLARE r record;
BEGIN
  FOR r IN SELECT schemaname, tablename FROM pg_tables
            WHERE schemaname NOT IN ('pg_catalog', 'information_schema') LOOP
    EXECUTE format('SELECT 1 FROM %I.%I LIMIT 1', r.schemaname, r.tablename);
  END LOOP;
END \$\$;
BEGIN;
INSERT INTO data_sources (name, base_url) VALUES ('backup test source', 'https://example.com');
UPDATE data_sources SET description = 'updated by the app role' WHERE name = 'backup test source';
INSERT INTO backup_test_series (title) VALUES ('written by the app role');
INSERT INTO audit.events (note) VALUES ('written by the app role');
INSERT INTO __diesel_schema_migrations (version) VALUES ('99999999999999');
ROLLBACK;
SELECT 'app access ok';"

# The server is set up like production: all of it owned by econgraph.
psql_in "$ECON" postgres econ_graph "$FOREIGN_OWNED_SQL" >"$tmp/foreign-owned"
if [ -s "$tmp/foreign-owned" ]; then
    echo "the scratch econ_graph has objects not owned by econgraph:" >&2
    cat "$tmp/foreign-owned" >&2
    exit 1
fi
psql_in "$ECON" postgres econ_graph "$TABLE_OWNERS_SQL" >"$tmp/econ.owners"
psql_in "$ECON" postgres econ_graph "$EXTENSIONS_SQL" >"$tmp/econ.extensions"
if [ "$(cat "$tmp/econ.extensions")" != "$PRODUCTION_EXTENSIONS" ]; then
    echo "the scratch econ_graph's extensions ($(cat "$tmp/econ.extensions")) differ from production's ($PRODUCTION_EXTENSIONS)" >&2
    exit 1
fi
echo "# scratch econ_graph tables by owner: $(tr '\n' ' ' <"$tmp/econ.owners"); extensions: $(cat "$tmp/econ.extensions")"

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

# The mock is up once it serves the backups bucket. (Listing an empty prefix
# would not do: `aws s3 ls` exits 1 when nothing matches.)
s3_ready=false
for _ in $(seq 1 60); do
    if docker run --rm --network "$NET" -e AWS_ACCESS_KEY_ID=test -e AWS_SECRET_ACCESS_KEY=test \
        -e AWS_DEFAULT_REGION=us-east-1 -e AWS_CONFIG_FILE=/aws/config -v "$tmp/aws:/aws:ro" \
        "$AWS_IMAGE" s3api head-bucket --bucket backups --endpoint-url http://s3mock:9090 \
        >>"$LOG" 2>&1; then
        s3_ready=true
        break
    fi
    sleep 1
done
if ! $s3_ready; then
    echo "the S3 mock never served bucket backups; output so far:" >&2
    cat "$LOG" >&2
    exit 1
fi

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
check "no dump deleted" nothing_deleted

# --- 2. restore.sh ------------------------------------------------------------

echo "# restore.sh"

# KEY=VALUE lines for docker --env-file, from the Job restore-postgres-backup.sh
# applies (its shell variables blanked), with the test password for the Secret.
python3 - "$ROOT/scripts/deploy/restore-postgres-backup.sh" "$ECON_PW" >"$tmp/restore.env" <<'PY'
import re, sys, yaml
text = open(sys.argv[1]).read()
manifest = re.search(r"<<EOF \| kc apply -f -\n(.*?)\nEOF\n", text, re.S).group(1)
manifest = re.sub(r"\$\{[A-Z_]+\}", "", manifest)
(container,) = yaml.safe_load(manifest)["spec"]["template"]["spec"]["containers"]
for env in container["env"]:
    if "value" in env:
        print(f"{env['name']}={env['value']}")
    else:
        ref = env["valueFrom"]["secretKeyRef"]
        assert (ref["name"], ref["key"]) == ("econ-graph-postgres", "postgres-password"), ref
        print(f"{env['name']}={sys.argv[2]}")
PY

# run_restore [docker run options...]: restore.sh as the restore Job runs it
# (uid 999, read-only root, the backup volume read-only). Its output goes to
# $tmp/out; prints its exit status.
run_restore() {
    local rc=0
    docker run --rm --network "$NET" --user 999:999 --read-only --cap-drop ALL \
        --security-opt no-new-privileges --env-file "$tmp/restore.env" "$@" \
        -v "$tmp/scripts:/scripts:ro" -v "$tmp/backups:/backups:ro" \
        "$PG_IMAGE" /scripts/restore.sh >"$tmp/out" 2>&1 || rc=$?
    cat "$tmp/out" >>"$LOG"
    echo "$rc"
}

# restored_like_production CONTAINER HOST LABEL: econ_graph on that server
# holds the dump's rows, owned by econgraph, which the app can use.
restored_like_production() {
    check "$3: row counts are back" diff <(counts "$1" postgres econ_graph) "$tmp/econ.counts"
    check "$3: every table is owned by econgraph ($(tr '\n' ' ' <"$tmp/econ.owners"))" \
        diff <(psql_in "$1" postgres econ_graph "$TABLE_OWNERS_SQL") "$tmp/econ.owners"
    psql_in "$1" postgres econ_graph "$FOREIGN_OWNED_SQL" >"$tmp/foreign-owned"
    check "$3: so is every sequence, view, index, function, type and schema" [ ! -s "$tmp/foreign-owned" ]
    [ ! -s "$tmp/foreign-owned" ] || sed 's/^/    owned by another role: /' "$tmp/foreign-owned"
    check "$3: the extensions are in place, owned by the superuser ($(cat "$tmp/econ.extensions"))" \
        [ "$(psql_in "$1" postgres econ_graph "$EXTENSIONS_SQL")" = "$(cat "$tmp/econ.extensions")" ]
    check "$3: econgraph, connecting as the app does, can read every table and write" \
        [ "$(app_sql "$2" <<<"$APP_ACCESS_SQL" 2>>"$LOG")" = "app access ok" ]
}

psql_in "$ECON" postgres econ_graph "DELETE FROM backup_test_points WHERE id > 100; DELETE FROM audit.events;"
rc="$(run_restore)"
check "restore.sh of latest.dump exits 0" [ "$rc" = 0 ]
restored_like_production "$ECON" postgres-service "over the existing database"

echo "# restore.sh on a newly initialised server (the init script only)"
start_econ "$FRESH" postgres-fresh
wait_pg "$FRESH" postgres econ_graph
rc="$(run_restore -e PGHOST=postgres-fresh)"
check "restore.sh of latest.dump exits 0" [ "$rc" = 0 ]
restored_like_production "$FRESH" postgres-fresh "on a new server"
docker rm -f "$FRESH" >/dev/null

echo "# restore.sh failures"
# An object econgraph can't drop, and one dropped last: the schema audit,
# owned by postgres, with econgraph still able to use it. --clean drops every
# table before it, so this checks that the whole step is one transaction: the
# restore stops at DROP SCHEMA and every table comes back as it was.
psql_in "$ECON" postgres econ_graph "DELETE FROM backup_test_points WHERE id > 100;
  ALTER SCHEMA audit OWNER TO postgres; GRANT USAGE, CREATE ON SCHEMA audit TO econgraph;"
counts "$ECON" postgres econ_graph >"$tmp/before"
rc="$(run_restore)"
check "a schema owned by another role: restore.sh exits non-zero" [ "$rc" != 0 ]
check "  and says why" grep -q "must be owner of schema audit" "$tmp/out"
check "  and rolls back, leaving every table and row as it was" diff <(counts "$ECON" postgres econ_graph) "$tmp/before"
# The grant goes before the owner changes back: after it, the REVOKE would
# take the privileges from the owner itself.
psql_in "$ECON" postgres econ_graph "REVOKE USAGE, CREATE ON SCHEMA audit FROM econgraph; ALTER SCHEMA audit OWNER TO econgraph;"
rc="$(run_restore)"
check "  once the schema is econgraph's again, restore.sh exits 0" [ "$rc" = 0 ]
check "  and the row counts are back" diff <(counts "$ECON" postgres econ_graph) "$tmp/econ.counts"

# A backup finishing during the restore moves latest.dump to a newer dump.
# restore.sh must keep reading the dump it listed: another dump's TOC ids
# name other objects. The swap happens right after pg_restore --list, through
# a wrapper first on PATH. Dump B has an extra table, so its ids differ.
echo "# restore.sh while latest.dump moves to another dump"
mkdir -p "$tmp/swap" "$tmp/wrap"
chmod 0777 "$tmp/swap"
cp "$tmp/backups/$econ_dump" "$tmp/swap/$econ_dump"
app_sql postgres-service <<<"CREATE TABLE aaa_swap_test (id int); INSERT INTO aaa_swap_test VALUES (1);"
docker exec "$ECON" pg_dump -U postgres -d econ_graph --format=custom --no-owner --no-privileges \
    >"$tmp/swap/econ_graph-29990101-000000.dump"
app_sql postgres-service <<<"DROP TABLE aaa_swap_test;"
ln -s "/backups/$econ_dump" "$tmp/swap/latest.dump"
cat >"$tmp/wrap/pg_restore" <<'SH'
#!/bin/sh
/usr/lib/postgresql/18/bin/pg_restore "$@" || exit $?
case " $* " in
  *" --list "*) ln -sfn /backups/econ_graph-29990101-000000.dump /backups/latest.dump ;;
esac
SH
chmod 0555 "$tmp/wrap/pg_restore"
chmod 0755 "$tmp/wrap"
psql_in "$ECON" postgres econ_graph "DELETE FROM backup_test_points WHERE id > 100;"
rc=0
docker run --rm --network "$NET" --user 999:999 --read-only --cap-drop ALL \
    --security-opt no-new-privileges --env-file "$tmp/restore.env" \
    -e PATH=/wrap:/usr/lib/postgresql/18/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
    -v "$tmp/scripts:/scripts:ro" -v "$tmp/wrap:/wrap:ro" -v "$tmp/swap:/backups" \
    "$PG_IMAGE" /scripts/restore.sh >"$tmp/out" 2>&1 || rc=$?
cat "$tmp/out" >>"$LOG"
check "latest.dump moved mid-restore: restore.sh exits 0" [ "$rc" = 0 ]
check "  the link did move" [ "$(basename "$(readlink "$tmp/swap/latest.dump")")" = "econ_graph-29990101-000000.dump" ]
check "  and the listed dump was restored, every row of it" diff <(counts "$ECON" postgres econ_graph) "$tmp/econ.counts"
check "  and the other dump's table was not" \
    [ "$(psql_in "$ECON" postgres econ_graph "SELECT to_regclass('public.aaa_swap_test') IS NULL")" = t ]

rc="$(run_restore -e RESTORE_OWNER=no_such_role)"
check "a missing RESTORE_OWNER: restore.sh exits 1" [ "$rc" = 1 ]
check "  and says why" grep -q "Role no_such_role (RESTORE_OWNER) does not exist" "$tmp/out"

rc="$(run_restore -e BACKUP_FILE="/backups/$kc_dump")"
check "restore.sh refuses a keycloak dump for econ_graph" [ "$rc" = 1 ]
check "  and says why" grep -q "is not a econ_graph dump" "$tmp/out"
check "  and econ_graph is untouched" \
    [ "$(psql_in "$ECON" postgres econ_graph "SELECT count(*) FROM information_schema.tables WHERE table_name = 'user_entity'")" = 0 ]
check "no dump deleted" nothing_deleted

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
check "no output shows the keycloak password" bash -c "! grep -qF -- '$KC_PW' '$LOG'"
check "no output shows the econgraph password" bash -c "! grep -qF -- '$APP_PW' '$LOG'"

echo
if [ "$failures" -ne 0 ]; then
    echo "$failures check(s) failed; output of the scripts:"
    cat "$LOG"
    exit 1
fi
echo "All checks passed."
