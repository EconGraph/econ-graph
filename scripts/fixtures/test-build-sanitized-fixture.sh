#!/usr/bin/env bash
#
# Tests scripts/fixtures/build-sanitized-fixture.sh against synthetic dumps.
# Never touches a cluster, a real dump or the host's backup directories.
#
# It builds:
#   - the data and app databases from backend/migrations in a scratch
#     postgres:18, with synthetic rows and planted real-looking personal data
#     and secrets: e-mail addresses (in users, security events, audit details,
#     annotations, comments), names, a bcrypt hash, an OAuth subject, an
#     avatar URL, IP addresses and user agents, session rows, JWTs in audit
#     and security metadata and in crawl headers, an API key in a crawl URL;
#   - Keycloak's database by running Keycloak (the version
#     k8s/manifests/keycloak/deployment.yaml deploys) on a scratch postgres:18
#     with config/keycloak/econ-graph-realm.json, then creating users with
#     passwords, a Google identity link, a confidential client with a secret,
#     user and offline sessions, login and admin events and a login failure;
#     then planting an SMTP password, a JGroups certificate, a registration
#     token and a free-form user attribute;
# and dumps them as the host dump directory (APP_DATABASE=shared: econ_graph
# holds the app tables) and as the postgres-backup volume (separate app
# database, <name>-<stamp>.dump with latest*.dump links).
#
# Checks:
#   - --list-steps names every step; --dry-run resolves both layouts and
#     --latest, starts no container and writes nothing (no output directory,
#     no pseudonym key); a daemon that isn't rootless is refused unless
#     FIXTURE_ALLOW_ROOTFUL_DOCKER=true;
#   - a full build of each layout succeeds, and no planted value is left in
#     any output dump; pseudonyms are consistent (the app's user name equals
#     Keycloak's first and last name for the same address; audit rows carry
#     their account's name); Keycloak's realms, roles, clients and groups keep
#     their row counts; the MANIFEST's checksums, row counts and source
#     timestamps match; the inputs are byte-for-byte unchanged;
#   - Keycloak starts on the sanitized keycloak.dump and serves the realm
#     with new signing keys;
#   - a second run into the same output directory does nothing; a different
#     build into it is refused;
#   - leaving out any single sanitize step (FIXTURE_TEST_SKIP_STEPS) makes
#     verification fail the run with nothing written; so does an unknown
#     secret the steps don't cover;
#   - --ci-subset keeps at most the requested series from every source with
#     all their data points and vintages, every reference row, the whole app
#     and Keycloak databases, and passes verification; it reports its size;
#     a size cap it exceeds fails it with nothing written.
#
# Needs docker (FIXTURE_DOCKER_HOST, else DOCKER_HOST, else the rootless
# socket), curl and jq. Scratch files go under FIXTURE_TEST_DIR (default a
# new directory under TMPDIR). Usage: scripts/fixtures/test-build-sanitized-fixture.sh

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BUILD="${ROOT}/scripts/fixtures/build-sanitized-fixture.sh"
python3 "${ROOT}/scripts/fixtures/test_fixture_reuse.py"
PG_IMAGE="postgres:18"
KC_IMAGE="$(sed -n 's/^ *image: *\(quay.io\/keycloak\/keycloak:[^ ]*\).*/\1/p' \
    "${ROOT}/k8s/manifests/keycloak/deployment.yaml" | head -n 1)"
[[ -n "${KC_IMAGE}" ]] || { echo "cannot read the Keycloak image from deployment.yaml" >&2; exit 1; }

# CI runners have only the system's rootful daemon; anywhere else the test, like the builder,
# uses the rootless one and refuses a rootful daemon (here, the one the clusters run on).
if [[ "${CI:-}" == true ]]; then
    default_host=unix:///var/run/docker.sock
else
    default_host="unix:///run/user/$(id -u)/docker.sock"
fi
export FIXTURE_DOCKER_HOST="${FIXTURE_DOCKER_HOST:-${DOCKER_HOST:-${default_host}}}"
export DOCKER_HOST="${FIXTURE_DOCKER_HOST}"
unset DOCKER_CONTEXT
if ! docker info --format '{{json .SecurityOptions}}' | grep -q rootless; then
    if [[ "${CI:-}" != true ]]; then
        echo "${DOCKER_HOST} is not a rootless Docker daemon; outside CI this test runs only on one" >&2
        exit 1
    fi
    export FIXTURE_ALLOW_ROOTFUL_DOCKER=true
fi

W="${FIXTURE_TEST_DIR:-$(mktemp -d "${TMPDIR:-/tmp}/fixture-test.XXXXXX")}"
mkdir -p "${W}"
W="$(cd "${W}" && pwd)"
export HOME="${W}/home"
export XDG_CONFIG_HOME="${HOME}/.config"
# scripts/backup/lib.sh (for --latest) looks here, never at the host's dumps.
export BACKUP_ROOT="${W}/backups/host"
export ECONGRAPH_CONFIG_DIR="${HOME}/.config/econgraph"
mkdir -p "${HOME}"

ID="fixture-test-$$"
NET="${ID}-net"
SRC="${ID}-src"
KCDB="${ID}-kcdb"
KC="${ID}-kc"
BOOTDB="${ID}-bootdb"
BOOT="${ID}-boot"

cleanup() {
    docker rm -fv "${SRC}" "${KCDB}" "${KC}" "${BOOTDB}" "${BOOT}" >/dev/null 2>&1 || true
    docker network rm "${NET}" >/dev/null 2>&1 || true
    if [[ -z "${FIXTURE_TEST_KEEP:-}" ]]; then
        chmod -R u+w "${W}" 2>/dev/null || true
        rm -rf -- "${W}"
    fi
}
trap cleanup EXIT

PASS=0
FAIL=0
ok() { PASS=$((PASS + 1)); echo "ok   $*"; }
bad() { FAIL=$((FAIL + 1)); echo "FAIL $*"; }
check() { local what="$1"; shift; if "$@"; then ok "${what}"; else bad "${what}"; fi; }

