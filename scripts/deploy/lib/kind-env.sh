# shellcheck shell=bash
# shellcheck disable=SC2034 # the settings below are used by the scripts that source this
#
# Shared helpers for the scripts that operate the kind cluster `econ-graph` on
# its host storage (docs/deployment/HOST_STORAGE.md). Source it; don't run it.
#
# Callers set DRY_RUN=true|false and ASSUME_YES=true|false before using run()
# or confirm().

# shellcheck source=scripts/deploy/lib/agent-access.sh
source "$(dirname "${BASH_SOURCE[0]}")/agent-access.sh"
if econgraph_agent_session; then
    export ECONGRAPH_AGENT_SESSION=1
fi

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

# The production marker (ECO-468): kind_cluster_is_production below.
# shellcheck source=scripts/deploy/lib/cluster-role.sh
source "$(dirname "${BASH_SOURCE[0]}")/cluster-role.sh"

KIND_CLUSTER="${KIND_CLUSTER:-econ-graph}"
KIND_CONTROL_PLANE="${KIND_CLUSTER}-control-plane"
# The worker that bind-mounts the host subvolumes (terraform/k8s/kind-config.yaml).
KIND_STORAGE_NODE="${KIND_CLUSTER}-worker"
NAMESPACE="${NAMESPACE:-econ-graph}"

# The storage node sees the host subvolumes at these paths, which the PVs in
# k8s/manifests/host-storage.yaml name.
NODE_DATA_PLANE="/srv/econgraph/data-plane"
NODE_APP_PROD="/srv/econgraph/app-prod"

# Host side of the same subvolumes. In production they are bind-mounted at the
# same paths. ECONGRAPH_SCRATCH_STORAGE=DIR is for a throwaway test cluster
# only (KIND_CLUSTER other than econ-graph): the host side is then
# DIR/data-plane and DIR/app-prod, btrfs subvolumes owned by you rather than
# mount points, and that cluster's kind config bind-mounts them at the node
# paths above.
SCRATCH_STORAGE="${ECONGRAPH_SCRATCH_STORAGE:-}"
if [ -n "${SCRATCH_STORAGE}" ]; then
    if [ "${KIND_CLUSTER}" = "econ-graph" ]; then
        echo "error: ECONGRAPH_SCRATCH_STORAGE is for a throwaway cluster; set KIND_CLUSTER to its name" >&2
        exit 1
    fi
    HOST_DATA_PLANE="${SCRATCH_STORAGE}/data-plane"
    HOST_APP_PROD="${SCRATCH_STORAGE}/app-prod"
else
    # Only the production cluster may use the production host directories.
    if [ "${KIND_CLUSTER}" != "econ-graph" ]; then
        echo "error: KIND_CLUSTER=${KIND_CLUSTER} needs ECONGRAPH_SCRATCH_STORAGE; /srv/econgraph belongs to econ-graph" >&2
        exit 1
    fi
    HOST_DATA_PLANE="${NODE_DATA_PLANE}"
    HOST_APP_PROD="${NODE_APP_PROD}"
fi
# The data database (postgresql-0), the app database (app-postgres-0) and
# Keycloak's (keycloak-postgres-0).
HOST_PG_DIR="${HOST_DATA_PLANE}/pg"
HOST_APP_PG_DIR="${HOST_APP_PROD}/app-pg"
HOST_KEYCLOAK_PG_DIR="${HOST_APP_PROD}/keycloak-pg"
# Host UID that owns pg/, app-pg/ and keycloak-pg/ once Postgres (pod UID 999)
# has run. Rootful Docker has no user namespace, so it is 999 on the host too.
HOST_POSTGRES_UID="${ECONGRAPH_HOST_POSTGRES_UID:-999}"
# PGDATA of each database, as seen inside the storage node.
NODE_PG_VERSION_FILE="${NODE_DATA_PLANE}/pg/pgdata/PG_VERSION"
NODE_APP_PG_VERSION_FILE="${NODE_APP_PROD}/app-pg/pgdata/PG_VERSION"
NODE_KEYCLOAK_PG_VERSION_FILE="${NODE_APP_PROD}/keycloak-pg/18/docker/PG_VERSION"

BACKUP_ROOT="${ECONGRAPH_BACKUP_ROOT:-$HOME/econgraph-backups}"
ECONGRAPH_CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/econgraph"

