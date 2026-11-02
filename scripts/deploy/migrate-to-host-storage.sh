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
#      and load the same locally built images (checked by ID) into it
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
#       [--confirm-production UID-PREFIX]
#   scripts/deploy/migrate-to-host-storage.sh --rollback [--ref REF] [--resume-from DIR] [--dry-run] [--yes]
#       [--confirm-production UID-PREFIX]
#
# --dry-run runs every check and prints every step without changing anything.
# A dry run may create a temporary directory under $TMPDIR (a copy of the
# cluster's kubectl and kubeconfig), removed on exit; it changes nothing else.
# --yes answers the confirmation prompts (downtime, cluster delete), except
# the one recreate-kind-cluster.sh asks before it deletes a cluster with a
# production marker (or one whose marker can't be read): the first 8
# characters of the kube-system UID, given as --confirm-production or typed
# at a terminal. It is asked before the downtime starts; without either, the
# script stops there.

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/deploy/lib/kind-env.sh
source "${PROJECT_ROOT}/scripts/deploy/lib/kind-env.sh"

MODE="forward"
REF="origin/release/v4.0"
RESUME_FROM=""
CONFIRM_PRODUCTION=""
while [ $# -gt 0 ]; do
    case "$1" in
        --dry-run|-n) DRY_RUN="true"; shift ;;
        --yes|-y) ASSUME_YES="true"; shift ;;
        --resume-from) RESUME_FROM="$2"; shift 2 ;;
        --rollback) MODE="rollback"; shift ;;
        --ref) REF="$2"; shift 2 ;;
        --confirm-production)
            [ $# -ge 2 ] && [ -n "$2" ] || { echo "--confirm-production needs a UID prefix (see --help)" >&2; exit 2; }
            CONFIRM_PRODUCTION="$2"; shift 2 ;;
        -h|--help) awk 'NR < 3 { next } /^#/ { print; next } { exit }' "$0"; exit 0 ;;
        *) echo "unknown argument: $1 (see --help)" >&2; exit 2 ;;
    esac
done
# open-backup.sh substitutes a fresh tmpfs path for @BACKUP@. Persist the
# original bundle path so repeated opens still match RESTORED= state.
RESUME_FROM="$(backup_original_dir "$RESUME_FROM")"
export DRY_RUN ASSUME_YES

STATE_FILE="${ECONGRAPH_CACHE}/host-storage-migration.env"
WRITERS=(deployment/crawler-worker deployment/econ-graph-backend deployment/keycloak)

common_flags=()
[ "${DRY_RUN}" = "true" ] && common_flags+=(--dry-run)
# Passed only to recreate-kind-cluster.sh, which checks it against the cluster.
recreate_flags=()
[ -n "${CONFIRM_PRODUCTION}" ] && recreate_flags+=(--confirm-production "${CONFIRM_PRODUCTION}")

for tool in docker yq git mountpoint findmnt; do
    command -v "$tool" >/dev/null 2>&1 || die "$tool not found"
done
log "Docker daemon"
check_docker_daemon
# The cluster is recreated with the pinned kind (scripts/deploy/install-kind.sh).
require_kind_version

# recreate-kind-cluster.sh has its own --dry-run, so it runs (and checks the
# kind config and host mounts) even in a dry run; common_flags passes it on.
recreate_cluster() {
    # A new cluster holds none of the restored data, so forget any earlier
    # restore before the old cluster can be deleted; otherwise a resumed run
    # would skip the restore into the new, empty databases. Cleared first,
    # not on success, so a recreate that fails part-way also forgets it.
    # (restore-kind-databases.sh leaves a database alone when its counts
    # already match the backup, so restoring into the old cluster again is
    # harmless.)
    forget_restore
    echo "+ scripts/deploy/recreate-kind-cluster.sh $*"
    "${PROJECT_ROOT}/scripts/deploy/recreate-kind-cluster.sh" "$@" 2>&1 | sed 's/^/    /'
    return "${PIPESTATUS[0]}"
}

