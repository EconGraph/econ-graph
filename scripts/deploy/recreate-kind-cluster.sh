#!/bin/bash

# Recreate the kind cluster econ-graph from terraform/k8s/kind-config.yaml
# (ports 80/443/3000/8080 and the worker that bind-mounts the host Postgres
# subvolumes).
#
# Usage: scripts/deploy/recreate-kind-cluster.sh [--if-missing] [--yes|-y]
#            [--confirm-production UID-PREFIX] [--dry-run]
#
#   --if-missing   create the cluster only when there is none; when it exists,
#                  exit 0 and change nothing
#   --yes, -y      delete an existing cluster that is not marked production
#                  without asking
#   --confirm-production UID-PREFIX
#                  delete an existing cluster marked production (or whose
#                  marker can't be read): the first 8 characters of its
#                  kube-system UID, as shown. --yes alone does not do it, and a
#                  prefix that doesn't match is refused on any cluster.
#   --dry-run      show the cluster and the commands; change nothing
#
# DESTRUCTIVE: deleting the cluster destroys what its nodes hold (the Postgres
# backup volume, Grafana's data, the Secrets). Before deleting an existing
# cluster the script shows it (nodes, kube-system UID, production marker), read
# inside its control-plane container on the daemon kind deletes it from, then:
#   - marked production (scripts/deploy/mark-production-cluster.sh), or the
#     marker can't be read: the UID prefix, typed at a terminal or given with
#     --confirm-production, as for teardown.sh;
#   - otherwise: the cluster name typed at a terminal, or --yes. Without a
#     terminal and without --yes it refuses.
# A cluster whose kube-system UID can't be read (its API server doesn't answer)
# is not deleted.
#
# kind and docker run on the rootful Docker daemon the kind cluster runs on,
# whatever DOCKER_HOST the shell has (ECONGRAPH_DOCKER_HOST chooses another;
# cluster_role_kind_docker_host in lib/cluster-role.sh), and kind is the one
# cluster_role_kind runs. kind always gets --kubeconfig
# ${XDG_CACHE_HOME:-~/.cache}/econgraph/kubeconfig-econ-graph, so it never
# writes ~/.kube/config (or $KUBECONFIG) or switches a current context; the new
# cluster's kubectl is copied next to it, where the other deploy scripts look
# (cluster_role_default_kubectl).

set -euo pipefail

usage="usage: $0 [--if-missing] [--yes|-y] [--confirm-production UID-PREFIX] [--dry-run]"
ASSUME_YES="false"
IF_MISSING="false"
CONFIRM_PRODUCTION=""
DRY_RUN="false"
while [ $# -gt 0 ]; do
    case "$1" in
        --if-missing) IF_MISSING="true" ;;
        --yes|-y) ASSUME_YES="true" ;;
        --confirm-production)
            if [ $# -lt 2 ] || [ -z "$2" ]; then echo "$usage" >&2; exit 2; fi
            CONFIRM_PRODUCTION="$2"
            shift
            ;;
        --confirm-production=*) CONFIRM_PRODUCTION="${1#*=}" ;;
        --dry-run|-n) DRY_RUN="true" ;;
        -h|--help) sed -n '3,40p' "$0"; exit 0 ;;
        *) echo "$usage" >&2; exit 2 ;;
    esac
    shift
done

die() {
    echo "❌ $*" >&2
    exit 1
}

# run COMMAND...: run it, or with --dry-run only show it.
run() {
    if [ "${DRY_RUN}" = "true" ]; then
        echo "+ $*"
    else
        "$@"
    fi
}

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CLUSTER="econ-graph"
CONTEXT="kind-${CLUSTER}"
CONTROL_PLANE="${CLUSTER}-control-plane"
CONFIG="${PROJECT_ROOT}/terraform/k8s/kind-config.yaml"
KUBE_CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/econgraph"
KIND_KUBECONFIG="${KUBE_CACHE}/kubeconfig-${CLUSTER}"

# The cluster is identified with `docker exec` into its control plane; with
# another kind provider that would read one cluster while kind deleted another.
case "${KIND_EXPERIMENTAL_PROVIDER:-docker}" in
    docker) ;;
    *) die "KIND_EXPERIMENTAL_PROVIDER=${KIND_EXPERIMENTAL_PROVIDER} is not supported; nothing was changed" ;;
esac

# shellcheck source=scripts/deploy/lib/cluster-role.sh
source "${PROJECT_ROOT}/scripts/deploy/lib/cluster-role.sh"
cluster_role_kind_docker_host
kind() { cluster_role_kind "$@"; }

