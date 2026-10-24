# shellcheck shell=bash
# shellcheck disable=SC2034 # the settings below are used by the scripts that source this
#
# Shared helpers for the scripts that operate the kind cluster `econ-graph` on
# its host storage (docs/deployment/HOST_STORAGE.md). Source it; don't run it.
#
# Callers set DRY_RUN=true|false and ASSUME_YES=true|false before using run()
# or confirm().

KIND_CLUSTER="${KIND_CLUSTER:-econ-graph}"
KIND_CONTROL_PLANE="${KIND_CLUSTER}-control-plane"
# The worker that bind-mounts the host subvolumes (terraform/k8s/kind-config.yaml).
KIND_STORAGE_NODE="${KIND_CLUSTER}-worker"
NAMESPACE="${NAMESPACE:-econ-graph}"

HOST_DATA_PLANE="/srv/econgraph/data-plane"
HOST_APP_PROD="/srv/econgraph/app-prod"
HOST_PG_DIR="${HOST_DATA_PLANE}/pg"
HOST_KEYCLOAK_PG_DIR="${HOST_APP_PROD}/keycloak-pg"
# PGDATA of each database, as seen inside the storage node.
NODE_PG_VERSION_FILE="${HOST_PG_DIR}/pgdata/PG_VERSION"
NODE_KEYCLOAK_PG_VERSION_FILE="${HOST_KEYCLOAK_PG_DIR}/18/docker/PG_VERSION"

BACKUP_ROOT="${ECONGRAPH_BACKUP_ROOT:-$HOME/econgraph-backups}"
ECONGRAPH_CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/econgraph"

# Secrets that only exist in the cluster (scripts/deploy/create-secrets.sh keeps
# whatever is there). They're saved next to each dump: the databases were
# initialised with these passwords, and API keys can't be regenerated.
APP_SECRETS=(econ-graph-secrets econ-graph-postgres crawler-api-keys
    econ-graph-keycloak grafana-admin monitoring-auth)

# A kubectl go-template ($k/$v are template variables, not shell ones).
# shellcheck disable=SC2016
SECRET_TEMPLATE='apiVersion: v1
kind: Secret
metadata:
  name: {{.metadata.name}}
  namespace: {{.metadata.namespace}}
  labels:
    econgraph.io/restored-from-backup: "true"
{{- range $k, $v := .metadata.labels}}
    {{$k}}: "{{$v}}"
{{- end}}
type: {{.type}}
data:
{{- range $k, $v := .data}}
  {{$k}}: {{$v}}
{{- end}}
'

DRY_RUN="${DRY_RUN:-false}"
ASSUME_YES="${ASSUME_YES:-false}"

log() { echo "==> $*"; }
warn() { echo "warning: $*" >&2; }
die() {
    echo "error: $*" >&2
    exit 1
}

# Run a command, or only print it under --dry-run.
run() {
    echo "+ $*"
    if [ "${DRY_RUN}" != "true" ]; then
        "$@"
    fi
}

# Ask for a typed "yes" before a destructive step. --yes answers it; without a
# terminal and without --yes, refuse rather than guess.
confirm() {
    local prompt="$1" reply
    if [ "${ASSUME_YES}" = "true" ]; then
        echo "$prompt (--yes given)"
        return 0
    fi
    if [ "${DRY_RUN}" = "true" ]; then
        echo "$prompt (dry run: would ask for confirmation here)"
        return 0
    fi
    if [ ! -t 0 ]; then
        die "$prompt: needs confirmation; re-run on a terminal or pass --yes"
    fi
    read -r -p "$prompt Type 'yes' to continue: " reply
    [ "$reply" = "yes" ] || die "aborted"
}

kind_cluster_exists() {
    kind get clusters 2>/dev/null | grep -qx "${KIND_CLUSTER}"
}