wait_pg() {
    for _ in $(seq 1 60); do
        docker exec "$1" pg_isready -q -h 127.0.0.1 -U "$2" 2>/dev/null && return 0
        sleep 1
    done
    echo "postgres in $1 did not start" >&2
    return 1
}
psql_in() { docker exec -i "$1" psql -X -q -v ON_ERROR_STOP=1 -U "$2" -d "$3" "${@:4}"; }

# ------------------------------------------------------------ planted values

JWT="eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJhbGljZSIsImVtYWlsIjoiYWxpY2Uuam9obnNvbkBnbWFpbC5jb20ifQ.Xk3dQm9vZ2xlU2lnbmF0dXJlVmFsdWVfMTIzNDU2Nzg5MA"
API_KEY="4f9a2c81d7e35b60a1c9e8f27d4b3a65"
# shellcheck disable=SC2016 # a literal bcrypt hash
BCRYPT='$2b$12$R9h/cIPz0gi.URNNX3kh2OPST9/PgBkqquzi.Ss7KIUgO2t0jWMUW'
GOOGLE_SUB="109876543210987654321"
# Assembled here so that secret scanners don't take the fakes in this file
# for real Google credentials.
GOOGLE_SECRET="GOCSPX""-Rk7aQ2mZx9VbT4LcW8pYnE3sHdJf"
GOOGLE_CLIENT_ID="123456789012-abc"".apps.googleusercontent.com"
BATCH_SECRET="Qm9vZ2xlU2VjcmV0VmFsdWUxMjM0NTY3ODkw"
SMTP_PASSWORD="Smtp-Pa55word-Secret-9f8e7d"
REG_TOKEN="eyJhbGciOiJIUzUxMiJ9.eyJqdGkiOiJyZWctdG9rZW4tMDAxIiwidHlwIjoiUmVnaXN0cmF0aW9uQWNjZXNzVG9rZW4ifQ.c2lnbmF0dXJlLWZvci1yZWdpc3RyYXRpb24tdG9rZW4"
PEM_LINE="-----BEGIN PRIVATE KEY-----"
SESSION_TOKEN_HASH="9b74c9897bac770ffc029102a200c5de2fb3e6c4d0a6b5f8e1d2c3b4a5968778"
UA="Mozilla/5.0 (Macintosh; Intel Mac OS X 14_5) AppleWebKit/605.1.15 Version/17.5 Safari/605.1.15"
# Every one of these must be gone from every output dump.
PLANTED=(
    "alice.johnson@gmail.com" "bob.smith@outlook.com" "carol.diaz@yahoo.com"
    "mallory.attacker@protonmail.com" "alice.johnson%40gmail.com"
    "Alice Johnson" "Bob Smith" "Carol Diaz" "Deleted Person" "Johnson"
    "Acme Research" "lh3.googleusercontent.com" "81.2.69.142" "203.0.113.77" "Safari/605.1.15"
    "${JWT}" "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9" "${API_KEY}" "${BCRYPT}" "${GOOGLE_SUB}"
    "${GOOGLE_SECRET}" "${BATCH_SECRET}" "${SMTP_PASSWORD}" "${REG_TOKEN}" "BEGIN PRIVATE KEY"
    "${SESSION_TOKEN_HASH}" "+1 415 555 0134"
)

# ------------------------------------------------------------ data and app

docker network create "${NET}" >/dev/null
docker run -d --name "${SRC}" --network "${NET}" -e POSTGRES_HOST_AUTH_METHOD=trust \
    -v "${ROOT}/backend/migrations:/migrations:ro" "${PG_IMAGE}" >/dev/null
docker run -d --name "${KCDB}" --network "${NET}" -e POSTGRES_USER=keycloak \
    -e POSTGRES_PASSWORD=kc-test-pw -e POSTGRES_DB=keycloak "${PG_IMAGE}" >/dev/null
wait_pg "${SRC}" postgres
wait_pg "${KCDB}" keycloak

# migrate DB SET...: apply the up.sql of each migration set in order, each in
# one transaction as the backend does, and record them in
# __diesel_schema_migrations.
migrate() {
    local db="$1" set dir version
    shift
    psql_in "${SRC}" postgres "${db}" -c \
        "CREATE TABLE IF NOT EXISTS __diesel_schema_migrations (version VARCHAR(50) PRIMARY KEY, run_on TIMESTAMP NOT NULL DEFAULT now())"
    for set in "$@"; do
        for dir in $(docker exec "${SRC}" sh -c "ls -d /migrations/${set}/*/" | sort); do
            version="$(basename "${dir}" | cut -d_ -f1 | tr -d -)"
            psql_in "${SRC}" postgres "${db}" -1 -f "${dir}up.sql" >/dev/null
            psql_in "${SRC}" postgres "${db}" -c \
                "INSERT INTO __diesel_schema_migrations (version) VALUES ('${version}') ON CONFLICT DO NOTHING" >/dev/null
        done
    done
}

DATA_SQL="$(cat <<SQL
-- The data baseline seeds the sources, countries and some series_metadata.
UPDATE data_sources SET description = description || '; questions to fred.help@stlouisfed.org'
    WHERE name LIKE '%(FRED)';
INSERT INTO datasets (source_id, code, name)
    SELECT id, 'default', name || ' series' FROM data_sources;
-- 40 series from each of three sources, plus one too long for the CI subset.
INSERT INTO economic_series (source_id, dataset_id, external_id, title, description, frequency)
    SELECT d.id, ds.id, left(d.name, 3) || '-' || g, d.name || ' series ' || g,
           'Synthetic series ' || g || ' for tests', 'Monthly'
    FROM data_sources d JOIN datasets ds ON ds.source_id = d.id, generate_series(1, 40) g
    WHERE d.name ~ '^(Federal|Bureau of Labor|World Bank)';
INSERT INTO economic_series (source_id, dataset_id, external_id, title, frequency)
    SELECT d.id, ds.id, 'LONG-' || left(d.name, 3), 'Long daily series', 'Daily'
    FROM data_sources d JOIN datasets ds ON ds.source_id = d.id
    WHERE d.name ~ '^(Federal|Bureau of Labor|World Bank)';
