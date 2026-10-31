#!/bin/bash

# Trigger an ad-hoc run of the nightly postgres-backup CronJob, without
# waiting for its schedule. Useful before a risky migration, or to verify
# the backup pipeline works.
#
# Usage:
#   ./scripts/deploy/backup-postgres-now.sh [--context CONTEXT] [--timeout SECONDS] [--dry-run]
#
#   --context CONTEXT  the kubectl context (default kind-econ-graph); the
#                      current context is not changed
#   --timeout SECONDS  how long to wait for the Job (default BACKUP_WAIT_TIMEOUT, else
#                      3600). A Job that fails ends the wait at once, so the default
#                      only matters for a backup that is still running: a large
#                      database's dump and read-back can take many minutes.
#   --dry-run          show the cluster and the Job it would start; start nothing
#
# One backup at a time: while a Job of the postgres-backup CronJob (scheduled or
# ad hoc) is running, no other is started; the script says which, and stops.
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
TIMEOUT="${BACKUP_WAIT_TIMEOUT:-3600}"
# Seconds between looks at the Job (the tests set 0).
POLL="${BACKUP_POLL_SECONDS:-5}"

while [ $# -gt 0 ]; do
    case "$1" in
        --context)
            [ $# -ge 2 ] && [ -n "$2" ] || { echo "--context needs a non-empty value" >&2; exit 2; }
            CONTEXT="$2"; shift 2 ;;
        --timeout)
            [ $# -ge 2 ] || { echo "--timeout needs a number of seconds" >&2; exit 2; }
            TIMEOUT="$2"; shift 2 ;;
        --dry-run|-n) DRY_RUN="true"; shift ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

[[ "${TIMEOUT}" =~ ^[1-9][0-9]*$ ]] || { echo "--timeout (or BACKUP_WAIT_TIMEOUT) must be a positive number of seconds, not '${TIMEOUT}'" >&2; exit 2; }

cluster_role_operator_context "${CONTEXT}" || { echo "Nothing was started." >&2; exit 1; }

# still_running JOB: say a backup Job is running and was not stopped, in the line
# release-deploy.sh reads, then stop.
still_running() {
    echo "   Wait for it, and don't start another backup until it has:"
    echo "   kubectl --context ${CONTEXT} -n ${NAMESPACE} wait --for=condition=complete job/$1 --timeout=3600s"
    echo "backup-postgres-now: still running: job/$1"
    exit 1
}

# A Job of the CronJob, by its owner or its name, that has neither completed nor
# failed: two backups at once would dump the same databases side by side. Unfinished
# rather than with active pods, so a Job between a failed pod and its retry counts.
jobs="$(kc get jobs -n "${NAMESPACE}" \
    -o 'jsonpath={range .items[*]}{.metadata.name}{"\t"}{.metadata.ownerReferences[0].name}{"\t"}{range .status.conditions[?(@.status=="True")]}{.type}{" "}{end}{"\n"}{end}')" ||
    { echo "could not list the Jobs in ${NAMESPACE}, to check no backup is running. Nothing was started." >&2; exit 1; }
running="$(awk -F'\t' '($2 == "postgres-backup" || $1 ~ /^postgres-backup-/) && $3 !~ /(^| )(Complete|Failed) / { print $1; exit }' <<<"${jobs}")"

if [ "${DRY_RUN}" = "true" ]; then
    if [ -n "${running}" ]; then
        echo "Dry run: job/${running} is running, so no backup would be started."
    else
        echo "Dry run: would create job/${JOB_NAME} from cronjob/postgres-backup in ${NAMESPACE} on context ${CONTEXT}, wait up to ${TIMEOUT}s for it and print its logs."
    fi
    exit 0
fi

if [ -n "${running}" ]; then
    echo "⏳ job/${running} from cronjob/postgres-backup is running; not starting another backup beside it."
    still_running "${running}"
fi

echo "🗄️  Triggering an ad-hoc backup job: ${JOB_NAME}"
kc create job "${JOB_NAME}" --from=cronjob/postgres-backup -n "${NAMESPACE}"

# Until the Job completes or fails, or the timeout: `kubectl wait` for Complete alone
# would sit out the whole timeout on a Job that failed.
echo "⏳ Waiting up to ${TIMEOUT}s for it to complete..."
# A Job that is gone, or a cluster that can't be read five times in a row, ends the
# wait at once: neither is "still running".
deadline=$((SECONDS + TIMEOUT))
conditions=""
errors=0
while :; do
    if conditions="$(kc get "job/${JOB_NAME}" -n "${NAMESPACE}" \
        -o 'jsonpath={range .status.conditions[?(@.status=="True")]}{.type}{"\n"}{end}' 2>&1)"; then
        errors=0
    elif grep -q NotFound <<<"${conditions}"; then
        echo "❌ job/${JOB_NAME} is gone (deleted while it ran?); no backup is known to have completed."
        exit 1
    else
        errors=$((errors + 1))
        if [ "${errors}" -ge 5 ]; then
            echo "❌ could not read job/${JOB_NAME} five times in a row; it was not stopped:"
            printf '   %s\n' "${conditions}"
            exit 1
        fi
        conditions=""
    fi
    if grep -qx -e Complete -e Failed <<<"${conditions}" || [ "${SECONDS}" -ge "${deadline}" ]; then
        break
    fi
    sleep "${POLL}"
done

if ! grep -qx Complete <<<"${conditions}"; then
    if ! grep -qx Failed <<<"${conditions}"; then
        # Still running after the wait: it was not stopped, and may still finish. Say
        # so, so nobody starts a second backup beside it.
        echo "⏳ job/${JOB_NAME} is still running after ${TIMEOUT}s; it was not stopped and may still finish."
        still_running "${JOB_NAME}"
    fi
    echo "❌ Backup job did not complete successfully; logs:"
    kc logs "job/${JOB_NAME}" -n "${NAMESPACE}" --all-containers=true --prefix=true
    exit 1
fi

echo "✅ Backup complete."
kc logs "job/${JOB_NAME}" -n "${NAMESPACE}" --all-containers=true --prefix=true
