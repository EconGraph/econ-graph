#!/bin/bash

# Move the live econ-graph deployment's databases onto the host btrfs
# subvolumes (docs/deployment/HOST_STORAGE.md), or roll that back.
#
# Prerequisite (sudo, done once by the owner): scripts/host/setup-subvolumes.sh,
# which mounts /srv/econgraph/{data-plane,app-prod,snapshots} owned by you.
#
# Forward (default):
#   1. preflight: host subvolumes mounted, pg/ and keycloak-pg/ empty
#   2. stop the writers (backend, crawler-worker, keycloak)        [downtime]
#   3. pg_dump both databases + row counts + Secrets to ~/econgraph-backups/<ts>/
#   4. delete and recreate the kind cluster with the host mounts   [DESTRUCTIVE]
#   5. recreate the Secrets, the static PVs and both Postgres StatefulSets
#   6. restore both dumps, check row counts
#   7. run deploy.sh for everything else
#   8. verify: row counts, PVs bound to the host paths, file ownership
#
# Idempotent: re-running after a failure picks up where it stopped (the backup
# directory in use is remembered in ~/.cache/econgraph/host-storage-migration.env;
# override with --resume-from DIR). Once migrated, a re-run only verifies.
#
# Rollback (--rollback): back up the current databases, recreate the cluster
# from the kind config and manifests of --ref (default origin/release/v4.0,
# i.e. node-local storage as before), restore, deploy that ref. The host
# subvolume contents are left untouched.
#
# Usage:
#   scripts/deploy/migrate-to-host-storage.sh [--dry-run] [--yes] [--resume-from DIR]
#   scripts/deploy/migrate-to-host-storage.sh --rollback [--ref REF] [--resume-from DIR] [--dry-run] [--yes]
#
# --dry-run runs every check and prints every step without changing anything.
# --yes answers the confirmation prompts (downtime, cluster delete).

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/deploy/lib/kind-env.sh
source "${PROJECT_ROOT}/scripts/deploy/lib/kind-env.sh"

MODE="forward"
REF="origin/release/v4.0"
RESUME_FROM=""
while [ $# -gt 0 ]; do
    case "$1" in
        --dry-run|-n) DRY_RUN="true"; shift ;;
        --yes|-y) ASSUME_YES="true"; shift ;;
        --resume-from) RESUME_FROM="$2"; shift 2 ;;
        --rollback) MODE="rollback"; shift ;;
        --ref) REF="$2"; shift 2 ;;
        -h|--help) sed -n '3,36p' "$0"; exit 0 ;;
        *) echo "unknown argument: $1 (see --help)" >&2; exit 2 ;;
    esac
done
export DRY_RUN ASSUME_YES

STATE_FILE="${ECONGRAPH_CACHE}/host-storage-migration.env"
WRITERS=(deployment/crawler-worker deployment/econ-graph-backend deployment/keycloak)

common_flags=()
[ "${DRY_RUN}" = "true" ] && common_flags+=(--dry-run)

for tool in kind docker yq git mountpoint findmnt; do
    command -v "$tool" >/dev/null 2>&1 || die "$tool not found"
done

# recreate-kind-cluster.sh has its own --dry-run, so it runs (and checks the
# kind config and host mounts) even in a dry run; common_flags passes it on.
recreate_cluster() {
    echo "+ scripts/deploy/recreate-kind-cluster.sh $*"
    "${PROJECT_ROOT}/scripts/deploy/recreate-kind-cluster.sh" "$@" 2>&1 | sed 's/^/    /'
    return "${PIPESTATUS[0]}"
}

remember_backup() {
    BACKUP_DIR="$1"
    if [ "${DRY_RUN}" != "true" ]; then
        mkdir -p "${ECONGRAPH_CACHE}"
        echo "BACKUP_DIR=$1" >"${STATE_FILE}"
    fi
}