-- Two vintages of 24 months each; the long series 6000 days.
INSERT INTO data_points (series_id, date, value, revision_date, is_original_release)
    SELECT s.id, date '2020-01-01' + (m || ' months')::interval, m * 1.5 + v,
           date '2022-01-01' + v * 30, v = 0
    FROM economic_series s, generate_series(0, 23) m, generate_series(0, 1) v
    WHERE s.external_id NOT LIKE 'LONG-%';
INSERT INTO data_points (series_id, date, value, revision_date)
    SELECT s.id, date '2000-01-01' + d, d, date '2022-01-01'
    FROM economic_series s, generate_series(0, 5999) d WHERE s.external_id LIKE 'LONG-%';
INSERT INTO series_metadata (source_id, external_id, title, dataset_id)
    SELECT source_id, external_id, title, dataset_id FROM economic_series;
INSERT INTO series_fetch_validators (series_id, etag, content_sha256)
    SELECT id, md5(id::text), encode(sha256(convert_to(id::text, 'UTF8')), 'hex') FROM economic_series;
INSERT INTO crawl_attempts (series_id, crawl_method, crawl_url, user_agent, request_headers, response_headers)
    SELECT id, 'api',
           'https://api.stlouisfed.org/fred/series/observations?series_id=' || external_id
               || '&api_key=${API_KEY}&file_type=json',
           'EconGraph-Crawler/4.0 (+ops.alerts@econgraph.com)',
           '{"Authorization": "Bearer ${JWT}", "X-Api-Key": "${API_KEY}", "Accept": "application/json"}',
           '{"Set-Cookie": "session=${SESSION_TOKEN_HASH}", "Content-Type": "application/json"}'
    FROM economic_series LIMIT 5;
INSERT INTO crawl_queue (source, series_id, error_message)
    VALUES ('FRED', 'GDP', 'HTTP 400 for https://api.stlouisfed.org/fred/series?series_id=GDP&api_key=${API_KEY}');
INSERT INTO reference_file_cache (source_id, url, etag, payload)
    SELECT id, base_url || '/releases', 'W/"' || md5(name) || '"', '{"releases": [1, 2, 3]}' FROM data_sources;
SQL
)"

APP_SQL="$(cat <<SQL
INSERT INTO users (id, email, name, avatar_url, provider, provider_id, password_hash, organization) VALUES
    ('0194a1b2-0000-7000-8000-000000000001', 'alice.johnson@gmail.com', 'Alice Johnson',
     'https://lh3.googleusercontent.com/a/ACg8ocJx7RkPq', 'keycloak', '0194a1b2-0000-7000-8000-000000000001',
     NULL, 'Acme Research LLC'),
    ('0194a1b2-0000-7000-8000-000000000002', 'Bob.Smith@outlook.com', 'Bob Smith', NULL, 'email', NULL,
     '${BCRYPT}', NULL),
    ('0194a1b2-0000-7000-8000-000000000003', 'carol.diaz@yahoo.com', 'Carol Diaz', NULL, 'google',
     '${GOOGLE_SUB}', NULL, NULL);
INSERT INTO user_sessions (user_id, token_hash, expires_at, user_agent, ip_address)
    SELECT id, '${SESSION_TOKEN_HASH}', now() + interval '1 day', '${UA}', '81.2.69.142' FROM users;
INSERT INTO audit_logs (user_id, user_name, action, resource_type, resource_id, ip_address, user_agent, details) VALUES
    ('0194a1b2-0000-7000-8000-000000000001', 'Alice Johnson', 'login', 'session', NULL, '81.2.69.142', '${UA}',
     '{"email": "alice.johnson@gmail.com", "token": "${JWT}", "note": "shared with bob.smith@outlook.com"}'),
    ('0194a1b2-0000-7000-8000-000000000002', 'Bob Smith', 'update', 'chart', 'c1', '203.0.113.77', '${UA}', NULL),
    (NULL, 'Deleted Person', 'delete', 'user', 'u9', '81.2.69.142', '${UA}', '{"by": "admin"}');
INSERT INTO security_events (event_type, user_id, user_email, severity, ip_address, user_agent, description, metadata) VALUES
    ('login_failure', NULL, 'mallory.attacker@protonmail.com', 'high', '203.0.113.77', '${UA}',
     'Repeated failures for mallory.attacker@protonmail.com', '{"bearer": "${JWT}"}');
INSERT INTO chart_annotations (user_id, series_id, annotation_date, title, description) VALUES
    ('0194a1b2-0000-7000-8000-000000000001', 'Fed-1', '2020-03-01', 'Shock',
     'Ask me: alice.johnson@gmail.com, or see https://example.org/share?to=alice.johnson%40gmail.com');
INSERT INTO annotation_comments (annotation_id, user_id, content)
    SELECT id, '0194a1b2-0000-7000-8000-000000000003', 'cc carol.diaz@yahoo.com' FROM chart_annotations;
INSERT INTO chart_collaborators (chart_id, user_id, invited_by)
    VALUES ('0194a1b2-0000-7000-8000-0000000000c1', '0194a1b2-0000-7000-8000-000000000002',
            '0194a1b2-0000-7000-8000-000000000001');
SQL
)"

echo "== building the synthetic data and app databases"
for db in econ_graph econ_graph_data econ_graph_app; do
    psql_in "${SRC}" postgres postgres -c "CREATE DATABASE ${db}" >/dev/null
done
migrate econ_graph data app                 # APP_DATABASE=shared
migrate econ_graph_data data                # APP_DATABASE=separate
migrate econ_graph_app app
psql_in "${SRC}" postgres econ_graph >/dev/null <<<"${DATA_SQL}"
psql_in "${SRC}" postgres econ_graph >/dev/null <<<"${APP_SQL}"
psql_in "${SRC}" postgres econ_graph_data >/dev/null <<<"${DATA_SQL}"
psql_in "${SRC}" postgres econ_graph_app >/dev/null <<<"${APP_SQL}"

