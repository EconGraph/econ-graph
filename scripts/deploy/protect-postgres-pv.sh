#!/bin/bash

# Set the PersistentVolumes backing PostgreSQL and its backups to Retain.
# Safe to re-run, including from teardown before deleting the namespace.
#
# This patches the PV directly rather than relying on a StorageClass with
# reclaimPolicy: Retain, so it works regardless of which provisioner the
# cluster uses (kind's local-path, microk8s-hostpath, EBS, etc.).
#
# Usage: scripts/deploy/protect-postgres-pv.sh [--dry-run]
#   --dry-run  reads the PVCs and PVs and prints the patches; changes nothing.

set -euo pipefail

DRY_RUN=false
for arg in "$@"; do
    case "$arg" in
        --dry-run|-n) DRY_RUN=true ;;
        *) echo "usage: $0 [--dry-run]" >&2; exit 2 ;;
    esac
done

# Every call goes through kc (scripts/deploy/lib/cluster-role.sh): the KUBECTL
# and CLUSTER_ROLE_CONTEXT that deploy.sh and teardown.sh export, or, run by
# hand with neither, the kind cluster's own kubectl and kubeconfig kept in
# ~/.cache/econgraph (cluster_role_default_kubectl); without them it stops.
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/deploy/lib/cluster-role.sh
source "$PROJECT_ROOT/scripts/deploy/lib/cluster-role.sh"
cluster_role_default_kubectl || exit 1

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
    elif [ "${DRY_RUN}" = true ]; then
        echo "+ kubectl patch pv ${pv_name} (backing ${pvc_name}): persistentVolumeReclaimPolicy ${current_policy} -> Retain"
    else
        kc patch pv "${pv_name}" -p '{"spec":{"persistentVolumeReclaimPolicy":"Retain"}}'
        echo "✅ ${pv_name} (backing ${pvc_name}) is now Retain."
    fi
}

echo "🔒 Protecting PostgreSQL data and backup volumes..."
protect_pvc "postgresql-data-postgresql-0" true
protect_pvc "app-postgres-data-app-postgres-0" false
protect_pvc "keycloak-postgres-data-keycloak-postgres-0" false
protect_pvc "postgres-backup-data" false
echo "   Released volumes must be manually reclaimed after namespace deletion."
