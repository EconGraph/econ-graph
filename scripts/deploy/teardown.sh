#!/bin/bash

# Teardown EconGraph from local Kubernetes cluster
# This script removes the application and optionally the cluster
#
# Usage: scripts/deploy/teardown.sh [--yes|-y] [--delete-cluster|--keep-cluster]
#            [--confirm-production UID-PREFIX] [--dry-run]
#
# It first shows the cluster it will act on and asks to go ahead, before it
# changes anything. It then sets the PVs to Retain
# (scripts/deploy/protect-postgres-pv.sh), deletes the application, and asks
# before deleting the econ-graph namespace and the kind cluster. --yes answers
# the first two questions; the cluster has its own --delete-cluster or
# --keep-cluster, never implied by --yes.
#
# On a cluster marked production (scripts/deploy/mark-production-cluster.sh),
# going ahead takes the first 8 characters of the kube-system UID shown: typed
# at the terminal, or given as --confirm-production UID-PREFIX. --yes alone
# does not answer it.
#
# Deleting the econ-graph namespace deletes the PostgreSQL PVCs and the backup
# PVC; their PVs, set to Retain, are left Released with their contents in
# place. Both databases live on host btrfs subvolumes, bind-mounted into the
# cluster (docs/deployment/HOST_STORAGE.md), so deleting the namespace or the
# kind cluster leaves them alone. The cluster's nodes do hold the backup volume
# and Grafana's data, so deleting the cluster destroys those.
#
# --dry-run shows the cluster and what would be deleted, and changes nothing:
# no context is switched, no PV is patched, nothing is deleted or asked.
#
# A confirmation that can't be typed (stdin is not a terminal) stops the script
# before it changes anything: without a terminal, pass --yes and one of
# --delete-cluster or --keep-cluster (and, on production, --confirm-production),
# or --dry-run.
#
# Every kubectl call goes to one kubectl with --context kind-econ-graph, through
# kc (scripts/deploy/lib/cluster-role.sh): ${KUBECTL}, or, when neither KUBECTL
# nor KUBECONFIG is set, the kind cluster's own kubectl and kubeconfig kept in
# ~/.cache/econgraph, as for mark-production-cluster.sh. Plain `kubectl` may
# reach another cluster (on the production host it is the MicroK8s snap
# wrapper). `kind` and `docker` use the caller's DOCKER_HOST, and one host may
# run two Docker daemons with a kind cluster econ-graph each, so before
# anything changes the script checks that the context's kube-system UID is the
# one the control-plane container on that daemon reports, and stops if not. It
# never deletes a PV or anything on the host.

set -eu

usage="usage: $0 [--yes|-y] [--delete-cluster|--keep-cluster] [--confirm-production UID-PREFIX] [--dry-run]"
ASSUME_YES="false"
DELETE_CLUSTER_FLAG="false"
KEEP_CLUSTER_FLAG="false"
CONFIRM_PRODUCTION=""
DRY_RUN="false"
while [ $# -gt 0 ]; do
    case "$1" in
        --yes|-y) ASSUME_YES="true" ;;
        --delete-cluster) DELETE_CLUSTER_FLAG="true" ;;
        --keep-cluster) KEEP_CLUSTER_FLAG="true" ;;
        --confirm-production)
            [ $# -ge 2 ] && [ -n "$2" ] || { echo "$usage" >&2; exit 2; }
            CONFIRM_PRODUCTION="$2"
            shift
            ;;
        --confirm-production=*) CONFIRM_PRODUCTION="${1#*=}" ;;
        --dry-run) DRY_RUN="true" ;;
        -h|--help) sed -n '3,45p' "$0"; exit 0 ;;
        *) echo "$usage" >&2; exit 2 ;;
    esac
    shift
done
if [ "${DELETE_CLUSTER_FLAG}" = "true" ] && [ "${KEEP_CLUSTER_FLAG}" = "true" ]; then
    echo "--delete-cluster and --keep-cluster contradict each other" >&2
    exit 2
fi

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
# stop halfway. Refuse before anything changes.
if [ "${DRY_RUN}" != "true" ] && [ ! -t 0 ]; then
    if [ "${ASSUME_YES}" != "true" ]; then
        die "going ahead and deleting the namespace need a confirmation and there is no terminal" \
            "to confirm on (pass --yes); nothing was changed"
    fi
    if [ "${DELETE_CLUSTER_FLAG}" != "true" ] && [ "${KEEP_CLUSTER_FLAG}" != "true" ]; then
        die "whether to delete the kind cluster needs a confirmation and there is no terminal" \
            "to confirm on (--yes doesn't answer it; pass --delete-cluster or --keep-cluster," \
            "or run it at a terminal); nothing was changed"
    fi
fi

echo "🗑️  Tearing down EconGraph from local Kubernetes cluster..."

# Get the project root directory
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$PROJECT_ROOT"

CLUSTER="econ-graph"
EXPECTED_CONTEXT="kind-${CLUSTER}"
CONTROL_PLANE="${CLUSTER}-control-plane"

# kind runs on Docker here; with another provider, `docker exec` below would
# check one cluster while `kind delete` removed another.
case "${KIND_EXPERIMENTAL_PROVIDER:-docker}" in
    docker) ;;
    *) die "KIND_EXPERIMENTAL_PROVIDER=${KIND_EXPERIMENTAL_PROVIDER} is not supported; nothing was changed" ;;
esac

# Check if kind cluster exists
if ! kind get clusters | grep -q "econ-graph"; then
    echo "❌ Kind cluster 'econ-graph' not found."
    exit 1
fi