# Secrets that only exist in the cluster (scripts/deploy/create-secrets.sh keeps
# whatever is there). They're saved next to each dump: the databases were
# initialised with these passwords, and API keys can't be regenerated.
APP_SECRETS=(econ-graph-secrets econ-graph-postgres econ-graph-app-postgres
    crawler-api-keys econ-graph-keycloak grafana-admin monitoring-auth cloudflare-tunnel)

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

# --- the kind CLI ---------------------------------------------------------------

# Every kind command in these scripts runs the kind that
# scripts/deploy/install-kind.sh installs (KIND_PINNED_BIN, see
# scripts/deploy/lib/kind-version.sh), not whatever `kind` is on PATH.
# shellcheck source=scripts/deploy/lib/kind-version.sh
source "$(dirname "${BASH_SOURCE[0]}")/kind-version.sh"

# kind_bin [SUBCOMMAND]: print the path of the kind to run SUBCOMMAND with.
# The pinned binary when it is installed. Otherwise only the read-only
# `kind get` may use another kind from PATH, with a warning: listing clusters,
# nodes or a kubeconfig works with older kind, but creating a cluster or
# loading images into one from the pinned node image does not. Anything else
# fails, saying to run install-kind.sh.
kind_bin() {
    local other
    if [ -x "${KIND_PINNED_BIN}" ]; then
        echo "${KIND_PINNED_BIN}"
        return 0
    fi
    other="$(type -P kind)" || other=""
    if [ "${1:-}" = "get" ] && [ -n "$other" ]; then
        if [ "${KIND_FALLBACK_WARNED:-}" != "true" ]; then
            warn "kind ${KIND_VERSION} is not installed at ${KIND_PINNED_BIN}; using ${other} for the read-only 'kind get'. Install it with scripts/deploy/install-kind.sh"
        fi
        echo "$other"
        return 0
    fi
    echo "error: kind ${KIND_VERSION} is not installed at ${KIND_PINNED_BIN}; run scripts/deploy/install-kind.sh (once, no sudo)${other:+. ${other} is not used for kind ${1:-}}" >&2
    return 1
}

# `kind ...` in these scripts and everything they source runs kind_bin's
# choice. A caller that defines its own kind() first (deploy.sh --dry-run's
# stub) keeps it; KIND_WRAPPER says which one is in use.
KIND_WRAPPER=false
if ! declare -F kind >/dev/null; then
    KIND_WRAPPER=true
    kind() {
        local bin
        bin="$(kind_bin "${1:-}")" || exit 1
        "$bin" "$@"
    }
fi

# Stop unless the pinned kind is installed and reports at least
# KIND_MIN_VERSION. Scripts that create clusters, load images or delete
# clusters call it before they change anything.
require_kind_version() {
    local bin have
    bin="$(kind_bin)" || exit 1
    # `|| have=""`: a failing kind must reach the die below, not stop a
    # `set -e -o pipefail` caller here without a message.
    have="$("$bin" version 2>/dev/null | awk '{print $2}')" || have=""
    [ -n "$have" ] || die "${bin} does not run; reinstall it with scripts/deploy/install-kind.sh"
    version_at_least "$have" "${KIND_MIN_VERSION}" ||
        die "${bin} is kind ${have}, older than ${KIND_MIN_VERSION}; run scripts/deploy/install-kind.sh"
}

