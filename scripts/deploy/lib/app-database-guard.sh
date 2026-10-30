# shellcheck shell=bash
#
# require_app_tables_moved: stop unless the data database (econ_graph on postgresql-0) is free
# of the app tables. create-secrets.sh calls it before it points app-database-url at the app
# database (APP_DATABASE=separate) on a cluster whose URL does not already name it. While
# econ_graph still holds public.users, the app database is empty or a stale copy: the backend
# would serve it, every existing account would seem gone, and new sign-ins would land there.
# docs/deployment/APP_DATA_CONVERSION.md moves the rows; until then APP_DATABASE=shared.
#
# Read-only. The caller provides kc (lib/cluster-role.sh), die and NAMESPACE. No postgresql-0
# and no data volume is a new install and passes; a data volume without a running
# postgresql-0, or any lookup that fails, stops (fail closed).

require_app_tables_moved() {
    local pod pvcs found
    pod="$(kc -n "$NAMESPACE" get pod postgresql-0 --ignore-not-found -o name)" ||
        die "could not look up postgresql-0 to check for app tables in the data database; nothing more was written"
    if [[ -z "$pod" ]]; then
        pvcs="$(kc -n "$NAMESPACE" get persistentvolumeclaim -o name)" ||
            die "could not list the volumes to check for app tables in the data database; nothing more was written"
        if grep -q '^persistentvolumeclaim/postgresql-data-postgresql-' <<<"$pvcs"; then
            die "the data database has a volume but postgresql-0 is not running, so it can't be checked" \
                "for app tables before app-database-url moves to the app database. Start it, or keep" \
                "APP_DATABASE=shared; nothing more was written"
        fi
        return 0
    fi
    found="$(kc -n "$NAMESPACE" exec postgresql-0 -c postgresql -- \
        psql -U postgres -d econ_graph -Atc "select to_regclass('public.users') is not null")" ||
        die "could not check the data database for app tables; nothing more was written"
    case "$found" in
        f) return 0 ;;
        t) die "the data database econ_graph still holds the app tables (public.users)." \
            "APP_DATABASE=separate would point the backend at the app database without its" \
            "accounts. Keep APP_DATABASE=shared until docs/deployment/APP_DATA_CONVERSION.md" \
            "has moved them; nothing more was written" ;;
        *) die "unexpected answer while checking the data database for app tables; nothing more was written" ;;
    esac
}