forget_restore() {
    if [ "${DRY_RUN}" != "true" ] && [ -f "${STATE_FILE}" ]; then
        sed -i '/^RESTORED=/d' "${STATE_FILE}"
    fi
}

remember_backup() {
    BACKUP_DIR="$1"
    if [ "${DRY_RUN}" != "true" ]; then
        mkdir -p "${ECONGRAPH_CACHE}"
        echo "BACKUP_DIR=$1" >"${STATE_FILE}"
    fi
}

# The production marker (scripts/deploy/lib/cluster-role.sh) is bound to the
# cluster's kube-system UID, so the recreated cluster is not production until
# it is marked again, and restore_secrets leaves the saved tunnel token out of
# a cluster that is not. So: note before the delete whether the old cluster was
# production (kept in the state file for a resumed run), and have the new one
# marked before anything is restored into it.
WAS_PRODUCTION="false"
note_production() {
    local rc=0
    kind_cluster_is_production || rc=$?
    case "$rc" in
        0)
            WAS_PRODUCTION="true"
            log "The cluster is marked production; the new one will be marked before the restore"
            ;;
        1) ;;
        *)
            # Unknown is not "not production": the new cluster would then not be
            # marked, and production's tunnel token would be left out of it.
            die "could not tell whether kind cluster '${KIND_CLUSTER}' is production: ${CLUSTER_ROLE_REASON}. Nothing was deleted."
            ;;
    esac
}

# recreate-kind-cluster.sh asks for the kube-system UID prefix before deleting
# a cluster with a production marker (any ConfigMap
# kube-system/econgraph-cluster-role except role exactly `none`; see
# cluster_role_marker_guards_delete) or one whose marker can't be read, and
# --yes does not answer it. Get it here, before any
# confirmation that starts the downtime: from --confirm-production, which must
# match this cluster, or typed at a terminal; without either, stop. It is then
# passed on as --confirm-production, so recreate-kind-cluster.sh, which checks
# it against the cluster again, asks nothing once the writers are stopped.
# Call after ensure_kind_kubectl and note_production.
require_recreate_confirmation() {
    local uid answer
    [ "${DRY_RUN}" != "true" ] || return 0
    # The kind kubectl on its own context, as kind_cluster_is_production asks.
    local -x KUBECTL="${ECONGRAPH_KUBE_DIR:-${ECONGRAPH_CACHE}}/bin/kubectl"
    local -x CLUSTER_ROLE_CONTEXT="kind-${KIND_CLUSTER}"
    if [ -z "${CONFIRM_PRODUCTION}" ]; then
        # A marker that can't be read asks, as a production marker does.
        if [ "${WAS_PRODUCTION}" != "true" ] && cluster_role_read_marker kc &&
            ! cluster_role_marker_guards_delete "${CLUSTER_ROLE_MARKER_EXISTS}" \
                "${CLUSTER_ROLE_MARKER_ROLE}" "${CLUSTER_ROLE_MARKER_UID}"; then
            return 0
        fi
    fi
    uid="$(kube_system_uid)" ||
        die "could not read the kube-system UID of kind cluster '${KIND_CLUSTER}'; nothing was changed"
    if [ -n "${CONFIRM_PRODUCTION}" ]; then
        [ "${CONFIRM_PRODUCTION}" = "${uid:0:8}" ] ||
            die "--confirm-production ${CONFIRM_PRODUCTION} does not match kind cluster '${KIND_CLUSTER}' (kube-system UID ${uid}); nothing was changed"
        return 0
    fi
    [ -t 0 ] ||
        die "kind cluster '${KIND_CLUSTER}' has a production marker (or it can't be read): recreating it needs the first 8 characters of its kube-system UID, and there is no terminal to type them on. Pass --confirm-production UID-PREFIX. Nothing was changed."
    echo "!!! kind cluster '${KIND_CLUSTER}' (kube-system UID ${uid}) has a production marker, or it can't be read."
    echo "    It is deleted and recreated after the downtime starts; --yes does not answer this."
    echo "Type the first 8 characters of the kube-system UID above to go on:"
    read -r answer || answer=""
    [ "${answer}" = "${uid:0:8}" ] || die "confirmation did not match; nothing was changed"
    CONFIRM_PRODUCTION="${answer}"
    recreate_flags=(--confirm-production "${answer}")
}