# version_at_least HAVE MIN: whether vX.Y.Z HAVE >= MIN, numerically per part.
# Plain bash, so it needs no GNU `sort -V`; a pre-release suffix
# (v0.34.0-alpha) is ignored, and a part that isn't a number fails.
version_at_least() {
    local -a have min
    local i h m
    IFS=. read -r -a have <<<"${1#v}"
    IFS=. read -r -a min <<<"${2#v}"
    for i in 0 1 2; do
        h="${have[i]:-0}" m="${min[i]:-0}"
        h="${h%%-*}" m="${m%%-*}"
        [[ "$h" =~ ^[0-9]+$ && "$m" =~ ^[0-9]+$ ]] || return 1
        if ((10#$h != 10#$m)); then
            ((10#$h > 10#$m))
            return
        fi
    done
}

kind_cluster_exists() {
    local clusters
    # Resolved here rather than inside $(...) below, whose stderr is
    # discarded, so a missing pinned kind is reported; warned about once.
    # Not when the caller brought its own kind() (deploy.sh --dry-run).
    if [ "${KIND_WRAPPER}" = "true" ]; then
        kind_bin get >/dev/null || exit 1
        KIND_FALLBACK_WARNED=true
    fi
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
    if [ "${ECONGRAPH_AGENT_SESSION:-}" = 1 ]; then
        # Never mint cluster-admin credentials from Docker in a restricted session.
        local agent_dir="${ECONGRAPH_KUBE_DIR:-}"
        [ -n "$agent_dir" ] && [ -x "$agent_dir/bin/kubectl" ] &&
            [ "${KUBECONFIG:-}" = "$agent_dir/kubeconfig-${KIND_CLUSTER}" ] || {
            warn "restricted kubeconfig missing; ask the operator to renew it"
            return 1
        }
        "$agent_dir/bin/kubectl" --kubeconfig "$KUBECONFIG" \
            version --request-timeout=15s >/dev/null 2>&1
        return
    fi
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
    if [ "${ECONGRAPH_AGENT_SESSION:-}" = 1 ]; then
        try_use_kind_kubectl
        return
    fi
    local dir="${ECONGRAPH_KUBE_DIR:-${ECONGRAPH_CACHE}}"
    if [ "${KUBECONFIG:-}" = "${dir}/kubeconfig-${KIND_CLUSTER}" ] &&
        [ "$(command -v kubectl)" = "${dir}/bin/kubectl" ] && [ -x "${dir}/bin/kubectl" ]; then
        return 0
    fi
    try_use_kind_kubectl
}

k() { kubectl -n "${NAMESPACE}" "$@"; }

# True when the kind cluster is marked production (lib/cluster-role.sh), asked
# with the kind kubectl on its own context, never another KUBECTL or context.
# CLUSTER_ROLE_REASON says why not. Call after ensure_kind_kubectl.
kind_cluster_is_production() {
    KUBECTL="${ECONGRAPH_KUBE_DIR:-${ECONGRAPH_CACHE}}/bin/kubectl" CLUSTER_ROLE_CONTEXT="kind-${KIND_CLUSTER}" \
        CLUSTER_ROLE_DRY_RUN=false cluster_is_marked_production
}

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

# A directory the kind config bind-mounts into the storage node: a mount point,
# or under ECONGRAPH_SCRATCH_STORAGE the root of a btrfs subvolume (inode 256).
bind_source_ok() {
    if [ -n "${SCRATCH_STORAGE}" ]; then
        [ -d "$1" ] && [ "$(stat -c %i "$1")" = 256 ] && [ "$(stat -f -c %T "$1")" = btrfs ]
    else
        mountpoint -q "$1" 2>/dev/null
    fi
}

# The subvolumes are created and mounted by scripts/host/setup-subvolumes.sh
# (sudo). Check, without sudo, that each root is a mounted btrfs filesystem
# owned by the current user.
check_host_storage_on_host() {
    local dir ok=true fstype owner
    for dir in "${HOST_DATA_PLANE}" "${HOST_APP_PROD}"; do
        if ! bind_source_ok "$dir"; then
            if [ -n "${SCRATCH_STORAGE}" ]; then
                echo "  $dir is not the root of a btrfs subvolume" >&2
            else
                echo "  $dir is not a mount point" >&2
            fi
            ok=false
            continue
        fi
        fstype="$(stat -f -c %T "$dir")"
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

# Check that the storage node sees the host subvolumes at the node paths, i.e.
# the cluster was created from a kind config with the extraMounts.
check_host_storage_in_node() {
    local dir
    for dir in "${NODE_DATA_PLANE}" "${NODE_APP_PROD}"; do
        docker exec "${KIND_STORAGE_NODE}" mountpoint -q "$dir" 2>/dev/null || return 1
    done
}

# --- backups -----------------------------------------------------------------

new_backup_dir() {
    echo "${BACKUP_ROOT}/$(date -u +%Y%m%dT%H%M%SZ)"
}

# pg_dump the data, app and Keycloak databases, verify the archives, record
# row counts and save the app Secrets into $1, which must not exist yet.
# Needs use_kind_kubectl first.
backup_databases() {
    local dir="$1" name
    [ ! -e "$dir" ] || die "$dir already exists; refusing to overwrite a backup"
    log "Backing up to $dir"
    pod_running postgresql-0 || die "postgresql-0 is not running; cannot take a backup"
    if [ "${DRY_RUN}" = "true" ]; then
        # The same read-only checks as a real run, so a dry run fails where it would.
        echo "+ pg_dump -Fc econ_graph (postgresql-0) > $dir/econ_graph.dump"
        if pod_running app-postgres-0; then
            echo "+ pg_dump -Fc econ_graph_app (app-postgres-0) > $dir/econ_graph_app.dump"
        else
            require_not_deployed app-postgres "the app database" ALLOW_MISSING_APP_DUMP
        fi
        if pod_running keycloak-postgres-0; then
            echo "+ pg_dump -Fc keycloak (keycloak-postgres-0) > $dir/keycloak.dump"
        else
            require_not_deployed keycloak-postgres "Keycloak's database" ALLOW_MISSING_KEYCLOAK_DUMP
        fi
        echo "+ save row counts to $dir/counts.env and Secrets to $dir/secrets/"
        return 0
    fi
    (umask 077 && mkdir -p "$dir")

    dump_one postgresql-0 postgresql postgres econ_graph "$dir/econ_graph.dump"
    if pod_running app-postgres-0; then
        dump_one app-postgres-0 postgres postgres econ_graph_app "$dir/econ_graph_app.dump"
    else
        require_not_deployed app-postgres "the app database" ALLOW_MISSING_APP_DUMP
    fi
    if pod_running keycloak-postgres-0; then
        dump_one keycloak-postgres-0 postgres keycloak keycloak "$dir/keycloak.dump"
    else
        require_not_deployed keycloak-postgres "Keycloak's database" ALLOW_MISSING_KEYCLOAK_DUMP
    fi

    # Each count is taken (and checked) on its own before counts.env is
    # written: inside echo "...$(...)" a failed query would leave an empty
    # value and the backup would still be reported complete.
    local series points realms="" users="" app_users="" app_annotations="" app_tables
    series="$(count_rows econ_graph 'select count(*) from economic_series')" \
        || die "counting economic_series failed; backup in $dir is incomplete"
    points="$(count_rows econ_graph 'select count(*) from data_points')" \
        || die "counting data_points failed; backup in $dir is incomplete"
    [[ "$series" =~ ^[0-9]+$ && "$points" =~ ^[0-9]+$ ]] \
        || die "non-numeric row counts '$series' '$points'; backup in $dir is incomplete"
    if [ -s "$dir/econ_graph_app.dump" ]; then
        # Before the backend's first start the app database has no tables; its
        # dump is kept, with no counts, and there is nothing to restore from it.
        app_tables="$(count_rows econ_graph_app "select to_regclass('public.users') is not null")" \
            || die "inspecting the app database failed; backup in $dir is incomplete"
        if [ "$app_tables" = "t" ]; then
            app_users="$(count_rows econ_graph_app 'select count(*) from users')" \
                || die "counting app users failed; backup in $dir is incomplete"
            app_annotations="$(count_rows econ_graph_app 'select count(*) from chart_annotations')" \
                || die "counting chart_annotations failed; backup in $dir is incomplete"
            [[ "$app_users" =~ ^[0-9]+$ && "$app_annotations" =~ ^[0-9]+$ ]] \
                || die "non-numeric app row counts '$app_users' '$app_annotations'; backup in $dir is incomplete"
        else
            warn "the app database has no tables yet; its dump holds no rows"
        fi
    fi
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
        if [ -n "$app_users" ]; then
            echo "APP_USER_COUNT=$app_users"
            echo "APP_ANNOTATION_COUNT=$app_annotations"
        fi
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

# require_not_deployed STATEFULSET WHAT ALLOW_VAR: called when STATEFULSET's
# pod isn't running, so its database can't be dumped. Every caller of
# backup_databases goes on to delete or replace something, so stop unless the
# database doesn't exist (no StatefulSet and no PVC) or ALLOW_VAR=1. A failed
# lookup counts as "exists" (fail closed). The PVC outlives a deleted
# StatefulSet, so it is checked too.
require_not_deployed() {
    local sts="$1" what="$2" allow="$3" found pvc
    pvc="${sts}-data-${sts}-0"
    if [ "${!allow:-}" != 1 ]; then
        found="$(k get statefulset "$sts" -o name --ignore-not-found)" ||
            die "could not tell whether statefulset/$sts exists, so no backup of $what can be promised; nothing was deleted"
        [ -z "$found" ] ||
            die "statefulset/$sts exists but $sts-0 is not running, so $what can't be backed up; nothing was deleted. Start it, or set $allow=1 to go on without a dump of $what"
        found="$(k get persistentvolumeclaim "$pvc" -o name --ignore-not-found)" ||
            die "could not tell whether persistentvolumeclaim/$pvc exists, so no backup of $what can be promised; nothing was deleted"
        [ -z "$found" ] ||
            die "persistentvolumeclaim/$pvc may still hold $what but $sts-0 is not running, so it can't be backed up; nothing was deleted. Start $sts, or set $allow=1 to go on without a dump of $what"
    fi
    warn "$sts-0 is not running; no dump of $what taken"
}

# count_rows DB SQL: run a count query in the right pod.
count_rows() {
    local db="$1" sql="$2"
    case "$db" in
        econ_graph) k exec postgresql-0 -c postgresql -- psql -U postgres -d econ_graph -Atc "$sql" ;;
        econ_graph_app) k exec app-postgres-0 -c postgres -- psql -U postgres -d econ_graph_app -Atc "$sql" ;;
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
        elif [ "$name" = cloudflare-tunnel ] && ! kind_cluster_is_production; then
            # The tunnel token goes only into the production cluster (ECO-468):
            # restoring production's backup elsewhere must not hand it over.
            echo "  - secret $name skipped: ${CLUSTER_ROLE_REASON}. Only the production cluster"
            echo "    may hold the tunnel token: mark it (scripts/deploy/mark-production-cluster.sh)"
            echo "    and restore again, or enter the token with scripts/deploy/set-crawler-keys.sh --tunnel"
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
# DATA_POINTS_COUNT, APP_USER_COUNT, APP_ANNOTATION_COUNT, KEYCLOAK_REALM_COUNT
# and KEYCLOAK_USER_COUNT. Parsed, not sourced, and every value must be a
# number. Keys the file lacks stay empty.
read_counts() {
    local file="$1/counts.env" key value
    SERIES_COUNT="" DATA_POINTS_COUNT="" KEYCLOAK_REALM_COUNT="" KEYCLOAK_USER_COUNT=""
    APP_USER_COUNT="" APP_ANNOTATION_COUNT=""
    [ -s "$file" ] || die "$file is missing or empty"
    while IFS='=' read -r key value; do
        case "$key" in
            SERIES_COUNT | DATA_POINTS_COUNT | KEYCLOAK_REALM_COUNT | KEYCLOAK_USER_COUNT | \
                APP_USER_COUNT | APP_ANNOTATION_COUNT)
                [[ "$value" =~ ^[0-9]+$ ]] || die "$file: $key='$value' is not a number"
                printf -v "$key" '%s' "$value"
                ;;
        esac
    done < <(grep -E '^[A-Z_]+=' "$file")
}

# bootstrap_databases SRC_ROOT BACKUP_DIR: namespace, the Secrets saved in
# BACKUP_DIR (plus any create-secrets.sh still needs), the host-storage PVs and
# the Postgres StatefulSets from SRC_ROOT's manifests (the app database only if
# SRC_ROOT has one), so a dump can be restored before any app (backend
# migrations, Keycloak) starts against an empty database. Idempotent: existing
# Secrets are kept, the rest is applied.
bootstrap_databases() {
    local src="$1" backup="$2"
    log "Creating namespace, Secrets and databases from ${src}"
    if [ "${DRY_RUN}" = "true" ]; then
        echo "+ kubectl apply -f $src/k8s/manifests/namespace.yaml"
        echo "+ create the Secrets saved in ${backup}/secrets/ that don't exist yet:"
        local f
        for f in "${backup}"/secrets/*.yaml; do
            [ -e "$f" ] || continue
            case "$(basename "$f" .yaml)" in
                cloudflare-tunnel) echo "    cloudflare-tunnel (only if the cluster is marked production)" ;;
                *) echo "    $(basename "$f" .yaml)" ;;
            esac
        done
        echo "+ scripts/deploy/create-secrets.sh (keeps existing Secrets, generates missing ones)"
        [ -f "$src/k8s/manifests/host-storage.yaml" ] && echo "+ scripts/deploy/apply-host-storage.sh"
        echo "+ kubectl apply postgres-init, postgres-deployment, postgres; wait for postgresql-0"
        [ -f "$src/k8s/manifests/app-postgres.yaml" ] &&
            echo "+ kubectl apply app-postgres-init, app-postgres; wait for app-postgres-0"
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
    if [ -f "$src/k8s/manifests/app-postgres.yaml" ]; then
        kubectl apply -f "$src/k8s/manifests/app-postgres-init.yaml"
        kubectl apply -f "$src/k8s/manifests/app-postgres.yaml"
        k rollout status statefulset/app-postgres --timeout=600s
        wait_postgres_settled app-postgres-0 postgres
    fi
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
