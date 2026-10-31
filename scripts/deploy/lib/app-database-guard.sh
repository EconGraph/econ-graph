# shellcheck shell=bash
#
# Guards for create-secrets.sh on which database app-database-url names. While the app
# records are in the data database (econ_graph), pointing the backend at the app database
# (econ_graph_app) would serve it no accounts and send new sign-ins there; once
# docs/deployment/APP_DATA_CONVERSION.md has removed them from econ_graph, pointing it back at
# econ_graph would do the same, and the backend would create empty app tables there.
#
#   require_app_tables_moved [strict]  before a change to separate: econ_graph holds no app
#                                      tables (strict: and econ_graph is there to say so)
#   require_app_tables_not_moved       before shared, on every run: econ_graph holds the app
#                                      tables, or econ_graph_app does not hold them either
#   require_app_tables_not_left_behind before a kept separate: econ_graph holds no app tables
#                                      (a fresh data database never does)
#
# Read-only. The caller provides kc (lib/cluster-role.sh), die, log and NAMESPACE, and
# KUBECTL with CLUSTER_ROLE_CONTEXT as kc uses them. A database whose pod and volume are both
# missing is not there yet and holds nothing. One that can't be asked (a volume without a
# running pod, a lookup or query that fails or takes longer than APP_DB_GUARD_TIMEOUT
# seconds, default 60, or an unexpected answer) refuses a change, unless
# ALLOW_UNCHECKED_APP_DATABASE=1 says to go ahead deliberately (for example to redeploy
# while repairing a database); a positive answer that the tables are elsewhere is always
# refused, and so is shared when econ_graph answers that it holds none while econ_graph_app
# can't be asked (half of a positive answer).

APP_DB_GUARD_RETRY="wait until postgresql-0 and app-postgres-0 run and accept connections, then rerun; to go ahead without the check (for example to repair a database), rerun with ALLOW_UNCHECKED_APP_DATABASE=1"

# app_db_guard_users POD CONTAINER DB PVC_PREFIX: prints t or f (DB holds public.users),
# "absent" when neither the pod nor a volume exists, or "unknown: REASON".
app_db_guard_users() {
    local pod="$1" container="$2" db="$3" pvc_prefix="$4" found pvcs
    found="$(kc -n "$NAMESPACE" get pod "$pod" --ignore-not-found -o name --request-timeout=30s)" ||
        { echo "unknown: could not look up $pod"; return 0; }
    if [[ -z "$found" ]]; then
        pvcs="$(kc -n "$NAMESPACE" get persistentvolumeclaim -o name --request-timeout=30s)" ||
            { echo "unknown: could not list the volumes"; return 0; }
        if grep -q "^persistentvolumeclaim/${pvc_prefix}" <<<"$pvcs"; then
            echo "unknown: $db has a volume but $pod is not running"
        else
            echo absent
        fi
        return 0
    fi
    # Bounded: an exec into a pod that does not answer must not hold a deploy.
    found="$(timeout -s KILL "${APP_DB_GUARD_TIMEOUT:-60}" "${KUBECTL:-kubectl}" \
        ${CLUSTER_ROLE_CONTEXT:+--context "$CLUSTER_ROLE_CONTEXT"} -n "$NAMESPACE" exec "$pod" \
        -c "$container" -- psql -U postgres -d "$db" -Atc "select to_regclass('public.users') is not null")" ||
        { echo "unknown: could not check $db"; return 0; }
    case "$found" in
        t | f) echo "$found" ;;
        *) echo "unknown: unexpected answer from $db" ;;
    esac
}

# app_db_guard_unchecked WHY: refuse, or with ALLOW_UNCHECKED_APP_DATABASE=1 warn and go on.
app_db_guard_unchecked() {
    if [[ "${ALLOW_UNCHECKED_APP_DATABASE:-}" == 1 ]]; then
        log "warning: $1; app-database-url not checked against the databases (ALLOW_UNCHECKED_APP_DATABASE=1)"
        return 0
    fi
    die "$1, so which database holds the app tables can't be checked: $APP_DB_GUARD_RETRY; nothing more was written"
}