# Put a kubectl that talks to the kind cluster first on PATH, for this script
# and everything it runs (deploy.sh calls plain `kubectl`). On this host
# `kubectl` is the microk8s snap wrapper, which ignores ~/.kube/config, so copy
# the cluster's own kubectl binary out of the control-plane node (always the
# matching version) and point KUBECONFIG at `kind get kubeconfig`. Nothing in
# ~/.kube is changed. Call again after the cluster is recreated (new certs).
use_kind_kubectl() {
    kind_cluster_exists || die "kind cluster '${KIND_CLUSTER}' not found"
    mkdir -p "${ECONGRAPH_CACHE}/bin"
    docker cp -q -L "${KIND_CONTROL_PLANE}:/usr/bin/kubectl" "${ECONGRAPH_CACHE}/bin/kubectl.new"
    chmod 0755 "${ECONGRAPH_CACHE}/bin/kubectl.new"
    mv -f "${ECONGRAPH_CACHE}/bin/kubectl.new" "${ECONGRAPH_CACHE}/bin/kubectl"
    (umask 077 && kind get kubeconfig --name "${KIND_CLUSTER}" >"${ECONGRAPH_CACHE}/kubeconfig-${KIND_CLUSTER}")
    export KUBECONFIG="${ECONGRAPH_CACHE}/kubeconfig-${KIND_CLUSTER}"
    case ":${PATH}:" in
        *":${ECONGRAPH_CACHE}/bin:"*) ;;
        *) export PATH="${ECONGRAPH_CACHE}/bin:${PATH}" ;;
    esac
    kubectl version >/dev/null 2>&1 || die "cannot reach the kind API server with ${ECONGRAPH_CACHE}/bin/kubectl"
}

# use_kind_kubectl, unless a parent script already did.
ensure_kind_kubectl() {
    if [ "${KUBECONFIG:-}" = "${ECONGRAPH_CACHE}/kubeconfig-${KIND_CLUSTER}" ] &&
        [ "$(command -v kubectl)" = "${ECONGRAPH_CACHE}/bin/kubectl" ]; then
        return 0
    fi
    use_kind_kubectl
}

k() { kubectl -n "${NAMESPACE}" "$@"; }

pod_running() {
    [ "$(k get pod "$1" -o jsonpath='{.status.phase}' --ignore-not-found 2>/dev/null)" = "Running" ]
}

