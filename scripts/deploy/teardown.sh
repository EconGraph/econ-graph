#!/bin/bash

# Teardown EconGraph from local Kubernetes cluster
# This script removes the application and optionally the cluster
#
# Usage: scripts/deploy/teardown.sh [--yes|-y] [--delete-cluster] [--dry-run]
#
# Deleting the econ-graph namespace deletes the PostgreSQL PVCs and the backup
# PVC. Their PVs are set to Retain first (scripts/deploy/protect-postgres-pv.sh),
# so they are left Released, with their contents in place. By default this
# script asks for confirmation before that step; pass --yes to skip that prompt
# (e.g. for CI).
#
# Both databases live on host btrfs subvolumes, bind-mounted into the cluster
# (docs/deployment/HOST_STORAGE.md), so deleting the namespace or the kind
# cluster leaves them alone. The cluster's nodes do hold the backup volume and
# Grafana's data, so deleting the cluster destroys those. That is why --yes
# does NOT also delete the kind cluster: that needs its own --delete-cluster
# flag (or the interactive prompt), never implied by --yes.
#
# --dry-run shows the cluster and what would be deleted, and changes nothing:
# no PV is patched, nothing is deleted and nothing is asked.
#
# A confirmation that can't be typed (stdin is not a terminal) stops the script
# before it changes anything: without a terminal, pass --yes and
# --delete-cluster, or --dry-run.
#
# Every kubectl call goes to one kubectl on one context, kind-econ-graph,
# through kc (scripts/deploy/lib/cluster-role.sh): ${KUBECTL}, or, when
# neither KUBECTL nor KUBECONFIG is set, the kind cluster's own kubectl and
# kubeconfig kept in ~/.cache/econgraph, as for mark-production-cluster.sh.
# Plain `kubectl` may reach another cluster (on the production host it is the
# MicroK8s snap wrapper). Before anything is changed, the script stops if that
# context can't be selected or doesn't reach the kind cluster econ-graph's
# nodes, and shows the cluster: context, API server, nodes, kube-system UID and
# production marker. It never deletes a PV or anything on the host.

set -eu

usage="usage: $0 [--yes|-y] [--delete-cluster] [--dry-run]"
ASSUME_YES="false"
DELETE_CLUSTER_FLAG="false"
DRY_RUN="false"
for arg in "$@"; do
    case "$arg" in
        --yes|-y) ASSUME_YES="true" ;;
        --delete-cluster) DELETE_CLUSTER_FLAG="true" ;;
        --dry-run) DRY_RUN="true" ;;
        -h|--help) sed -n '3,36p' "$0"; exit 0 ;;
        *) echo "$usage" >&2; exit 2 ;;
    esac
done

die() {
    echo "❌ $*" >&2
    exit 1
}

# say MESSAGE: report a change that was made (nothing, with --dry-run).
say() {
    [ "${DRY_RUN}" = "true" ] || echo "$@"
}

# run COMMAND...: run it, or with --dry-run only show it.
run() {
    if [ "${DRY_RUN}" = "true" ]; then
        echo "+ $*"
    else
        "$@"
    fi
}

# Without a terminal, a prompt below could not be answered, and the script would
# stop halfway, after deleting the application. Refuse before anything changes.
if [ "${DRY_RUN}" != "true" ] && [ ! -t 0 ]; then
    if [ "${ASSUME_YES}" != "true" ]; then
        die "deleting the namespace needs a confirmation and there is no terminal to confirm on" \
            "(pass --yes to delete it); nothing was changed"
    fi
    if [ "${DELETE_CLUSTER_FLAG}" != "true" ]; then
        die "whether to delete the kind cluster needs a confirmation and there is no terminal" \
            "to confirm on (--yes doesn't answer it; pass --delete-cluster to delete it too," \
            "or run it at a terminal); nothing was changed"
    fi
fi

echo "🗑️  Tearing down EconGraph from local Kubernetes cluster..."

# Get the project root directory
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$PROJECT_ROOT"

CLUSTER="econ-graph"
EXPECTED_CONTEXT="kind-${CLUSTER}"

# Check if kind cluster exists
if ! kind get clusters | grep -q "econ-graph"; then
    echo "❌ Kind cluster 'econ-graph' not found."
    exit 1
fi

