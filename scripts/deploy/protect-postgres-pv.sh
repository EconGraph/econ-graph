#!/bin/bash

# Set the PersistentVolume backing the PostgreSQL data PVC to Retain, so
# deleting the PVC (e.g. via teardown.sh, or the StatefulSet being
# recreated) doesn't delete the underlying volume/data. Safe to re-run.
#
# This patches the PV directly rather than relying on a StorageClass with
# reclaimPolicy: Retain, so it works regardless of which provisioner the
# cluster uses (kind's local-path, microk8s-hostpath, EBS, etc.).

set -euo pipefail

NAMESPACE="econ-graph"
PVC_NAME="postgresql-data-postgresql-0"

echo "🔒 Setting reclaim policy to Retain for the PostgreSQL PV..."

if ! kubectl get pvc "${PVC_NAME}" -n "${NAMESPACE}" >/dev/null 2>&1; then
    echo "❌ PVC ${PVC_NAME} not found in namespace ${NAMESPACE} (has PostgreSQL been deployed?)"
    exit 1
fi

PV_NAME="$(kubectl get pvc "${PVC_NAME}" -n "${NAMESPACE}" -o jsonpath='{.spec.volumeName}')"

if [ -z "${PV_NAME}" ]; then
    echo "❌ PVC ${PVC_NAME} is not bound yet; wait for it to be Bound and re-run."
    exit 1
fi

CURRENT_POLICY="$(kubectl get pv "${PV_NAME}" -o jsonpath='{.spec.persistentVolumeReclaimPolicy}')"

if [ "${CURRENT_POLICY}" = "Retain" ]; then
    echo "✅ ${PV_NAME} is already set to Retain."
    exit 0
fi

kubectl patch pv "${PV_NAME}" -p '{"spec":{"persistentVolumeReclaimPolicy":"Retain"}}'
echo "✅ ${PV_NAME} (backing ${PVC_NAME}) is now Retain. Deleting the PVC will no longer delete this data;"
echo "   a stale Released PV must be manually reclaimed or deleted once you're sure the data isn't needed."
