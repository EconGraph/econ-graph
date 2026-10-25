#!/bin/bash
# Restore a host-side logical dump into the live cluster database.
# DESTRUCTIVE: pg_restore --clean drops and recreates every object in the
# dump. This is a "critical" change under the repo's rules: get the maintainer's
# sign-off and take snapshot-now.sh + dump-databases.sh first.
#
# Usage: restore-dump.sh [--dry-run] [--yes] <database> <dump file>
#   <database> is econ_graph or keycloak (see BACKUP_DATABASES in lib.sh).
#   --dry-run checks the dump (pg_restore --list) and the pod, prints the
#   plan and changes nothing. Without it you are asked to type the
#   database name; --yes skips that.
#
# Scale the backend, crawler and (for keycloak) Keycloak to 0 first: a live
# connection holding a lock stalls --clean, and a crawler writing mid-restore
# races it. Runs in one transaction, so a failure leaves the database as it
# was.

set -euo pipefail

# shellcheck source=scripts/backup/lib.sh
source "$(dirname "$(readlink -f "$0")")/lib.sh"

yes=0
dry_run=0
args=()
for a in "$@"; do
    case "$a" in
        --yes|-y) yes=1 ;;
        --dry-run|-n) dry_run=1 ;;
        -h|--help) sed -n '2,16p' "$0"; exit 0 ;;
        -*) die "unknown option: $a" ;;
        *) args+=("$a") ;;
    esac
done
[[ ${#args[@]} -eq 2 ]] || die "usage: $(basename "$0") [--dry-run] [--yes] <database> <dump file>"
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
toc="$("${RESTORE[@]}" --list "$(readlink -f "${file}")")" || die "${file} is not a readable pg_dump archive"
# The archive must be a dump of the database it is restored into: --clean
# drops same-named objects, so keycloak.dump into econ_graph would wreck it.
# Compared as a string, not a pattern.
archive_db="$(sed -nE 's/^;[[:space:]]+dbname: //p' <<<"${toc}" | head -n 1)"
[[ "${archive_db}" == "${db}" ]] \
    || die "${file} is a dump of ${archive_db:-an unknown database}, not of ${db}; refusing"

# Relations (tables, views, sequences; not those an extension owns) in the
# target that the dump does not have: --clean drops only what is in the dump, so these survive the restore
# and leave a mixed schema. Listed before the confirmation (and in a dry
# run) so the operator decides with them in view; nothing here drops them.
extra_relations() {
    local dumped live
    dumped="$(sed -E '/ (TABLE DATA|SEQUENCE SET|SEQUENCE OWNED BY) /d' <<<"${toc}" \
        | sed -nE 's/^[0-9]+; [0-9]+ [0-9]+ (TABLE|VIEW|SEQUENCE|MATERIALIZED VIEW) ([^ ]+) ([^ ]+) .*/\2.\3/p' | sort -u)"
    live="$(kube exec "${pod}" -- psql --username="${user}" --dbname="${db}" -Atc \
        "SELECT n.nspname || '.' || c.relname FROM pg_class c
           JOIN pg_namespace n ON n.oid = c.relnamespace
          WHERE n.nspname NOT LIKE 'pg\\_%' AND n.nspname <> 'information_schema'
            AND c.relkind IN ('r', 'p', 'v', 'm', 'S')
            AND NOT EXISTS (SELECT 1 FROM pg_depend d
                             WHERE d.classid = 'pg_class'::regclass
                               AND d.objid = c.oid AND d.deptype = 'e')" | sort -u)" || return 1
    comm -13 <(printf '%s\n' "${dumped}") <(printf '%s\n' "${live}") | sed '/^$/d'
}
report_extra_relations() {
    local extra
    if ! extra="$(extra_relations)"; then
        echo "warning: could not list the relations in ${db}; objects the dump lacks would survive the restore"
        return 0
    fi
    if [[ -n "${extra}" ]]; then
        echo "warning: ${db} has $(wc -l <<<"${extra}") relation(s) that are not in the dump; --clean leaves them in place:"
        while IFS= read -r rel; do echo "    ${rel}"; done <<<"${extra}"
        echo "  For an exact copy of the dump, restore into an empty database instead."
    fi
}

if (( dry_run )); then
    check_docker_daemon
    if phase="$(kube get pod "${pod}" -o jsonpath='{.status.phase}' --ignore-not-found 2>/dev/null)"; then
        phase="${phase:-not found}"
    else
        phase="API server not reachable through ${KIND_NODE}"
    fi
    echo "Dry run: would OVERWRITE database ${db} in pod ${pod} (${K8S_NAMESPACE}, ${phase}) from ${file}:"
    echo "+ kubectl exec -i ${pod} -- pg_restore --username=${user} --dbname=${db} --clean --if-exists --single-transaction --exit-on-error <${file}"
    [[ "${phase}" == Running ]] && report_extra_relations
    echo "(dry run: nothing restored)"
    exit 0
fi

echo "About to OVERWRITE database ${db} in pod ${pod} (${K8S_NAMESPACE}) from ${file}."
report_extra_relations
if (( ! yes )); then
    read -r -p "Type the database name to proceed: " answer
    [[ "${answer}" == "${db}" ]] || die "aborted"
fi

log "restoring ${file} into ${db}"
kube exec -i "${pod}" -- pg_restore --username="${user}" --dbname="${db}" \
    --clean --if-exists --single-transaction --exit-on-error <"${file}"
log "restore of ${db} complete; check diesel migration state and scale services back up"