remember_production() {
    if [ "${WAS_PRODUCTION}" = "true" ] && [ "${DRY_RUN}" != "true" ]; then
        echo "WAS_PRODUCTION=true" >>"${STATE_FILE}"
    fi
}

mark_if_was_production() {
    if [ "${WAS_PRODUCTION}" != "true" ] && ! { [ -f "${STATE_FILE}" ] && grep -qx 'WAS_PRODUCTION=true' "${STATE_FILE}"; }; then
        return 0
    fi
    if [ "${DRY_RUN}" = "true" ]; then
        echo "+ scripts/deploy/mark-production-cluster.sh (the old cluster was production; asks for a typed confirmation)"
        return 0
    fi
    kind_cluster_is_production && return 0
    log "The old cluster was production: marking the new one before the restore, so its tunnel token is restored too"
    "${PROJECT_ROOT}/scripts/deploy/mark-production-cluster.sh" ||
        die "the new cluster is not marked production. Run scripts/deploy/mark-production-cluster.sh, then re-run this script; it resumes here."
}

recalled_backup() {
    if [ -n "${RESUME_FROM}" ]; then
        echo "${RESUME_FROM}"
    elif [ -f "${STATE_FILE}" ]; then
        sed -n 's/^BACKUP_DIR=//p' "${STATE_FILE}"
    fi
}

# True when the dir is absent or empty. Once Postgres has used it, it belongs
# to host uid ${HOST_POSTGRES_UID} and can't be listed as you, which also means
# "not empty".
host_dir_empty() {
    local dir="$1"
    [ ! -e "$dir" ] && return 0
    [ "$(stat -c %u "$dir")" = "$(id -u)" ] || return 1
    [ -z "$(ls -A "$dir")" ]
}

# Locally built images (econ-graph-*, no registry) exist only in the old
# cluster's nodes and the host's Docker; a new cluster has none of them, and
# rebuilding from this checkout would change what runs. So record exactly the
# images running now, check the host holds the same ones by ID, and load those
# into the new cluster (load-kind-images.sh). The images are looked up in
# ECONGRAPH_IMAGE_DOCKER_HOST (default: the cluster's daemon); images built
# before the move to rootful Docker are in the rootless daemon.
IMAGE_DOCKER_HOST="${ECONGRAPH_IMAGE_DOCKER_HOST:-${DOCKER_HOST}}"
LOCAL_IMAGES=()

check_local_images() {
    local image host_id node_id
    mapfile -t LOCAL_IMAGES < <(k get deploy,sts -o jsonpath='{range .items[*]}{range .spec.template.spec.containers[*]}{.image}{"\n"}{end}{end}' |
        grep -E '^econ-graph-' | sort -u)
    [ "${#LOCAL_IMAGES[@]}" -gt 0 ] || die "found no locally built econ-graph-* images in namespace ${NAMESPACE}"
    for image in "${LOCAL_IMAGES[@]}"; do
        host_id="$(docker -H "${IMAGE_DOCKER_HOST}" image inspect --format '{{.Id}}' "$image" 2>/dev/null)" ||
            die "$image runs in the cluster but is not in ${IMAGE_DOCKER_HOST}; nothing was changed"
        node_id="$(docker exec "${KIND_CLUSTER}-worker" crictl inspecti "docker.io/library/$image" 2>/dev/null |
            sed -n 's/.*"id": "\(sha256:[0-9a-f]*\)".*/\1/p' | head -1)"
        [ "$node_id" = "$host_id" ] ||
            die "$image in ${IMAGE_DOCKER_HOST} (${host_id}) is not the one running in the cluster (${node_id:-not found}); nothing was changed"
        echo "  image: $image ${host_id:7:12}"
    done
}

save_local_images() {
    if [ "${DRY_RUN}" != "true" ]; then
        printf '%s\n' "${LOCAL_IMAGES[@]}" >"$1/local-images.txt"
    fi
}