# One kubectl, one context for everything below, chosen as
# mark-production-cluster.sh chooses it and pinned as deploy.sh pins it. A
# CLUSTER_ROLE_CONTEXT from the environment is replaced, never followed.
# kubectl becomes a function that calls kc; KUBECTL and CLUSTER_ROLE_CONTEXT are
# exported, so protect-postgres-pv.sh uses the same kubectl and context.
# shellcheck source=scripts/deploy/lib/cluster-role.sh
source "$PROJECT_ROOT/scripts/deploy/lib/cluster-role.sh"
unset CLUSTER_ROLE_CONTEXT
cluster_role_default_kubectl
export KUBECTL="${KUBECTL:-kubectl}"
command -v "${KUBECTL}" >/dev/null 2>&1 || die "${KUBECTL} not found; nothing was changed"
cluster_role_use_context "${EXPECTED_CONTEXT}" ||
    die "${KUBECTL} has no context ${EXPECTED_CONTEXT}; nothing was changed"
kubectl() { kc "$@"; }

current_context="$(kc config current-context)" ||
    die "could not read the current context of ${KUBECTL}; nothing was changed"
[ "${current_context}" = "${EXPECTED_CONTEXT}" ] ||
    die "${KUBECTL}'s context is ${current_context}, not ${EXPECTED_CONTEXT}; nothing was changed"

# The context's name alone proves nothing: it must reach the nodes of the kind
# cluster that `kind` (through Docker) knows by that name.
kind_nodes="$(kind get nodes --name "${CLUSTER}")" ||
    die "could not list the nodes of kind cluster ${CLUSTER}; nothing was changed"
[ -n "${kind_nodes}" ] || die "kind cluster ${CLUSTER} has no nodes; nothing was changed"
nodes="$(kc get nodes -o 'jsonpath={.items[*].metadata.name}')" ||
    die "could not list the nodes on context ${EXPECTED_CONTEXT}; nothing was changed"
for node in ${kind_nodes}; do
    # An HA kind cluster's load balancer is a container, not a node.
    case "${node}" in *-external-load-balancer) continue ;; esac
    case " ${nodes} " in
        *" ${node} "*) ;;
        *) die "context ${EXPECTED_CONTEXT} does not reach kind cluster ${CLUSTER}: its nodes are" \
            "'${nodes}', not ${node}; nothing was changed" ;;
    esac
done
server="$(kc config view --minify -o 'jsonpath={.clusters[0].cluster.server}')" || server="(unknown)"
uid="$(kube_system_uid)" ||
    die "could not read the kube-system namespace UID on context ${EXPECTED_CONTEXT}; nothing was changed"
marker_status=0
cluster_is_marked_production || marker_status=$?
case "${marker_status}" in
    0) marker="marked production: this is the cluster serving econgraph.com" ;;
    *) marker="${CLUSTER_ROLE_REASON}" ;;
esac

echo "  kubectl         ${KUBECTL}"
echo "  KUBECONFIG      ${KUBECONFIG:-(unset)}"
echo "  context         ${EXPECTED_CONTEXT}"
echo "  API server      ${server:-(unknown)}"
echo "  nodes           ${nodes}"
echo "  kube-system UID ${uid}"
echo "  production      ${marker}"
if [ "${DRY_RUN}" = "true" ]; then
    echo "Dry run: nothing is patched, deleted or asked; the commands are shown, all on context ${EXPECTED_CONTEXT}."
fi

# Preserve both data and backup PVs before any resource or namespace deletion.
# A failed patch stops teardown before it can remove the namespace.
run "$PROJECT_ROOT/scripts/deploy/protect-postgres-pv.sh"

