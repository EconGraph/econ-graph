#!/usr/bin/env bash
# Cron/Kubernetes entrypoint. Dry-run unless explicit apply configuration is supplied.
set -Eeuo pipefail
: "${ICEBERG_COMPACTION_MAX_ROWS:?set an explicit tested compaction row budget}"
command=("${TIMESERIES_MAINTENANCE_BINARY:-timeseries}" maintenance --max-rows "$ICEBERG_COMPACTION_MAX_ROWS")
case "${TIMESERIES_MAINTENANCE_APPLY:-false}" in
  true) command+=(--apply) ;;
  false) ;;
  *) echo 'TIMESERIES_MAINTENANCE_APPLY must be true or false' >&2; exit 2 ;;
esac
if [[ -n "${ICEBERG_EXPIRE_BEFORE:-}" ]]; then
  command+=(--expire-before "$ICEBERG_EXPIRE_BEFORE" --retain-last "${ICEBERG_RETAIN_LAST:-1}")
fi
exec "${command[@]}"
