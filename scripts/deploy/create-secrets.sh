#!/usr/bin/env bash
#
# Create or update the Kubernetes Secrets EconGraph needs. No credential is
# committed to the repository; this script is the only place they come from.
#
# Usage:
#   [VAR=value ...] scripts/deploy/create-secrets.sh
#
# Every input is an environment variable. The script is idempotent: run it as
# often as you like (scripts/deploy/deploy.sh runs it on every deploy).
#
#   Secret (namespace econ-graph)  Key                       Source
#   econ-graph-secrets             database-url              built from app-password
#   econ-graph-postgres            postgres-password         POSTGRES_PASSWORD (generated)
#                                  app-password              APP_DB_PASSWORD (generated)
#   crawler-api-keys               fred-api-key, bls-api-key, bea-api-key, census-api-key
#                                                            FRED_API_KEY, BLS_API_KEY,
#                                                            BEA_API_KEY, CENSUS_API_KEY (optional)
#   econ-graph-keycloak            admin-username            KEYCLOAK_ADMIN_USERNAME (default admin)
#                                  admin-password            KEYCLOAK_ADMIN_PASSWORD (generated)
#                                  db-password               KEYCLOAK_DB_PASSWORD (generated)
#                                  google-idp-client-id      KEYCLOAK_GOOGLE_CLIENT_ID (optional)
#                                  google-idp-client-secret  KEYCLOAK_GOOGLE_CLIENT_SECRET (optional)
#   grafana-admin                  admin-password            GRAFANA_ADMIN_PASSWORD (generated)
#   monitoring-auth                auth                      MONITORING_BASIC_AUTH (optional,
#                                                            one htpasswd line, e.g. from
#                                                            `htpasswd -nB admin`, which prompts)
#
# Rules:
#   - A variable set to the empty string counts as unset, for every key.
#   - "generated": when the variable is unset, the value already in the cluster
#     is kept; only when there is none is a random one generated (openssl rand).
#     Passwords are never rotated by accident.
#   - "optional": when the variable is unset, the value already in the cluster
#     is kept; when there is none, the key is omitted. To remove a key, delete
#     it with kubectl.
#   - monitoring-auth is only written when MONITORING_BASIC_AUTH is set.
#   - Obvious placeholders ("password", "your-...", "change-me", "admin123", ...)
#     are refused; placeholder values already in the cluster are replaced.
#   - If econ-graph-postgres does not exist yet but a Postgres data volume
#     (PVC postgresql-data-postgresql-*) does, the script stops: that database
#     was initialised with other passwords, which new ones would not match.
#     Follow the recovery steps in k8s/README.md, or set
#     ALLOW_EXISTING_DB_VOLUME=1 to go ahead anyway.
#   - Likewise, if grafana-admin/admin-password would be generated but a
#     Grafana volume (PVC grafana-storage-grafana-*) exists, the script stops
#     unless GRAFANA_ADMIN_PASSWORD is given (and set in Grafana) or
#     ALLOW_EXISTING_GRAFANA_VOLUME=1.
#   - If monitoring-auth was created from a manifest (it still carries the
#     kubectl last-applied-configuration annotation, as the old committed one
#     does) and MONITORING_BASIC_AUTH is unset, the script stops unless
#     ALLOW_MANIFEST_MONITORING_AUTH=1.
#   - If the cluster cannot be queried, the script stops before writing
#     anything rather than generating new passwords.
#   - Postgres and Grafana only read their passwords when their data volume is
#     first initialised. Changing POSTGRES_PASSWORD / APP_DB_PASSWORD /
#     GRAFANA_ADMIN_PASSWORD later updates the Secret but not the running
#     database or Grafana; change them there too (see k8s/README.md).
#
# Values are never printed. They reach kubectl through files in a private
# temporary directory, not the command line, and are applied with server-side
# apply, so no copy lands in the last-applied-configuration annotation.
#
# Other environment: NAMESPACE (default econ-graph), KUBECTL (default kubectl).

set -euo pipefail
# Never trace: the values handled below are credentials.
set +x

NAMESPACE="${NAMESPACE:-econ-graph}"
KUBECTL="${KUBECTL:-kubectl}"

APP_DB_USER="econgraph"
DB_HOST="postgres-service"
DB_PORT="5432"
DB_NAME="econ_graph"

