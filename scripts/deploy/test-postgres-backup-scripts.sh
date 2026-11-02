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
#   - restore.sh, with the restore Job's environment (RESTORE_OWNER=econgraph),
#     puts econ_graph back with econgraph owning everything but the
#     extensions and able to write its tables, and every extension untouched;
#     an existing extension keeps the target's own comment; on a newly
#     initialised server (postgres-init.yaml's script, uuid-ossp dropped, and
#     RESTORE_OWNER unset) it creates and logs the two extensions that server
#     lacks, with the dump's comments, and leaves everything else econgraph's; a restore that fails at
#     its last drop rolls every table back; latest.dump moved to another dump
#     right after the list doesn't change which dump is restored; it refuses
#     a dump with no table, a missing owner role, and an app or Keycloak dump;
#   - when any database's dump fails (server down, wrong password, no tables)
#     the others are still written and synced, and sync.sh fails the Job;
#     without BACKUP_STATUS_FILE, backup.sh itself exits 1;
#   - backup.sh's db_target reads each kind of app-database-url as intended
#     (case, Service suffixes, IP literals, options, unreadable URLs);
#   - the app database is dumped when app-database-url names
#     app-postgres-service:5432/econ_graph_app (APP_DATABASE=separate, also in
#     upper case with the full Service name and options); when it names the
#     database dumped as econ_graph (APP_DATABASE=shared, also through the
#     alias postgres-service) it is skipped with one log line and the run
#     succeeds; when it names any other database (another host, the same host
#     with another database, an IP, a host in the options) or can't be read,
#     the run fails, whatever APP_REQUIRED says; a missing app-database-url
#     is skipped, or fails with APP_REQUIRED=true; in separate mode a missing
#     password or an unreachable server fails it;
#   - without the Keycloak password, Keycloak is a failed dump when
#     KEYCLOAK_REQUIRED=true (the manifest's value) and skipped when it is
#     false, with the others backed up either way;
#   - a table locked past PG_DUMP_LOCK_WAIT_TIMEOUT fails that dump promptly
#     and keeps the others;
#   - the Job has a deadline and does not retry a pod whose container failed;
#   - no run deletes a dump, and a dump removed from the volume by hand stays
#     in S3 after the next sync;
#   - no output shows any password.
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

# Bind-mount fixtures must be owned by the invoking host account after umask
# 077. Rootless Docker maps that account to container root; a rootful daemon
# maps its ordinary uid directly. The separate encryption smoke test still
# exercises the production uid 999 with a read-only root and no capabilities.
TEST_USER="$(id -u):$(id -g)"
case "$(docker info --format '{{json .SecurityOptions}}')" in
    *rootless*) TEST_USER=0:0 ;;
esac

ID="pgbackup-test-$$"
NET="$ID"
ECON="$ID-econ"
APP="$ID-app"
FRESH="$ID-fresh"
KC="$ID-keycloak"
S3="$ID-s3"
ECON_PW="econ-pw-$RANDOM$RANDOM"
APP_PW="app-pw-$RANDOM$RANDOM"
APP_ROLE_PW="app-role-pw-$RANDOM$RANDOM"
DATA_ROLE_PW="data-role-pw-$RANDOM$RANDOM"
KC_PW="keycloak-pw-$RANDOM$RANDOM"
# The backend's URLs (econ-graph-secrets app-database-url and data-database-url,
# as create-secrets.sh writes them: econgraph, the URL-encoded password, host,
# port, database). The app one is APP_DATABASE=separate; SHARED_APP_URL is
# APP_DATABASE=shared. The passwords hold encoded '@' and ':' on purpose.
DATA_URL="postgresql://econgraph:${DATA_ROLE_PW}%40x%3Ay@data-postgres-service:5432/econ_graph"
APP_URL="postgresql://econgraph:${APP_ROLE_PW}%40x%3Ay@app-postgres-service:5432/econ_graph_app"
SHARED_APP_URL="$DATA_URL"

tmp="$(mktemp -d)"
STAGE_ROOT=""
LOG="$tmp/all-output.log"
: >"$LOG"

cleanup() {
    docker rm -f "$ECON" "$APP" "$FRESH" "$KC" "$S3" >/dev/null 2>&1 || true
    docker network rm "$NET" >/dev/null 2>&1 || true
    rm -rf "$tmp" "$STAGE_ROOT" 2>/dev/null || true
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
# Created in the parent: cleanup can remove staged plaintext even when an
# interrupted run_sync is executing inside a command-substitution subshell.
STAGE_ROOT="$(mktemp -d /dev/shm/econgraph-encrypt-stage.XXXXXX)"

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

# Install the same checksum-pinned age release as the Job, then generate only
# a disposable synthetic identity on the test host. No identity enters a pod.
mkdir -p "$tmp/tools" "$tmp/ciphertexts"
# A retained plaintext backup and an unfinished ciphertext must never ship.
printf 'legacy plaintext fixture' >"$tmp/ciphertexts/legacy.dump"
printf 'unfinished ciphertext' >"$tmp/ciphertexts/incomplete.dump.age.partial"
printf 'not an archive' >"$tmp/ciphertexts/other.age"
ln -s legacy.dump "$tmp/ciphertexts/escape.dump.age"
CURL_IMAGE="$(python3 - "$CRONJOB" <<'PYIMAGE'
import sys, yaml
spec = yaml.safe_load(open(sys.argv[1]))['spec']['jobTemplate']['spec']['template']['spec']
print(next(c['image'] for c in spec['initContainers'] if c['name'] == 'install-age'))
PYIMAGE
)"
docker run --rm --user "$TEST_USER" --read-only --cap-drop ALL \
    --security-opt no-new-privileges --entrypoint /scripts/install-age.sh \
    -v "$tmp/scripts:/scripts:ro" -v "$tmp/tools:/tools" "$CURL_IMAGE" >>"$LOG" 2>&1
