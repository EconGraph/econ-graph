#!/usr/bin/env bash
#
# Take a verified dump of Keycloak's database before a deploy that changes the
# Keycloak image. A new Keycloak version migrates the schema on its first start
# and an older one can't run on the migrated schema, so this dump is the way
# back (scripts/deploy/restore-keycloak-db.sh).
#
# Usage:
#   scripts/deploy/backup-keycloak-db.sh [--dry-run] [--context CONTEXT]
#
#   --dry-run          check the cluster and print what would happen; change
#                      nothing and write nothing
#   --context CONTEXT  the kubectl context (default kind-${KIND_CLUSTER}, i.e.
#                      kind-econ-graph, the one deploy.sh deploys to); the
#                      current context is not changed
#
# Steps:
#   1. Scale deployment/keycloak to 0 and wait for its pods to go, so the dump
#      is consistent. It stays at 0: the next scripts/deploy/deploy.sh brings it
#      back (or `kubectl -n econ-graph scale deployment/keycloak --replicas=1`).
#   2. pg_dump --format=custom of the `keycloak` database from the pod
#      keycloak-postgres-0 into ~/econgraph-backups/keycloak/<UTC time>/
#      (${ECONGRAPH_BACKUP_ROOT}/keycloak when that is set; KEYCLOAK_BACKUP_ROOT
#      overrides both), with exact row counts of every table, the Keycloak
#      image that wrote it and the cluster's identity (the kube-system
#      namespace UID), which restore-keycloak-db.sh checks. Directories are 0700 and files 0600: the
#      dump holds password hashes and client secrets.
#   3. Restore the dump into a scratch database in the same Postgres
#      (keycloak_verify_<time>), compare its row counts with the live ones,
#      then drop the scratch database.
#   4. Only then mark the directory VERIFIED and print its path. A failed run
#      keeps what it wrote as <UTC time>.failed.
#
# Each run adds a new directory; nothing is ever deleted.
#
# The nightly scripts/backup/dump-databases.sh also dumps `keycloak`, with
# Keycloak running and checked by a read-through only; that dump is restored
# with pg_restore --clean (scripts/backup/restore-dump.sh), which would leave
# behind the tables a newer Keycloak added. Before a Keycloak upgrade take this
# one: restore-keycloak-db.sh restores it.
#
# kubectl is the kind cluster's own binary and kubeconfig, as in deploy.sh
# (lib/kind-env.sh: KIND_CLUSTER, ECONGRAPH_DOCKER_HOST). KUBECTL, when set, is
# used instead, with the KUBECONFIG it finds.

set -euo pipefail

KDB_SCRIPT="backup-keycloak-db"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/deploy/lib/keycloak-db.sh
source "$PROJECT_ROOT/scripts/deploy/lib/keycloak-db.sh"

DRY_RUN=false
CONTEXT="$KDB_DEFAULT_CONTEXT"
while [ $# -gt 0 ]; do
    case "$1" in
        --dry-run) DRY_RUN=true; shift ;;
        --context)
            [ $# -ge 2 ] || { echo "${KDB_SCRIPT}: error: --context needs a value" >&2; exit 2; }
            CONTEXT="$2"; shift 2 ;;
        -h | --help) awk 'NR > 1 && /^#/ { sub(/^# ?/, ""); print; next } NR > 1 { exit }' "$0"; exit 0 ;;
        *) echo "${KDB_SCRIPT}: error: unknown argument: $1" >&2; exit 2 ;;
    esac
done

kdb_use_context "$CONTEXT"
kdb_check_pod
replicas="$(kdb_keycloak_replicas)"
image="$(kdb_keycloak_image)"
cluster_uid="$(kdb_cluster_uid)"
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
dest="${KDB_BACKUP_ROOT}/${stamp}"

if [ "$DRY_RUN" = true ]; then
    echo "${KDB_SCRIPT}: dry run on context ${CONTEXT}, nothing changed:"
    echo "  would scale deployment/${KDB_DEPLOYMENT} in ${KDB_NAMESPACE} from ${replicas} to 0 and leave it there"
    echo "  Keycloak image now: ${image}; cluster (kube-system UID): ${cluster_uid}"
    echo "  would dump database ${KDB_DATABASE} from ${KDB_POD} to ${dest}/${KDB_DATABASE}.dump"
    echo "  would verify it by restoring into keycloak_verify_${stamp,,} there and comparing row counts"
    exit 0
fi

umask 077
mkdir -p "$KDB_BACKUP_ROOT"
chmod 0700 "$KDB_BACKUP_ROOT"
# One directory per second: wait for a free name rather than reuse one.
while [ -e "$dest" ] || [ -e "${dest}.partial" ] || [ -e "${dest}.failed" ]; do
    sleep 1
    stamp="$(date -u +%Y%m%dT%H%M%SZ)"
    dest="${KDB_BACKUP_ROOT}/${stamp}"
done
work="${dest}.partial"
mkdir "$work"

scratch_db=""
on_exit() {
    local status=$?
    if [ -n "$scratch_db" ]; then
        kdb_drop_scratch "$scratch_db" ||
            echo "${KDB_SCRIPT}: warning: could not drop the scratch database ${scratch_db}" >&2
    fi
    if [ "$status" -ne 0 ] && [ -d "$work" ]; then
        mv "$work" "${dest}.failed"
        echo "${KDB_SCRIPT}: failed; what was written is in ${dest}.failed" >&2
    fi
    exit "$status"
}
trap on_exit EXIT

kdb_stop_keycloak

echo "${KDB_SCRIPT}: dumping ${KDB_DATABASE} from ${KDB_POD} (context ${CONTEXT})"
kdb_counts "$KDB_DATABASE" >"${work}/counts.tsv"
kdb_exec pg_dump -U "$KDB_USER" -d "$KDB_DATABASE" --format=custom >"${work}/${KDB_DATABASE}.dump"
[ -s "${work}/${KDB_DATABASE}.dump" ] || { echo "${KDB_SCRIPT}: error: the dump is empty" >&2; exit 1; }

# Lower case: an unquoted database name in SQL is folded to lower case.
scratch_db="keycloak_verify_${stamp,,}"
echo "${KDB_SCRIPT}: verifying by restoring into ${scratch_db}"
kdb_exec createdb -U "$KDB_USER" "$scratch_db"
kdb_restore_and_compare "$scratch_db" "${work}/${KDB_DATABASE}.dump" "${work}/counts.tsv"
kdb_drop_scratch "$scratch_db"
scratch_db=""

{
    echo "context=${CONTEXT}"
    echo "cluster_uid=${cluster_uid}"
    echo "keycloak_image=${image}"
    echo "database=${KDB_DATABASE}"
    echo "tables=$(wc -l <"${work}/counts.tsv")"
    echo "verified=restored into a scratch database with equal row counts"
} >"${work}/VERIFIED"
mv "$work" "$dest"
echo "${KDB_SCRIPT}: verified dump in ${dest}"
echo "${KDB_SCRIPT}: copy that directory to a second location, off this disk"
echo "${KDB_SCRIPT}: deployment/${KDB_DEPLOYMENT} is at 0 replicas; scripts/deploy/deploy.sh starts it again"