# One kubectl, one context for everything below, chosen as
# mark-production-cluster.sh chooses it. A CLUSTER_ROLE_CONTEXT from the
# environment is dropped, never followed, and kind-econ-graph is pinned without
# switching the kubeconfig's current context: kc passes --context on every call.
# kubectl becomes a function that calls kc; KUBECTL and CLUSTER_ROLE_CONTEXT are
# exported, so protect-postgres-pv.sh uses the same kubectl and context.
# shellcheck source=scripts/deploy/lib/cluster-role.sh
source "$PROJECT_ROOT/scripts/deploy/lib/cluster-role.sh"
unset CLUSTER_ROLE_CONTEXT
cluster_role_default_kubectl
export KUBECTL="${KUBECTL:-kubectl}"
command -v "${KUBECTL}" >/dev/null 2>&1 || die "${KUBECTL} not found; nothing was changed"
export CLUSTER_ROLE_CONTEXT="${EXPECTED_CONTEXT}"
kc config get-contexts -o name "${EXPECTED_CONTEXT}" >/dev/null 2>&1 ||
    die "${KUBECTL} has no context ${EXPECTED_CONTEXT}; nothing was changed"
kubectl() { kc "$@"; }

# The context's name proves nothing: it must reach the very cluster that kind
# and docker (on this DOCKER_HOST) would delete. Its kube-system UID, new for
# every cluster, must be the one the control-plane container reports.
uid="$(kube_system_uid)" ||
    die "could not read the kube-system namespace UID on context ${EXPECTED_CONTEXT}; nothing was changed"
kind_uid="$(docker exec "${CONTROL_PLANE}" kubectl --kubeconfig /etc/kubernetes/admin.conf \
    get namespace kube-system -o 'jsonpath={.metadata.uid}')" ||
    die "could not read the kube-system UID inside ${CONTROL_PLANE} (DOCKER_HOST=${DOCKER_HOST:-unset});" \
        "nothing was changed"
[ "${kind_uid}" = "${uid}" ] ||
    die "context ${EXPECTED_CONTEXT} (kube-system UID ${uid}) is not the kind cluster ${CLUSTER} on" \
        "DOCKER_HOST=${DOCKER_HOST:-unset} (kube-system UID ${kind_uid:-none}); set DOCKER_HOST, KUBECTL" \
        "and KUBECONFIG to the same cluster; nothing was changed"
nodes="$(kc get nodes -o 'jsonpath={.items[*].metadata.name}')" || nodes="(unknown)"
server="$(kc config view --minify -o 'jsonpath={.clusters[0].cluster.server}')" || server="(unknown)"
production="false"
marker_status=0
cluster_is_marked_production || marker_status=$?
case "${marker_status}" in
    0) production="true"; marker="marked production: this is the cluster serving econgraph.com" ;;
    *) marker="${CLUSTER_ROLE_REASON}" ;;
esac
code="${uid:0:8}"

echo "  DOCKER_HOST     ${DOCKER_HOST:-(unset)}${DOCKER_CONTEXT:+, DOCKER_CONTEXT ${DOCKER_CONTEXT}}"
echo "  kubectl         ${KUBECTL}"
echo "  KUBECONFIG      ${KUBECONFIG:-(unset)}"
echo "  context         ${EXPECTED_CONTEXT}"
echo "  API server      ${server:-(unknown)}"
echo "  nodes           ${nodes:-(none)}"
echo "  kube-system UID ${uid} (the same inside ${CONTROL_PLANE})"
echo "  production      ${marker}"

# One confirmation for everything below, before anything changes. Production
# takes the UID prefix (typed, or --confirm-production), never --yes alone.
if [ "${DRY_RUN}" = "true" ]; then
    echo "Dry run: nothing is patched, deleted or asked; the commands are shown, all on context ${EXPECTED_CONTEXT}."
    if [ "${production}" = "true" ]; then
        echo "ℹ️  Dry run: would require the first 8 characters of the kube-system UID (${code}) to go ahead."
    elif [ "${ASSUME_YES}" != "true" ]; then
        echo "ℹ️  Dry run: would ask whether to go ahead."
    fi
elif [ "${production}" = "true" ]; then
    echo ""
    echo "!!! WARNING: this is the cluster marked production, serving econgraph.com."
    if [ -n "${CONFIRM_PRODUCTION}" ]; then
        [ "${CONFIRM_PRODUCTION}" = "${code}" ] ||
            die "--confirm-production ${CONFIRM_PRODUCTION} does not match this cluster's kube-system UID; nothing was changed"
    elif [ -t 0 ]; then
        read -r -p "Type the first 8 characters of the kube-system UID above to tear it down: " answer || answer=""
        [ "${answer}" = "${code}" ] || die "confirmation did not match; nothing was changed"
    else
        die "tearing down the production cluster needs --confirm-production with the first 8 characters" \
            "of its kube-system UID, or a terminal to type them on; nothing was changed"
    fi
elif [ "${ASSUME_YES}" != "true" ]; then
    echo ""
    read -r -p "Tear down EconGraph on this cluster (set its PVs to Retain, delete the application)? (y/N): " answer ||
        answer=""
    if [[ ! "${answer}" =~ ^[Yy]$ ]]; then
        echo "Nothing was changed."
        exit 0
    fi
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

# Ask if user wants to delete the cluster, unless --delete-cluster or
# --keep-cluster answered it. This is separate from --yes: the
# cluster's nodes hold the backup volume and Grafana's data, so this step
# destroys them no matter what happened above. Both databases are on host
# subvolumes outside the cluster and survive it.
DELETE_CLUSTER="${DELETE_CLUSTER_FLAG}"
if [ "${DELETE_CLUSTER_FLAG}" = "true" ] || [ "${KEEP_CLUSTER_FLAG}" = "true" ]; then
    :
elif [ "${DRY_RUN}" = "true" ]; then
    echo "ℹ️  Dry run: would ask whether to delete the entire kind cluster."
else
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