"$tmp/tools/age/age-keygen" -o "$tmp/test-identity" >>"$LOG" 2>&1
RECIPIENT="$("$tmp/tools/age/age-keygen" -y "$tmp/test-identity")"
AGE_BIN="$tmp/tools/age/age" AGE_KEYGEN_BIN="$tmp/tools/age/age-keygen" \
    BACKUP_ENCRYPT_IMAGE="$CURL_IMAGE" python3 "$ROOT/scripts/deploy/test-postgres-backup-encryption.py"

# KEY=VALUE lines for docker --env-file, from the pg-dump init container.
python3 - "$CRONJOB" "$ECON_PW" "$APP_PW" "$KC_PW" "$APP_URL" "$DATA_URL" >"$tmp/dump.env" <<'PY'
import sys, yaml
doc = yaml.safe_load(open(sys.argv[1]))
secrets = {
    ("econ-graph-postgres", "postgres-password"): sys.argv[2],
    ("econ-graph-app-postgres", "postgres-password"): sys.argv[3],
    ("econ-graph-keycloak", "db-password"): sys.argv[4],
    ("econ-graph-secrets", "app-database-url"): sys.argv[5],
    ("econ-graph-secrets", "data-database-url"): sys.argv[6],
}
spec = doc["spec"]["jobTemplate"]["spec"]["template"]["spec"]
(container,) = [c for c in spec["initContainers"] if c["name"] == "pg-dump"]
for env in container["env"]:
    if "value" in env:
        # Unit-stage fixtures live on the test host, never on a cluster PVC.
        value = "/backups" if env["name"] == "BACKUP_DIR" else env["value"]
        print(f"{env['name']}={value}")
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

# backup.sh's db_target, on its own in a POSIX sh: what it makes of each
# app-database-url ("" is a URL it can't read, which fails the app dump).
awk '/^db_target\(\) \{/,/^}/' "$tmp/scripts/backup.sh" >"$tmp/db_target.sh"
check "db_target found in backup.sh" grep -q '^db_target() {' "$tmp/db_target.sh"
db_target_of() { # db_target_of URL: db_target's output, under sh
    sh -c '. "$1"; db_target "$2"' sh "$tmp/db_target.sh" "$1"
}
while IFS='|' read -r url want; do
    [ -n "$url" ] || continue
    got="$(db_target_of "$url")"
    check "db_target ${url#*@} -> '${want}'" [ "$got" = "$want" ]
done <<'CASES'
postgresql://econgraph:p%40x%3Ay@app-postgres-service:5432/econ_graph_app|app-postgres-service:5432/econ_graph_app
postgres://econgraph:pw@app-postgres-service/econ_graph_app|app-postgres-service:5432/econ_graph_app
postgresql://econgraph:pw@APP-Postgres-Service.econ-graph.svc.cluster.local:5432/econ_graph_app?sslmode=disable|app-postgres-service:5432/econ_graph_app
postgresql://econgraph:pw@app-postgres-service.econ-graph.svc/econ_graph_app|app-postgres-service:5432/econ_graph_app
postgresql://econgraph:pw@app-postgres-service.econ-graph/econ_graph_app|app-postgres-service:5432/econ_graph_app
postgresql://econgraph:pw@postgres-service:5432/econ_graph|data-postgres-service:5432/econ_graph
postgresql://econgraph:pw@data-postgres-service:5432/econ_graph?application_name=a@b|data-postgres-service:5432/econ_graph
postgresql://econgraph:pw@app-postgres-service:5432/other_db|app-postgres-service:5432/other_db
postgresql://econgraph:pw@app-postgres-service:5433/econ_graph_app|app-postgres-service:5433/econ_graph_app
postgresql://econgraph:pw@app-postgres-service.other-ns.svc/econ_graph_app|app-postgres-service.other-ns.svc:5432/econ_graph_app
postgresql://econgraph:pw@10.96.0.5:5432/econ_graph_app|10.96.0.5:5432/econ_graph_app
postgresql://econgraph:pw@[FD00::5]:5432/econ_graph_app|[fd00::5]:5432/econ_graph_app
postgresql://econgraph:pw@[fd00::5]/econ_graph_app|[fd00::5]:5432/econ_graph_app
postgresql://econgraph:pw@app-postgres-service:5432/econ_graph_app?host=elsewhere|
postgresql://econgraph:pw@app-postgres-service:5432/econ_graph_app?sslmode=require&hostaddr=10.0.0.1|
postgresql://econgraph:pw@app-postgres-service:5432/econ_graph_app?dbname=other|
postgresql://econgraph:pw@app-postgres-service:5432/econ_graph_app?port=6543|
postgresql://econgraph:pw@a:5432,b:5432/econ_graph_app|
postgresql://econgraph:pw@app-postgres-service:5432|
postgresql://econgraph:pw@app-postgres-service:5432/|
postgresql://econgraph:pw@app-postgres-service:x/econ_graph_app|
mysql://econgraph:pw@app-postgres-service/econ_graph_app|
CASES
check "10.96.0.5 and 10.96.0.9 are different databases" \
    [ "$(db_target_of postgresql://u:p@10.96.0.5/x)" != "$(db_target_of postgresql://u:p@10.96.0.9/x)" ]

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

