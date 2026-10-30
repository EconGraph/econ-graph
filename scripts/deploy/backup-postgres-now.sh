#!/bin/bash

# Trigger an ad-hoc run of the nightly postgres-backup CronJob, without
# waiting for its schedule. Useful before a risky migration, or to verify
# the backup pipeline works.
#
# Usage:
#   ./scripts/deploy/backup-postgres-now.sh [--context CONTEXT] [--dry-run]
#
#   --context CONTEXT  the kubectl context (default kind-econ-graph); the
#                      current context is not changed
#   --dry-run          show the cluster and the Job it would start; start nothing
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
DRY_RUN="false"

while [ $# -gt 0 ]; do
    case "$1" in
        --context)
            [ $# -ge 2 ] && [ -n "$2" ] || { echo "--context needs a non-empty value" >&2; exit 2; }
            CONTEXT="$2"; shift 2 ;;
        --dry-run|-n) DRY_RUN="true"; shift ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

cluster_role_operator_context "${CONTEXT}" || { echo "Nothing was started." >&2; exit 1; }

if [ "${DRY_RUN}" = "true" ]; then
    echo "Dry run: would create job/${JOB_NAME} from cronjob/postgres-backup in ${NAMESPACE} on context ${CONTEXT}, wait for it and print its logs."
    exit 0
fi

echo "🗄️  Triggering an ad-hoc backup job: ${JOB_NAME}"
kc create job "${JOB_NAME}" --from=cronjob/postgres-backup -n "${NAMESPACE}"

echo "⏳ Waiting for it to complete..."
kc wait --for=condition=complete "job/${JOB_NAME}" -n "${NAMESPACE}" --timeout=300s || {
    # A large database can take longer than the wait. A Job that is still
    # running was not stopped, and may still finish: say so, so nobody starts
    # a second backup beside it.
    active="$(kc get "job/${JOB_NAME}" -n "${NAMESPACE}" -o 'jsonpath={.status.active}' 2>/dev/null || true)"
    if [ -n "${active}" ] && [ "${active}" != 0 ]; then
        echo "⏳ job/${JOB_NAME} is still running after 5 minutes; it was not stopped and may still finish."
        echo "   Wait for it, and don't start another backup until it has:"
        echo "   kubectl --context ${CONTEXT} -n ${NAMESPACE} wait --for=condition=complete job/${JOB_NAME} --timeout=3600s"
        echo "backup-postgres-now: still running: job/${JOB_NAME}"
        exit 1
    fi
    echo "❌ Backup job did not complete successfully; logs:"
    kc logs "job/${JOB_NAME}" -n "${NAMESPACE}" --all-containers=true --prefix=true
    exit 1
}

echo "✅ Backup complete."
kc logs "job/${JOB_NAME}" -n "${NAMESPACE}" --all-containers=true --prefix=true