# On a fresh data directory the postgres image's entrypoint runs a temporary
# server (socket only) for its init scripts, and pg_isready, and so the
# readiness probe, already pass against it; it then shuts down and the real
# server starts. Wait for the real one before restoring into it.
wait_postgres_settled() {
    local pod="$1" container="$2" logs started age
    for _ in $(seq 1 150); do
        logs="$(k logs "$pod" -c "$container" 2>/dev/null || true)"
        started="$(k get pod "$pod" -o jsonpath="{.status.containerStatuses[?(@.name==\"$container\")].state.running.startedAt}" 2>/dev/null || true)"
        age=0
        if [ -n "$started" ]; then
            age=$(($(date +%s) - $(date -d "$started" +%s)))
        fi
        # Long-running containers may have rotated the startup lines away.
        if { grep -q "PostgreSQL init process complete" <<<"$logs" ||
            grep -q "Skipping initialization" <<<"$logs" || [ "$age" -gt 300 ]; } &&
            k exec "$pod" -c "$container" -- pg_isready -q 2>/dev/null; then
            return 0
        fi
        sleep 2
    done
    die "$pod did not finish starting"
}

# --- host storage ------------------------------------------------------------

# The subvolumes are created and mounted by scripts/host/setup-subvolumes.sh
# (sudo). Check, without sudo, that each root is a mounted btrfs filesystem
# owned by the current user.
check_host_storage_on_host() {
    local dir ok=true fstype owner
    for dir in "${HOST_DATA_PLANE}" "${HOST_APP_PROD}"; do
        if ! mountpoint -q "$dir" 2>/dev/null; then
            echo "  $dir is not a mount point" >&2
            ok=false
            continue
        fi
        fstype="$(findmnt -no FSTYPE --mountpoint "$dir")"
        owner="$(stat -c %U "$dir")"
        if [ "$fstype" != "btrfs" ]; then
            echo "  $dir is $fstype, not btrfs" >&2
            ok=false
        fi
        if [ "$owner" != "$(id -un)" ]; then
            echo "  $dir is owned by $owner, not $(id -un)" >&2
            ok=false
        fi
    done
    if [ "$ok" != true ]; then
        echo "  Create them with: sudo scripts/host/setup-subvolumes.sh (docs/deployment/HOST_STORAGE.md)" >&2
        return 1
    fi
}

# Check that the storage node sees the host subvolumes at the same paths, i.e.
# the cluster was created from a kind config with the extraMounts.
check_host_storage_in_node() {
    local dir
    for dir in "${HOST_DATA_PLANE}" "${HOST_APP_PROD}"; do
        docker exec "${KIND_STORAGE_NODE}" mountpoint -q "$dir" 2>/dev/null || return 1
    done
}

# --- backups -----------------------------------------------------------------

new_backup_dir() {
    echo "${BACKUP_ROOT}/$(date -u +%Y%m%dT%H%M%SZ)"
}

# pg_dump both databases, verify the archives, record row counts and save the
# app Secrets into $1, which must not exist yet. Needs use_kind_kubectl first.
backup_databases() {
    local dir="$1" name
    [ ! -e "$dir" ] || die "$dir already exists; refusing to overwrite a backup"
    log "Backing up to $dir"
    if [ "${DRY_RUN}" = "true" ]; then
        echo "+ pg_dump -Fc econ_graph (postgresql-0) > $dir/econ_graph.dump"
        echo "+ pg_dump -Fc keycloak (keycloak-postgres-0) > $dir/keycloak.dump"
        echo "+ save row counts to $dir/counts.env and Secrets to $dir/secrets/"
        return 0
    fi
    pod_running postgresql-0 || die "postgresql-0 is not running; cannot take a backup"
    (umask 077 && mkdir -p "$dir")

    dump_one postgresql-0 postgresql postgres econ_graph "$dir/econ_graph.dump"
    if pod_running keycloak-postgres-0; then
        dump_one keycloak-postgres-0 postgres keycloak keycloak "$dir/keycloak.dump"
    else
        warn "keycloak-postgres-0 is not running; no Keycloak dump taken"
    fi

    {
        echo "# Row counts at backup time, compared after a restore."
        echo "SERIES_COUNT=$(count_rows econ_graph 'select count(*) from economic_series')"
        echo "DATA_POINTS_COUNT=$(count_rows econ_graph 'select count(*) from data_points')"
        if [ -s "$dir/keycloak.dump" ]; then
            echo "KEYCLOAK_REALM_COUNT=$(count_rows keycloak 'select count(*) from realm')"
            echo "KEYCLOAK_USER_COUNT=$(count_rows keycloak 'select count(*) from user_entity')"
        fi
    } >"$dir/counts.env"

    # One minimal manifest per Secret (name, type, labels, data) without the
    # server-set fields, so it can be created in a new cluster. Rendered by
    # kubectl itself: values stay base64, and no yq/jq flavour is assumed.
    (umask 077 && mkdir -p "$dir/secrets")
    for name in "${APP_SECRETS[@]}"; do
        if [ -n "$(k get secret "$name" -o name --ignore-not-found)" ]; then
            (umask 077 && k get secret "$name" -o go-template="${SECRET_TEMPLATE}" >"$dir/secrets/$name.yaml")
        fi
    done
    chmod 0600 "$dir"/*.dump "$dir"/counts.env
    log "Backup complete:"
    ls -la "$dir" "$dir/secrets"
    sed 's/^/    /' "$dir/counts.env"
}

# dump_one POD CONTAINER USER DB FILE
dump_one() {
    local pod="$1" container="$2" user="$3" db="$4" file="$5"
    echo "  pg_dump $db from $pod"
    k exec "$pod" -c "$container" -- pg_dump -Fc -U "$user" -d "$db" >"$file.partial"
    # A truncated or corrupt archive fails to read back in full (as the nightly
    # backup job checks it). Not `pg_restore -l`: that stops reading stdin
    # after the TOC, and kubectl exec then hangs pushing the rest.
    k exec -i "$pod" -c "$container" -- pg_restore -f /dev/null <"$file.partial" ||
        die "$file.partial does not read back as a pg_dump archive"
    mv "$file.partial" "$file"
}

# count_rows DB SQL: run a count query in the right pod.
count_rows() {
    local db="$1" sql="$2"
    case "$db" in
        econ_graph) k exec postgresql-0 -c postgresql -- psql -U postgres -d econ_graph -Atc "$sql" ;;
        keycloak) k exec keycloak-postgres-0 -c postgres -- psql -U keycloak -d keycloak -Atc "$sql" ;;
        *) die "unknown database $db" ;;
    esac
}

# Create the Secrets saved in a backup that don't exist in the cluster yet.
# Existing ones are left alone. The namespace must exist.
restore_secrets() {
    local file name found=false
    for file in "$1"/secrets/*.yaml; do
        [ -s "$file" ] || continue
        found=true
        name="$(basename "$file" .yaml)"
        if [ -n "$(k get secret "$name" -o name --ignore-not-found 2>/dev/null)" ]; then
            echo "  = secret $name already exists"
        elif [ "${DRY_RUN}" = "true" ]; then
            echo "+ create secret $name from $file"
        else
            # `create`, not `apply`: no last-applied annotation holding the values.
            k create -f "$file" >/dev/null
            echo "  + secret $name created"
        fi
    done
    if [ "$found" != true ]; then
        warn "no saved Secrets in $1/secrets; create-secrets.sh will generate new passwords"
    fi
}
