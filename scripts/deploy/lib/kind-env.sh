# shellcheck shell=bash
# shellcheck disable=SC2034 # the settings below are used by the scripts that source this
#
# Shared helpers for the scripts that operate the kind cluster `econ-graph` on
# its host storage (docs/deployment/HOST_STORAGE.md). Source it; don't run it.
#
# Callers set DRY_RUN=true|false and ASSUME_YES=true|false before using run()
# or confirm().

# The Docker daemon the kind cluster runs on: the rootful system daemon
# (data on ext4). kind can't start under rootless Docker on this host, whose
# data root is on btrfs (kind then bind-mounts /dev/mapper into the nodes).
# The operator's login shell exports DOCKER_HOST=<rootless socket> for everyday use, so
# that variable is deliberately not the override; ECONGRAPH_DOCKER_HOST is.
# Exported, so kind, docker and every script run from here use the same daemon.
CALLER_DOCKER_HOST="${DOCKER_HOST:-}"
ECONGRAPH_DOCKER_HOST="${ECONGRAPH_DOCKER_HOST:-unix:///var/run/docker.sock}"
export DOCKER_HOST="${ECONGRAPH_DOCKER_HOST}"
unset DOCKER_CONTEXT

KIND_CLUSTER="${KIND_CLUSTER:-econ-graph}"
KIND_CONTROL_PLANE="${KIND_CLUSTER}-control-plane"
# The worker that bind-mounts the host subvolumes (terraform/k8s/kind-config.yaml).
KIND_STORAGE_NODE="${KIND_CLUSTER}-worker"
NAMESPACE="${NAMESPACE:-econ-graph}"

# The public host this cluster serves (ECO-457): deploy.sh adds it to the
# ingress, Keycloak, the realm and the backend (scripts/deploy/render-public-host.py),
# and the cloudflared tunnel carries https://econgraph.com to it. Set
# PUBLIC_HOST= (empty) to deploy for http://localhost only.
PUBLIC_HOST="${PUBLIC_HOST-econgraph.com}"

HOST_DATA_PLANE="/srv/econgraph/data-plane"
HOST_APP_PROD="/srv/econgraph/app-prod"
HOST_PG_DIR="${HOST_DATA_PLANE}/pg"
# Host UID that owns pg/ and keycloak-pg/ once Postgres (pod UID 999) has run.
# Rootful Docker has no user namespace, so it is 999 on the host too.
HOST_POSTGRES_UID="${ECONGRAPH_HOST_POSTGRES_UID:-999}"
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
    econ-graph-keycloak grafana-admin monitoring-auth cloudflare-tunnel)

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

# Check that the Docker daemon answers and say which one it is. Exits if it
# doesn't answer: "no cluster" must never be concluded from "no daemon".
check_docker_daemon() {
    local info mode=rootful
    info="$(docker info --format '{{.ServerVersion}} {{.DockerRootDir}} {{.SecurityOptions}}' 2>/dev/null)" ||
        die "cannot reach the Docker daemon at ${DOCKER_HOST} (is it running, and are you in the docker group?)"
    case "$info" in *rootless*) mode=rootless ;; esac
    echo "  docker: ${DOCKER_HOST} (${mode}, server $(cut -d' ' -f1 <<<"$info"), data $(cut -d' ' -f2 <<<"$info"))"
    if [ "$mode" = rootless ]; then
        warn "${DOCKER_HOST} is a rootless daemon; kind cannot start there on this host. Unset ECONGRAPH_DOCKER_HOST to use the rootful one."
    fi
}

kind_cluster_exists() {
    local clusters
    clusters="$(kind get clusters 2>/dev/null)" || die "kind get clusters failed against ${DOCKER_HOST}"
    grep -qx "${KIND_CLUSTER}" <<<"$clusters"
}