# As in the cluster: postgres-init.yaml creates extensions as the superuser and
# the role econgraph, which owns the public schema; the data migrations, run as
# econgraph, create (and so own) a trusted extension, the schemas and tables.
psql_in "$ECON" postgres econ_graph "
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE EXTENSION IF NOT EXISTS \"uuid-ossp\";
CREATE USER econgraph WITH ENCRYPTED PASSWORD '$DATA_ROLE_PW';
GRANT ALL PRIVILEGES ON DATABASE econ_graph TO econgraph;
ALTER SCHEMA public OWNER TO econgraph;
GRANT ALL ON SCHEMA public TO econgraph;
SET ROLE econgraph;
CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE TABLE economic_series (id serial PRIMARY KEY, title text NOT NULL);
CREATE INDEX economic_series_title_trgm ON economic_series USING gin (title gin_trgm_ops);
CREATE TABLE data_points (id bigserial PRIMARY KEY, series_id int REFERENCES economic_series(id), value numeric);
CREATE SCHEMA audit;
CREATE TABLE audit.events (id serial PRIMARY KEY, note text);
INSERT INTO economic_series (title) SELECT 'series ' || g FROM generate_series(1, 25) g;
INSERT INTO data_points (series_id, value) SELECT 1 + g % 25, g * 1.5 FROM generate_series(1, 5000) g;
INSERT INTO audit.events (note) VALUES ('a'), ('b'), ('c');
ALTER TABLE data_points ADD CONSTRAINT data_points_value_nonnegative CHECK (value >= 0);
CREATE FUNCTION audit.note_upper(t text) RETURNS text LANGUAGE sql IMMUTABLE AS 'SELECT upper(t)';
CREATE VIEW series_point_counts AS SELECT series_id, count(*) AS points FROM data_points GROUP BY series_id;"
# Comments of their own on two extensions, so a restore that creates them can
# be told to have carried the dump's comment over.
psql_in "$ECON" postgres econ_graph "COMMENT ON EXTENSION pgcrypto IS 'econ_graph''s pgcrypto';
COMMENT ON EXTENSION \"uuid-ossp\" IS 'econ_graph''s uuid-ossp';"

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

# The schema without owners or grants: every table, index, constraint, view,
# function and extension, in pg_dump's stable order. A restore that loses any
# of them (say, an over-wide filter on the dump's TOC) differs from the source.
# pg_dump's comments and its per-run \restrict key lines are left out.
schema() { # schema CONTAINER USER DB
    docker exec "$1" pg_dump -U "$2" -d "$3" --schema-only --no-owner --no-privileges |
        grep -v -e '^--' -e '^\\restrict ' -e '^\\unrestrict ' -e '^$'
}
schema "$ECON" postgres econ_graph >"$tmp/econ.schema"
schema "$APP" postgres econ_graph_app >"$tmp/app.schema"
check "the source schema has the trigram index" \
    grep -q 'CREATE INDEX economic_series_title_trgm' "$tmp/econ.schema"
check "  the CHECK constraint" grep -q 'data_points_value_nonnegative' "$tmp/econ.schema"
check "  the function" grep -q 'CREATE FUNCTION audit.note_upper' "$tmp/econ.schema"
check "  the view" grep -q 'CREATE VIEW public.series_point_counts' "$tmp/econ.schema"
check "  and the extensions" grep -q 'CREATE EXTENSION IF NOT EXISTS pg_trgm' "$tmp/econ.schema"
counts "$KC" keycloak keycloak >"$tmp/kc.counts"

# --- running the CronJob's containers -----------------------------------------

# A fresh status emptyDir for each run, like each Job pod gets.
new_status() {
    rm -rf "$tmp/status"
    mkdir "$tmp/status"
    chmod 0777 "$tmp/status"
}

# run_backup [docker run options...]: backup.sh as the pg-dump init container
# runs it (the host-mapped fixture uid, read-only root, no capabilities). Prints its exit status.
run_backup() {
    local rc=0
    docker run --rm --network "$NET" --user "$TEST_USER" --read-only --cap-drop ALL \
        --security-opt no-new-privileges --env-file "$tmp/dump.env" "$@" \
        -v "$tmp/scripts:/scripts:ro" -v "$tmp/backups:/backups" -v "$tmp/status:/status" \
        "$PG_IMAGE" /scripts/backup.sh >>"$LOG" 2>&1 || rc=$?
    echo "$rc"
}

# run_sync PREFIX: sync.sh as the backup-sync container runs it, to the mock.
run_sync() {
    local rc=0
    # Exercise the production encryption stage before the production sync.
    # Earlier dump fixtures remain for the restore assertions; a real pod's
    # plaintext emptyDir holds only this run, so stage only new fixtures here.
    local stage
    stage="$(mktemp -d "$STAGE_ROOT/run.XXXXXX")"
    for file in "$tmp/backups"/*-*.dump; do
        [ -f "$file" ] && [ ! -L "$file" ] || continue
        [ -f "$tmp/ciphertexts/${file##*/}.age" ] || cp "$file" "$stage/"
    done
    for link in "$tmp/backups"/latest*.dump; do
        [ ! -L "$link" ] || cp -P "$link" "$stage/"
    done
    BACKUP_PLAINTEXT_DIR="$stage" BACKUP_DIR="$tmp/ciphertexts" \
        BACKUP_AGE_RECIPIENT="$RECIPIENT" AGE_BIN="$tmp/tools/age/age" \
        BACKUP_STATUS_FILE="$tmp/status/failed" sh "$tmp/scripts/encrypt.sh" >>"$LOG" 2>&1 || rc=$?
    for file in "$tmp/backups"/*-*.dump; do
        [ -f "$file" ] && [ ! -L "$file" ] || continue
        [ -f "$tmp/ciphertexts/${file##*/}.age" ] || continue
        { "$tmp/tools/age/age" -d -i "$tmp/test-identity" \
            "$tmp/ciphertexts/${file##*/}.age" | cmp - "$file"; } >>"$LOG" 2>&1 || rc=1
    done
    rm -rf "$stage"
    docker run --rm --network "$NET" --user "$TEST_USER" --read-only --cap-drop ALL \
        --security-opt no-new-privileges --entrypoint /scripts/sync.sh \
        -e BACKUP_DIR=/backups -e BACKUP_DESTINATION=s3 -e BACKUP_S3_BUCKET=backups \
        -e BACKUP_S3_PREFIX="$1" -e BACKUP_S3_ENDPOINT=http://s3mock:9090 \
        -e BACKUP_STATUS_FILE=/status/failed \
        -e AWS_ACCESS_KEY_ID=test -e AWS_SECRET_ACCESS_KEY=test -e AWS_DEFAULT_REGION=us-east-1 \
        -e AWS_CONFIG_FILE=/aws/config \
        -v "$tmp/scripts:/scripts:ro" -v "$tmp/ciphertexts:/backups:ro" -v "$tmp/status:/status:ro" \
        -v "$tmp/aws:/aws:ro" \
        "$AWS_IMAGE" >>"$LOG" 2>&1 || rc=$?
    echo "$rc"
}