die() {
    echo "create-secrets: error: $*" >&2
    exit 1
}

log() {
    echo "create-secrets: $*"
}

command -v "$KUBECTL" >/dev/null 2>&1 || die "$KUBECTL not found"
command -v openssl >/dev/null 2>&1 || die "openssl not found"

WORKDIR="$(umask 077 && mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

# True when the value is an obvious placeholder rather than a real credential.
is_placeholder() {
    local v
    v="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
    case "$v" in
        password | passw0rd | password123 | secret | changeme | change-me | change_me | \
            admin123 | placeholder | example | todo | xxx* | \
            your-* | your_* | *change-in-production* | *changeme* | *change-me* | demo-* | \
            "<"*">")
            return 0
            ;;
    esac
    return 1
}

# Validate a value supplied through the environment.
check_input() {
    local var="$1" value="$2"
    [[ -n "$value" ]] || die "$var is set but empty"
    if is_placeholder "$value"; then
        die "$var looks like a placeholder; set a real value or unset it"
    fi
}

# Print the current value of KEY in Secret NAME, or nothing if the Secret or
# the key is absent (--ignore-not-found makes a missing Secret a success).
# Returns non-zero if the query itself fails. Callers run it in a command
# substitution and must propagate that: current="$(existing ...)" || exit 1.
# A broken connection must never look like "no value" and trigger generation.
existing() {
    local name="$1" key="$2" encoded
    encoded="$("$KUBECTL" -n "$NAMESPACE" get secret "$name" --ignore-not-found \
        -o "jsonpath={.data.$key}")" ||
        die "could not read Secret $name from the cluster; nothing more was written"
    [[ -n "$encoded" ]] || return 0
    printf '%s' "$encoded" | base64 -d ||
        die "Secret $name key $key is not valid base64"
}

# True if Secret NAME exists. Exits the script if the query fails.
secret_exists() {
    local out
    out="$("$KUBECTL" -n "$NAMESPACE" get secret "$1" --ignore-not-found -o name)" ||
        die "could not read Secret $1 from the cluster; nothing more was written"
    [[ -n "$out" ]]
}

# True if a PVC whose name starts with PREFIX exists. StatefulSets name their
# claims <template>-<statefulset>-<n>: postgresql-data-postgresql-0 for
# postgres-deployment.yaml, grafana-storage-grafana-0 for Grafana.
# Exits the script if the query fails.
pvc_exists() {
    local out
    out="$("$KUBECTL" -n "$NAMESPACE" get pvc -o name)" ||
        die "could not list PersistentVolumeClaims; nothing was written"
    grep -q "^persistentvolumeclaim/$1" <<<"$out"
}

# True if Secret NAME carries a client-side-apply annotation, i.e. it was
# created by `kubectl apply -f` from a manifest rather than by this script
# (which uses server-side apply and strips that annotation). Exits the script
# if the query fails.
secret_from_manifest() {
    local out
    out="$("$KUBECTL" -n "$NAMESPACE" get secret "$1" --ignore-not-found \
        -o 'jsonpath={.metadata.annotations.kubectl\.kubernetes\.io/last-applied-configuration}')" ||
        die "could not read Secret $1 from the cluster; nothing was written"
    [[ -n "$out" ]]
}

generate() {
    openssl rand -hex 32
}

