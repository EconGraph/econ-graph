#!/bin/bash

# Delete and recreate the kind cluster `econ-graph` from
# terraform/k8s/kind-config.yaml (ports 80/443/3000/8080, and the worker that
# bind-mounts the host Postgres subvolumes), on the rootful Docker daemon
# (scripts/deploy/lib/kind-env.sh; override with ECONGRAPH_DOCKER_HOST), then
# install ingress-nginx.
#
# DESTRUCTIVE: everything stored inside the cluster's node containers is lost
# (Grafana data, the in-cluster postgres-backup-data PVC, Secrets, and on a
# cluster that predates host storage, the databases themselves). So, before
# deleting an existing cluster, this script:
#   1. pg_dumps both databases, with row counts and the app Secrets, to
#      ~/econgraph-backups/<UTC timestamp>/ (see backup-kind-databases.sh);
#      if that fails, nothing is deleted;
#   2. asks for a typed confirmation (or --yes).
# A cluster whose API server doesn't answer (e.g. its certificates expired)
# can't be backed up; it is only deleted with --delete-unreachable.
# If the kind config bind-mounts host directories, they must be mounted
# first (scripts/host/setup-subvolumes.sh), or nothing is deleted.
#
# Usage:
#   scripts/deploy/recreate-kind-cluster.sh [--yes] [--dry-run] [--config FILE]
# A dry run may create a temporary directory under $TMPDIR (a copy of the
# cluster's kubectl and kubeconfig), removed on exit; it changes nothing else.
#       [--if-missing]          keep a cluster that exists and answers (only
#                               make sure ingress-nginx is installed); create
#                               one if there is none. Safe to re-run.
#       [--delete-unreachable]  allow deleting an existing cluster whose API
#                               server doesn't answer (nothing can be backed
#                               up from it); still asks unless --yes
#       [--no-backup]           only for callers that took a backup just before
#                               (migrate-to-host-storage.sh); still asks unless --yes
#
# Afterwards: load-kind-images.sh, restore-kind-databases.sh, deploy.sh
# (docs/deployment/HOST_STORAGE.md, "Recovering production").

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/deploy/lib/kind-env.sh
source "${PROJECT_ROOT}/scripts/deploy/lib/kind-env.sh"

CONFIG="${PROJECT_ROOT}/terraform/k8s/kind-config.yaml"
BACKUP="true"
IF_MISSING="false"
DELETE_UNREACHABLE="false"
usage="usage: $0 [--yes] [--dry-run] [--config FILE] [--if-missing] [--delete-unreachable] [--no-backup]"
while [ $# -gt 0 ]; do
    case "$1" in
        --yes|-y) ASSUME_YES="true"; shift ;;
        --dry-run|-n) DRY_RUN="true"; shift ;;
        --config) CONFIG="$2"; shift 2 ;;
        --no-backup) BACKUP="false"; shift ;;
        --if-missing) IF_MISSING="true"; shift ;;
        --delete-unreachable) DELETE_UNREACHABLE="true"; shift ;;
        -h|--help) echo "$usage"; exit 0 ;;
        *) echo "$usage" >&2; exit 2 ;;
    esac
done
# Read once, so --config may also be a pipe (a dry run passes the config of
# another ref that way, without writing it to disk).
[ -r "$CONFIG" ] || die "kind config $CONFIG not found"
config_text="$(cat "$CONFIG")"
KIND_KUBECONFIG="${ECONGRAPH_CACHE}/kubeconfig-${KIND_CLUSTER}"
[ "${DRY_RUN}" = "true" ] && log "DRY RUN: checks run for real; nothing is changed"

log "Docker daemon"
check_docker_daemon