s3_keys() { # s3_keys PREFIX: compare encrypted object names with their plaintext fixture names
    local keys key
    keys="$(docker run --rm --network "$NET" -e AWS_ACCESS_KEY_ID=test -e AWS_SECRET_ACCESS_KEY=test \
        -e AWS_DEFAULT_REGION=us-east-1 -e AWS_CONFIG_FILE=/aws/config -v "$tmp/aws:/aws:ro" \
        "$AWS_IMAGE" s3 ls --recursive --endpoint-url http://s3mock:9090 "s3://backups/$1/" |
        awk '{ print $4 }')" || return 1
    [ -n "$keys" ] || return 0
    while IFS= read -r key; do
        case "$key" in
            *.dump.age) ;;
            *)
                echo "Non-ciphertext object uploaded: $key" >&2
                # Callers also use process substitutions, whose exit status
                # is ignored; retain a parent-visible failure for the final check.
                : >"$tmp/invalid-s3-key"
                return 1
                ;;
        esac
    done <<<"$keys"
    printf '%s\n' "$keys" | sed "s|^$1/||; s|\\.age$||" | sort
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
        diff <(schema "$APP" postgres "$2") "$tmp/app.schema" &&
        [ "$(psql_in "$APP" postgres "$2" "SELECT count(*) FROM pg_tables
              WHERE schemaname = 'public' AND tableowner <> 'econgraph'")" = 0 ] &&
        psql_in "$APP" econgraph "$2" "INSERT INTO users (email) VALUES ('new@example.com');
              INSERT INTO chart_annotations (user_id, note)
                SELECT id, 'new' FROM users WHERE email = 'new@example.com'" &&
        psql_in "$APP" postgres postgres "DROP DATABASE $2"
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

# --- production UID on private container tmpfs -------------------------------

# Keep this pair independent of host bind-mount ownership. Both scripts run as
# the actual production uid, and share only a private tmpfs for their synthetic
# archive. This also works with rootless Docker's uid mapping.
if docker run --rm --network "$NET" --user 999:999 --read-only --cap-drop ALL \
    --security-opt no-new-privileges --entrypoint /bin/sh \
    --env-file "$tmp/dump.env" -e RESTORE_OWNER=econgraph \
    --tmpfs /backups:rw,uid=999,gid=999,mode=0700 \
    --tmpfs /status:rw,uid=999,gid=999,mode=0700 \
    -v "$tmp/scripts:/scripts:ro" "$PG_IMAGE" -c '
        set -eu
        test "$(id -u)" = 999
        /scripts/backup.sh
        test -z "$(cat /status/failed)"
        test "$(stat -c %a /backups/econ_graph-*.dump)" = 600
        /scripts/restore.sh
    ' >>"$LOG" 2>&1; then
    pass "backup.sh and restore.sh run as production uid 999 on private tmpfs"
else
    fail "backup.sh or restore.sh failed as production uid 999 (see test log)"
fi
counts "$ECON" postgres econ_graph >"$tmp/uid999.counts"
check "uid 999 restore preserves every data-table row count" \
    diff -u "$tmp/econ.counts" "$tmp/uid999.counts"
schema "$ECON" postgres econ_graph >"$tmp/uid999.schema"
check "uid 999 restore preserves source schema" \
    diff -u "$tmp/econ.schema" "$tmp/uid999.schema"
check "uid 999 restore keeps application tables owned by econgraph" \
    [ "$(psql_in "$ECON" postgres econ_graph "SELECT count(*) FROM pg_tables
        WHERE schemaname IN ('public', 'audit') AND tableowner <> 'econgraph'")" = 0 ]

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
check "no dump deleted" nothing_deleted
# --- 2. restore.sh ------------------------------------------------------------

echo "# restore.sh"

# KEY=VALUE lines for docker --env-file, from the Job restore-postgres-backup.sh
# applies (its shell variables blanked), with the test password for the Secret.
python3 - "$ROOT/scripts/deploy/restore-postgres-backup.sh" "$ECON_PW" >"$tmp/restore.env" <<'PY'
import re, sys, yaml
text = open(sys.argv[1]).read()
manifest = re.search(r"<<EOF \| \S+ apply -f -\n(.*?)\nEOF\n", text, re.S).group(1)
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
check "the restore Job recreates everything as econgraph (RESTORE_OWNER)" \
    grep -qx 'RESTORE_OWNER=econgraph' "$tmp/restore.env"
check "  and restores the data database through postgres-service" \
    grep -qx 'PGHOST=postgres-service' "$tmp/restore.env"
grep -v '^RESTORE_OWNER=' "$tmp/restore.env" >"$tmp/restore-default-owner.env"