# Percent-encode a string for use in the userinfo part of a URL.
urlencode() {
    local LC_ALL=C s="$1" out="" c i
    for ((i = 0; i < ${#s}; i++)); do
        c="${s:i:1}"
        case "$c" in
            [a-zA-Z0-9.~_-]) out+="$c" ;;
            *) out+="$(printf '%%%02X' "'$c")" ;;
        esac
    done
    printf '%s' "$out"
}

# Per-secret state, filled by the add_* helpers and consumed by apply_secret.
declare -a FILE_ARGS=()
declare -a SUMMARY=()
declare -a DROPPED_KEYS=()

begin_secret() {
    FILE_ARGS=()
    SUMMARY=()
    DROPPED_KEYS=()
    mkdir -p "$WORKDIR/$1"
    CURRENT_SECRET="$1"
}

# Store VALUE under KEY in the Secret being built.
put() {
    local key="$1" value="$2" how="$3" file
    file="$WORKDIR/$CURRENT_SECRET/$key"
    printf '%s' "$value" >"$file"
    FILE_ARGS+=("--from-file=$key=$file")
    SUMMARY+=("$key ($how)")
}

# Resolve a value that must always exist: env var, else current, else generated.
# Sets RESOLVED and RESOLVED_HOW.
resolve_required() {
    local var="$1" key="$2" current
    local value="${!var-}"
    if [[ -n "$value" ]]; then
        check_input "$var" "$value"
        current="$(existing "$CURRENT_SECRET" "$key")" || exit 1
        if [[ -n "$current" && "$current" != "$value" ]]; then
            RESOLVED_HOW="from $var, changed"
        else
            RESOLVED_HOW="from $var"
        fi
        RESOLVED="$value"
        return
    fi
    current="$(existing "$CURRENT_SECRET" "$key")" || exit 1
    if [[ -n "$current" ]] && ! is_placeholder "$current"; then
        RESOLVED="$current"
        RESOLVED_HOW="kept"
        return
    fi
    RESOLVED="$(generate)"
    if [[ -n "$current" ]]; then
        RESOLVED_HOW="generated, replaced a placeholder"
    else
        RESOLVED_HOW="generated"
    fi
}

add_required() {
    resolve_required "$1" "$2"
    put "$2" "$RESOLVED" "$RESOLVED_HOW"
}

# Optional value: env var, else current, else omitted.
add_optional() {
    local var="$1" key="$2" current
    local value="${!var-}"
    if [[ -n "$value" ]]; then
        check_input "$var" "$value"
        put "$key" "$value" "from $var"
        return
    fi
    current="$(existing "$CURRENT_SECRET" "$key")" || exit 1
    if [[ -n "$current" ]] && ! is_placeholder "$current"; then
        put "$key" "$current" "kept"
    elif [[ -n "$current" ]]; then
        SUMMARY+=("$key (dropped a placeholder)")
        DROPPED_KEYS+=("$key")
    fi
}

# Remove KEY from the Secret being built if the cluster still has it (a key an
# older layout wrote that nothing reads any more).
drop_obsolete() {
    local key="$1" current
    current="$(existing "$CURRENT_SECRET" "$key")" || exit 1
    if [[ -n "$current" ]]; then
        SUMMARY+=("$key (obsolete, dropped)")
        DROPPED_KEYS+=("$key")
    fi
}

apply_secret() {
    local name="$CURRENT_SECRET" joined
    if ((${#FILE_ARGS[@]} == 0)); then
        if ((${#SUMMARY[@]} == 0)); then
            log "$name: no keys, skipped"
            return
        fi
        # Every key the cluster held was a placeholder: remove the Secret so
        # nothing keeps reading the placeholder values.
        "$KUBECTL" -n "$NAMESPACE" delete secret "$name" --ignore-not-found >/dev/null
        joined="$(printf '%s, ' "${SUMMARY[@]}")"
        log "$name: ${joined%, }, deleted"
        return
    fi
    # Server-side apply: unlike client-side apply it does not store the whole
    # object (data included) in the last-applied-configuration annotation.
    # --force-conflicts takes over fields last written by other managers.
    # Whether it prunes keys another manager wrote (such as a client-side
    # apply of the old secret.yaml) depends on the kubectl version, so dropped
    # keys are removed explicitly below.
    "$KUBECTL" -n "$NAMESPACE" create secret generic "$name" "${FILE_ARGS[@]}" \
        --dry-run=client -o yaml |
        "$KUBECTL" -n "$NAMESPACE" apply --server-side --force-conflicts \
            --field-manager=create-secrets -f - >/dev/null
    # Secrets written by an older client-side apply carry a copy of their data
    # in this annotation; drop it.
    "$KUBECTL" -n "$NAMESPACE" annotate secret "$name" \
        kubectl.kubernetes.io/last-applied-configuration- >/dev/null 2>&1 || true
    local key current
    for key in "${DROPPED_KEYS[@]}"; do
        current="$(existing "$name" "$key")" || exit 1
        if [[ -n "$current" ]]; then
            "$KUBECTL" -n "$NAMESPACE" patch secret "$name" --type=json \
                -p "[{\"op\":\"remove\",\"path\":\"/data/$key\"}]" >/dev/null ||
                die "could not remove $key from $name"
        fi
    done
    joined="$(printf '%s, ' "${SUMMARY[@]}")"
    log "$name: ${joined%, }"
}

"$KUBECTL" get namespace "$NAMESPACE" >/dev/null 2>&1 ||
    die "namespace $NAMESPACE not found; apply k8s/manifests/namespace.yaml first"

# Validate every supplied value before anything is written, so a bad input
# never leaves the Secrets half updated.
for var in POSTGRES_PASSWORD APP_DB_PASSWORD \
    FRED_API_KEY BLS_API_KEY BEA_API_KEY CENSUS_API_KEY KEYCLOAK_ADMIN_USERNAME \
    KEYCLOAK_ADMIN_PASSWORD KEYCLOAK_DB_PASSWORD KEYCLOAK_GOOGLE_CLIENT_ID \
    KEYCLOAK_GOOGLE_CLIENT_SECRET GRAFANA_ADMIN_PASSWORD MONITORING_BASIC_AUTH; do
    [[ -z "${!var-}" ]] || check_input "$var" "${!var}"
done
if [[ -n "${MONITORING_BASIC_AUTH-}" ]]; then
    [[ "$MONITORING_BASIC_AUTH" =~ ^[^:[:space:]]+:[^[:space:]]+$ ]] ||
        die "MONITORING_BASIC_AUTH must be one htpasswd line (user:hash)"
fi

# --- existing-volume guard -------------------------------------------------
# Checked before anything is written.
if ! secret_exists econ-graph-postgres && pvc_exists postgresql-data-postgresql-; then
    if [[ "${ALLOW_EXISTING_DB_VOLUME-}" == 1 ]]; then
        log "warning: a Postgres data volume exists without econ-graph-postgres;" \
            "continuing because ALLOW_EXISTING_DB_VOLUME=1"
    else
        die "a Postgres data volume (PVC postgresql-data-postgresql-*) exists but the" \
            "econ-graph-postgres Secret does not. That database was initialised with" \
            "other passwords, so new ones would not work. See 'Secrets' in" \
            "k8s/README.md to recreate or migrate the volume, or rerun with" \
            "ALLOW_EXISTING_DB_VOLUME=1 to continue anyway."
    fi
fi

# --- Grafana volume guard ---------------------------------------------------
# Grafana reads GF_SECURITY_ADMIN_PASSWORD only when its volume is first
# initialised. If the password would be generated now but Grafana already has
# a volume, the new value would not match the one Grafana actually uses (on
# clusters deployed before these Secrets, the old committed default).
grafana_current="$(existing grafana-admin admin-password)" || exit 1
if [[ -z "${GRAFANA_ADMIN_PASSWORD-}" ]] &&
    { [[ -z "$grafana_current" ]] || is_placeholder "$grafana_current"; } &&
    pvc_exists grafana-storage-grafana-; then
    if [[ "${ALLOW_EXISTING_GRAFANA_VOLUME-}" == 1 ]]; then
        log "warning: Grafana already has a data volume; the generated grafana-admin" \
            "password will not work until you reset it in Grafana" \
            "(continuing because ALLOW_EXISTING_GRAFANA_VOLUME=1)"
    else
        die "Grafana already has a data volume (PVC grafana-storage-grafana-*) but" \
            "grafana-admin/admin-password would be generated now, so it would not" \
            "match the password Grafana uses. Either choose a password, pass it as" \
            "GRAFANA_ADMIN_PASSWORD and set it in Grafana:" \
            "printf '%s' \"\$GRAFANA_ADMIN_PASSWORD\" | kubectl -n $NAMESPACE exec -i" \
            "grafana-0 -- grafana cli admin reset-admin-password --password-from-stdin" \
            "; or rerun with ALLOW_EXISTING_GRAFANA_VOLUME=1 to continue anyway." \
            "See 'Upgrading' in k8s/README.md."
    fi
fi

# --- monitoring-auth guard ---------------------------------------------------
# Deploys before these Secrets applied monitoring-auth from
# ingress-cloudflare-dns01.yaml with a committed htpasswd entry. Such a Secret
# still has the client-side-apply annotation; this script never leaves one.
if [[ -z "${MONITORING_BASIC_AUTH-}" ]] && secret_from_manifest monitoring-auth; then
    if [[ "${ALLOW_MANIFEST_MONITORING_AUTH-}" == 1 ]]; then
        log "warning: keeping monitoring-auth created from a manifest" \
            "(ALLOW_MANIFEST_MONITORING_AUTH=1)"
        keep_manifest_monitoring_auth=1
    else
        die "the monitoring-auth Secret was created from a manifest and probably" \
            "still holds the old committed password. Set MONITORING_BASIC_AUTH to a" \
            "new htpasswd line (e.g. \$(htpasswd -nB admin), which prompts) to replace" \
            "it, or rerun with ALLOW_MANIFEST_MONITORING_AUTH=1 if you created it" \
            "yourself. See 'Upgrading' in k8s/README.md."
    fi
fi

# --- econ-graph-postgres ---------------------------------------------------
begin_secret econ-graph-postgres
add_required POSTGRES_PASSWORD postgres-password
resolve_required APP_DB_PASSWORD app-password
APP_PASSWORD="$RESOLVED"
put app-password "$APP_PASSWORD" "$RESOLVED_HOW"
apply_secret
case "${SUMMARY[*]}" in
    *changed*)
        log "warning: a running database keeps its old passwords until you change" \
            "them there too (see k8s/README.md)"
        ;;
esac

# --- econ-graph-secrets ----------------------------------------------------
begin_secret econ-graph-secrets
put database-url \
    "postgresql://${APP_DB_USER}:$(urlencode "$APP_PASSWORD")@${DB_HOST}:${DB_PORT}/${DB_NAME}" \
    "built for user $APP_DB_USER"
drop_obsolete database-password # written by the old k8s/manifests/secret.yaml
# The in-house login (AUTH-6) is gone, so nothing reads these; sign-in is Keycloak's.
for key in jwt-secret google-client-id google-client-secret facebook-app-id \
    facebook-app-secret facebook-access-token; do
    drop_obsolete "$key"
done
apply_secret

# --- crawler-api-keys ------------------------------------------------------
begin_secret crawler-api-keys
add_optional FRED_API_KEY fred-api-key
add_optional BLS_API_KEY bls-api-key
add_optional BEA_API_KEY bea-api-key
add_optional CENSUS_API_KEY census-api-key
apply_secret

# --- econ-graph-keycloak ---------------------------------------------------
begin_secret econ-graph-keycloak
current="$(existing econ-graph-keycloak admin-username)" || exit 1
if [[ -n "${KEYCLOAK_ADMIN_USERNAME-}" ]]; then
    put admin-username "$KEYCLOAK_ADMIN_USERNAME" "from KEYCLOAK_ADMIN_USERNAME"
elif [[ -n "$current" ]]; then
    put admin-username "$current" "kept"
else
    put admin-username admin "default"
fi
add_required KEYCLOAK_ADMIN_PASSWORD admin-password
add_required KEYCLOAK_DB_PASSWORD db-password
add_optional KEYCLOAK_GOOGLE_CLIENT_ID google-idp-client-id
add_optional KEYCLOAK_GOOGLE_CLIENT_SECRET google-idp-client-secret
apply_secret

# --- grafana-admin ---------------------------------------------------------
begin_secret grafana-admin
add_required GRAFANA_ADMIN_PASSWORD admin-password
apply_secret

# --- monitoring-auth (basic auth for the monitoring ingress) ----------------
if [[ -n "${MONITORING_BASIC_AUTH-}" ]]; then
    begin_secret monitoring-auth
    put auth "$MONITORING_BASIC_AUTH" "from MONITORING_BASIC_AUTH"
    apply_secret
elif secret_exists monitoring-auth; then
    if [[ -n "${keep_manifest_monitoring_auth-}" ]]; then
        # Drop the client-side-apply annotation (it holds a copy of the data) so
        # the guard above does not fire again on the next run.
        "$KUBECTL" -n "$NAMESPACE" annotate secret monitoring-auth \
            kubectl.kubernetes.io/last-applied-configuration- >/dev/null ||
            die "could not remove the last-applied annotation from monitoring-auth"
    fi
    log "monitoring-auth: MONITORING_BASIC_AUTH not set, kept as is"
else
    log "warning: monitoring-auth not created (MONITORING_BASIC_AUTH not set)." \
        "If k8s/manifests/ingress-cloudflare-dns01.yaml is applied, the monitoring" \
        "host returns 503 until this Secret exists."
fi

log "done. Pods read Secrets at start: restart deployments to pick up changes."
