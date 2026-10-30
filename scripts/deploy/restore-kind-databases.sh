#!/bin/bash

# Restore a backup taken by backup-kind-databases.sh (or recreate-kind-cluster.sh)
# into postgresql-0 (econ_graph), app-postgres-0 (econ_graph_app) and
# keycloak-postgres-0 (keycloak), then check the row counts against the ones
# recorded at backup time (counts.env). Any mismatch is an error.
#
# First, unless --no-bootstrap, it makes sure the databases exist to restore
# into: namespace, the Secrets saved with the backup (the passwords the data
# was created with), the host-storage PVs and the Postgres StatefulSets. On a
# freshly created cluster that is everything before deploy.sh; on a running
# one it changes nothing (existing Secrets are kept, manifests re-applied).
#
# Idempotent and safe by default, per database:
#   - empty database (a fresh cluster)          -> restored
#   - row counts already match the backup        -> left alone
#   - anything else (it holds different data)    -> refused, unless --replace,
#     which drops and recreates every object in the dump (pg_restore --clean)
#     after a typed confirmation (or --yes)
# Each restore is one transaction, so a failure leaves the database as it was.
#
# --expect-series N / --expect-data-points N: also require counts.env to say
# exactly that before anything is restored (guards against the wrong backup).
#
# Secret values are never printed; only Secret names are.
#
# Usage:
#   scripts/deploy/restore-kind-databases.sh --from ~/econgraph-backups/<ts> \
#       [--expect-series N] [--expect-data-points N] \
#       [--only econ_graph|econ_graph_app|keycloak] [--replace] [--no-bootstrap] [--yes] [--dry-run]
# A dry run may create a temporary directory under $TMPDIR (a copy of the
# cluster's kubectl and kubeconfig), removed on exit; it changes nothing else.
#
# For restoring a nightly in-cluster backup (postgres-backup-data PVC) instead,
# use restore-postgres-backup.sh.

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/deploy/lib/kind-env.sh
source "${PROJECT_ROOT}/scripts/deploy/lib/kind-env.sh"

FROM=""
ONLY=""
REPLACE="false"
BOOTSTRAP="true"
EXPECT_SERIES=""
EXPECT_DATA_POINTS=""
usage="usage: $0 --from DIR [--expect-series N] [--expect-data-points N] [--only econ_graph|econ_graph_app|keycloak] [--replace] [--no-bootstrap] [--yes] [--dry-run]"
while [ $# -gt 0 ]; do
    case "$1" in
        --from) FROM="$2"; shift 2 ;;
        --only) ONLY="$2"; shift 2 ;;
        --expect-series) EXPECT_SERIES="$2"; shift 2 ;;
        --expect-data-points) EXPECT_DATA_POINTS="$2"; shift 2 ;;
        --replace) REPLACE="true"; shift ;;
        --no-bootstrap) BOOTSTRAP="false"; shift ;;
        --yes|-y) ASSUME_YES="true"; shift ;;
        --dry-run|-n) DRY_RUN="true"; shift ;;
        -h|--help) echo "$usage"; exit 0 ;;
        *) echo "$usage" >&2; exit 2 ;;
    esac
done
case "$ONLY" in
    "" | econ_graph | econ_graph_app | keycloak) ;;
    *) die "--only must be econ_graph, econ_graph_app or keycloak" ;;
esac
[ -n "$FROM" ] || die "--from DIR is required"
[ -d "$FROM" ] || die "$FROM is not a directory"
FROM="$(cd "$FROM" && pwd)"
FROM="$(backup_source_dir "$FROM")"
[ "${DRY_RUN}" = "true" ] && log "DRY RUN: checks run for real; nothing is changed"

# Fail loudly: a restore that doesn't reproduce the backup must not look like
# a success to whoever runs the next step.
fail_counts() {
    {
        echo
        echo "!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!"
        echo "!! RESTORE VERIFICATION FAILED"
        echo "!! $*"
        echo "!! Do not deploy on top of this. Nothing else was changed by this step."
        echo "!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!"
    } >&2
    exit 1
}