# Every bind-mounted host directory must already exist as a real mount, or
# kind would fail half-way (or, for a missing mount under an existing
# directory, bind an empty directory and Postgres would refuse to start).
# (grep, not yq: this host's yq is the jq-based one, other machines have the Go one.)
mounts="$(grep -E '"?hostPath"?:' <<<"$config_text" | sed -E 's/.*hostPath"?:[[:blank:]]*"?([^"]*)"?.*/\1/' || true)"
if [ -n "$mounts" ]; then
    log "Checking the host directories the kind config bind-mounts"
    if [ "$CONFIG" = "${PROJECT_ROOT}/terraform/k8s/kind-config.yaml" ]; then
        check_host_storage_on_host || die "host subvolumes are not ready; nothing was deleted"
    fi
    for m in $mounts; do
        # A scratch cluster may bind-mount only scratch storage.
        if [ -n "${SCRATCH_STORAGE}" ]; then
            case "$m" in
                "${SCRATCH_STORAGE%/}"/*) ;;
                *) die "$m is outside ${SCRATCH_STORAGE}; nothing was deleted" ;;
            esac
        fi
        bind_source_ok "$m" || die "$m is not mounted; nothing was deleted"
        echo "  ok: $m"
    done
fi

# kind's node containers publish these host ports (extraPortMappings).
host_ports="$(grep -E '"?hostPort"?:' <<<"$config_text" | grep -oE '[0-9]+' || true)"
ports_in_use() {
    local p used=""
    for p in $host_ports; do
        if ss -Hltn "sport = :$p" 2>/dev/null | grep -q .; then used="$used $p"; fi
    done
    echo "${used# }"
}

delete_cluster() {
    run kind delete cluster --name "${KIND_CLUSTER}" --kubeconfig "${KIND_KUBECONFIG}"
}

if kind_cluster_exists; then
    log "kind cluster '${KIND_CLUSTER}' exists on ${DOCKER_HOST}"
    if try_use_kind_kubectl; then
        if [ "$IF_MISSING" = "true" ]; then
            check_host_storage_in_node ||
                die "the existing cluster does not bind-mount the host subvolumes; recreate it without --if-missing (it is backed up first)"
            log "Keeping the existing cluster (--if-missing)"
            install_ingress_nginx
            check_restart_on_boot
            exit 0
        fi
        if [ "$BACKUP" = "true" ]; then
            backup_databases "$(new_backup_dir)"
        else
            log "Skipping the pre-delete backup (--no-backup)"
        fi
        confirm "About to DELETE kind cluster '${KIND_CLUSTER}' and everything stored in its nodes."
    else
        echo "  its API server does not answer; nothing can be backed up from it."
        docker ps -a --filter "label=io.x-k8s.kind.cluster=${KIND_CLUSTER}" \
            --format '  {{.Names}}  {{.Status}}  created {{.CreatedAt}}'
        if [ "$DELETE_UNREACHABLE" != "true" ]; then
            die "kind cluster '${KIND_CLUSTER}' exists on ${DOCKER_HOST} but is unreachable. Inspect it (docker exec ${KIND_CONTROL_PLANE} journalctl -u kubelet), then re-run with --delete-unreachable to delete it without a backup. Nothing was deleted."
        fi
        confirm "About to DELETE the unreachable kind cluster '${KIND_CLUSTER}' WITHOUT a backup; everything in its nodes is lost."
    fi
    log "Deleting kind cluster ${KIND_CLUSTER}"
    delete_cluster
else
    log "No kind cluster '${KIND_CLUSTER}' on ${DOCKER_HOST}; nothing to back up or delete"
    used="$(ports_in_use)"
    [ -z "$used" ] || die "host port(s) $used are already in use; kind could not publish them. Nothing was created."
fi

log "Creating kind cluster ${KIND_CLUSTER} from ${CONFIG}"
# --kubeconfig: kind would otherwise merge the new context into ~/.kube/config.
[ "${DRY_RUN}" = "true" ] || mkdir -p "${ECONGRAPH_CACHE}"
run kind create cluster --name "${KIND_CLUSTER}" --config "$CONFIG" --kubeconfig "${KIND_KUBECONFIG}"
if [ "${DRY_RUN}" = "true" ]; then
    echo "+ kubectl wait --for=condition=Ready nodes --all --timeout=300s"
    install_ingress_nginx
    log "Dry run complete; nothing was changed"
    exit 0
fi

use_kind_kubectl
kubectl wait --for=condition=Ready nodes --all --timeout=300s
check_host_storage_in_node || die "node ${KIND_STORAGE_NODE} does not see ${NODE_DATA_PLANE} and ${NODE_APP_PROD}"
install_ingress_nginx
check_restart_on_boot

log "Kind cluster recreated"
echo "  Frontend: http://localhost:3000   Backend: http://localhost:8080"
echo "  HTTP: http://localhost:80          HTTPS: https://localhost:443"
echo "  Next: scripts/deploy/load-kind-images.sh, restore-kind-databases.sh, deploy.sh"