recalled_backup() {
    if [ -n "${RESUME_FROM}" ]; then
        echo "${RESUME_FROM}"
    elif [ -f "${STATE_FILE}" ]; then
        sed -n 's/^BACKUP_DIR=//p' "${STATE_FILE}"
    fi
}

# True when the dir is absent or empty. Once Postgres has used it, it belongs
# to host uid 100998 and can't be listed as you, which also means "not empty".
host_dir_empty() {
    local dir="$1"
    [ ! -e "$dir" ] && return 0
    [ "$(stat -c %u "$dir")" = "$(id -u)" ] || return 1
    [ -z "$(ls -A "$dir")" ]
}

db_has_data() {
    pod_running postgresql-0 &&
        [ "$(k exec postgresql-0 -c postgresql -- psql -U postgres -d econ_graph -Atc \
            "select to_regclass('public.economic_series') is not null" 2>/dev/null)" = "t" ]
}

stop_writers() {
    log "Stopping writers: ${WRITERS[*]}"
    local w
    for w in "${WRITERS[@]}"; do
        if [ -n "$(k get "$w" -o name --ignore-not-found)" ]; then
            run k scale "$w" --replicas=0
        fi
    done
    if [ "${DRY_RUN}" != "true" ]; then
        # No writer pods left means nothing can change the data after the dump.
        k wait --for=delete pod -l 'app in (econ-graph-backend,crawler-worker,keycloak)' --timeout=300s ||
            die "writer pods are still running; not taking the backup"
    fi
}

# bootstrap SRC_ROOT: Secrets, storage and the two Postgres StatefulSets from
# the manifests in SRC_ROOT, so the dumps are restored before any app
# (backend migrations, Keycloak) starts against an empty database.
bootstrap_databases() {
    local src="$1"
    log "Creating namespace, Secrets and databases from ${src}"
    if [ "${DRY_RUN}" = "true" ]; then
        echo "+ kubectl apply -f $src/k8s/manifests/namespace.yaml"
        echo "+ create Secrets from ${BACKUP_DIR}/secrets/ that don't exist yet"
        [ -f "$src/k8s/manifests/host-storage.yaml" ] && echo "+ scripts/deploy/apply-host-storage.sh"
        echo "+ kubectl apply postgres-init, postgres-deployment, postgres; wait"
        echo "+ kubectl apply keycloak/postgres; wait"
        return 0
    fi
    kubectl apply -f "$src/k8s/manifests/namespace.yaml"
    restore_secrets "${BACKUP_DIR}"
    "${PROJECT_ROOT}/scripts/deploy/create-secrets.sh"
    if [ -f "$src/k8s/manifests/host-storage.yaml" ]; then
        "${PROJECT_ROOT}/scripts/deploy/apply-host-storage.sh"
    fi
    kubectl apply -f "$src/k8s/manifests/postgres-init.yaml"
    kubectl apply -f "$src/k8s/manifests/postgres-deployment.yaml"
    kubectl apply -f "$src/k8s/manifests/postgres.yaml"
    k rollout status statefulset/postgresql --timeout=600s
    wait_postgres_settled postgresql-0 postgresql
    if [ -s "${BACKUP_DIR}/keycloak.dump" ]; then
        kubectl apply -f "$src/k8s/manifests/keycloak/postgres.yaml"
        k rollout status statefulset/keycloak-postgres --timeout=600s
        wait_postgres_settled keycloak-postgres-0 postgres
    fi
}

restore_and_deploy() {
    local src="$1"
    run "${PROJECT_ROOT}/scripts/deploy/restore-kind-databases.sh" --from "${BACKUP_DIR}" "${common_flags[@]}"
    log "Deploying the rest of the stack with $src/scripts/deploy/deploy.sh"
    if [ "${DRY_RUN}" = "true" ]; then
        echo "+ (cd $src && scripts/deploy/deploy.sh)"
    else
        (cd "$src" && ./scripts/deploy/deploy.sh)
    fi
}

