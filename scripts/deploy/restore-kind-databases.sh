#!/bin/bash

# Restore a backup taken by backup-kind-databases.sh (or recreate-kind-cluster.sh)
# into the running postgresql-0 and keycloak-postgres-0, then check the row
# counts against the ones recorded at backup time.
#
# Idempotent and safe by default, per database:
#   - empty database (a fresh cluster)          -> restored
#   - row counts already match the backup        -> left alone
#   - anything else (it holds different data)    -> refused, unless --replace,
#     which drops and recreates every object in the dump (pg_restore --clean)
#     after a typed confirmation (or --yes)
# Each restore is one transaction, so a failure leaves the database as it was.
#
# Usage:
#   scripts/deploy/restore-kind-databases.sh --from ~/econgraph-backups/<ts> \
#       [--only econ_graph|keycloak] [--replace] [--yes] [--dry-run]
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
while [ $# -gt 0 ]; do
    case "$1" in
        --from) FROM="$2"; shift 2 ;;
        --only) ONLY="$2"; shift 2 ;;
        --replace) REPLACE="true"; shift ;;
        --yes|-y) ASSUME_YES="true"; shift ;;
        --dry-run|-n) DRY_RUN="true"; shift ;;
        *) echo "usage: $0 --from DIR [--only econ_graph|keycloak] [--replace] [--yes] [--dry-run]" >&2; exit 2 ;;
    esac
done
[ -n "$FROM" ] || die "--from DIR is required"
[ -s "$FROM/econ_graph.dump" ] || [ -s "$FROM/keycloak.dump" ] || die "no dumps in $FROM"
[ -s "$FROM/counts.env" ] || die "$FROM/counts.env missing"

SERIES_COUNT="" DATA_POINTS_COUNT="" KEYCLOAK_REALM_COUNT="" KEYCLOAK_USER_COUNT=""
# shellcheck source=/dev/null
source "$FROM/counts.env"

ensure_kind_kubectl

# restore_db DB POD CONTAINER USER MARKER_TABLE EXPECTED_COUNTS COUNT_SQL
restore_db() {
    local db="$1" pod="$2" container="$3" user="$4" marker="$5" expected="$6" sql="$7"
    local dump="$FROM/$db.dump" has_tables actual
    if [ ! -s "$dump" ]; then
        warn "no $db.dump in $FROM; skipping $db"
        return 0
    fi
    pod_running "$pod" || die "$pod is not running"
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
        return 0
    fi
    k exec -i "$pod" -c "$container" -- pg_restore -U "$user" -d "$db" \
        --clean --if-exists --single-transaction --exit-on-error <"$dump"
    k exec "$pod" -c "$container" -- psql -U "$user" -d "$db" -qc "ANALYZE"

    actual="$(k exec "$pod" -c "$container" -- psql -U "$user" -d "$db" -Atc "$sql" | paste -sd' ')"
    [ "$actual" = "$expected" ] || die "$db row counts after restore ($actual) differ from the backup ($expected)"
    log "$db restored; row counts match the backup ($actual)"
}

if [ -z "$ONLY" ] || [ "$ONLY" = "econ_graph" ]; then
    restore_db econ_graph postgresql-0 postgresql postgres economic_series \
        "${SERIES_COUNT} ${DATA_POINTS_COUNT}" \
        "select count(*) from economic_series union all select count(*) from data_points"
fi
if [ -z "$ONLY" ] || [ "$ONLY" = "keycloak" ]; then
    restore_db keycloak keycloak-postgres-0 postgres keycloak realm \
        "${KEYCLOAK_REALM_COUNT} ${KEYCLOAK_USER_COUNT}" \
        "select count(*) from realm union all select count(*) from user_entity"
fi
