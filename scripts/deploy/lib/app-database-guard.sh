# shellcheck shell=bash
#
# Guards for create-secrets.sh when app-database-url changes database. While the app records
# are in the data database (econ_graph), pointing the backend at the app database
# (econ_graph_app) would serve it no accounts and send new sign-ins there; once
# docs/deployment/APP_DATA_CONVERSION.md has removed them from econ_graph, pointing it back at
# econ_graph would do the same, and the backend would create empty app tables there.
#
#   require_app_tables_moved      before APP_DATABASE=separate: econ_graph holds no app tables
#   require_app_tables_not_moved  before APP_DATABASE=shared: econ_graph holds the app tables,
#                                 or econ_graph_app does not hold them either
#
# Read-only. The caller provides kc (lib/cluster-role.sh), die and NAMESPACE. A database whose
# pod and volume are both missing is not there yet and holds nothing; a volume without a
# running pod, or a lookup or query that fails, stops the caller (fail closed).

APP_DB_GUARD_RETRY="wait until postgresql-0 and app-postgres-0 run and accept connections, then rerun; or, if app-database-url already names the database you want, leave APP_DATABASE unset"

# app_db_guard_users POD CONTAINER DB PVC_PREFIX: prints t or f (DB holds public.users), or
# "absent" when neither the pod nor a volume exists. Exits through die otherwise.
app_db_guard_users() {
    local pod="$1" container="$2" db="$3" pvc_prefix="$4" found pvcs
    found="$(kc -n "$NAMESPACE" get pod "$pod" --ignore-not-found -o name)" ||
        die "could not look up $pod to check which database holds the app tables: $APP_DB_GUARD_RETRY; nothing more was written"
    if [[ -z "$found" ]]; then
        pvcs="$(kc -n "$NAMESPACE" get persistentvolumeclaim -o name)" ||
            die "could not list the volumes to check which database holds the app tables: $APP_DB_GUARD_RETRY; nothing more was written"
        if grep -q "^persistentvolumeclaim/${pvc_prefix}" <<<"$pvcs"; then
            die "$db has a volume but $pod is not running, so it can't be checked for app tables:" \
                "$APP_DB_GUARD_RETRY; nothing more was written"
        fi
        echo absent
        return 0
    fi
    found="$(kc -n "$NAMESPACE" exec "$pod" -c "$container" -- \
        psql -U postgres -d "$db" -Atc "select to_regclass('public.users') is not null")" ||
        die "could not check $db for app tables: $APP_DB_GUARD_RETRY; nothing more was written"
    case "$found" in
        t | f) echo "$found" ;;
        *) die "unexpected answer while checking $db for app tables: $APP_DB_GUARD_RETRY; nothing more was written" ;;
    esac
}

require_app_tables_moved() {
    local data
    data="$(app_db_guard_users postgresql-0 postgresql econ_graph postgresql-data-postgresql-)" || exit 1
    [[ "$data" != t ]] ||
        die "the data database econ_graph still holds the app tables (public.users)." \
            "APP_DATABASE=separate would point the backend at the app database without its" \
            "accounts. Keep APP_DATABASE=shared until docs/deployment/APP_DATA_CONVERSION.md" \
            "has moved them; nothing more was written"
}

require_app_tables_not_moved() {
    local data app
    data="$(app_db_guard_users postgresql-0 postgresql econ_graph postgresql-data-postgresql-)" || exit 1
    [[ "$data" != t ]] || return 0
    app="$(app_db_guard_users app-postgres-0 postgres econ_graph_app app-postgres-data-app-postgres-)" || exit 1
    [[ "$app" != t ]] ||
        die "the app tables are no longer in econ_graph but in the app database econ_graph_app" \
            "(the conversion removed them). APP_DATABASE=shared would point the backend at" \
            "econ_graph, which would create empty app tables and lose every account. Use" \
            "APP_DATABASE=separate, or first put them back with scripts/deploy/convert-app-data.sh" \
            "--restore-source-app (docs/deployment/APP_DATA_CONVERSION.md); nothing more was written"
}