# Called by backup_databases after verification and before age publication.
backup_prepare_payload() { save_local_images "$1"; }

load_local_images() {
    local image
    if [ "${#LOCAL_IMAGES[@]}" -eq 0 ] && [ -s "$(backup_source_dir "$BACKUP_DIR")/local-images.txt" ]; then
        mapfile -t LOCAL_IMAGES <"$(backup_source_dir "$BACKUP_DIR")/local-images.txt"
    fi
    [ "${#LOCAL_IMAGES[@]}" -gt 0 ] || die "${BACKUP_DIR}/local-images.txt missing; can't tell which images to load"
    log "Loading the images that ran before into the new cluster"
    "${PROJECT_ROOT}/scripts/deploy/load-kind-images.sh" --from-docker-host "${IMAGE_DOCKER_HOST}" \
        "${common_flags[@]}" "${LOCAL_IMAGES[@]}" 2>&1 | sed 's/^/    /'
    return "${PIPESTATUS[0]}"
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
        local selector='app in (econ-graph-backend,crawler-worker,keycloak)'
        if [ -n "$(k get pod -l "$selector" -o name)" ]; then
            k wait --for=delete pod -l "$selector" --timeout=300s ||
                die "writer pods are still running; not taking the backup"
        fi
    fi
}

restore_and_deploy() {
    local src="$1"
    # Once restored, a resumed run must not restore again: deploy.sh may have
    # started the crawler, which adds rows the backup doesn't have.
    if [ -f "${STATE_FILE}" ] && grep -qxF "RESTORED=${BACKUP_DIR}" "${STATE_FILE}"; then
        log "Databases already restored from ${BACKUP_DIR} by an earlier run"
    else
        run "${PROJECT_ROOT}/scripts/deploy/restore-kind-databases.sh" --from "$(backup_source_dir "$BACKUP_DIR")" --no-bootstrap "${common_flags[@]}"
        if [ "${DRY_RUN}" != "true" ]; then
            echo "RESTORED=${BACKUP_DIR}" >>"${STATE_FILE}"
        fi
    fi
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
    local series points ok=true
    SERIES_COUNT=0 DATA_POINTS_COUNT=0
    if [ -n "${BACKUP_DIR:-}" ] && [ -f "$(backup_source_dir "$BACKUP_DIR")/counts.env" ]; then
        read_counts "${BACKUP_DIR}"
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
        echo "  host ownership (expect ${HOST_POSTGRES_UID} = pod uid 999 under rootful Docker):"
        stat -c '    %n %u:%g %A' "${HOST_PG_DIR}" "${HOST_KEYCLOAK_PG_DIR}"
        [ "$(stat -c %u "${HOST_PG_DIR}")" = "${HOST_POSTGRES_UID}" ] || ok=false
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
        check_local_images
        note_production
        require_recreate_confirmation
        confirm "This stops econ-graph (downtime starts), backs up both databases, then DELETES and recreates kind cluster '${KIND_CLUSTER}'."
        backup_encryption_preflight
        backup_check_tmpfs_space
        stop_writers
        remember_backup "$(new_backup_dir)"
        remember_production
        backup_databases "${BACKUP_DIR}"
        save_local_images
        recreate_cluster --yes --no-backup "${recreate_flags[@]}" "${common_flags[@]}"
    else
        BACKUP_DIR="$(recalled_backup)"
        [ -n "${BACKUP_DIR}" ] || die "no backup to resume from; pass --resume-from ~/econgraph-backups/<ts>"
        [ -s "$(backup_source_dir "$BACKUP_DIR")/econ_graph.dump" ] || die "${BACKUP_DIR}/econ_graph.dump missing"
        log "Resuming with backup ${BACKUP_DIR}"
        if [ "$cluster" = "absent" ]; then
            recreate_cluster --yes --no-backup "${recreate_flags[@]}" "${common_flags[@]}"
        fi
    fi

    if [ "${DRY_RUN}" != "true" ]; then use_kind_kubectl; fi
    mark_if_was_production
    load_local_images
    bootstrap_databases "${PROJECT_ROOT}" "${BACKUP_DIR}"
    restore_and_deploy "${PROJECT_ROOT}"
    verify
    if [ "${DRY_RUN}" != "true" ]; then rm -f "${STATE_FILE}"; fi
    if [ "${DRY_RUN}" = "true" ]; then
        log "Dry run complete; nothing was changed"
    else
        log "Migration complete. Backup kept at ${BACKUP_DIR}"
    fi
    remind_public_host
}

