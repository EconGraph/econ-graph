#!/bin/bash

# Restore a PostgreSQL backup produced by the postgres-backup CronJob
# (k8s/manifests/postgres-backup-cronjob.yaml) into the running database.
#
# This is DESTRUCTIVE: pg_restore --clean drops and recreates every object
# in the backup before restoring it, overwriting current data. It prompts
# for confirmation unless --yes is given.
#
# Usage:
#   ./scripts/deploy/restore-postgres-backup.sh --list
#   ./scripts/deploy/restore-postgres-backup.sh [--file econ_graph-20260101-080000.dump] [--yes]
#
# See docs/deployment/POSTGRES_BACKUP_RESTORE.md for the full runbook.

set -euo pipefail

NAMESPACE="econ-graph"
JOB_NAME="postgres-restore-$(date +%s)"
BACKUP_FILE=""
ASSUME_YES="false"
LIST_ONLY="false"

while [ $# -gt 0 ]; do
    case "$1" in
        --file) BACKUP_FILE="$2"; shift 2 ;;
        --yes|-y) ASSUME_YES="true"; shift ;;
        --list) LIST_ONLY="true"; shift ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

if [ "${LIST_ONLY}" = "true" ]; then
    echo "📋 Backups available in postgres-backup-data:"
    kubectl run "postgres-backup-list-$(date +%s)" --rm -i --restart=Never \
        --image=busybox:1.36 -n "${NAMESPACE}" \
        --overrides="{\"spec\":{\"containers\":[{\"name\":\"list\",\"image\":\"busybox:1.36\",\"command\":[\"ls\",\"-la\",\"/backups\"],\"volumeMounts\":[{\"name\":\"backup-data\",\"mountPath\":\"/backups\"}]}],\"volumes\":[{\"name\":\"backup-data\",\"persistentVolumeClaim\":{\"claimName\":\"postgres-backup-data\"}}]}}" \
        -- true
    exit 0
fi

if [ "${ASSUME_YES}" != "true" ]; then
    echo "⚠️  This will DROP and recreate database objects in 'econ_graph' on the"
    echo "   ${NAMESPACE} cluster, restoring from ${BACKUP_FILE:-the latest backup}."
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
cat <<EOF | kubectl apply -f -
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
              name: econ-graph-secrets
              key: postgres-superuser-password
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
kubectl wait --for=condition=complete "job/${JOB_NAME}" -n "${NAMESPACE}" --timeout=600s || {
    echo "❌ Restore did not complete successfully; logs:"
    kubectl logs "job/${JOB_NAME}" -n "${NAMESPACE}"
    exit 1
}

kubectl logs "job/${JOB_NAME}" -n "${NAMESPACE}"
kubectl delete job "${JOB_NAME}" -n "${NAMESPACE}" --ignore-not-found=true
echo "✅ Restore complete."