# ------------------------------------------------------------ keycloak

echo "== building the synthetic Keycloak database with ${KC_IMAGE}"
KC_WEB_BASE_URL=http://localhost KC_WEB_E2E_BASE_URL=http://localhost KC_WEB_DIRECT_GRANTS=true \
    KC_GOOGLE_ENABLED=true KC_GOOGLE_CLIENT_ID="${GOOGLE_CLIENT_ID}" \
    KC_GOOGLE_CLIENT_SECRET="${GOOGLE_SECRET}" \
    "${ROOT}/scripts/keycloak/render-realm.sh" "${ROOT}/config/keycloak/econ-graph-realm.json" >"${W}/realm.json"
chmod 0644 "${W}/realm.json"

# start_keycloak NAME DBCONTAINER [ARGS...]: start Keycloak on that database,
# wait for the econ-graph realm, and print its base URL.
start_keycloak() {
    local name="$1" db="$2" port
    shift 2
    docker run -d --name "${name}" --network "${NET}" -p 127.0.0.1::8080 \
        -e KC_DB=postgres -e "KC_DB_URL=jdbc:postgresql://${db}:5432/keycloak" \
        -e KC_DB_USERNAME=keycloak -e KC_DB_PASSWORD=kc-test-pw \
        -e KC_BOOTSTRAP_ADMIN_USERNAME=admin -e KC_BOOTSTRAP_ADMIN_PASSWORD=admin-test-pw \
        -e KC_HTTP_ENABLED=true -e KC_HOSTNAME_STRICT=false "$@" >/dev/null
    port="$(docker port "${name}" 8080/tcp | head -n 1 | sed 's/.*://')"
    for _ in $(seq 1 120); do
        if curl -sf "http://127.0.0.1:${port}/realms/econ-graph" >/dev/null; then
            echo "http://127.0.0.1:${port}"
            return 0
        fi
        sleep 2
    done
    docker logs "${name}" 2>&1 | tail -n 30 >&2
    return 1
}

KCURL="$(start_keycloak "${KC}" "${KCDB}" -v "${W}/realm.json:/opt/keycloak/data/import/realm.json:ro" \
    "${KC_IMAGE}" start-dev --import-realm)"
TOKEN="$(curl -sf -d grant_type=password -d client_id=admin-cli -d username=admin -d password=admin-test-pw \
    "${KCURL}/realms/master/protocol/openid-connect/token" | jq -r .access_token)"
kc_api() {
    local method="$1" path="$2"
    shift 2
    curl -sf -X "${method}" -H "Authorization: Bearer ${TOKEN}" -H 'Content-Type: application/json' \
        "${KCURL}/admin/realms${path}" "$@"
}
kc_api PUT /econ-graph -d '{"eventsEnabled": true, "adminEventsEnabled": true, "adminEventsDetailsEnabled": true}'
kc_api POST /econ-graph/groups -d '{"name": "analysts"}'
for u in "alice.johnson@gmail.com:Alice:Johnson" "bob.smith@outlook.com:Bob:Smith" "carol.diaz@yahoo.com:Carol:Diaz"; do
    IFS=: read -r email first last <<<"${u}"
    kc_api POST /econ-graph/users -d "{\"username\": \"${email}\", \"email\": \"${email}\",
        \"firstName\": \"${first}\", \"lastName\": \"${last}\", \"enabled\": true, \"emailVerified\": true,
        \"attributes\": {\"locale\": [\"en\"]},
        \"credentials\": [{\"type\": \"password\", \"value\": \"Pw-${first}-123\", \"temporary\": false}]}"
done
alice_id="$(kc_api GET '/econ-graph/users?username=alice.johnson@gmail.com&exact=true' | jq -r '.[0].id')"
kc_api POST "/econ-graph/users/${alice_id}/federated-identity/google" \
    -d "{\"identityProvider\": \"google\", \"userId\": \"${GOOGLE_SUB}\", \"userName\": \"alice.johnson@gmail.com\"}"
kc_api POST /econ-graph/clients -d "{\"clientId\": \"econ-graph-batch\", \"publicClient\": false,
    \"serviceAccountsEnabled\": true, \"secret\": \"${BATCH_SECRET}\"}"
grant() {
    curl -sf -d grant_type=password -d client_id=econ-graph-web -d "username=$1" -d "password=$2" -d "scope=$3" \
        "${KCURL}/realms/econ-graph/protocol/openid-connect/token" >/dev/null
}
grant alice.johnson@gmail.com Pw-Alice-123 "openid offline_access"
grant bob.smith@outlook.com Pw-Bob-123 openid
grant carol.diaz@yahoo.com wrong-password openid || true
docker stop -t 30 "${KC}" >/dev/null

psql_in "${KCDB}" keycloak keycloak >/dev/null <<SQL
INSERT INTO realm_smtp_config (realm_id, name, value)
    SELECT id, 'password', '${SMTP_PASSWORD}' FROM realm WHERE name = 'econ-graph';
INSERT INTO realm_smtp_config (realm_id, name, value)
    SELECT id, 'from', 'noreply@econgraph.com' FROM realm WHERE name = 'econ-graph';
INSERT INTO server_config (server_config_key, value)
    VALUES ('crt_jgroups', '${PEM_LINE}\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQC7\n-----END PRIVATE KEY-----');
UPDATE client SET registration_token = '${REG_TOKEN}' WHERE client_id = 'econ-graph-batch';
INSERT INTO user_attribute (id, name, value, user_id)
    SELECT gen_random_uuid()::text, 'phone', '+1 415 555 0134', id FROM user_entity WHERE username = 'alice.johnson@gmail.com';
SQL