verify() {
    log "Verifying"
    if [ "${DRY_RUN}" = "true" ]; then
        echo "+ compare row counts with ${BACKUP_DIR:-the backup}/counts.env; check PVs, pods, ownership"
        return 0
    fi
    local SERIES_COUNT=0 DATA_POINTS_COUNT=0 series points ok=true
    if [ -n "${BACKUP_DIR:-}" ] && [ -f "${BACKUP_DIR}/counts.env" ]; then
        # shellcheck source=/dev/null
        source "${BACKUP_DIR}/counts.env"
    fi
    series="$(count_rows econ_graph 'select count(*) from economic_series')"
    points="$(count_rows econ_graph 'select count(*) from data_points')"
    echo "  economic_series: $series (backup: ${SERIES_COUNT})"
    echo "  data_points:     $points (backup: ${DATA_POINTS_COUNT})"
    # The crawler may add rows after the restore, never remove them.
    [ "$series" -ge "${SERIES_COUNT}" ] && [ "$points" -ge "${DATA_POINTS_COUNT}" ] || ok=false
    if pod_running keycloak-postgres-0; then
        echo "  keycloak realms: $(count_rows keycloak 'select count(*) from realm')"
    fi
    kubectl get pv -o custom-columns='PV:.metadata.name,STATUS:.status.phase,CLAIM:.spec.claimRef.name,PATH:.spec.hostPath.path,POLICY:.spec.persistentVolumeReclaimPolicy' |
        sed 's/^/  /'
    if [ "${MODE}" = "forward" ]; then
        local pv
        for pv in econgraph-data-plane-pg econgraph-app-prod-keycloak-pg; do
            [ "$(kubectl get pv "$pv" -o jsonpath='{.status.phase}')" = "Bound" ] || {
                echo "  $pv is not Bound" >&2
                ok=false
            }
        done
        echo "  host ownership (expect 100998 = pod uid 999 under rootless Docker):"
        stat -c '    %n %u:%g %A' "${HOST_PG_DIR}" "${HOST_KEYCLOAK_PG_DIR}"
        [ "$(stat -c %u "${HOST_PG_DIR}")" = "100998" ] || ok=false
    fi
    k get pods -o wide | sed 's/^/  /'
    [ "$ok" = true ] || die "verification failed"
    log "Verified"
}

# --- forward -------------------------------------------------------------------

forward() {
    log "Preflight"
    check_host_storage_on_host || die "host subvolumes are not ready"
    echo "  ok: ${HOST_DATA_PLANE} and ${HOST_APP_PROD} are mounted btrfs subvolumes owned by $(id -un)"

    local cluster="absent" new_layout="false"
    if kind_cluster_exists; then
        cluster="present"
        ensure_kind_kubectl
        check_host_storage_in_node && new_layout="true"
    fi
    echo "  cluster: ${cluster}; host mounts in node: ${new_layout}"

    if [ "$cluster" = "present" ] && [ "$new_layout" = "true" ] && db_has_data &&
        [ "$(kubectl get pv econgraph-data-plane-pg -o jsonpath='{.status.phase}' --ignore-not-found)" = "Bound" ] &&
        [ -z "$(recalled_backup)" ]; then
        log "Already migrated; only verifying"
        verify
        return 0
    fi

    if [ "$cluster" = "present" ] && [ "$new_layout" = "false" ]; then
        # The original cluster, data still inside its node.
        for d in "${HOST_PG_DIR}" "${HOST_KEYCLOAK_PG_DIR}"; do
            host_dir_empty "$d" || die "$d is not empty. Move it aside first (mv $d $d.old-\$(date -u +%Y%m%dT%H%MZ)); nothing was changed."
        done
        db_has_data || die "postgresql-0 is not running or econ_graph is empty; refusing to back up and delete"
        echo "  live data: economic_series=$(count_rows econ_graph 'select count(*) from economic_series')" \
            "data_points=$(count_rows econ_graph 'select count(*) from data_points')"
        confirm "This stops econ-graph (downtime starts), backs up both databases, then DELETES and recreates kind cluster '${KIND_CLUSTER}'."
        stop_writers
        remember_backup "$(new_backup_dir)"
        backup_databases "${BACKUP_DIR}"
        recreate_cluster --yes --no-backup "${common_flags[@]}"
    else
        BACKUP_DIR="$(recalled_backup)"
        [ -n "${BACKUP_DIR}" ] || die "no backup to resume from; pass --resume-from ~/econgraph-backups/<ts>"
        [ -s "${BACKUP_DIR}/econ_graph.dump" ] || die "${BACKUP_DIR}/econ_graph.dump missing"
        log "Resuming with backup ${BACKUP_DIR}"
        if [ "$cluster" = "absent" ]; then
            recreate_cluster --yes --no-backup "${common_flags[@]}"
        fi
    fi

    if [ "${DRY_RUN}" != "true" ]; then use_kind_kubectl; fi
    bootstrap_databases "${PROJECT_ROOT}"
    restore_and_deploy "${PROJECT_ROOT}"
    verify
    if [ "${DRY_RUN}" != "true" ]; then rm -f "${STATE_FILE}"; fi
    if [ "${DRY_RUN}" = "true" ]; then
        log "Dry run complete; nothing was changed"
    else
        log "Migration complete. Backup kept at ${BACKUP_DIR}"
    fi
}