# run_restore [docker run options...]: restore.sh as the restore Job runs it
# (uid 999, read-only root, the backup volume read-only), with RESTORE_ENV
# (default the Job's environment) and RESTORE_BACKUPS (default the backup
# volume) mounted at /backups. Its output goes to $tmp/out; prints its exit
# status.
run_restore() {
    local rc=0
    docker run --rm --network "$NET" --user "$TEST_USER" --read-only --cap-drop ALL \
        --security-opt no-new-privileges --env-file "${RESTORE_ENV:-$tmp/restore.env}" "$@" \
        -v "$tmp/scripts:/scripts:ro" -v "${RESTORE_BACKUPS:-$tmp/backups}:/backups:ro" \
        "$PG_IMAGE" /scripts/restore.sh >"$tmp/out" 2>&1 || rc=$?
    cat "$tmp/out" >>"$LOG"
    echo "$rc"
}

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
EXTENSIONS_SQL="SELECT string_agg(extname || ':' || extowner::regrole::text, ' ' ORDER BY extname) FROM pg_extension"
# The comments the source gives the two extensions a new server lacks.
EXT_COMMENTS_SQL="SELECT string_agg(extname || '=' || coalesce(obj_description(oid, 'pg_extension'), '<none>'), ' | ' ORDER BY extname)
  FROM pg_extension WHERE extname IN ('pgcrypto', 'uuid-ossp')"
# What the app does with econ_graph: write rows (sequences, defaults,
# constraints) in every schema, and read the view and the function.
APP_WRITE_SQL="INSERT INTO economic_series (title) VALUES ('new'); INSERT INTO audit.events (note) VALUES ('new');
     INSERT INTO data_points (series_id, value) SELECT id, 1 FROM economic_series WHERE title = 'new';
     SELECT count(*) FROM series_point_counts; SELECT audit.note_upper('x');
     DELETE FROM data_points WHERE series_id = (SELECT id FROM economic_series WHERE title = 'new');
     DELETE FROM audit.events WHERE note = 'new'; DELETE FROM economic_series WHERE title = 'new';"
app_writes() { psql_in "$1" econgraph econ_graph "$APP_WRITE_SQL" >/dev/null; }

check "the source econ_graph is all econgraph's, but for extensions" \
    [ -z "$(psql_in "$ECON" postgres econ_graph "$FOREIGN_OWNED_SQL")" ]
psql_in "$ECON" postgres econ_graph "$EXTENSIONS_SQL" >"$tmp/econ.extensions"
psql_in "$ECON" postgres econ_graph "$EXT_COMMENTS_SQL" >"$tmp/econ.ext-comments"
check "the source's comments on pgcrypto and uuid-ossp are its own ($(cat "$tmp/econ.ext-comments"))" \
    [ "$(cat "$tmp/econ.ext-comments")" = "pgcrypto=econ_graph's pgcrypto | uuid-ossp=econ_graph's uuid-ossp" ]
echo "# source econ_graph extensions: $(cat "$tmp/econ.extensions")"

echo "# restore.sh over the existing database"
psql_in "$ECON" postgres econ_graph "DELETE FROM data_points WHERE id > 100; DELETE FROM audit.events;"
rc="$(run_restore)"
check "restore.sh of latest.dump exits 0" [ "$rc" = 0 ]
check "  restoring as econgraph" grep -q "owned by econgraph" "$tmp/out"
check "  creating no extension (they are all there)" \
    grep -q "Every extension in the dump is already in 'econ_graph'" "$tmp/out"
check "econ_graph row counts are back" diff <(counts "$ECON" postgres econ_graph) "$tmp/econ.counts"
psql_in "$ECON" postgres econ_graph "$FOREIGN_OWNED_SQL" >"$tmp/foreign-owned"
check "  econgraph still owns every table, sequence, view, index, function, type and schema" \
    [ ! -s "$tmp/foreign-owned" ]
[ ! -s "$tmp/foreign-owned" ] || sed 's/^/    owned by another role: /' "$tmp/foreign-owned"
check "  and can write to them" app_writes "$ECON"
check "  and the schema is the source's (tables, indexes, constraints, views, functions, extensions)" \
    diff <(schema "$ECON" postgres econ_graph) "$tmp/econ.schema"
# The schema comparison itself catches a lost index: a restore from a TOC
# without the INDEX entries differs.
psql_in "$ECON" postgres postgres "DROP DATABASE IF EXISTS econ_noindex"
psql_in "$ECON" postgres postgres "CREATE DATABASE econ_noindex"
docker exec -i "$ECON" sh -c 'cat >/tmp/econ.dump && pg_restore --list /tmp/econ.dump |
    awk "\$4 != \"INDEX\"" >/tmp/econ.list &&
    pg_restore -U postgres -d econ_noindex --no-owner --no-privileges --exit-on-error \
        --single-transaction --use-list=/tmp/econ.list /tmp/econ.dump' <"$tmp/backups/$econ_dump"
differs() { ! diff -q "$1" "$2" >/dev/null; }
check "  (and that comparison fails for a restore that lost an index)" \
    differs <(schema "$ECON" postgres econ_noindex) "$tmp/econ.schema"
psql_in "$ECON" postgres postgres "DROP DATABASE econ_noindex"
check "  and every extension is untouched, owner included ($(cat "$tmp/econ.extensions"))" \
    [ "$(psql_in "$ECON" postgres econ_graph "$EXTENSIONS_SQL")" = "$(cat "$tmp/econ.extensions")" ]
# An extension already there keeps its comment: the dump has pg_trgm's
# default one, the target a comment of its own.
PG_TRGM_DEFAULT_COMMENT="$(psql_in "$ECON" postgres econ_graph "SELECT obj_description(oid, 'pg_extension') FROM pg_extension WHERE extname = 'pg_trgm'")"
psql_in "$ECON" postgres econ_graph "COMMENT ON EXTENSION pg_trgm IS 'set on the target only'"
rc="$(run_restore)"
check "restore.sh over a target with its own pg_trgm comment exits 0" [ "$rc" = 0 ]
check "  and the target's comment survives" \
    [ "$(psql_in "$ECON" postgres econ_graph "SELECT obj_description(oid, 'pg_extension') FROM pg_extension WHERE extname = 'pg_trgm'")" = "set on the target only" ]