# Remove application resources
echo "📋 Removing application resources..."
run kubectl delete -f k8s/manifests/admin-ingress.yaml --ignore-not-found=true
run kubectl delete -f k8s/manifests/graphql-ingress.yaml --ignore-not-found=true
run kubectl delete -f k8s/manifests/ingress.yaml --ignore-not-found=true
run kubectl delete -f k8s/manifests/frontend-service.yaml --ignore-not-found=true
run kubectl delete -f k8s/manifests/frontend-deployment.yaml --ignore-not-found=true
run kubectl delete -f k8s/manifests/admin-frontend-service.yaml --ignore-not-found=true
run kubectl delete -f k8s/manifests/admin-frontend-deployment.yaml --ignore-not-found=true
run kubectl delete -f k8s/manifests/crawler-worker.yaml --ignore-not-found=true
run kubectl delete -f k8s/manifests/backend-service.yaml --ignore-not-found=true
run kubectl delete -f k8s/manifests/backend-deployment.yaml --ignore-not-found=true
run kubectl delete -f k8s/manifests/chart-api-service.yaml --ignore-not-found=true
run kubectl delete -f k8s/manifests/chart-api-deployment.yaml --ignore-not-found=true
run kubectl delete -f k8s/manifests/postgres.yaml --ignore-not-found=true
run kubectl delete -f k8s/manifests/postgres-deployment.yaml --ignore-not-found=true
run kubectl delete -f k8s/manifests/postgres-init.yaml --ignore-not-found=true
run kubectl delete cronjob postgres-backup -n econ-graph --ignore-not-found=true
# Secrets created by scripts/deploy/create-secrets.sh (the namespace deletion
# below would remove them too; listed so the intent is explicit).
run kubectl -n econ-graph delete secret econ-graph-secrets econ-graph-postgres crawler-api-keys \
    econ-graph-keycloak grafana-admin monitoring-auth --ignore-not-found=true
run kubectl delete -f k8s/manifests/configmap.yaml --ignore-not-found=true

say "✅ Application resources removed successfully!"

# Deleting the namespace deletes the PostgreSQL and backup PVC objects.
# Their bound PVs have been set to Retain above, so their contents survive.
# Still confirm because reattaching Released volumes requires manual work.
DELETE_NAMESPACE="${ASSUME_YES}"
if [ "${ASSUME_YES}" != "true" ] && [ "${DRY_RUN}" = "true" ]; then
    echo "ℹ️  Dry run: would ask whether to delete the 'econ-graph' namespace (and its PVCs)."
elif [ "${ASSUME_YES}" != "true" ]; then
    echo ""
    echo "⚠️  Deleting the 'econ-graph' namespace deletes its PostgreSQL PVCs."
    echo "   Their retained PVs must be manually reattached after deletion."
    read -p "Delete the namespace (and its PVCs)? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        DELETE_NAMESPACE="true"
    fi
fi

if [ "${DELETE_NAMESPACE}" = "true" ]; then
    run kubectl delete -f k8s/manifests/namespace.yaml --ignore-not-found=true
    say "✅ Namespace and its PVCs removed."
else
    echo "ℹ️  Namespace kept, so its PVCs (including PostgreSQL data and backups) were not deleted."
    echo "   Delete it later with: ${KUBECTL} --context ${EXPECTED_CONTEXT} delete -f k8s/manifests/namespace.yaml"
fi

# Ask if user wants to delete the cluster. This is separate from --yes: the
# cluster's nodes hold the backup volume and Grafana's data, so this step
# destroys them no matter what happened above. Both databases are on host
# subvolumes outside the cluster and survive it.
DELETE_CLUSTER="${DELETE_CLUSTER_FLAG}"
if [ "${DELETE_CLUSTER_FLAG}" != "true" ] && [ "${DRY_RUN}" = "true" ]; then
    echo "ℹ️  Dry run: would ask whether to delete the entire kind cluster."
elif [ "${DELETE_CLUSTER_FLAG}" != "true" ]; then
    echo ""
    echo "⚠️  Deleting the kind cluster destroys the data its nodes hold: the"
    echo "   PostgreSQL backup volume and Grafana's data (Retain policy doesn't"
    echo "   help here: the cluster's node is what stores them). Both databases"
    echo "   are on host subvolumes (docs/deployment/HOST_STORAGE.md) and are kept."
    read -p "Do you want to delete the entire kind cluster? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        DELETE_CLUSTER="true"
    fi
fi

if [ "${DELETE_CLUSTER}" = "true" ]; then
    echo "🗑️  Deleting kind cluster..."
    run kind delete cluster --name econ-graph
    say "✅ Kind cluster deleted successfully!"
else
    echo "ℹ️  Kind cluster kept running. You can delete it later with:"
    echo "   kind delete cluster --name econ-graph"
fi
if [ "${DRY_RUN}" = "true" ]; then
    echo "Dry run complete: nothing was changed."
fi