[ "${DRY_RUN}" = "true" ] && echo "Dry run: the checks run; nothing is deleted, created or asked."

clusters="$(kind get clusters)" ||
    die "could not list the kind clusters on DOCKER_HOST=${DOCKER_HOST}; nothing was changed"
exists="false"
if grep -qx "${CLUSTER}" <<<"$clusters"; then
    exists="true"
fi

if [ "${exists}" = "true" ] && [ "${IF_MISSING}" = "true" ]; then
    echo "✅ kind cluster '${CLUSTER}' exists on ${DOCKER_HOST}; kept (--if-missing). Nothing was changed."
    exit 0
fi

# terraform/k8s/kind-config.yaml bind-mounts the host btrfs subvolumes that hold
# the Postgres data into econ-graph-worker (docs/deployment/HOST_STORAGE.md).
# Docker would create a missing bind-mount source as an empty root-owned
# directory, so check them before the old cluster is deleted.
for dir in /srv/econgraph/data-plane /srv/econgraph/app-prod; do
    if ! mountpoint -q "$dir"; then
        echo "❌ $dir is not mounted; nothing was changed. Create the subvolumes first:" >&2
        echo "   sudo scripts/host/setup-subvolumes.sh --dry-run (then without --dry-run)" >&2
        exit 1
    fi
done

# in_node ARGS...: kubectl inside the control-plane container, on the daemon
# kind would delete the cluster from, so what is shown is that very cluster.
in_node() {
    docker exec "${CONTROL_PLANE}" kubectl --kubeconfig /etc/kubernetes/admin.conf "$@"
}

if [ "${exists}" = "true" ]; then
    echo "🔎 kind cluster '${CLUSTER}' exists on ${DOCKER_HOST}:"
    nodes="$(kind get nodes --name "${CLUSTER}" | tr '\n' ' ')" || nodes="(unknown)"
    uid="$(in_node get namespace "${CLUSTER_ROLE_NAMESPACE}" -o 'jsonpath={.metadata.uid}')" || uid=""
    if [ -z "${uid}" ]; then
        die "could not read the kube-system UID inside ${CONTROL_PLANE} on DOCKER_HOST=${DOCKER_HOST}" \
            "(its API server may not answer), so the cluster can't be identified; nothing was changed." \
            "Inspect it: docker exec ${CONTROL_PLANE} journalctl -u kubelet"
    fi
    # The marker as cluster_is_marked_production (lib/cluster-role.sh) reads
    # it: production only with role=production and this cluster's UID; one
    # that can't be read is treated as production.
    production="false"
    if marker="$(in_node -n "${CLUSTER_ROLE_NAMESPACE}" get configmap "${CLUSTER_ROLE_CONFIGMAP}" \
        --ignore-not-found -o 'jsonpath={.data.role}{" "}{.data.kube-system-uid}')"; then
        read -r role marker_uid <<<"$marker" || true
        if [ "${role:-}" = "production" ] && [ "${marker_uid:-}" = "${uid}" ]; then
            production="true"
            role_text="marked production: this is the cluster serving econgraph.com"
        elif [ -z "${role:-}" ] && [ -z "${marker_uid:-}" ]; then
            role_text="not production (no ConfigMap ${CLUSTER_ROLE_NAMESPACE}/${CLUSTER_ROLE_CONFIGMAP})"
        else
            role_text="not production (the marker says role=${role:-none}, UID ${marker_uid:-missing}; made on another cluster or not production)"
        fi
    else
        production="true"
        role_text="could not read ConfigMap ${CLUSTER_ROLE_NAMESPACE}/${CLUSTER_ROLE_CONFIGMAP}; treated as production for the confirmation"
    fi
    code="${uid:0:8}"
    echo "  DOCKER_HOST     ${DOCKER_HOST}"
    echo "  nodes           ${nodes% }"
    echo "  kube-system UID ${uid} (read inside ${CONTROL_PLANE})"
    echo "  production      ${role_text}"

    # --confirm-production names one cluster: refuse it for any other, marked or not.
    if [ -n "${CONFIRM_PRODUCTION}" ] && [ "${CONFIRM_PRODUCTION}" != "${code}" ]; then
        die "--confirm-production ${CONFIRM_PRODUCTION} does not match this cluster's kube-system UID; nothing was changed"
    fi

    if [ "${DRY_RUN}" = "true" ]; then
        if [ "${production}" = "true" ]; then
            echo "ℹ️  Dry run: deleting it would require the first 8 characters of the kube-system UID (${code})."
        elif [ "${ASSUME_YES}" != "true" ]; then
            echo "ℹ️  Dry run: would ask to type '${CLUSTER}' before deleting it."
        fi
    elif [ "${production}" = "true" ]; then
        echo ""
        echo "!!! WARNING: deleting this cluster destroys what its nodes hold; it is (or may be) production."
        if [ -n "${CONFIRM_PRODUCTION}" ]; then
            : # It matched the UID above.
        elif [ -t 0 ]; then
            read -r -p "Type the first 8 characters of the kube-system UID above to delete it: " answer || answer=""
            [ "${answer}" = "${code}" ] || die "confirmation did not match; nothing was changed"
        else
            die "deleting a production cluster needs --confirm-production with the first 8 characters" \
                "of its kube-system UID, or a terminal to type them on; nothing was changed"
        fi
    elif [ "${ASSUME_YES}" != "true" ]; then
        if [ ! -t 0 ]; then
            die "deleting the cluster needs a confirmation and there is no terminal to confirm on" \
                "(pass --yes, or --if-missing to keep it); nothing was changed"
        fi
        echo ""
        echo "⚠️  Deleting the cluster destroys what its nodes hold (the Postgres backup volume, Grafana's data)."
        read -r -p "Type '${CLUSTER}' to delete it and create it again: " answer || answer=""
        [ "${answer}" = "${CLUSTER}" ] || die "confirmation did not match; nothing was changed"
    fi

    echo "🗑️  Deleting kind cluster ${CLUSTER}..."
    run mkdir -p "${KUBE_CACHE}"
    run kind delete cluster --name "${CLUSTER}" --kubeconfig "${KIND_KUBECONFIG}"