# wanted DB: true if DB is to be restored (no --only, or --only DB).
wanted() { [ -z "$ONLY" ] || [ "$ONLY" = "$1" ]; }

log "Checking backup ${FROM}"
read_counts "$FROM"
for db in econ_graph econ_graph_app keycloak; do
    wanted "$db" || continue
    f="$FROM/$db.dump"
    if [ -s "$f" ]; then
        # pg_dump -Fc archives start with "PGDMP".
        [ "$(head -c5 "$f")" = "PGDMP" ] || die "$f is not a pg_dump custom-format archive"
        echo "  $db.dump: $(du -h "$f" | cut -f1)"
    fi
done
[ -s "$FROM/econ_graph.dump" ] || [ -s "$FROM/econ_graph_app.dump" ] || [ -s "$FROM/keycloak.dump" ] ||
    die "no dumps in $FROM"
if [ -s "$FROM/econ_graph.dump" ] && wanted econ_graph; then
    [ -n "$SERIES_COUNT" ] && [ -n "$DATA_POINTS_COUNT" ] ||
        die "$FROM/counts.env lacks SERIES_COUNT/DATA_POINTS_COUNT; can't verify an econ_graph restore"
fi
# An app dump without counts is of an app database that had no tables yet:
# there is nothing in it to restore (backup_databases in lib/kind-env.sh).
if [ -s "$FROM/econ_graph_app.dump" ] && wanted econ_graph_app && [ -z "$APP_USER_COUNT" ]; then
    warn "$FROM/counts.env has no app row counts: the app database had no tables when it was dumped; not restoring it"
fi
if [ -s "$FROM/keycloak.dump" ] && wanted keycloak; then
    [ -n "$KEYCLOAK_REALM_COUNT" ] && [ -n "$KEYCLOAK_USER_COUNT" ] ||
        die "$FROM/counts.env lacks KEYCLOAK_REALM_COUNT/KEYCLOAK_USER_COUNT; can't verify a keycloak restore"
fi
echo "  counts.env: economic_series=${SERIES_COUNT:-?} data_points=${DATA_POINTS_COUNT:-?}" \
    "users=${APP_USER_COUNT:-?} chart_annotations=${APP_ANNOTATION_COUNT:-?}" \
    "realm=${KEYCLOAK_REALM_COUNT:-?} user_entity=${KEYCLOAK_USER_COUNT:-?}"
if [ -n "$EXPECT_SERIES" ] && [ "$EXPECT_SERIES" != "$SERIES_COUNT" ]; then
    fail_counts "counts.env has economic_series=${SERIES_COUNT}, expected ${EXPECT_SERIES} (--expect-series). Wrong backup?"
fi
if [ -n "$EXPECT_DATA_POINTS" ] && [ "$EXPECT_DATA_POINTS" != "$DATA_POINTS_COUNT" ]; then
    fail_counts "counts.env has data_points=${DATA_POINTS_COUNT}, expected ${EXPECT_DATA_POINTS} (--expect-data-points). Wrong backup?"
fi
[ -z "$EXPECT_SERIES$EXPECT_DATA_POINTS" ] || echo "  ok: counts.env matches --expect-series/--expect-data-points"
if [ -d "$FROM/secrets" ]; then
    echo "  saved Secrets: $(find "$FROM/secrets" -name '*.yaml' -printf '%f ' | sed 's/\.yaml//g')"
else
    warn "no secrets/ in $FROM: the restored data's passwords won't match newly generated Secrets"
fi

log "Docker daemon"
check_docker_daemon
if ! try_use_kind_kubectl; then
    [ "${DRY_RUN}" = "true" ] || die "cannot reach kind cluster '${KIND_CLUSTER}' on ${DOCKER_HOST}; run recreate-kind-cluster.sh first"
    echo "  (dry run) the cluster isn't reachable, so this is the plan, not checked against it:"
    [ "$BOOTSTRAP" = "true" ] && bootstrap_databases "${PROJECT_ROOT}" "$FROM"
    echo "+ pg_restore --single-transaction each dump into its (empty) database, then compare row counts with counts.env"
    log "Dry run complete; nothing was changed"
    exit 0
