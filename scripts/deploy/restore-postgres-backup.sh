#!/bin/bash

# Restore a PostgreSQL backup produced by the postgres-backup CronJob
# (k8s/manifests/postgres-backup-cronjob.yaml) into the running database.
#
# This is DESTRUCTIVE: pg_restore --clean drops and recreates every object
# in the backup before restoring it, overwriting current data. It prompts
# for confirmation unless --yes is given.
#
# Usage:
#   ./scripts/deploy/restore-postgres-backup.sh --list [--context CONTEXT] [--dry-run]
#   ./scripts/deploy/restore-postgres-backup.sh [--file econ_graph-20260101-080000.dump] [--yes]
#       [--context CONTEXT] [--dry-run]
#
#   --context CONTEXT  the kubectl context (default kind-econ-graph); the
#                      current context is not changed
#   --dry-run          show the cluster and what would be listed or restored;
#                      start no pod or Job and change nothing
#
# Every kubectl call goes through kc (scripts/deploy/lib/cluster-role.sh) on
# that one context. With none of KUBECTL, KUBECONFIG and CLUSTER_ROLE_CONTEXT
# set, it uses the kind cluster's own kubectl and kubeconfig when copies are
# kept in ~/.cache/econgraph (bin/kubectl, kubeconfig-econ-graph): plain
# kubectl may be another cluster's (the microk8s snap wrapper ignores
# ~/.kube/config). It shows the cluster's identity before the confirmation.
#
# See docs/deployment/POSTGRES_BACKUP_RESTORE.md for the full runbook.

set -euo pipefail

# shellcheck source=scripts/deploy/lib/cluster-role.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/cluster-role.sh"

NAMESPACE="econ-graph"
JOB_NAME="postgres-restore-$(date +%s)"
BACKUP_FILE=""
ASSUME_YES="false"
LIST_ONLY="false"
DRY_RUN="false"
CONTEXT="kind-econ-graph"

while [ $# -gt 0 ]; do
    case "$1" in
        --file)
            [ $# -ge 2 ] || { echo "--file needs a value" >&2; exit 2; }
            BACKUP_FILE="$2"; shift 2 ;;
        --yes|-y) ASSUME_YES="true"; shift ;;
        --list) LIST_ONLY="true"; shift ;;
        --dry-run|-n) DRY_RUN="true"; shift ;;
        --context)
            [ $# -ge 2 ] && [ -n "$2" ] || { echo "--context needs a non-empty value" >&2; exit 2; }
            CONTEXT="$2"; shift 2 ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

cluster_role_operator_context "${CONTEXT}" || { echo "Nothing was changed." >&2; exit 1; }

if [ "${LIST_ONLY}" = "true" ]; then
    if [ "${DRY_RUN}" = "true" ]; then
        echo "Dry run: would list postgres-backup-data with a short-lived pod in ${NAMESPACE} on context ${CONTEXT}."
        exit 0
    fi
    echo "📋 Backups available in postgres-backup-data:"
    kc run "postgres-backup-list-$(date +%s)" --rm -i --restart=Never \
        --image=busybox:1.38 -n "${NAMESPACE}" \
        --overrides="{\"spec\":{\"containers\":[{\"name\":\"list\",\"image\":\"busybox:1.38\",\"command\":[\"ls\",\"-la\",\"/backups\"],\"volumeMounts\":[{\"name\":\"backup-data\",\"mountPath\":\"/backups\"}]}],\"volumes\":[{\"name\":\"backup-data\",\"persistentVolumeClaim\":{\"claimName\":\"postgres-backup-data\"}}]}}" \
        -- true
    exit 0
fi

if [ "${DRY_RUN}" = "true" ]; then
    echo "Dry run: would DROP and recreate database objects in 'econ_graph' in namespace ${NAMESPACE}"
    echo "  on context ${CONTEXT}, restoring from ${BACKUP_FILE:-the latest backup}, through a restore Job."
    [ "${ASSUME_YES}" = "true" ] || echo "  (would ask for confirmation first)"
    exit 0
fi

if [ "${ASSUME_YES}" != "true" ]; then
    echo "⚠️  This will DROP and recreate database objects in 'econ_graph' in namespace"
    echo "   ${NAMESPACE} on the cluster above (context ${CONTEXT}), restoring from"
    echo "   ${BACKUP_FILE:-the latest backup}."
    [ -t 0 ] || { echo "The confirmation is read from a terminal only; pass --yes to skip it." >&2; exit 1; }
    read -r -p "Type 'yes' to continue: " CONFIRM
    if [ "${CONFIRM}" != "yes" ]; then
        echo "Aborted."
        exit 1
    fi
fi

BACKUP_FILE_ENV=""
if [ -n "${BACKUP_FILE}" ]; then
    BACKUP_FILE_ENV="/backups/${BACKUP_FILE}"
fi

echo "🔁 Running restore job ${JOB_NAME}..."
cat <<EOF | kc apply -f -
apiVersion: batch/v1
kind: Job
metadata:
  name: ${JOB_NAME}
  namespace: ${NAMESPACE}
  labels:
    app: postgres-restore
    component: restore
spec:
  backoffLimit: 0
  template:
    spec:
      restartPolicy: Never
      securityContext:
        runAsNonRoot: true
        runAsUser: 999
        fsGroup: 999
      containers:
      - name: pg-restore
        image: postgres:18
        command: ["/scripts/restore.sh"]
        env:
        - name: PGHOST
          value: "postgres-service"
        - name: PGPORT
          value: "5432"
        - name: PGUSER
          value: "postgres"
        - name: PGPASSWORD
          valueFrom:
            secretKeyRef:
              name: econ-graph-postgres
              key: postgres-password
        - name: PGDATABASE
          value: "econ_graph"
        - name: BACKUP_DIR
          value: "/backups"
        - name: BACKUP_FILE
          value: "${BACKUP_FILE_ENV}"
        volumeMounts:
        - name: scripts
          mountPath: /scripts
        - name: backup-data
          mountPath: /backups
          readOnly: true
        securityContext:
          allowPrivilegeEscalation: false
          readOnlyRootFilesystem: true
          capabilities:
            drop: ["ALL"]
          seccompProfile:
            type: RuntimeDefault
      volumes:
      - name: scripts
        configMap:
          name: postgres-backup-scripts
          defaultMode: 0555
      - name: backup-data
        persistentVolumeClaim:
          claimName: postgres-backup-data
EOF

echo "⏳ Waiting for the restore to complete..."
kc wait --for=condition=complete "job/${JOB_NAME}" -n "${NAMESPACE}" --timeout=600s || {
    echo "❌ Restore did not complete successfully; logs:"
    kc logs "job/${JOB_NAME}" -n "${NAMESPACE}"
    exit 1
}

kc logs "job/${JOB_NAME}" -n "${NAMESPACE}"
kc delete job "${JOB_NAME}" -n "${NAMESPACE}" --ignore-not-found=true
echo "✅ Restore complete."