else
    echo "No kind cluster '${CLUSTER}' on ${DOCKER_HOST}; nothing to delete."
fi

echo "🏗️  Creating kind cluster ${CLUSTER} from ${CONFIG}..."
run mkdir -p "${KUBE_CACHE}/bin"
run kind create cluster --name "${CLUSTER}" --config "${CONFIG}" --kubeconfig "${KIND_KUBECONFIG}"

if [ "${DRY_RUN}" = "true" ]; then
    echo "+ docker cp -q -L ${CONTROL_PLANE}:/usr/bin/kubectl ${KUBE_CACHE}/bin/kubectl"
    echo "+ ${KUBE_CACHE}/bin/kubectl --context ${CONTEXT} wait --for=condition=Ready nodes --all --timeout=300s"
    echo "Dry run complete: nothing was changed."
    exit 0
fi

# The new cluster's own kubectl next to its kubeconfig, where the other scripts
# look for them (cluster_role_default_kubectl): the copies of the old cluster no
# longer reach it, and on the production host plain `kubectl` is the MicroK8s
# snap wrapper.
docker cp -q -L "${CONTROL_PLANE}:/usr/bin/kubectl" "${KUBE_CACHE}/bin/kubectl.new"
chmod 0755 "${KUBE_CACHE}/bin/kubectl.new"
mv -f "${KUBE_CACHE}/bin/kubectl.new" "${KUBE_CACHE}/bin/kubectl"
(umask 077 && kind get kubeconfig --name "${CLUSTER}" >"${KIND_KUBECONFIG}")
echo "🔑 kubectl ${KUBE_CACHE}/bin/kubectl, KUBECONFIG=${KIND_KUBECONFIG}"

echo "⏳ Waiting for cluster to be ready..."
KUBECONFIG="${KIND_KUBECONFIG}" "${KUBE_CACHE}/bin/kubectl" --context "${CONTEXT}" \
    wait --for=condition=Ready nodes --all --timeout=300s

echo "✅ Kind cluster recreated successfully!"
echo ""
# The new cluster has a new kube-system UID, so an earlier production marker no
# longer counts (scripts/deploy/lib/cluster-role.sh).
echo "🚇 If this is the production cluster (the one serving econgraph.com), mark it again"
echo "   before deploying with PUBLIC_HOST: ./scripts/deploy/mark-production-cluster.sh"
echo ""
echo "📋 Available ports:"
echo "  - Frontend: http://localhost:3000"
echo "  - Backend:  http://localhost:8080"
echo "  - Grafana:  ClusterIP only; kubectl port-forward service/grafana-service 3000:3000 -n econ-graph"
echo "  - HTTP:     http://localhost:80"
echo "  - HTTPS:    https://localhost:443"
echo ""
echo "🔒 Internal Services (not exposed externally):"
echo "  - Chart API Service: chart-api-service.econ-graph.svc.cluster.local:3001"
echo ""
echo "🚀 Ready to deploy the application stack!"