psql_in "$ECON" postgres econ_graph "COMMENT ON EXTENSION pg_trgm IS '$PG_TRGM_DEFAULT_COMMENT'"

# A newly initialised server: postgres-init.yaml's script only, which creates
# uuid-ossp, pg_stat_statements and pg_trgm, but not pgcrypto (the data
# migrations create that). Restored without RESTORE_OWNER, so the default
# owner applies.
echo "# restore.sh on a newly initialised server, without RESTORE_OWNER"
mkdir -m 0755 "$tmp/initdb"
python3 - "$ROOT/k8s/manifests/postgres-init.yaml" "$tmp/initdb" <<'PY'
import os, sys, yaml
for key, text in yaml.safe_load(open(sys.argv[1]))["data"].items():
    path = os.path.join(sys.argv[2], key)
    with open(path, "w") as f:
        f.write(text)
    os.chmod(path, 0o555)
PY
docker run -d --name "$FRESH" --network "$NET" --network-alias postgres-fresh \
    -e POSTGRES_PASSWORD="$ECON_PW" -e POSTGRES_DB=econ_graph -e APP_DB_PASSWORD="$DATA_ROLE_PW" \
    -v "$tmp/initdb:/docker-entrypoint-initdb.d:ro" "$PG_IMAGE" >/dev/null
# Ready over TCP only once the init script has run (the image's temporary
# server for it listens on the socket alone).
wait_pg "$FRESH" postgres econ_graph
fresh_before="$(psql_in "$FRESH" postgres econ_graph "$EXTENSIONS_SQL")"
check "the new server lacks pgcrypto ($fresh_before)" \
    [ "$fresh_before" = "pg_stat_statements:postgres pg_trgm:postgres plpgsql:postgres uuid-ossp:postgres" ]
# Without uuid-ossp too: the dump's TOC quotes that name in its comment entry
# ("uuid-ossp"), not in its extension entry.
psql_in "$FRESH" postgres econ_graph 'DROP EXTENSION "uuid-ossp"'
rc="$(RESTORE_ENV="$tmp/restore-default-owner.env" run_restore -e PGHOST=postgres-fresh)"
check "restore.sh of latest.dump exits 0" [ "$rc" = 0 ]
check "  restoring as econgraph, the default owner" grep -q "owned by econgraph" "$tmp/out"
check "  creating pgcrypto, and logging it" grep -qx "Created extension pgcrypto" "$tmp/out"
check "  creating uuid-ossp, and logging it" grep -qx "Created extension uuid-ossp" "$tmp/out"
check "  and no other extension" [ "$(grep -c '^Created extension ' "$tmp/out")" = 2 ]
check "  the extensions are those already there plus the two, owned by the superuser" \
    [ "$(psql_in "$FRESH" postgres econ_graph "$EXTENSIONS_SQL")" = "pg_stat_statements:postgres pg_trgm:postgres pgcrypto:postgres plpgsql:postgres uuid-ossp:postgres" ]
check "  each created extension has the dump's comment ($(cat "$tmp/econ.ext-comments"))" \
    [ "$(psql_in "$FRESH" postgres econ_graph "$EXT_COMMENTS_SQL")" = "$(cat "$tmp/econ.ext-comments")" ]
check "  and pg_trgm, already there, keeps the init script's" \
    [ "$(psql_in "$FRESH" postgres econ_graph "SELECT obj_description(oid, 'pg_extension') FROM pg_extension WHERE extname = 'pg_trgm'")" = "$PG_TRGM_DEFAULT_COMMENT" ]
check "  the row counts are the source's" diff <(counts "$FRESH" postgres econ_graph) "$tmp/econ.counts"
psql_in "$FRESH" postgres econ_graph "$FOREIGN_OWNED_SQL" >"$tmp/foreign-owned"
check "  everything else is econgraph's" [ ! -s "$tmp/foreign-owned" ]
[ ! -s "$tmp/foreign-owned" ] || sed 's/^/    owned by another role: /' "$tmp/foreign-owned"
check "  and econgraph can write to it" app_writes "$FRESH"
docker rm -f "$FRESH" >/dev/null

echo "# restore.sh failures"
# An object econgraph can't drop, and one dropped last: the schema audit,
# owned by postgres, with econgraph still able to use it. --clean drops every
# table before it, so this checks that the whole step is one transaction: the
# restore stops at DROP SCHEMA and every table comes back as it was.
psql_in "$ECON" postgres econ_graph "DELETE FROM data_points WHERE id > 100;
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
psql_in "$ECON" econgraph econ_graph "CREATE TABLE aaa_swap_test (id int); INSERT INTO aaa_swap_test VALUES (1);"
docker exec "$ECON" pg_dump -U postgres -d econ_graph --format=custom --no-owner --no-privileges \
    >"$tmp/swap/econ_graph-29990101-000000.dump"
psql_in "$ECON" econgraph econ_graph "DROP TABLE aaa_swap_test;"
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
psql_in "$ECON" postgres econ_graph "DELETE FROM data_points WHERE id > 100;"
rc=0
docker run --rm --network "$NET" --user "$TEST_USER" --read-only --cap-drop ALL \
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

# A dump named like econ_graph's that holds no table (another database's).
mkdir -m 0755 "$tmp/notable"
docker exec "$ECON" pg_dump -U postgres -d postgres --format=custom --no-owner --no-privileges \
    >"$tmp/notable/econ_graph-20000101-000000.dump"
