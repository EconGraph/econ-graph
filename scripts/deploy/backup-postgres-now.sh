#!/bin/bash

# Trigger an ad-hoc run of the nightly postgres-backup CronJob, without
# waiting for its schedule. Useful before a risky migration, or to verify
# the backup pipeline works.
#
# Usage:
#   ./scripts/deploy/backup-postgres-now.sh [--context CONTEXT]
#
#   --context CONTEXT  the kubectl context (default kind-econ-graph); the
#                      current context is not changed
#
# Every kubectl call goes through kc (scripts/deploy/lib/cluster-role.sh) on
# that one context. With none of KUBECTL, KUBECONFIG and CLUSTER_ROLE_CONTEXT
# set, it uses the kind cluster's own kubectl and kubeconfig when copies are
# kept in ~/.cache/econgraph (bin/kubectl, kubeconfig-econ-graph): plain
# kubectl may be another cluster's (the microk8s snap wrapper ignores
# ~/.kube/config). It shows the cluster's identity before starting the Job.

set -euo pipefail

# shellcheck source=scripts/deploy/lib/cluster-role.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/cluster-role.sh"

NAMESPACE="econ-graph"
JOB_NAME="postgres-backup-manual-$(date +%s)"
CONTEXT="kind-econ-graph"

while [ $# -gt 0 ]; do
    case "$1" in
        --context)
            [ $# -ge 2 ] || { echo "--context needs a value" >&2; exit 2; }
            CONTEXT="$2"; shift 2 ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

# One kubectl and one context for every call below, never a
# CLUSTER_ROLE_CONTEXT inherited from the environment.
unset CLUSTER_ROLE_CONTEXT
cluster_role_default_kubectl
KUBECTL="${KUBECTL:-kubectl}"
command -v "${KUBECTL}" >/dev/null 2>&1 || { echo "${KUBECTL} not found" >&2; exit 1; }
cluster_role_pin_named_context "${CONTEXT}" ||
    { echo "${KUBECTL} has no context ${CONTEXT} (KUBECONFIG=${KUBECONFIG:-unset}); nothing was started" >&2; exit 1; }

echo "Cluster:"
cluster_role_show_identity

echo "🗄️  Triggering an ad-hoc backup job: ${JOB_NAME}"
kc create job "${JOB_NAME}" --from=cronjob/postgres-backup -n "${NAMESPACE}"

echo "⏳ Waiting for it to complete..."
kc wait --for=condition=complete "job/${JOB_NAME}" -n "${NAMESPACE}" --timeout=300s || {
    echo "❌ Backup job did not complete successfully; logs:"
    kc logs "job/${JOB_NAME}" -n "${NAMESPACE}" --all-containers=true --prefix=true
    exit 1
}

echo "✅ Backup complete."
kc logs "job/${JOB_NAME}" -n "${NAMESPACE}" --all-containers=true --prefix=true
