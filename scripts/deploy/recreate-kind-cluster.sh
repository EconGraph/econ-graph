#!/bin/bash

# Delete and recreate the kind cluster `econ-graph` from
# terraform/k8s/kind-config.yaml (ports 80/443/3000/8080, and the worker that
# bind-mounts the host Postgres subvolumes), on the rootful Docker daemon
# (scripts/deploy/lib/kind-env.sh; override with ECONGRAPH_DOCKER_HOST), then
# install the gateway.
#
# DESTRUCTIVE: everything stored inside the cluster's node containers is lost
# (Grafana data, the in-cluster postgres-backup-data PVC, Secrets, and on a
# cluster that predates host storage, the databases themselves). So, before
# deleting an existing cluster, this script:
#   1. shows it (nodes, kube-system UID, production marker), read inside its
#      control-plane container on the daemon kind deletes it from; a cluster
#      whose kube-system UID can't be read there is not deleted;
#   2. asks for a confirmation:
#      - a cluster with a production marker (kube-system/econgraph-cluster-role
#        with any role other than `none`, valid or not), or whose marker can't
#        be read: the first 8 characters of its kube-system UID, typed at a
#        terminal or given as --confirm-production; --yes does not answer it;
#      - any other cluster: a typed `yes`, or --yes;
#      a --confirm-production that doesn't match the cluster is refused, and
#      without a terminal a confirmation that isn't given on the command line
#      is refused;
#   3. pg_dumps both databases, with row counts and the app Secrets, to
#      ~/econgraph-backups/<UTC timestamp>/ (see backup-kind-databases.sh);
#      if that fails, nothing is deleted.
# All of these run before anything is backed up, deleted or created.
# A cluster whose API server doesn't answer (e.g. its certificates expired)
# can't be backed up; it is only deleted with --delete-unreachable, and only
# after the same identification and confirmation. One whose kube-system UID
# can't be read is not deleted: start it or renew its certificates first
# (docs/deployment/HOST_STORAGE.md, "A cluster whose API server doesn't
# answer").
# If the kind config bind-mounts host directories, they must be mounted
# first (scripts/host/setup-subvolumes.sh), or nothing is deleted.
#
# Usage:
#   scripts/deploy/recreate-kind-cluster.sh [--yes] [--dry-run] [--config FILE]
# A dry run may create a temporary directory under $TMPDIR (a copy of the
# cluster's kubectl and kubeconfig), removed on exit; it changes nothing else.
#       [--if-missing]          keep a cluster that exists and answers (only
#                               make sure the gateway is installed); create
#                               one if there is none. Safe to re-run.
#       [--delete-unreachable]  allow deleting an existing cluster whose API
#                               server doesn't answer (nothing can be backed
#                               up from it); still confirmed as above
#       [--no-backup]           only for callers that took a backup just before
#                               (migrate-to-host-storage.sh); still confirmed
#                               as above
#       [--confirm-production UID-PREFIX]
#                               the first 8 characters of the kube-system UID
#                               of the cluster to delete, for a cluster with a
#                               production marker when there is no terminal
#
# Afterwards: on production only, mark-production-cluster.sh first (before the
# restore, so the tunnel token is restored too); then load-kind-images.sh,
# restore-kind-databases.sh, deploy.sh
# (docs/deployment/HOST_STORAGE.md, "Restoring production from a backup").

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/deploy/lib/kind-env.sh
source "${PROJECT_ROOT}/scripts/deploy/lib/kind-env.sh"

CONFIG="${PROJECT_ROOT}/terraform/k8s/kind-config.yaml"
BACKUP="true"
IF_MISSING="false"
DELETE_UNREACHABLE="false"
CONFIRM_PRODUCTION=""
usage="usage: $0 [--yes] [--dry-run] [--config FILE] [--if-missing] [--delete-unreachable] [--no-backup] [--confirm-production UID-PREFIX]"
while [ $# -gt 0 ]; do
    case "$1" in
        --yes|-y) ASSUME_YES="true"; shift ;;
        --dry-run|-n) DRY_RUN="true"; shift ;;
        --config) CONFIG="$2"; shift 2 ;;
        --no-backup) BACKUP="false"; shift ;;
        --if-missing) IF_MISSING="true"; shift ;;
        --delete-unreachable) DELETE_UNREACHABLE="true"; shift ;;
        --confirm-production)
            if [ $# -lt 2 ] || [ -z "$2" ]; then echo "$usage" >&2; exit 2; fi
            CONFIRM_PRODUCTION="$2"; shift 2 ;;
        --confirm-production=*) CONFIRM_PRODUCTION="${1#*=}"; shift ;;
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