# Put a kubectl that talks to the kind cluster first on PATH, for this script
# and everything it runs (deploy.sh calls plain `kubectl`). On this host
# `kubectl` is the microk8s snap wrapper, which ignores ~/.kube/config, so copy
# the cluster's own kubectl binary out of the control-plane node (always the
# matching version) and point KUBECONFIG at `kind get kubeconfig`. Nothing in
# ~/.kube is changed. Call again after the cluster is recreated (new certs).
#
# They live in ${ECONGRAPH_CACHE}. Under DRY_RUN=true nothing is written
# there: a cached copy that still reaches the API server is used as it is,
# and otherwise fresh copies go to a private temporary directory that is
# removed when the script exits. ECONGRAPH_KUBE_DIR (exported) says which
# directory is in use, so child scripts reuse it.
use_kind_kubectl() {
    try_use_kind_kubectl || die "cannot reach the API server of kind cluster '${KIND_CLUSTER}' on ${DOCKER_HOST}"
}

# use_kind_kubectl, but return non-zero instead of exiting when the cluster is
# missing or its API server doesn't answer (e.g. expired certificates).
try_use_kind_kubectl() {
    kind_cluster_exists || {
        warn "kind cluster '${KIND_CLUSTER}' not found on ${DOCKER_HOST}"
        return 1
    }
    local dir="${ECONGRAPH_CACHE}"
    if [ "${DRY_RUN:-false}" = "true" ]; then
        if [ -x "${dir}/bin/kubectl" ] && [ -s "${dir}/kubeconfig-${KIND_CLUSTER}" ] &&
            KUBECONFIG="${dir}/kubeconfig-${KIND_CLUSTER}" "${dir}/bin/kubectl" \
                version --request-timeout=15s >/dev/null 2>&1; then
            point_kubectl_at "${dir}"
            return 0
        fi
        dir="$(mktemp -d "${TMPDIR:-/tmp}/econgraph-kube.XXXXXX")" || return 1
        add_exit_cleanup "rm -rf '${dir}'"
    fi
    mkdir -p "${dir}/bin"
    docker cp -q -L "${KIND_CONTROL_PLANE}:/usr/bin/kubectl" "${dir}/bin/kubectl.new" || return 1
    chmod 0755 "${dir}/bin/kubectl.new" || return 1
    mv -f "${dir}/bin/kubectl.new" "${dir}/bin/kubectl" || return 1
    (umask 077 && kind get kubeconfig --name "${KIND_CLUSTER}" >"${dir}/kubeconfig-${KIND_CLUSTER}") || return 1
    point_kubectl_at "${dir}"
    kubectl version --request-timeout=15s >/dev/null 2>&1
}

# point_kubectl_at DIR: use DIR/bin/kubectl and DIR/kubeconfig-<cluster>.
point_kubectl_at() {
    export ECONGRAPH_KUBE_DIR="$1"
    export KUBECONFIG="$1/kubeconfig-${KIND_CLUSTER}"
    case ":${PATH}:" in
        *":$1/bin:"*) ;;
        *) export PATH="$1/bin:${PATH}" ;;
    esac
}

# add_exit_cleanup CMD: run CMD on exit as well as whatever EXIT trap is
# already set. A caller that sets its own EXIT trap later must chain it.
add_exit_cleanup() {
    local prev
    prev="$(trap -p EXIT | sed -E "s/^trap -- '(.*)' EXIT$/\1/")"
    # shellcheck disable=SC2064  # expand now: the trap keeps this exact command
    trap "${prev:+${prev}; }$1" EXIT
}

# use_kind_kubectl, unless a parent script already did.
ensure_kind_kubectl() {
    ensure_kind_kubectl_quietly ||
        die "cannot reach the API server of kind cluster '${KIND_CLUSTER}' on ${DOCKER_HOST}"
}

