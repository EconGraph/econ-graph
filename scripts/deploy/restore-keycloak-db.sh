#!/usr/bin/env bash
#
# Put Keycloak's database back to a dump taken by
# scripts/deploy/backup-keycloak-db.sh: the way back from a Keycloak upgrade,
# whose schema migration an older Keycloak can't run on.
#
# Usage:
#   scripts/deploy/restore-keycloak-db.sh --list
#   scripts/deploy/restore-keycloak-db.sh DUMP_DIR [--dry-run] [--yes]
#       [--allow-other-cluster] [--context CONTEXT]
#
# Encrypted bundles prompt for the maintainer's identity path at a terminal
# before contacting the cluster; their plaintext is disposable tmpfs only.
# A legacy plaintext verified directory remains supported.
#
#   DUMP_DIR               a directory under ~/econgraph-backups/keycloak/ (or
#                          ${ECONGRAPH_BACKUP_ROOT}/keycloak, or
#                          KEYCLOAK_BACKUP_ROOT) with a VERIFIED marker
#   --list                 list the verified dumps, newest last
#   --dry-run              check the dump and the cluster, print what would
#                          happen, change nothing
#   --yes                  don't ask; otherwise it asks at a terminal for the
#                          database name, and refuses without a terminal
#   --allow-other-cluster  restore a dump taken on another cluster (its VERIFIED
#                          names another kube-system UID, or none); refused
#                          without it
#   --context CONTEXT      the kubectl context (default kind-${KIND_CLUSTER},
#                          i.e. kind-econ-graph); the current context is not
#                          changed
#
# Steps:
#   1. Scale deployment/keycloak to 0 and wait for its pods to go.
#   2. Take a verified dump of the database as it is now, with
#      backup-keycloak-db.sh. If that fails, nothing else happens.
#   3. Restore DUMP_DIR/keycloak.dump into a new database
#      keycloak_restore_<time>, in one transaction, and check that every
#      table's row count equals DUMP_DIR/counts.tsv. On any failure up to here
#      `keycloak` is untouched and the new database is dropped.
#   4. In one transaction, end any connection to `keycloak`, rename it to
#      keycloak_pre_restore_<time> and rename the restored database to
#      `keycloak`. A fresh database, not `pg_restore --clean`: a newer Keycloak
#      adds tables that a clean restore would leave behind.
# keycloak_pre_restore_<time> is kept and never dropped by this script; drop it
# by hand once Keycloak works again. Keycloak stays at 0 replicas: deploy the
# Keycloak image the dump was taken with next (see "Rolling Keycloak back" in
# docs/release/v4.0.0-qa.md).
#
# Running it again restores the same dump again. No dump is ever deleted.
#
# Not scripts/backup/restore-dump.sh: its pg_restore --clean would leave behind
# the tables a newer Keycloak added.
#
# kubectl is the kind cluster's own binary and kubeconfig, as in deploy.sh
# (lib/kind-env.sh: KIND_CLUSTER, ECONGRAPH_DOCKER_HOST). KUBECTL, when set, is
# used instead, with the KUBECONFIG it finds.

set -euo pipefail

KDB_SCRIPT="restore-keycloak-db"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/deploy/lib/keycloak-db.sh
source "$PROJECT_ROOT/scripts/deploy/lib/keycloak-db.sh"

die() {
    echo "${KDB_SCRIPT}: error: $*" >&2
    exit 1
}

DRY_RUN=false
ASSUME_YES=false
LIST=false
ALLOW_OTHER_CLUSTER=false
CONTEXT="$KDB_DEFAULT_CONTEXT"
DUMP_DIR=""
original_args=("$@")
while [ $# -gt 0 ]; do
    case "$1" in
        --dry-run) DRY_RUN=true; shift ;;
        --yes | -y) ASSUME_YES=true; shift ;;
        --list) LIST=true; shift ;;
        --allow-other-cluster) ALLOW_OTHER_CLUSTER=true; shift ;;
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
if [[ -f "$DUMP_DIR/backup.tar.age" ]]; then
    opened_args=()
    for arg in "${original_args[@]}"; do
        if [[ "$arg" == "$DUMP_DIR" ]]; then opened_args+=("@BACKUP@"); else opened_args+=("$arg"); fi
    done
    exec "$PROJECT_ROOT/scripts/backup/open-backup.sh" "$DUMP_DIR" -- "$0" "${opened_args[@]}"
fi
dump="${DUMP_DIR}/${KDB_DATABASE}.dump"
counts="${DUMP_DIR}/counts.tsv"
[ -f "${DUMP_DIR}/VERIFIED" ] || die "${DUMP_DIR} has no VERIFIED marker; restore only a verified dump"
[ -s "$dump" ] || die "${dump} is missing or empty"
[ -s "$counts" ] || die "${counts} is missing or empty"
verified_value() { sed -n "s/^$1=//p" "${DUMP_DIR}/VERIFIED"; }
taken_with="$(verified_value keycloak_image)"
dump_uid="$(verified_value cluster_uid)"

