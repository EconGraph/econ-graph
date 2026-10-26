#!/bin/bash

# Set the PersistentVolumes backing both PostgreSQL databases and the backups to Retain.
# Safe to re-run, including from teardown before deleting the namespace.
#
# This patches the PV directly rather than relying on a StorageClass with
# reclaimPolicy: Retain, so it works regardless of which provisioner the
# cluster uses (kind's local-path, microk8s-hostpath, EBS, etc.). It covers
# the PVCs of both databases (the host-storage PVs econgraph-data-plane-pg and
# econgraph-app-prod-keycloak-pg are Retain already) and the backup PVC
# postgres-backup-data, whose dynamically provisioned PV starts out Delete.
#
# Every kubectl call goes through kc (scripts/deploy/lib/cluster-role.sh):
# ${KUBECTL:-kubectl} on the context deploy.sh pinned, or, run by hand, the
# kind cluster's own kubectl and kubeconfig from ~/.cache/econgraph when
# KUBECTL and KUBECONFIG are unset, pinned to its current context.

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/deploy/lib/cluster-role.sh
source "${PROJECT_ROOT}/scripts/deploy/lib/cluster-role.sh"
cluster_role_default_kubectl
if ! cluster_role_pin_context; then
    echo "❌ ${KUBECTL:-kubectl} has no current context" >&2
    exit 1
fi
echo "   kubectl: ${KUBECTL:-kubectl}, context ${CLUSTER_ROLE_CONTEXT}"

NAMESPACE="econ-graph"
protect_pvc() {
    local pvc_name="$1"
    local required="$2"
    local pv_name current_policy

    local pvc_resource
    pvc_resource="$(kc get pvc "${pvc_name}" -n "${NAMESPACE}" -o name --ignore-not-found)"
    if [ -z "${pvc_resource}" ]; then
        if [ "${required}" = true ]; then
            echo "❌ PVC ${pvc_name} not found in namespace ${NAMESPACE}." >&2
            return 1
        fi
        echo "ℹ️  PVC ${pvc_name} does not exist yet."
        return 0
    fi

    pv_name="$(kc get pvc "${pvc_name}" -n "${NAMESPACE}" -o jsonpath='{.spec.volumeName}')"
    if [ -z "${pv_name}" ]; then
        # A Pending claim has no volume or backups to preserve yet.
        echo "ℹ️  PVC ${pvc_name} is not bound yet; check again after the first backup job."
        if [ "${required}" = true ]; then return 1; fi
        return 0
    fi

    current_policy="$(kc get pv "${pv_name}" -o jsonpath='{.spec.persistentVolumeReclaimPolicy}')"
    if [ "${current_policy}" = Retain ]; then
        echo "✅ ${pv_name} (backing ${pvc_name}) is already set to Retain."
    else
        kc patch pv "${pv_name}" -p '{"spec":{"persistentVolumeReclaimPolicy":"Retain"}}'
        echo "✅ ${pv_name} (backing ${pvc_name}) is now Retain."
    fi
}

echo "🔒 Protecting PostgreSQL data and backup volumes..."
protect_pvc "postgresql-data-postgresql-0" true
protect_pvc "keycloak-postgres-data-keycloak-postgres-0" false
protect_pvc "postgres-backup-data" false
echo "   Released volumes must be manually reclaimed after namespace deletion."