# deploy.sh serves the public site only with PUBLIC_HOST set (and on a cluster
# marked production); say so when this run deployed without it.
remind_public_host() {
    if [ -z "${PUBLIC_HOST:-}" ]; then
        echo "  Note: PUBLIC_HOST was not set, so https://econgraph.com is not served from this"
        echo "  cluster. On production: scripts/deploy/mark-production-cluster.sh (if not marked),"
        echo "  then PUBLIC_HOST=econgraph.com scripts/deploy/deploy.sh"
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
        check_local_images
        note_production
        require_recreate_confirmation
        confirm "This stops econ-graph, backs up both databases, then DELETES kind cluster '${KIND_CLUSTER}' and redeploys ${REF} with node-local storage."
        backup_encryption_preflight
        backup_check_tmpfs_space
        stop_writers
        remember_backup "$(new_backup_dir)"
        remember_production
        backup_databases "${BACKUP_DIR}"
    else
        BACKUP_DIR="$(recalled_backup)"
        [ -n "${BACKUP_DIR}" ] || die "no running database to back up; pass --resume-from ~/econgraph-backups/<ts>"
    fi
    [ "${DRY_RUN}" = "true" ] || [ -s "$(backup_source_dir "$BACKUP_DIR")/econ_graph.dump" ] || die "${BACKUP_DIR}/econ_graph.dump missing"
    log "Rolling back to ${REF} with backup ${BACKUP_DIR}"

    if [ -d "$src" ]; then
        run git -C "$src" checkout -q --detach "${REF}"
    else
        run git -C "${PROJECT_ROOT}" worktree add -q --detach "$src" "${REF}"
    fi
    local config="$src/terraform/k8s/kind-config.yaml" config_from_ref="false"
    if [ "${DRY_RUN}" = "true" ] && [ ! -d "$src" ]; then
        # No worktree in a dry run: print the ref's kind config and hand it
        # to recreate-kind-cluster.sh through a pipe, never through a file.
        config_from_ref="true"
        echo "  kind config of ${REF} (terraform/k8s/kind-config.yaml, not written to disk):"
        git -C "${PROJECT_ROOT}" show "${REF}:terraform/k8s/kind-config.yaml" | sed 's/^/    | /'
    fi
    # A rollback always replaces the cluster; the backup above (or the one
    # given) is what gets restored.
    if kind_cluster_exists; then
        confirm "About to DELETE kind cluster '${KIND_CLUSTER}' for the rollback."
    fi
    if [ "$config_from_ref" = "true" ]; then
        recreate_cluster --yes --no-backup "${recreate_flags[@]}" \
            --config <(git -C "${PROJECT_ROOT}" show "${REF}:terraform/k8s/kind-config.yaml") "${common_flags[@]}"
    else
        recreate_cluster --yes --no-backup "${recreate_flags[@]}" --config "$config" "${common_flags[@]}"
    fi
    if [ "${DRY_RUN}" != "true" ]; then use_kind_kubectl; fi
    mark_if_was_production
    load_local_images
    bootstrap_databases "$src" "${BACKUP_DIR}"
    restore_and_deploy "$src"
    verify
    if [ "${DRY_RUN}" != "true" ]; then rm -f "${STATE_FILE}"; fi
    if [ "${DRY_RUN}" = "true" ]; then log "Dry run complete; nothing was changed"; else log "Rollback complete. Host subvolume contents were not touched."; fi
    remind_public_host
}

if [ "${DRY_RUN}" = "true" ]; then
    log "DRY RUN: checks run for real; nothing is changed"
fi
"${MODE}"