rc="$(RESTORE_BACKUPS="$tmp/notable" run_restore -e BACKUP_FILE=/backups/econ_graph-20000101-000000.dump)"
check "a dump with no table: restore.sh exits 1" [ "$rc" = 1 ]
check "  and says why" grep -q "lists no table to restore" "$tmp/out"
check "  and econ_graph is untouched" diff <(counts "$ECON" postgres econ_graph) "$tmp/econ.counts"

rc="$(run_restore -e RESTORE_OWNER=no_such_role)"
check "a missing RESTORE_OWNER: restore.sh exits 1" [ "$rc" = 1 ]
check "  and says why" grep -q "Role no_such_role (RESTORE_OWNER) does not exist" "$tmp/out"

rc="$(run_restore -e BACKUP_FILE="/backups/$kc_dump")"
check "restore.sh refuses a keycloak dump for econ_graph" [ "$rc" = 1 ]
check "  and says why" grep -q "is not a econ_graph dump" "$tmp/out"
check "  and econ_graph is untouched" \
    [ "$(psql_in "$ECON" postgres econ_graph "SELECT count(*) FROM information_schema.tables WHERE table_name = 'user_entity'")" = 0 ]
rc="$(run_restore -e BACKUP_FILE=/backups/latest-econ_graph_app.dump)"
check "restore.sh refuses an econ_graph_app dump (through its latest link) for econ_graph" [ "$rc" = 1 ]
check "  and says why" grep -q "is not a econ_graph dump" "$tmp/out"
check "  and econ_graph is untouched" \
    [ "$(psql_in "$ECON" postgres econ_graph "SELECT count(*) FROM information_schema.tables WHERE table_name = 'chart_annotations'")" = 0 ]
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
docker run --rm --network "$NET" --user "$TEST_USER" --read-only --cap-drop ALL \
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
# A separate app database (app-database-url names it) that has no tables.
rc="$(run_backup -e APP_PGDATABASE=postgres \
    -e APP_DATABASE_URL="postgresql://econgraph:pw%40x@app-postgres-service:5432/postgres")"
check "app database with no tables: econ_graph_app recorded as failed" [ "$(status)" = "econ_graph_app" ]
check "  as holding no table" grep -q "\[econ_graph_app\] the dump holds no table" "$LOG"
check "  and no app dump of it kept" [ "$(newest econ_graph_app)" = "$before_app" ]
check "no .partial left behind" [ -z "$(partials)" ]
check "no dump deleted" nothing_deleted

# --- 4c. which app database: APP_DATABASE=shared or separate -------------------------

# backup_with ENV_FILE [docker run options...]: backup.sh with that environment,
# its output in $tmp/out (and the log). Prints its exit status.
backup_with() {
    local rc=0 env_file="$1"
    shift
    docker run --rm --network "$NET" --user "$TEST_USER" --read-only --cap-drop ALL \
        --security-opt no-new-privileges --env-file "$env_file" "$@" \
        -v "$tmp/scripts:/scripts:ro" -v "$tmp/backups:/backups" -v "$tmp/status:/status" \
        "$PG_IMAGE" /scripts/backup.sh >"$tmp/out" 2>&1 || rc=$?
    cat "$tmp/out" >>"$LOG"
    echo "$rc"
}
app_lines() { grep -c '^\[econ_graph_app\]' "$tmp/out" || true; }

check "the manifest sets APP_REQUIRED=false" grep -qx 'APP_REQUIRED=false' "$tmp/dump.env"
check "the manifest reads app-database-url" grep -qx "APP_DATABASE_URL=$APP_URL" "$tmp/dump.env"
check "the data side is the dumped target (PG* env), not a second URL" \
    bash -c "! grep -q '^DATA_DATABASE_URL=' '$tmp/dump.env'"

echo "# APP_DATABASE=shared: app-database-url names the database dumped as econ_graph"
while read -r shared_url required; do
    sleep 1
    new_status
    before_econ="$(newest econ_graph)"
    before_kc="$(newest keycloak)"
    rc="$(backup_with "$tmp/dump.env" -e APP_DATABASE_URL="$shared_url" -e APP_REQUIRED="$required")"
    check "backup.sh exits 0 (APP_REQUIRED=$required)" [ "$rc" = 0 ]
    check "nothing recorded as failed" [ "$(status)" = "" ]
    check "one log line for the app database" [ "$(app_lines)" = 1 ]
    check "  saying it is in the econ_graph dump" \
        grep -q "^\[econ_graph_app\] app-database-url names the data database (APP_DATABASE=shared): the app tables are in the econ_graph dump; skipping" "$tmp/out"
    check "no app dump" [ "$(newest econ_graph_app)" = "$before_app" ]
    check "a new econ_graph dump is kept" [ "$(newest econ_graph)" != "$before_econ" ]
    check "a new keycloak dump is kept" [ "$(newest keycloak)" != "$before_kc" ]
done <<SHARED
$SHARED_APP_URL false
postgresql://econgraph:pw%40x@Postgres-Service.econ-graph.svc.cluster.local:5432/econ_graph?sslmode=disable&application_name=a@b false
$SHARED_APP_URL true
SHARED
rc="$(run_sync run4c)"
check "sync.sh exits 0" [ "$rc" = 0 ]
check "no dump deleted" nothing_deleted

echo "# APP_DATABASE=separate, named in upper case with the full Service name and options"
sleep 1
new_status
rc="$(backup_with "$tmp/dump.env" -e APP_DATABASE_URL="postgresql://econgraph:pw%40x@APP-Postgres-Service.econ-graph.svc.cluster.local:5432/econ_graph_app?sslmode=disable&application_name=a@b")"
check "backup.sh exits 0, nothing failed" [ "$rc:$(status)" = "0:" ]
check "  and a new app dump is kept" [ "$(newest econ_graph_app)" != "$before_app" ]
before_app="$(newest econ_graph_app)"

