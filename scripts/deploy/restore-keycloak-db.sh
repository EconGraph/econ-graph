#!/usr/bin/env bash
#
# Put Keycloak's database back to a dump taken by
# scripts/deploy/backup-keycloak-db.sh: the way back from a Keycloak upgrade,
# whose schema migration an older Keycloak can't run on.
#
# Usage:
#   scripts/deploy/restore-keycloak-db.sh --list [--context CONTEXT]
#   scripts/deploy/restore-keycloak-db.sh DUMP_DIR [--dry-run] [--yes] [--context CONTEXT]
#
#   DUMP_DIR           a directory under ~/econgraph-backups/keycloak/ (or
#                      KEYCLOAK_BACKUP_ROOT) with a VERIFIED marker
#   --list             list the verified dumps, newest last
#   --dry-run          check the dump and the cluster, print what would happen,
#                      change nothing
#   --yes              don't ask; otherwise it asks at a terminal for the
#                      database name, and refuses without a terminal
#   --context CONTEXT  the kubectl context (default kind-econ-graph); the
#                      current context is not changed
#
# Steps:
#   1. Scale deployment/keycloak to 0 and wait for its pods to go.
#   2. Take a verified dump of the database as it is now, with
#      backup-keycloak-db.sh, so the restore itself can be undone.
#   3. Drop the `keycloak` database and create it empty: a newer Keycloak adds
#      tables that `pg_restore --clean` would leave behind.
#   4. Restore DUMP_DIR/keycloak.dump into it and check that every table's row
#      count equals DUMP_DIR/counts.tsv.
# Keycloak stays at 0 replicas. Deploy the Keycloak image the dump was taken
# with next (see "Rolling Keycloak back" in docs/release/v4.0.0-qa.md).
#
# Running it again restores the same dump again. No dump is ever deleted.
# Other environment: KUBECTL (default kubectl).

set -euo pipefail

KDB_SCRIPT="restore-keycloak-db"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/deploy/lib/cluster-role.sh
source "$PROJECT_ROOT/scripts/deploy/lib/cluster-role.sh"
# shellcheck source=scripts/deploy/lib/keycloak-db.sh
source "$PROJECT_ROOT/scripts/deploy/lib/keycloak-db.sh"

die() {
    echo "${KDB_SCRIPT}: error: $*" >&2
    exit 1
}

DRY_RUN=false
ASSUME_YES=false
LIST=false
CONTEXT="kind-econ-graph"
DUMP_DIR=""
while [ $# -gt 0 ]; do
    case "$1" in
        --dry-run) DRY_RUN=true; shift ;;
        --yes | -y) ASSUME_YES=true; shift ;;
        --list) LIST=true; shift ;;
        --context)
            [ $# -ge 2 ] || { echo "${KDB_SCRIPT}: error: --context needs a value" >&2; exit 2; }
            CONTEXT="$2"; shift 2 ;;
        -h | --help) awk 'NR > 1 && /^#/ { sub(/^# ?/, ""); print; next } NR > 1 { exit }' "$0"; exit 0 ;;
        -*) echo "${KDB_SCRIPT}: error: unknown argument: $1" >&2; exit 2 ;;
        *)
            [ -z "$DUMP_DIR" ] || { echo "${KDB_SCRIPT}: error: one dump directory only" >&2; exit 2; }
            DUMP_DIR="$1"; shift ;;
    esac
done

if [ "$LIST" = true ]; then
    find "$KDB_BACKUP_ROOT" -mindepth 2 -maxdepth 2 -name VERIFIED -printf '%h\n' 2>/dev/null | sort
    exit 0
fi

[ -n "$DUMP_DIR" ] || { echo "${KDB_SCRIPT}: error: give the dump directory (--list shows them)" >&2; exit 2; }
dump="${DUMP_DIR}/${KDB_DATABASE}.dump"
counts="${DUMP_DIR}/counts.tsv"
[ -f "${DUMP_DIR}/VERIFIED" ] || die "${DUMP_DIR} has no VERIFIED marker; restore only a verified dump"
[ -s "$dump" ] || die "${dump} is missing or empty"
[ -s "$counts" ] || die "${counts} is missing or empty"

kdb_use_context "$CONTEXT"
kdb_check_pod
replicas="$(kdb_keycloak_replicas)"

if [ "$DRY_RUN" = true ]; then
    echo "${KDB_SCRIPT}: dry run on context ${CONTEXT}, nothing changed:"
    echo "  would scale deployment/${KDB_DEPLOYMENT} in ${KDB_NAMESPACE} from ${replicas} to 0 and leave it there"
    echo "  would take a verified dump of the current database with backup-keycloak-db.sh"
    echo "  would drop and recreate database ${KDB_DATABASE} in ${KDB_POD}, restore ${dump} into it"
    echo "  and check its row counts against ${counts} ($(wc -l <"$counts") tables)"
    echo "  the dump was taken with Keycloak image $(sed -n 's/^keycloak_image=//p' "${DUMP_DIR}/VERIFIED")"
    exit 0
fi

if [ "$ASSUME_YES" != true ]; then
    [ -t 0 ] || die "no terminal to confirm on; pass --yes to restore without asking"
    echo "${KDB_SCRIPT}: this replaces database ${KDB_DATABASE} on context ${CONTEXT} with ${dump}"
    echo "(after a verified dump of the current one). Type the database name to go on:"
    read -r answer
    [ "$answer" = "$KDB_DATABASE" ] || die "not confirmed; nothing changed"
fi

kdb_stop_keycloak
"$PROJECT_ROOT/scripts/deploy/backup-keycloak-db.sh" --context "$CONTEXT"

echo "${KDB_SCRIPT}: recreating ${KDB_DATABASE} and restoring ${dump}"
kdb_exec psql -U "$KDB_USER" -d postgres -v ON_ERROR_STOP=1 -X \
    -c "DROP DATABASE IF EXISTS ${KDB_DATABASE}" \
    -c "CREATE DATABASE ${KDB_DATABASE} OWNER ${KDB_USER}"
kdb_restore_and_compare "$KDB_DATABASE" "$dump" "$counts"
echo "${KDB_SCRIPT}: ${KDB_DATABASE} restored from ${DUMP_DIR}; row counts match"
taken_with="$(sed -n 's/^keycloak_image=//p' "${DUMP_DIR}/VERIFIED")"
echo "${KDB_SCRIPT}: deployment/${KDB_DEPLOYMENT} is at 0 replicas; deploy the Keycloak image the dump was taken with: ${taken_with:-see the VERIFIED file}"