require_app_tables_moved() {
    local data
    data="$(app_db_guard_users postgresql-0 postgresql econ_graph postgresql-data-postgresql-)"
    case "$data" in
        t) die "the data database econ_graph still holds the app tables (public.users)." \
            "APP_DATABASE=separate would point the backend at the app database without its" \
            "accounts. Keep APP_DATABASE=shared until docs/deployment/APP_DATA_CONVERSION.md" \
            "has moved them; nothing more was written" ;;
        f) ;;
        absent)
            [[ "${1:-}" != strict ]] ||
                app_db_guard_unchecked "econ-graph-secrets has no app-database-url, so the app records are in the data database, but postgresql-0 is not there to confirm they have been moved (or set APP_DATABASE=shared)" ;;
        *) app_db_guard_unchecked "${data#unknown: }" ;;
    esac
}

require_app_tables_not_moved() {
    local data app
    data="$(app_db_guard_users postgresql-0 postgresql econ_graph postgresql-data-postgresql-)"
    [[ "$data" != t ]] || return 0
    app="$(app_db_guard_users app-postgres-0 postgres econ_graph_app app-postgres-data-app-postgres-)"
    case "$app" in
        t) if [[ "$data" == unknown:* ]]; then
            app_db_guard_unchecked "${data#unknown: }, and econ_graph_app holds app tables"
        else
            die "the app tables are no longer in econ_graph but in the app database econ_graph_app" \
                "(the conversion removed them). APP_DATABASE=shared would point the backend at" \
                "econ_graph, which would create empty app tables and lose every account. Use" \
                "APP_DATABASE=separate, or first put them back with scripts/deploy/convert-app-data.sh" \
                "--restore-source-app (docs/deployment/APP_DATA_CONVERSION.md); nothing more was written"
        fi ;;
        # Not in econ_graph_app, or no econ_graph_app: they can't be only there.
        f | absent) ;;
        # econ_graph_app can't be asked: the tables may be only there.
        *) if [[ "$data" == f ]]; then
            # econ_graph answered that it holds no app tables: half of a positive answer that
            # they moved to econ_graph_app, so the override does not apply.
            die "the data database econ_graph holds no app tables (public.users), and the app" \
                "database econ_graph_app can't be asked (${app#unknown: }): the conversion may" \
                "have moved the accounts there, for example if it stopped after removing them" \
                "and before switching app-database-url. APP_DATABASE=shared would point the" \
                "backend at econ_graph, which would create empty app tables and lose every" \
                "account. Start app-postgres-0 and rerun; or use APP_DATABASE=separate if the" \
                "conversion finished, or if this cluster never had accounts in econ_graph (a lab," \
                "or a data-only dump) (docs/deployment/APP_DATA_CONVERSION.md)." \
                "ALLOW_UNCHECKED_APP_DATABASE=1 does not bypass this; nothing more was written"
        elif [[ "$data" == unknown:* ]]; then
            app_db_guard_unchecked "${data#unknown: }, and ${app#unknown: }"
        else
            # econ_graph absent (no pod or volume): it says nothing about where the tables are.
            app_db_guard_unchecked "${app#unknown: }"
        fi ;;
    esac
}

require_app_tables_not_left_behind() {
    local data
    data="$(app_db_guard_users postgresql-0 postgresql econ_graph postgresql-data-postgresql-)"
    # Only a positive answer refuses: a kept setting is not blocked by a database that is down.
    [[ "$data" != t ]] ||
        die "app-database-url names the app database, but the data database econ_graph holds app" \
            "tables too, as after a rollback that stopped before pointing app-database-url back" \
            "at econ_graph. If that rollback restored them, point it back with" \
            "APP_DATABASE=shared scripts/deploy/create-secrets.sh --app-database-only" \
            "--context=<the cluster's context>; otherwise compare the two databases before" \
            "starting the backend (docs/deployment/APP_DATA_CONVERSION.md); nothing more was written"
}