# Row counts of the tables a sanitized Keycloak keeps whole.
kc_shape() {
    psql_in "$1" "$2" "$3" -At -F ' ' -c "select 'realm', count(*) from realm
        union all select 'keycloak_role', count(*) from keycloak_role
        union all select 'client', count(*) from client
        union all select 'keycloak_group', count(*) from keycloak_group
        union all select 'user_entity', count(*) from user_entity
        union all select 'user_role_mapping', count(*) from user_role_mapping"
}
KC_SHAPE="$(kc_shape "${KCDB}" keycloak keycloak)"
check "the synthetic Keycloak has sessions, credentials, events, keys and a federated identity" \
    test "$(psql_in "${KCDB}" keycloak keycloak -At -c "select (select count(*) from offline_user_session) > 0
        and (select count(*) from credential) >= 4 and (select count(*) from event_entity) > 0
        and (select count(*) from admin_event_entity) > 0 and (select count(*) from federated_identity) = 1
        and (select count(*) from login_failure) = 1
        and (select count(*) from component where provider_type = 'org.keycloak.keys.KeyProvider') > 0")" = t

# ------------------------------------------------------------ the dumps

HOSTDIR="${BACKUP_ROOT}/20261027T0800Z"
OLDHOSTDIR="${BACKUP_ROOT}/20261026T0800Z"
PVCDIR="${W}/backups/pvc"
STAMPDIR="${W}/backups/s3copy"
mkdir -p "${HOSTDIR}" "${OLDHOSTDIR}" "${PVCDIR}" "${STAMPDIR}"
docker exec "${SRC}" pg_dump --format=custom -U postgres -d econ_graph >"${HOSTDIR}/econ_graph.dump"
docker exec "${KCDB}" pg_dump --format=custom -U keycloak -d keycloak >"${HOSTDIR}/keycloak.dump"
echo "stamp=20261027T0800Z" >"${HOSTDIR}/MANIFEST"
date -u +%Y-%m-%dT%H:%M:%SZ >"${HOSTDIR}/VERIFIED"
cp "${HOSTDIR}/econ_graph.dump" "${HOSTDIR}/keycloak.dump" "${OLDHOSTDIR}/"
date -u +%Y-%m-%dT%H:%M:%SZ >"${OLDHOSTDIR}/VERIFIED"
S1=20261027-080000
docker exec "${SRC}" pg_dump --format=custom --no-owner --no-privileges -U postgres -d econ_graph_data \
    >"${PVCDIR}/econ_graph-${S1}.dump"
docker exec "${SRC}" pg_dump --format=custom --no-owner --no-privileges -U postgres -d econ_graph_app \
    >"${PVCDIR}/econ_graph_app-${S1}.dump"
