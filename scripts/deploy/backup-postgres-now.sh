#!/bin/bash

# Trigger an ad-hoc run of the nightly postgres-backup CronJob, without
# waiting for its schedule. Useful before a risky migration, or to verify
# the backup pipeline works.

set -euo pipefail

NAMESPACE="econ-graph"
JOB_NAME="postgres-backup-manual-$(date +%s)"

echo "🗄️  Triggering an ad-hoc backup job: ${JOB_NAME}"
kubectl create job "${JOB_NAME}" --from=cronjob/postgres-backup -n "${NAMESPACE}"

echo "⏳ Waiting for it to complete..."
kubectl wait --for=condition=complete "job/${JOB_NAME}" -n "${NAMESPACE}" --timeout=300s || {
    echo "❌ Backup job did not complete successfully; logs:"
    kubectl logs "job/${JOB_NAME}" -n "${NAMESPACE}" --all-containers=true --prefix=true
    exit 1
}

echo "✅ Backup complete."
kubectl logs "job/${JOB_NAME}" -n "${NAMESPACE}" --all-containers=true --prefix=true