# --- rollback ------------------------------------------------------------------

rollback() {
    local src="${ECONGRAPH_CACHE}/rollback-src"
    git -C "${PROJECT_ROOT}" rev-parse --verify -q "${REF}^{commit}" >/dev/null || die "unknown ref ${REF}"

    if kind_cluster_exists; then
        ensure_kind_kubectl
    fi
    if [ -n "${RESUME_FROM}" ]; then
        BACKUP_DIR="${RESUME_FROM}"
    elif kind_cluster_exists && db_has_data; then
        confirm "This stops econ-graph, backs up both databases, then DELETES kind cluster '${KIND_CLUSTER}' and redeploys ${REF} with node-local storage."
        stop_writers
        remember_backup "$(new_backup_dir)"
        backup_databases "${BACKUP_DIR}"
    else
        BACKUP_DIR="$(recalled_backup)"
        [ -n "${BACKUP_DIR}" ] || die "no running database to back up; pass --resume-from ~/econgraph-backups/<ts>"
    fi
    [ "${DRY_RUN}" = "true" ] || [ -s "${BACKUP_DIR}/econ_graph.dump" ] || die "${BACKUP_DIR}/econ_graph.dump missing"
    log "Rolling back to ${REF} with backup ${BACKUP_DIR}"

    if [ -d "$src" ]; then
        run git -C "$src" checkout -q --detach "${REF}"
    else
        run git -C "${PROJECT_ROOT}" worktree add -q --detach "$src" "${REF}"
    fi
    local config="$src/terraform/k8s/kind-config.yaml"
    if [ "${DRY_RUN}" = "true" ] && [ ! -d "$src" ]; then
        config="${ECONGRAPH_CACHE}/rollback-kind-config.yaml"
        mkdir -p "${ECONGRAPH_CACHE}"
        git -C "${PROJECT_ROOT}" show "${REF}:terraform/k8s/kind-config.yaml" >"$config"
    fi
    # A rollback always replaces the cluster; the backup above (or the one
    # given) is what gets restored.
    if kind_cluster_exists; then
        confirm "About to DELETE kind cluster '${KIND_CLUSTER}' for the rollback."
    fi
    recreate_cluster --yes --no-backup --config "$config" "${common_flags[@]}"
    if [ "${DRY_RUN}" != "true" ]; then use_kind_kubectl; fi
    bootstrap_databases "$src"
    restore_and_deploy "$src"
    verify
    if [ "${DRY_RUN}" != "true" ]; then rm -f "${STATE_FILE}"; fi
    log "Rollback complete. Host subvolume contents were not touched."
}

if [ "${DRY_RUN}" = "true" ]; then
    log "DRY RUN: checks run for real; nothing is changed"
fi
"${MODE}"