cp "${HOSTDIR}/keycloak.dump" "${PVCDIR}/keycloak-${S1}.dump"
ln -s "${PVCDIR}/econ_graph-${S1}.dump" "${PVCDIR}/latest.dump"
ln -s "${PVCDIR}/econ_graph_app-${S1}.dump" "${PVCDIR}/latest-econ_graph_app.dump"
ln -s "${PVCDIR}/keycloak-${S1}.dump" "${PVCDIR}/latest-keycloak.dump"
cp "${PVCDIR}"/*-"${S1}".dump "${STAMPDIR}/"
for f in econ_graph econ_graph_app keycloak; do
    cp "${PVCDIR}/${f}-${S1}.dump" "${STAMPDIR}/${f}-20261020-080000.dump"
done
# An unknown secret no step covers: a GitHub token in a series description.
psql_in "${SRC}" postgres econ_graph_data -c \
    "UPDATE economic_series SET description = 'mirror token ghp_R2d2C3poL8sK9vB7nM4xQ1wE6tY5uI0oP3aS' WHERE external_id = 'Fed-7'" >/dev/null
docker exec "${SRC}" pg_dump --format=custom -U postgres -d econ_graph_data >"${W}/backups/leaky-econ_graph.dump"

# The inputs are read only from here on, and must not change.
chmod -R a-w "${W}/backups"
INPUT_SUMS="$(cd "${W}/backups" && find . -type f -print0 | sort -z | xargs -0 sha256sum)"
INPUT_LIST="$(cd "${W}/backups" && find . | sort)"
inputs_unchanged() {
    [[ "$(cd "${W}/backups" && find . -type f -print0 | sort -z | xargs -0 sha256sum)" == "${INPUT_SUMS}" ]] &&
        [[ "$(cd "${W}/backups" && find . | sort)" == "${INPUT_LIST}" ]]
}

# ------------------------------------------------------------ helpers

FIX="${HOME}/econgraph-fixtures"
LOG="${W}/log"
mkdir -p "${LOG}"
run_build() { local name="$1"; shift; "${BUILD}" "$@" </dev/null >"${LOG}/${name}.log" 2>&1; }
# No scratch container, data volume (it holds the unsanitized restore) or build directory left
no_scratch_left() {
    [[ -z "$(docker ps -aq --filter name=econgraph-fixture-)" ]] &&
        [[ -z "$(docker volume ls -q --filter label=econgraph.fixture-scratch=1)" ]] &&
        [[ -z "$(docker volume ls -q --filter name=econgraph-fixture-)" ]] &&
        [[ -z "$(find "${FIX}" "${W}/out" -maxdepth 1 -name '.build-*' 2>/dev/null)" ]]
}
# The text of every dump in DIR (pg_restore to SQL, the archive on stdin), to grep.
dump_text() {
    local f
    for f in "$1"/*.dump; do
        docker run --rm -i --network none "${PG_IMAGE}" pg_restore --file=- <"${f}"
    done
}
# Planted values found in the dumps of DIR, one per line. Fails when the dumps read back as no
# text at all, so an empty read can't pass as "nothing found".
leaks_in() {
    local text p
    text="$(dump_text "$1")"
    if [[ "${text}" != *"PostgreSQL database dump"* ]]; then
        echo "dump_text read nothing from $1" >&2
        return 1
    fi
    for p in "${PLANTED[@]}"; do
        if grep -qiF -- "${p}" <<<"${text}"; then echo "${p}"; fi
    done
}
manifest() { sed -n "s/^$2=//p" "$1/MANIFEST"; }

# ------------------------------------------------------------ --list-steps, --dry-run, guards

STEPS="$("${BUILD}" --list-steps | awk '{ print $1 }')"
check "--list-steps names the steps" test "$(wc -l <<<"${STEPS}")" -ge 15

before="$(find "${HOME}" | sort)"
if run_build dry-host --from-dir "${HOSTDIR}" --dry-run; then
    ok "--dry-run (host layout) succeeds"
else
    bad "--dry-run (host layout) succeeds"; cat "${LOG}/dry-host.log"
fi
check "--dry-run resolves the host layout" grep -q "econ_graph <- ${HOSTDIR}/econ_graph.dump" "${LOG}/dry-host.log"
check "--dry-run writes nothing (no fixture directory, no pseudonym key)" \
    test "$(find "${HOME}" | sort)" == "${before}"
check "--dry-run starts no container" no_scratch_left
run_build dry-pvc --from-dir "${PVCDIR}" --dry-run --ci-subset || true
check "--dry-run resolves the latest*.dump links" \
    grep -q "econ_graph_app <- ${PVCDIR}/econ_graph_app-${S1}.dump" "${LOG}/dry-pvc.log"
run_build dry-stamp --from-dir "${STAMPDIR}" --dry-run || true
check "--dry-run picks the newest stamped dumps" \
    grep -q "keycloak <- ${STAMPDIR}/keycloak-${S1}.dump" "${LOG}/dry-stamp.log"
run_build dry-latest --latest --dry-run || true
check "--latest picks the newest VERIFIED host directory" \
    grep -q "econ_graph <- ${HOSTDIR}/econ_graph.dump" "${LOG}/dry-latest.log"
check "--dry-run left the output untouched" test "$(find "${HOME}" | sort)" == "${before}"

mkdir -p "${W}/stub"
printf '#!/bin/sh\necho "28.0.0 [\\"name=seccomp,profile=builtin\\"]"\n' >"${W}/stub/docker"
chmod +x "${W}/stub/docker"
if PATH="${W}/stub:${PATH}" FIXTURE_ALLOW_ROOTFUL_DOCKER='' "${BUILD}" --from-dir "${HOSTDIR}" --dry-run >"${LOG}/rootful.log" 2>&1; then
    bad "a daemon that isn't rootless is refused"
else
    check "a daemon that isn't rootless is refused" grep -q "not a rootless Docker daemon" "${LOG}/rootful.log"
fi
if run_build missing --data "${W}/backups/nope.dump" --keycloak "${HOSTDIR}/keycloak.dump" --dry-run; then
    bad "a missing input is refused"
else
    ok "a missing input is refused"
fi

# ------------------------------------------------------------ full build, shared

# The scan itself works: on the unsanitized source dumps it finds every planted value.
FOUND="$(leaks_in "${HOSTDIR}" | sort -u)" || FOUND=""
MISSED="$(comm -23 <(printf '%s\n' "${PLANTED[@]}" | sort -u) <(printf '%s\n' "${FOUND}"))"
check "the leak scan finds every planted value in the unsanitized dumps${MISSED:+ (not found: ${MISSED//$'\n'/, })}" \
    test -z "${MISSED}"

echo "== full build, host layout (APP_DATABASE=shared)"
if run_build full-shared --from-dir "${HOSTDIR}"; then ok "full build (shared) succeeds"; else bad "full build (shared) succeeds"; cat "${LOG}/full-shared.log"; fi
OUT_SHARED="$(find "${FIX}" -mindepth 1 -maxdepth 1 -type d -name '2*' | head -n 1)"
check "the fixture is under ~/econgraph-fixtures/<UTC stamp>" test -n "${OUT_SHARED}"
check "it holds econ_graph.dump, keycloak.dump, MANIFEST, SHA256SUMS" \
    test -s "${OUT_SHARED}/econ_graph.dump" -a -s "${OUT_SHARED}/keycloak.dump" -a -s "${OUT_SHARED}/MANIFEST" -a -s "${OUT_SHARED}/SHA256SUMS"
check "SHA256SUMS matches" bash -c "cd '${OUT_SHARED}' && sha256sum --quiet -c SHA256SUMS"
check "MANIFEST records each output's sha256" \
    test "$(manifest "${OUT_SHARED}" output.keycloak.sha256)" == "$(sha256sum <"${OUT_SHARED}/keycloak.dump" | cut -d' ' -f1)"
check "MANIFEST records the source dump and its timestamp" \
    bash -c "grep -q '^input.econ_graph.file=20261027T0800Z/econ_graph.dump$' '${OUT_SHARED}/MANIFEST' &&
             grep -Eq '^input.keycloak.archive_created=[0-9]{4}-' '${OUT_SHARED}/MANIFEST'"
check "MANIFEST records row counts per table" \
    bash -c "grep -q '^rows.econ_graph.public.users=3$' '${OUT_SHARED}/MANIFEST' &&
             grep -q '^rows.econ_graph.public.user_sessions=0$' '${OUT_SHARED}/MANIFEST' &&
             grep -q '^rows.keycloak.public.credential=0$' '${OUT_SHARED}/MANIFEST' &&
             grep -q '^rows.econ_graph.public.data_points=' '${OUT_SHARED}/MANIFEST'"
LEAKS="$(leaks_in "${OUT_SHARED}")" || LEAKS="the dumps could not be read"
check "no planted value is left in the shared fixture${LEAKS:+ (left: ${LEAKS//$'\n'/, })}" test -z "${LEAKS}"
check "the pseudonym key was created 0600" \
    test "$(stat -c %a "${XDG_CONFIG_HOME}/econgraph/fixture-salt")" = 600
check "the fixture files are 0600" test "$(stat -c %a "${OUT_SHARED}/econ_graph.dump")" = 600

# Restore the fixture and look at it.
docker run -d --name "${BOOTDB}" --network "${NET}" -e POSTGRES_USER=keycloak \
    -e POSTGRES_PASSWORD=kc-test-pw -e POSTGRES_DB=keycloak "${PG_IMAGE}" >/dev/null
wait_pg "${BOOTDB}" keycloak
psql_in "${BOOTDB}" keycloak keycloak -c "CREATE DATABASE econ_graph" >/dev/null
docker exec -i "${BOOTDB}" pg_restore --no-owner --exit-on-error -U keycloak -d keycloak <"${OUT_SHARED}/keycloak.dump"
docker exec -i "${BOOTDB}" pg_restore --no-owner --exit-on-error -U keycloak -d econ_graph <"${OUT_SHARED}/econ_graph.dump"
q() { psql_in "${BOOTDB}" keycloak "$1" -At -c "$2"; }
check "users get user-<h>@example.invalid and 'User <h>' with the same h" \
    test "$(q econ_graph "select count(*) from users where email ~ '^user-[0-9a-f]{16}@example\.invalid$'
        and name = 'User ' || substring(email from 6 for 16)")" = 3
check "Keycloak's first and last name equal the app's name for the same address" \
    test "$(q keycloak "select string_agg(first_name || ' ' || last_name || '|' || email, ',' order by email)
        from user_entity where email is not null")" == \
         "$(q econ_graph "select string_agg(name || '|' || email, ',' order by email) from users")"
check "audit rows carry their account's pseudonym" \
    test "$(q econ_graph "select count(*) from audit_logs a join users u on u.id = a.user_id where a.user_name = u.name")" = 2
check "the same IP address gets the same stand-in" \
    test "$(q econ_graph "select count(distinct ip_address) from audit_logs")" = 2
check "the e-mail address in a URL becomes the same pseudonym, still encoded" \
    test "$(q econ_graph "select count(*) from chart_annotations a join users u on a.description like '%' || replace(u.email, '@', '%40') || '%'")" = 1
check "user_sessions is empty" test "$(q econ_graph "select count(*) from user_sessions")" = 0
check "the crawl URL keeps its other parameters" \
    test "$(q econ_graph "select count(*) from crawl_attempts where crawl_url like '%&api_key=REDACTED&file_type=json'")" = 5
check "crawl headers keep the harmless ones" \
    test "$(q econ_graph "select count(*) from crawl_attempts where request_headers = '{\"Accept\": \"application/json\"}'")" = 5
check "public content hashes are kept" \
    test "$(q econ_graph "select count(*) from series_fetch_validators where content_sha256 ~ '^[0-9a-f]{64}$'")" -gt 0
check "Keycloak keeps its realms, roles, clients, groups and role mappings" \
    test "$(kc_shape "${BOOTDB}" keycloak keycloak)" == "${KC_SHAPE}"
check "Keycloak's credentials, sessions, events and identity links are gone" \
    test "$(q keycloak "select (select count(*) from credential) + (select count(*) from offline_user_session)
        + (select count(*) from offline_client_session) + (select count(*) from event_entity)
        + (select count(*) from admin_event_entity) + (select count(*) from federated_identity)
        + (select count(*) from login_failure)")" = 0
check "client secrets are placeholders" \
    test "$(q keycloak "select count(*) from client where secret <> '' and secret <> 'fixture-client-secret'")" = 0
check "the service account keeps its name" \
    test "$(q keycloak "select count(*) from user_entity where username = 'service-account-econ-graph-batch'")" = 1

echo "== Keycloak on the sanitized keycloak.dump"
docker rm -fv "${KCDB}" >/dev/null
if BOOTURL="$(start_keycloak "${BOOT}" "${BOOTDB}" "${KC_IMAGE}" start-dev)"; then
    ok "Keycloak starts on the sanitized database"
    check "it serves the realm with new signing keys" \
        test "$(curl -sf "${BOOTURL}/realms/econ-graph/protocol/openid-connect/certs" | jq '[.keys[] | select(.use == "sig")] | length')" -ge 1
    check "the realm's roles are there" \
        test "$(q keycloak "select count(*) from keycloak_role r join realm m on m.id = r.realm_id where m.name = 'econ-graph' and r.name in ('user', 'support', 'admin', 'super_admin')")" = 4
else
    bad "Keycloak starts on the sanitized database"
fi
docker rm -fv "${BOOT}" >/dev/null 2>&1 || true

# ------------------------------------------------------------ idempotence

before="$(cd "${OUT_SHARED}" && sha256sum -- *)"
if run_build again --from-dir "${HOSTDIR}" --output-dir "${OUT_SHARED}"; then
    ok "a second run into the same directory exits 0"
else
    bad "a second run into the same directory exits 0"; cat "${LOG}/again.log"
fi
check "and says there is nothing to do" grep -q "already holds this fixture" "${LOG}/again.log"
check "and changes nothing" test "$(cd "${OUT_SHARED}" && sha256sum -- *)" == "${before}"
if run_build other --from-dir "${PVCDIR}" --output-dir "${OUT_SHARED}"; then
    bad "a different build into an existing fixture is refused"
else
    check "a different build into an existing fixture is refused" grep -q "already exists and is not this build" "${LOG}/other.log"
fi

# ------------------------------------------------------------ full build, separate

echo "== full build, backup-volume layout (APP_DATABASE=separate)"
mkdir -p "${W}/out"
if run_build full-separate --from-dir "${PVCDIR}" --output-dir "${W}/out/separate"; then
    ok "full build (separate) succeeds"
else
    bad "full build (separate) succeeds"; cat "${LOG}/full-separate.log"
fi
check "it holds the three dumps" \
    test -s "${W}/out/separate/econ_graph.dump" -a -s "${W}/out/separate/econ_graph_app.dump" -a -s "${W}/out/separate/keycloak.dump"
LEAKS="$(leaks_in "${W}/out/separate")" || LEAKS="the dumps could not be read"
check "no planted value is left in the separate fixture${LEAKS:+ (left: ${LEAKS//$'\n'/, })}" test -z "${LEAKS}"
check "the same key gives the same pseudonyms in both layouts" \
    test "$(manifest "${W}/out/separate" output.keycloak.sha256)" != "" -a \
         "$(dump_text "${W}/out/separate" | grep -o 'user-[0-9a-f]\{16\}@example.invalid' | sort -u)" == \
         "$(dump_text "${OUT_SHARED}" | grep -o 'user-[0-9a-f]\{16\}@example.invalid' | sort -u)"

# ------------------------------------------------------------ verification fails the run

echo "== leaving out each step"
while read -r step; do
    if FIXTURE_TEST_SKIP_STEPS="${step}" run_build "skip-${step}" --from-dir "${HOSTDIR}" --output-dir "${W}/out/skip-${step}"; then
        bad "without ${step} the run fails"
    elif ! grep -q "VERIFICATION FAILED" "${LOG}/skip-${step}.log"; then
        bad "without ${step} the run fails in verification"; tail -n 20 "${LOG}/skip-${step}.log"
    elif [[ -e "${W}/out/skip-${step}" ]]; then
        bad "without ${step} nothing is written"
    else
        ok "without ${step} verification fails: $(grep -m1 -o 'VERIFICATION FAILED.*' "${LOG}/skip-${step}.log")"
    fi
done <<<"${STEPS}"

if run_build leaky --data "${W}/backups/leaky-econ_graph.dump" --app "${PVCDIR}/econ_graph_app-${S1}.dump" \
    --keycloak "${PVCDIR}/keycloak-${S1}.dump" --output-dir "${W}/out/leaky"; then
    bad "an unknown secret fails the run"
else
    check "an unknown secret fails the run, naming where it is" \
        grep -q "VERIFICATION FAILED \[econ_graph\] FOUND: token in public.economic_series.description" "${LOG}/leaky.log"
    check "and nothing is written" test ! -e "${W}/out/leaky"
fi
check "no failed run leaves a container or scratch directory" no_scratch_left

# ------------------------------------------------------------ CI subset

echo "== CI subset"
if run_build subset --from-dir "${HOSTDIR}" --ci-subset --subset-series 30; then
    ok "--ci-subset succeeds"
else
    bad "--ci-subset succeeds"; cat "${LOG}/subset.log"
fi
OUT_SUB="$(find "${FIX}" -mindepth 1 -maxdepth 1 -type d -name '*-ci-subset' | head -n 1)"
check "it is under ~/econgraph-fixtures/<UTC stamp>-ci-subset" test -n "${OUT_SUB}"
SUB_KIB="$(manifest "${OUT_SUB}" total_kib)"
echo "     CI subset size: ${SUB_KIB} KiB ($(du -sh "${OUT_SUB}" | cut -f1) on disk)"
check "it reports its size" grep -q "fixture OK: .* (ci-subset, ${SUB_KIB} KiB)" "${LOG}/subset.log"
LEAKS="$(leaks_in "${OUT_SUB}")" || LEAKS="the dumps could not be read"
check "no planted value is left in the subset${LEAKS:+ (left: ${LEAKS//$'\n'/, })}" test -z "${LEAKS}"
psql_in "${BOOTDB}" keycloak keycloak -c "CREATE DATABASE subset" >/dev/null
docker exec -i "${BOOTDB}" pg_restore --no-owner --exit-on-error -U keycloak -d subset <"${OUT_SUB}/econ_graph.dump"
check "10 series from each of the 3 sources with series, none too long" \
    test "$(q subset "select string_agg(n::text, ',') from (select count(*) n from economic_series
        where external_id not like 'LONG-%' group by source_id) s")" = "10,10,10" -a \
         "$(q subset "select count(*) from economic_series")" = 30
check "every data point and vintage of the kept series" \
    test "$(q subset "select count(*) from data_points")" = $((30 * 48)) -a \
         "$(q subset "select count(distinct revision_date) from data_points")" = 2
check "every reference row: data sources and countries, and the datasets of kept series" \
    test "$(q subset "select (select count(*) from data_sources) || ',' || (select count(*) from countries) || ',' || (select count(*) from datasets)")" = \
         "$(q econ_graph "select (select count(*) from data_sources) || ',' || (select count(*) from countries)"),3"
check "series metadata and validators of the kept series only" \
    test "$(q subset "select (select count(*) from series_metadata) || ',' || (select count(*) from series_fetch_validators)")" = "30,30"
check "crawl state is emptied" \
    test "$(q subset "select (select count(*) from crawl_attempts) + (select count(*) from crawl_queue) + (select count(*) from reference_file_cache)")" = 0
check "the app tables are whole and sanitized" \
    test "$(q subset "select (select count(*) from users) || ',' || (select count(*) from audit_logs) || ',' || (select count(*) from user_sessions)")" = "3,3,0"
check "the subset carries the sanitized Keycloak dump" test -s "${OUT_SUB}/keycloak.dump"
# Reusing a real subset must still honor the caller's cap. No new Docker build
# is needed, and a refusal must leave the already published fixture untouched.
SUB_SUMS="$(cd "${OUT_SUB}" && sha256sum -- *.dump MANIFEST SHA256SUMS)"
if run_build subset-reuse-cap --from-dir "${HOSTDIR}" --ci-subset --subset-series 30 \
    --subset-max-kb 1 --output-dir "${OUT_SUB}"; then
    bad "an existing subset over a lower --subset-max-kb fails"
else
    check "an existing subset over a lower --subset-max-kb fails" \
        grep -q "over --subset-max-kb 1" "${LOG}/subset-reuse-cap.log"
fi
check "a cap refusal preserves the existing subset" \
    test "$(cd "${OUT_SUB}" && sha256sum -- *.dump MANIFEST SHA256SUMS)" = "${SUB_SUMS}"
if run_build subset-cap --from-dir "${HOSTDIR}" --ci-subset --subset-max-kb 1 --output-dir "${W}/out/subset-cap"; then
    bad "a subset over --subset-max-kb fails"
else
    check "a subset over --subset-max-kb fails" grep -q "over --subset-max-kb 1" "${LOG}/subset-cap.log"
    check "and nothing is written" test ! -e "${W}/out/subset-cap"
fi

check "no input dump or backup was changed, added or removed" inputs_unchanged
check "nothing is left behind" no_scratch_left

echo
echo "${PASS} passed, ${FAIL} failed"
[[ "${FAIL}" -eq 0 ]]
