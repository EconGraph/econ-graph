#!/bin/bash
# Restore a host-side logical dump into the live cluster database.
# DESTRUCTIVE: pg_restore --clean drops and recreates every object in the
# dump. This is a "critical" change under the repo's rules: get Joe's
# sign-off and take snapshot-now.sh + dump-databases.sh first.
#
# Usage: restore-dump.sh [--yes] <database> <dump file>
#   <database> is econ_graph or keycloak (see BACKUP_DATABASES in lib.sh).
#
# Scale the backend, crawler and (for keycloak) Keycloak to 0 first: a live
# connection holding a lock stalls --clean, and a crawler writing mid-restore
# races it. Runs in one transaction, so a failure leaves the database as it
# was.

set -euo pipefail

# shellcheck source=scripts/backup/lib.sh
source "$(dirname "$(readlink -f "$0")")/lib.sh"

yes=0
args=()
for a in "$@"; do
    case "$a" in
        --yes|-y) yes=1 ;;
        -h|--help) sed -n '2,14p' "$0"; exit 0 ;;
        *) args+=("$a") ;;
    esac
done
[[ ${#args[@]} -eq 2 ]] || die "usage: $(basename "$0") [--yes] <database> <dump file>"
db="${args[0]}"
file="${args[1]}"
[[ -s "${file}" ]] || die "dump not found or empty: ${file}"

pod="" user=""
for entry in ${BACKUP_DATABASES}; do
    IFS=: read -r d p u <<<"${entry}"
    if [[ "${d}" == "${db}" ]]; then pod="${p}"; user="${u}"; fi
done
[[ -n "${pod}" ]] || die "unknown database ${db} (known: ${BACKUP_DATABASES})"

read -r -a RESTORE <<<"$(pg_restore_cmd "$(dirname "$(readlink -f "${file}")")")"
"${RESTORE[@]}" --list "$(readlink -f "${file}")" >/dev/null || die "${file} is not a readable pg_dump archive"

echo "About to OVERWRITE database ${db} in pod ${pod} (${K8S_NAMESPACE}) from ${file}."
if (( ! yes )); then
    read -r -p "Type the database name to proceed: " answer
    [[ "${answer}" == "${db}" ]] || die "aborted"
fi

log "restoring ${file} into ${db}"
kube exec -i "${pod}" -- pg_restore --username="${user}" --dbname="${db}" \
    --clean --if-exists --single-transaction --exit-on-error <"${file}"
log "restore of ${db} complete; check diesel migration state and scale services back up"