# ensure_kind_kubectl, but return non-zero instead of exiting.
ensure_kind_kubectl_quietly() {
    local dir="${ECONGRAPH_KUBE_DIR:-${ECONGRAPH_CACHE}}"
    if [ "${KUBECONFIG:-}" = "${dir}/kubeconfig-${KIND_CLUSTER}" ] &&
        [ "$(command -v kubectl)" = "${dir}/bin/kubectl" ] && [ -x "${dir}/bin/kubectl" ]; then
        return 0
    fi
    try_use_kind_kubectl
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

    # Each count is taken (and checked) on its own before counts.env is
    # written: inside echo "...$(...)" a failed query would leave an empty
    # value and the backup would still be reported complete.
    local series points realms="" users=""
    series="$(count_rows econ_graph 'select count(*) from economic_series')" \
        || die "counting economic_series failed; backup in $dir is incomplete"
    points="$(count_rows econ_graph 'select count(*) from data_points')" \
        || die "counting data_points failed; backup in $dir is incomplete"
    [[ "$series" =~ ^[0-9]+$ && "$points" =~ ^[0-9]+$ ]] \
        || die "non-numeric row counts '$series' '$points'; backup in $dir is incomplete"
    if [ -s "$dir/keycloak.dump" ]; then
        realms="$(count_rows keycloak 'select count(*) from realm')" \
            || die "counting Keycloak realms failed; backup in $dir is incomplete"
        users="$(count_rows keycloak 'select count(*) from user_entity')" \
            || die "counting Keycloak users failed; backup in $dir is incomplete"
        [[ "$realms" =~ ^[0-9]+$ && "$users" =~ ^[0-9]+$ ]] \
            || die "non-numeric Keycloak row counts '$realms' '$users'; backup in $dir is incomplete"
    fi
    {
        echo "# Row counts at backup time, compared after a restore."
        echo "SERIES_COUNT=$series"
        echo "DATA_POINTS_COUNT=$points"
        if [ -n "$realms" ]; then
            echo "KEYCLOAK_REALM_COUNT=$realms"
            echo "KEYCLOAK_USER_COUNT=$users"
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

# Read the row counts of a backup (DIR/counts.env) into SERIES_COUNT,
# DATA_POINTS_COUNT, KEYCLOAK_REALM_COUNT and KEYCLOAK_USER_COUNT. Parsed, not
# sourced, and every value must be a number. Keys the file lacks stay empty.
read_counts() {
    local file="$1/counts.env" key value
    SERIES_COUNT="" DATA_POINTS_COUNT="" KEYCLOAK_REALM_COUNT="" KEYCLOAK_USER_COUNT=""
    [ -s "$file" ] || die "$file is missing or empty"
    while IFS='=' read -r key value; do
        case "$key" in
            SERIES_COUNT | DATA_POINTS_COUNT | KEYCLOAK_REALM_COUNT | KEYCLOAK_USER_COUNT)
                [[ "$value" =~ ^[0-9]+$ ]] || die "$file: $key='$value' is not a number"
                printf -v "$key" '%s' "$value"
                ;;
        esac
    done < <(grep -E '^[A-Z_]+=' "$file")
}

# bootstrap_databases SRC_ROOT BACKUP_DIR: namespace, the Secrets saved in
# BACKUP_DIR (plus any create-secrets.sh still needs), the host-storage PVs and
# both Postgres StatefulSets from SRC_ROOT's manifests, so a dump can be
# restored before any app (backend migrations, Keycloak) starts against an
# empty database. Idempotent: existing Secrets are kept, the rest is applied.
bootstrap_databases() {
    local src="$1" backup="$2"
    log "Creating namespace, Secrets and databases from ${src}"
    if [ "${DRY_RUN}" = "true" ]; then
        echo "+ kubectl apply -f $src/k8s/manifests/namespace.yaml"
        echo "+ create the Secrets saved in ${backup}/secrets/ that don't exist yet:"
        local f
        for f in "${backup}"/secrets/*.yaml; do
            [ -e "$f" ] && echo "    $(basename "$f" .yaml)"
        done
        echo "+ scripts/deploy/create-secrets.sh (keeps existing Secrets, generates missing ones)"
        [ -f "$src/k8s/manifests/host-storage.yaml" ] && echo "+ scripts/deploy/apply-host-storage.sh"
        echo "+ kubectl apply postgres-init, postgres-deployment, postgres; wait for postgresql-0"
        [ -s "${backup}/keycloak.dump" ] && echo "+ kubectl apply keycloak/postgres; wait for keycloak-postgres-0"
        return 0
    fi
    kubectl apply -f "$src/k8s/manifests/namespace.yaml"
    restore_secrets "${backup}"
    "${PROJECT_ROOT}/scripts/deploy/create-secrets.sh"
    if [ -f "$src/k8s/manifests/host-storage.yaml" ]; then
        "${PROJECT_ROOT}/scripts/deploy/apply-host-storage.sh"
    fi
    kubectl apply -f "$src/k8s/manifests/postgres-init.yaml"
    kubectl apply -f "$src/k8s/manifests/postgres-deployment.yaml"
    kubectl apply -f "$src/k8s/manifests/postgres.yaml"
    k rollout status statefulset/postgresql --timeout=600s
    wait_postgres_settled postgresql-0 postgresql
    if [ -s "${backup}/keycloak.dump" ]; then
        kubectl apply -f "$src/k8s/manifests/keycloak/postgres.yaml"
        k rollout status statefulset/keycloak-postgres --timeout=600s
        wait_postgres_settled keycloak-postgres-0 postgres
    fi
}

# --- cluster add-ons -------------------------------------------------------------

# ingress-nginx for kind. It binds hostPorts 80/443 on whatever node it runs
# on, and only the control plane (labelled ingress-ready=true) has those ports
# mapped to the host. The v1.13 kind manifest no longer selects that node, so
# the controller can land on the worker and every request to localhost is
# reset; pin it with a nodeSelector. Pinned version, so a recreate installs
# what was tested. Idempotent (kubectl apply, merge patch).
INGRESS_NGINX_MANIFEST="${INGRESS_NGINX_MANIFEST:-https://raw.githubusercontent.com/kubernetes/ingress-nginx/controller-v1.13.3/deploy/static/provider/kind/deploy.yaml}"

install_ingress_nginx() {
    log "Installing ingress-nginx (${INGRESS_NGINX_MANIFEST##*/ingress-nginx/})"
    run kubectl apply -f "${INGRESS_NGINX_MANIFEST}"
    run kubectl -n ingress-nginx patch deployment ingress-nginx-controller --type=merge \
        -p '{"spec":{"template":{"spec":{"nodeSelector":{"ingress-ready":"true"}}}}}'
    if [ "${DRY_RUN}" != "true" ]; then
        kubectl -n ingress-nginx rollout status deployment/ingress-nginx-controller --timeout=300s
    fi
}

# Will the cluster come back by itself after a host reboot? Needs the Docker
# daemon enabled at boot and a restart policy on every node container (kind
# sets on-failure:1, which dockerd honours when it starts). Read-only.
check_restart_on_boot() {
    local node policy ok=true
    log "Checking that the cluster restarts after a host reboot"
    for unit in docker.service docker.socket containerd.service; do
        echo "  systemctl is-enabled $unit: $(systemctl is-enabled "$unit" 2>/dev/null || echo unknown)"
    done
    [ "$(systemctl is-enabled docker.service 2>/dev/null)" = "enabled" ] || {
        warn "docker.service is not enabled at boot: sudo systemctl enable docker"
        ok=false
    }
    for node in $(kind get nodes --name "${KIND_CLUSTER}" 2>/dev/null); do
        policy="$(docker inspect -f '{{.HostConfig.RestartPolicy.Name}}:{{.HostConfig.RestartPolicy.MaximumRetryCount}}' "$node")"
        echo "  $node restart policy: $policy"
        case "$policy" in
            no:* | "") ok=false ;;
        esac
    done
    [ "$ok" = true ] || warn "the cluster will not come back by itself after a reboot"
}
