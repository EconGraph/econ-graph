#!/bin/bash
# Shared defaults and helpers for the host-side backup scripts in this
# directory. Sourced, not executed. See docs/deployment/BACKUPS.md.
#
# Every default below can be overridden from the environment or from
# ~/.config/econgraph/backup.env (a plain KEY=value file, sourced if present).

# shellcheck disable=SC2034  # variables are used by the scripts that source this

ECONGRAPH_CONFIG_DIR="${ECONGRAPH_CONFIG_DIR:-${HOME}/.config/econgraph}"
if [[ -f "${ECONGRAPH_CONFIG_DIR}/backup.env" ]]; then
    # shellcheck disable=SC1091
    source "${ECONGRAPH_CONFIG_DIR}/backup.env"
fi

# Where the cluster lives: kind on the rootful Docker daemon (see DOCKER_HOST
# below). The host's kubectl is a microk8s wrapper, so every cluster call goes
# through the kind node.
KIND_NODE="${KIND_NODE:-econ-graph-control-plane}"
K8S_NAMESPACE="${K8S_NAMESPACE:-econ-graph}"

# Databases to dump: space-separated "database:pod:user" entries.
BACKUP_DATABASES="${BACKUP_DATABASES:-econ_graph:postgresql-0:postgres keycloak:keycloak-postgres-0:keycloak}"

# Logical dumps: on the 870 SATA backup pool once
# scripts/host/setup-backup-pool.sh has mounted it, otherwise in $HOME
# (same NVMe array as the data, so only a stopgap).
if [[ -z "${BACKUP_ROOT:-}" ]]; then
    if [[ -d /srv/econgraph-backups/dumps && -w /srv/econgraph-backups/dumps ]]; then
        BACKUP_ROOT="/srv/econgraph-backups/dumps"
    else
        BACKUP_ROOT="${HOME}/econgraph-backups"
    fi
fi

# Snapshot layout (decided; see BACKUPS.md).
DATA_PLANE_DIR="${DATA_PLANE_DIR:-/srv/econgraph/data-plane}"
APP_PROD_DIR="${APP_PROD_DIR:-/srv/econgraph/app-prod}"
SNAPSHOT_DIR="${SNAPSHOT_DIR:-/srv/econgraph/snapshots}"

# Image whose client tools match the server major version (18).
PG_IMAGE="${PG_IMAGE:-postgres:18}"
PG_MAJOR="${PG_MAJOR:-18}"

# State written by the restore drill and read by the freshness check.
ECONGRAPH_STATE_DIR="${ECONGRAPH_STATE_DIR:-${XDG_STATE_HOME:-${HOME}/.local/state}/econgraph}"

# The Docker daemon the kind cluster runs on: the rootful system daemon. kind
# can't start under rootless Docker on this host (its data root is on btrfs),
# so the cluster moved to the rootful one (as in scripts/deploy/lib/kind-env.sh).
# The operator's login shell exports DOCKER_HOST=<rootless socket> for everyday use, so
# that variable is deliberately not the override; ECONGRAPH_DOCKER_HOST is
# (environment or backup.env). jmalicki is in the docker group, so the
# systemd --user units reach /var/run/docker.sock without sudo. Exported, so
# docker and everything run from here use the same daemon.
ECONGRAPH_DOCKER_HOST="${ECONGRAPH_DOCKER_HOST:-unix:///var/run/docker.sock}"
export DOCKER_HOST="${ECONGRAPH_DOCKER_HOST}"
unset DOCKER_CONTEXT

log() { printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*" >&2; }
die() { log "ERROR: $*"; exit 1; }

utc_stamp() { date -u +%Y%m%dT%H%MZ; }

# Check that the Docker daemon answers and say which one it is. Exits if it
# doesn't: "no kind node" must never be concluded from "no daemon".
check_docker_daemon() {
    local info mode=rootful
    info="$(docker info --format '{{.ServerVersion}} {{.SecurityOptions}}' 2>/dev/null)" \
        || die "cannot reach the Docker daemon at ${DOCKER_HOST} (is it running, and is $(id -un) in the docker group?)"
    case "${info}" in *rootless*) mode=rootless ;; esac
    log "docker: ${DOCKER_HOST} (${mode}, server ${info%% *})"
    if [[ "${mode}" == rootless ]]; then
        log "WARNING: ${DOCKER_HOST} is a rootless daemon; the kind cluster runs on the rootful one. Unset ECONGRAPH_DOCKER_HOST to use it."
    fi
}

# Run kubectl inside the kind node.
kube() { docker exec -i "${KIND_NODE}" kubectl -n "${K8S_NAMESPACE}" "$@"; }

# Print the command prefix to run pg_restore with: the host's when it is new
# enough, otherwise the PG_IMAGE container. Callers must pass files that live
# under the directory given as $1 (it is mounted read-only at the same path).
pg_restore_cmd() {
    local dir="$1" ver
    if ver="$(pg_restore --version 2>/dev/null)" \
        && [[ "${ver}" =~ ([0-9]+)\. ]] \
        && (( BASH_REMATCH[1] >= PG_MAJOR )); then
        echo "pg_restore"
    else
        echo "docker run --rm --network none -v ${dir}:${dir}:ro ${PG_IMAGE} pg_restore"
    fi
}

# Newest dump directory under BACKUP_ROOT that has a VERIFIED marker.
latest_verified_dump_dir() {
    local d
    d="$(find "${BACKUP_ROOT}" -mindepth 2 -maxdepth 2 -name VERIFIED -printf '%h\n' 2>/dev/null \
        | sort | tail -n 1)"
    [[ -n "${d}" ]] && echo "${d}"
}
