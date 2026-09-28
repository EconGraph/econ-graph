#!/bin/bash

# Set the PersistentVolumes backing PostgreSQL and its backups to Retain.
# Safe to re-run, including from teardown before deleting the namespace.
#
# This patches the PV directly rather than relying on a StorageClass with
# reclaimPolicy: Retain, so it works regardless of which provisioner the
# cluster uses (kind's local-path, microk8s-hostpath, EBS, etc.).

set -euo pipefail

NAMESPACE="econ-graph"
protect_pvc() {
    local pvc_name="$1"
    local required="$2"
    local pv_name current_policy

    if ! kubectl get pvc "${pvc_name}" -n "${NAMESPACE}" >/dev/null 2>&1; then
        if [ "${required}" = true ]; then
            echo "❌ PVC ${pvc_name} not found in namespace ${NAMESPACE}." >&2
            return 1
        fi
        echo "ℹ️  Backup PVC ${pvc_name} does not exist yet."
        return 0
    fi

    pv_name="$(kubectl get pvc "${pvc_name}" -n "${NAMESPACE}" -o jsonpath='{.spec.volumeName}')"
    if [ -z "${pv_name}" ]; then
        # A Pending claim has no volume or backups to preserve yet.
        echo "ℹ️  PVC ${pvc_name} is not bound yet; check again after the first backup job."
        if [ "${required}" = true ]; then return 1; fi
        return 0
    fi

    current_policy="$(kubectl get pv "${pv_name}" -o jsonpath='{.spec.persistentVolumeReclaimPolicy}')"
    if [ "${current_policy}" = Retain ]; then
        echo "✅ ${pv_name} (backing ${pvc_name}) is already set to Retain."
    else
        kubectl patch pv "${pv_name}" -p '{"spec":{"persistentVolumeReclaimPolicy":"Retain"}}'
        echo "✅ ${pv_name} (backing ${pvc_name}) is now Retain."
    fi
}

echo "🔒 Protecting PostgreSQL data and backup volumes..."
protect_pvc "postgresql-data-postgresql-0" true
protect_pvc "postgres-backup-data" false
echo "   Released volumes must be manually reclaimed after namespace deletion."