kdb_use_context "$CONTEXT"
kdb_check_pod
live_uid="$(kdb_cluster_uid)" || die "could not read the cluster's kube-system UID on context ${CONTEXT}"
if [ "$dump_uid" != "$live_uid" ]; then
    if [ "$ALLOW_OTHER_CLUSTER" != true ]; then
        die "${DUMP_DIR} was taken on another cluster (kube-system UID ${dump_uid:-not recorded}; context ${CONTEXT} is ${live_uid}). Pass --allow-other-cluster to restore it here anyway"
    fi
    echo "${KDB_SCRIPT}: WARNING: ${DUMP_DIR} WAS TAKEN ON ANOTHER CLUSTER (kube-system UID ${dump_uid:-not recorded}, this one is ${live_uid}); restoring it here because of --allow-other-cluster" >&2
fi
cluster_role="a cluster not marked production"
if cluster_is_marked_production; then
    cluster_role="a cluster marked PRODUCTION"
fi
replicas="$(kdb_keycloak_replicas)"
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
stamp="${stamp,,}"
restore_db="keycloak_restore_${stamp}"
pre_restore_db="keycloak_pre_restore_${stamp}"

if [ "$DRY_RUN" = true ]; then
    echo "${KDB_SCRIPT}: dry run on context ${CONTEXT} (${cluster_role}), nothing changed:"
    echo "  would scale deployment/${KDB_DEPLOYMENT} in ${KDB_NAMESPACE} from ${replicas} to 0 and leave it there"
    echo "  would take a verified dump of the current database with backup-keycloak-db.sh"
    echo "  would restore ${dump} into ${restore_db} in ${KDB_POD}"
    echo "  and check its row counts against ${counts} ($(wc -l <"$counts") tables)"
    echo "  would rename ${KDB_DATABASE} to ${pre_restore_db} (kept) and ${restore_db} to ${KDB_DATABASE}"
    echo "  the dump was taken with Keycloak image ${taken_with:-not recorded}"
    exit 0
fi

if [ "$ASSUME_YES" != true ]; then
    [ -t 0 ] || die "no terminal to confirm on; pass --yes to restore without asking"
    echo "${KDB_SCRIPT}: this replaces database ${KDB_DATABASE} on context ${CONTEXT}, ${cluster_role},"
    echo "with ${dump} (after a verified dump of the current one). Type the database name to go on:"
    read -r answer
    [ "$answer" = "$KDB_DATABASE" ] || die "not confirmed; nothing changed"
fi

swapped=false
created=false
on_exit() {
    local status=$?
    if [ "$created" = true ] && [ "$swapped" != true ]; then
        kdb_drop_scratch "$restore_db" ||
            echo "${KDB_SCRIPT}: warning: could not drop ${restore_db}" >&2
        echo "${KDB_SCRIPT}: failed before the swap; ${KDB_DATABASE} is untouched" >&2
    fi
    exit "$status"
}
trap on_exit EXIT

kdb_stop_keycloak
"$PROJECT_ROOT/scripts/deploy/backup-keycloak-db.sh" --context "$CONTEXT"

echo "${KDB_SCRIPT}: restoring ${dump} into ${restore_db}"
kdb_exec createdb -U "$KDB_USER" -O "$KDB_USER" "$restore_db"
created=true
kdb_restore_and_compare "$restore_db" "$dump" "$counts"

echo "${KDB_SCRIPT}: swapping ${restore_db} in as ${KDB_DATABASE}; the current one becomes ${pre_restore_db}"
kdb_exec psql -U "$KDB_USER" -d postgres -v ON_ERROR_STOP=1 -X --single-transaction \
    -c "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname IN ('${KDB_DATABASE}', '${restore_db}') AND pid <> pg_backend_pid()" \
    -c "ALTER DATABASE ${KDB_DATABASE} RENAME TO ${pre_restore_db}" \
    -c "ALTER DATABASE ${restore_db} RENAME TO ${KDB_DATABASE}" >/dev/null
swapped=true

echo "${KDB_SCRIPT}: ${KDB_DATABASE} restored from ${DUMP_DIR}; row counts match"
echo "${KDB_SCRIPT}: the previous database is kept as ${pre_restore_db}; drop it by hand once Keycloak works"
echo "${KDB_SCRIPT}: deployment/${KDB_DEPLOYMENT} is at 0 replicas; deploy the Keycloak image the dump was taken with: ${taken_with:-see the VERIFIED file}"