fi

if [ "$BOOTSTRAP" = "true" ]; then
    bootstrap_databases "${PROJECT_ROOT}" "$FROM"
fi

# restore_db DB POD CONTAINER USER MARKER_TABLE EXPECTED_COUNTS COUNT_SQL
restore_db() {
    local db="$1" pod="$2" container="$3" user="$4" marker="$5" expected="$6" sql="$7"
    local dump="$FROM/$db.dump" has_tables actual
    if [ ! -s "$dump" ]; then
        warn "no $db.dump in $FROM; skipping $db"
        return 0
    fi
    if ! pod_running "$pod"; then
        [ "${DRY_RUN}" = "true" ] || die "$pod is not running"
        echo "+ (once $pod runs) pg_restore --single-transaction -U $user -d $db < $dump; expect $expected"
        return 0
    fi
    wait_postgres_settled "$pod" "$container"

    has_tables="$(k exec "$pod" -c "$container" -- psql -U "$user" -d "$db" -Atc \
        "select to_regclass('public.$marker') is not null")"
    if [ "$has_tables" = "t" ]; then
        actual="$(k exec "$pod" -c "$container" -- psql -U "$user" -d "$db" -Atc "$sql" | paste -sd' ')"
        if [ "$actual" = "$expected" ]; then
            log "$db already matches the backup ($actual); not restoring"
            return 0
        fi
        if [ "$REPLACE" != "true" ]; then
            die "$db already holds data ($actual, backup has $expected). Pass --replace to overwrite it."
        fi
        confirm "This DROPS and recreates every object of $db in $pod from $dump."
    fi

    log "Restoring $db into $pod from $dump"
    if [ "${DRY_RUN}" = "true" ]; then
        echo "+ pg_restore --clean --if-exists --single-transaction --exit-on-error -U $user -d $db < $dump"
        echo "+ compare row counts with counts.env: expect $expected"
        return 0
    fi
    k exec -i "$pod" -c "$container" -- pg_restore -U "$user" -d "$db" \
        --clean --if-exists --single-transaction --exit-on-error <"$dump" ||
        fail_counts "pg_restore of $db failed; the transaction was rolled back"
    k exec "$pod" -c "$container" -- psql -U "$user" -d "$db" -qc "ANALYZE"

    actual="$(k exec "$pod" -c "$container" -- psql -U "$user" -d "$db" -Atc "$sql" | paste -sd' ')"
    [ "$actual" = "$expected" ] ||
        fail_counts "$db row counts after restore ($actual) differ from counts.env ($expected)"
    log "$db restored; row counts match counts.env ($actual)"
}

if wanted econ_graph; then
    restore_db econ_graph postgresql-0 postgresql postgres economic_series \
        "${SERIES_COUNT} ${DATA_POINTS_COUNT}" \
        "select count(*) from economic_series union all select count(*) from data_points"
fi
if wanted econ_graph_app && [ -n "$APP_USER_COUNT" ]; then
    restore_db econ_graph_app app-postgres-0 postgres postgres users \
        "${APP_USER_COUNT} ${APP_ANNOTATION_COUNT}" \
        "select count(*) from users union all select count(*) from chart_annotations"
fi
if wanted keycloak; then
    restore_db keycloak keycloak-postgres-0 postgres keycloak realm \
        "${KEYCLOAK_REALM_COUNT} ${KEYCLOAK_USER_COUNT}" \
        "select count(*) from realm union all select count(*) from user_entity"
fi
if [ "${DRY_RUN}" = "true" ]; then
    log "Dry run complete; nothing was changed"
else
    log "Restore verified: economic_series=${SERIES_COUNT:-n/a} data_points=${DATA_POINTS_COUNT:-n/a}" \
        "(app users=${APP_USER_COUNT:-n/a} chart_annotations=${APP_ANNOTATION_COUNT:-n/a})" \
        "(keycloak realm=${KEYCLOAK_REALM_COUNT:-n/a} user_entity=${KEYCLOAK_USER_COUNT:-n/a})"
fi