# The cluster is identified with `docker exec` into its control plane; with
# another kind provider that would read one cluster while kind deleted another.
case "${KIND_EXPERIMENTAL_PROVIDER:-docker}" in
    docker) ;;
    *) die "KIND_EXPERIMENTAL_PROVIDER=${KIND_EXPERIMENTAL_PROVIDER} is not supported; nothing was deleted" ;;
esac

log "Docker daemon"
check_docker_daemon
# Creating a cluster needs a kind new enough for the node image; keeping one
# (--if-missing) does not. Checked here, before anything is backed up or deleted.
if [ "$IF_MISSING" != "true" ] || ! kind_cluster_exists; then
    require_kind_version
fi

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
        # Compared as resolved paths, so ../ or a symlink can't reach production.
        if [ -n "${SCRATCH_STORAGE}" ]; then
            scratch_real="$(realpath -e -- "${SCRATCH_STORAGE}")" ||
                die "${SCRATCH_STORAGE} does not exist; nothing was deleted"
            mount_real="$(realpath -e -- "$m")" ||
                die "$m does not exist; nothing was deleted"
            case "$mount_real" in
                "${scratch_real%/}"/*) ;;
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

# in_node ARGS...: kubectl inside the control-plane container, on ${DOCKER_HOST}
# (the rootful daemon kind-env.sh chose, which kind deletes from), so what is
# read is the very cluster that would be deleted.
in_node() {
    docker exec "${KIND_CONTROL_PLANE}" kubectl --kubeconfig /etc/kubernetes/admin.conf --request-timeout=30s "$@"
}

# confirm_delete PROMPT: show the cluster, then get the confirmation its
# production marker calls for (see the top of this file), or exit having
# changed nothing. Runs before any backup or delete.
confirm_delete() {
    local prompt="$1" nodes uid marker role marker_uid production role_text code answer
    nodes="$(kind get nodes --name "${KIND_CLUSTER}" 2>/dev/null | tr '\n' ' ')" || nodes="(unknown)"
    uid="$(in_node get namespace "${CLUSTER_ROLE_NAMESPACE}" -o 'jsonpath={.metadata.uid}')" || uid=""
    if [ -z "${uid}" ]; then
        echo "error: could not read the kube-system UID inside ${KIND_CONTROL_PLANE} on ${DOCKER_HOST}" \
            "(its API server may not answer), so the cluster can't be identified and may be production;" \
            "nothing was deleted. Bring it back, then re-run this script (docs/deployment/HOST_STORAGE.md," \
            "\"A cluster whose API server doesn't answer\"):" >&2
        echo "  - stopped: DOCKER_HOST=${DOCKER_HOST} docker start ${KIND_CONTROL_PLANE} ${KIND_STORAGE_NODE}" >&2
        echo "  - expired certificates: DOCKER_HOST=${DOCKER_HOST} docker exec ${KIND_CONTROL_PLANE} kubeadm certs renew all," >&2
        echo "    then DOCKER_HOST=${DOCKER_HOST} docker restart ${KIND_CONTROL_PLANE}" >&2
        echo "  - last resort, skipping the backup and this check, everything in its nodes lost:" >&2
        echo "    DOCKER_HOST=${DOCKER_HOST} ${KIND_PINNED_BIN} delete cluster --name ${KIND_CLUSTER} --kubeconfig ${KIND_KUBECONFIG}" >&2
        echo "  Inspect it: DOCKER_HOST=${DOCKER_HOST} docker exec ${KIND_CONTROL_PLANE} journalctl -u kubelet" >&2
        exit 1
    fi
    # Broader than cluster_is_marked_production (lib/cluster-role.sh), which
    # decides whether a cluster may hold the tunnel token: before a delete, any
    # marker other than role=none asks for the UID prefix, and so does one that
    # can't be read.
    production="true"
    if marker="$(in_node -n "${CLUSTER_ROLE_NAMESPACE}" get configmap "${CLUSTER_ROLE_CONFIGMAP}" \
        --ignore-not-found -o 'jsonpath={.data.role}{" "}{.data.kube-system-uid}')"; then
        read -r role marker_uid <<<"$marker" || true
        if [ "${role:-}" = "production" ] && [ "${marker_uid:-}" = "${uid}" ]; then
            role_text="marked production: this is the cluster serving econgraph.com"
        elif cluster_role_marker_guards_delete "$marker"; then
            role_text="the marker says role=${role:-(none)}, kube-system-uid=${marker_uid:-(missing)}; treated as production"
        else
            production="false"
            if [ -z "${role:-}" ]; then
                role_text="not production (no ConfigMap ${CLUSTER_ROLE_NAMESPACE}/${CLUSTER_ROLE_CONFIGMAP})"
            else
                role_text="not production (role=${role})"
            fi
        fi
    else
        role_text="could not read ConfigMap ${CLUSTER_ROLE_NAMESPACE}/${CLUSTER_ROLE_CONFIGMAP}; treated as production"
    fi
    code="${uid:0:8}"
    echo "  DOCKER_HOST     ${DOCKER_HOST}"
    echo "  nodes           ${nodes% }"
    echo "  kube-system UID ${uid} (read inside ${KIND_CONTROL_PLANE})"
    echo "  production      ${role_text}"

    # --confirm-production names one cluster: refuse it for any other, marked or not.
    if [ -n "${CONFIRM_PRODUCTION}" ] && [ "${CONFIRM_PRODUCTION}" != "${code}" ]; then
        die "--confirm-production ${CONFIRM_PRODUCTION} does not match this cluster's kube-system UID; nothing was deleted"
    fi
    if [ "${production}" != "true" ]; then
        if [ "${ASSUME_YES}" != "true" ] && [ "${DRY_RUN}" != "true" ] && [ ! -t 0 ]; then
            die "$prompt It needs confirmation and there is no terminal to confirm on (pass --yes);" \
                "nothing was deleted"
        fi
        confirm "$prompt"
        return 0
    fi
    if [ "${DRY_RUN}" = "true" ]; then
        echo "$prompt (dry run: deleting it would require the first 8 characters of the kube-system UID, ${code})"
    elif [ -n "${CONFIRM_PRODUCTION}" ]; then
        echo "$prompt (--confirm-production ${CONFIRM_PRODUCTION} matches this cluster)"
    elif [ -t 0 ]; then
        echo "!!! $prompt It is, or may be, production; --yes does not answer this."
        # The prompt on its own line: a caller may pipe this script's output
        # through a line-buffered filter (migrate-to-host-storage.sh).
        echo "Type the first 8 characters of the kube-system UID above to delete it:"
        read -r answer || answer=""
        [ "${answer}" = "${code}" ] || die "confirmation did not match; nothing was deleted"
    else
        die "$prompt It is, or may be, production: deleting it needs --confirm-production with the" \
            "first 8 characters of its kube-system UID, or a terminal to type them on (--yes is not" \
            "enough); nothing was deleted"
    fi
}

if kind_cluster_exists; then
    log "kind cluster '${KIND_CLUSTER}' exists on ${DOCKER_HOST}"
    reachable="false"
    if try_use_kind_kubectl; then
        reachable="true"
    fi
    if [ "$reachable" = "true" ]; then
        if [ "$IF_MISSING" = "true" ]; then
            check_host_storage_in_node ||
                die "the existing cluster does not bind-mount the host subvolumes; recreate it without --if-missing (it is backed up first)"
            log "Keeping the existing cluster (--if-missing)"
            install_gateway
            check_restart_on_boot
            exit 0
        fi
        confirm_delete "About to DELETE kind cluster '${KIND_CLUSTER}' and everything stored in its nodes."
        if [ "$BACKUP" = "true" ]; then
            backup_databases "$(new_backup_dir)"
        else
            log "Skipping the pre-delete backup (--no-backup)"
        fi
    else
        echo "  its API server does not answer; nothing can be backed up from it."
        docker ps -a --filter "label=io.x-k8s.kind.cluster=${KIND_CLUSTER}" \
            --format '  {{.Names}}  {{.Status}}  created {{.CreatedAt}}'
        if [ "$DELETE_UNREACHABLE" != "true" ]; then
            die "kind cluster '${KIND_CLUSTER}' exists on ${DOCKER_HOST} but is unreachable. Inspect it (docker exec ${KIND_CONTROL_PLANE} journalctl -u kubelet), then re-run with --delete-unreachable to delete it without a backup. Nothing was deleted."
        fi
        confirm_delete "About to DELETE the unreachable kind cluster '${KIND_CLUSTER}' WITHOUT a backup; everything in its nodes is lost."
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
    install_gateway
    log "Dry run complete; nothing was changed"
    exit 0
fi

use_kind_kubectl
kubectl wait --for=condition=Ready nodes --all --timeout=300s
check_host_storage_in_node || die "node ${KIND_STORAGE_NODE} does not see ${NODE_DATA_PLANE} and ${NODE_APP_PROD}"
install_gateway
check_restart_on_boot

log "Kind cluster recreated"
echo "  Frontend: http://localhost:3000   Backend: http://localhost:8080"
echo "  HTTP: http://localhost:80          HTTPS: https://localhost:443"
# The new cluster has a new kube-system UID, so an earlier production marker no
# longer counts (scripts/deploy/lib/cluster-role.sh), and the restore only
# brings the tunnel token back into a marked cluster.
echo "  Next: on the production cluster (the one serving econgraph.com) only, first"
echo "  scripts/deploy/mark-production-cluster.sh; then scripts/deploy/load-kind-images.sh,"
echo "  restore-kind-databases.sh, deploy.sh (PUBLIC_HOST=econgraph.com on production)"