echo "# no app-database-url (Secret econ-graph-secrets or its key missing)"
grep -v '^APP_DATABASE_URL=' "$tmp/dump.env" >"$tmp/dump-no-url.env"
sleep 1
new_status
rc="$(backup_with "$tmp/dump-no-url.env")"
check "backup.sh exits 0, nothing failed" [ "$rc:$(status)" = "0:" ]
check "  one log line for the app database" [ "$(app_lines)" = 1 ]
check "  saying it is skipped for want of the URL" grep -q "APP_DATABASE_URL is empty .*; skipping" "$tmp/out"
sleep 1
new_status
rc="$(backup_with "$tmp/dump-no-url.env" -e APP_REQUIRED=true)"
check "  with APP_REQUIRED=true: econ_graph_app recorded as failed" [ "$(status)" = "econ_graph_app" ]

echo "# app-database-url names some other database, or can't be read: always a failure"
while read -r other_url; do
    sleep 1
    new_status
    before_econ="$(newest econ_graph)"
    before_kc="$(newest keycloak)"
    rc="$(backup_with "$tmp/dump.env" -e APP_DATABASE_URL="$other_url")"
    check "${other_url#*@}: econ_graph_app recorded as failed (APP_REQUIRED=false)" \
        [ "$rc:$(status)" = "0:econ_graph_app" ]
    check "  saying the database it names is not backed up" \
        grep -q "^\[econ_graph_app\] app-database-url names neither the data database (data-postgres-service:5432/econ_graph) nor app-postgres-service:5432/econ_graph_app, or can't be read; the app database it names is not backed up" "$tmp/out"
    check "  no app dump" [ "$(newest econ_graph_app)" = "$before_app" ]
    check "  new econ_graph and keycloak dumps are kept" \
        [ "$(newest econ_graph)/$(newest keycloak)" != "$before_econ/$before_kc" ]
done <<'OTHER'
postgresql://u:p@elsewhere:5432/other
postgresql://econgraph:pw@app-postgres-service:5432/econ_graph_app_old
postgresql://econgraph:pw@data-postgres-service:5432/econ_graph_app
postgresql://econgraph:pw@app-postgres-service:5432/econ_graph_app?host=elsewhere
postgresql://econgraph:pw@10.96.0.5:5432/econ_graph_app
postgresql://econgraph:pw@[fd00::5]:5432/econ_graph_app
not a URL at all
OTHER
rc="$(run_sync run4cx)"
check "sync.sh fails the Job" [ "$rc" = 1 ]
check "no dump deleted" nothing_deleted

echo "# APP_DATABASE=separate without the app database's password"
# Without the Secret, the optional secretKeyRef leaves the variable unset.
grep -v '^APP_PGPASSWORD=' "$tmp/dump.env" >"$tmp/dump-no-app.env"
sleep 1
new_status
before_econ="$(newest econ_graph)"
before_kc="$(newest keycloak)"
rc="$(backup_with "$tmp/dump-no-app.env")"
check "backup.sh exits 0 with a status file" [ "$rc" = 0 ]
check "econ_graph_app recorded as failed" [ "$(status)" = "econ_graph_app" ]
check "  and says the Secret is missing" \
    grep -q "APP_PGPASSWORD is empty (Secret econ-graph-app-postgres" "$tmp/out"
check "a new econ_graph dump is kept" [ "$(newest econ_graph)" != "$before_econ" ]
check "a new keycloak dump is kept" [ "$(newest keycloak)" != "$before_kc" ]
check "app dump unchanged" [ "$(newest econ_graph_app)" = "$before_app" ]
rc="$(run_sync run4d)"
check "sync.sh fails the Job" [ "$rc" = 1 ]
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
docker run --rm --network "$NET" --user "$TEST_USER" --read-only --cap-drop ALL \
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
docker run --rm --network "$NET" --user "$TEST_USER" --read-only --cap-drop ALL \
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

# --- 8. a dump removed from the volume by hand stays off-site ------------------------

echo "# a dump removed from the volume, then synced again"
check "the first keycloak dump is in S3 ($kc_dump)" grep -qx "$kc_dump" <(s3_keys run7)
# Operator removal covers both temporary test plaintext and its PVC ciphertext.
rm -f "$tmp/backups/$kc_dump" "$tmp/ciphertexts/$kc_dump.age"
grep -vx "$kc_dump" "$SEEN" >"$tmp/seen.new" || true
mv "$tmp/seen.new" "$SEEN"
new_status
printf '\n' >"$tmp/status/failed"
rc="$(run_sync run7)"
check "sync.sh exits 0" [ "$rc" = 0 ]
check "  and the removed dump is still in S3" grep -qx "$kc_dump" <(s3_keys run7)

check "all raw off-site object names end in .dump.age" [ ! -e "$tmp/invalid-s3-key" ]

# --- credentials ---------------------------------------------------------------------

check "no output shows the econ_graph password" bash -c "! grep -qF -- '$ECON_PW' '$LOG'"
check "no output shows the econ_graph_app password" bash -c "! grep -qF -- '$APP_PW' '$LOG'"
check "no output shows the passwords in the backend's URLs" \
    bash -c "! grep -qF -e '$APP_ROLE_PW' -e '$DATA_ROLE_PW' -e 'pw%40x' -e 'postgresql://' '$LOG'"
check "no output shows the keycloak password" bash -c "! grep -qF -- '$KC_PW' '$LOG'"

echo
if [ "$failures" -ne 0 ]; then
    echo "$failures check(s) failed; output of the scripts:"
    cat "$LOG"
    exit 1
fi
echo "All checks passed."
