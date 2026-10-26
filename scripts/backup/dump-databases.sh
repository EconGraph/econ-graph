#!/bin/bash
# Nightly encrypted host backup, including dumps, row counts and cluster Secrets.
# Uses the same payload as backup-kind-databases.sh so either can recover kind.
# Only the public BACKUP_AGE_RECIPIENT is needed. No backup is deleted.
# Usage: dump-databases.sh [--dry-run] [--target-dir DIR]
set -euo pipefail
umask 077
script_dir="$(dirname "$(readlink -f "$0")")"
# shellcheck source=scripts/backup/lib.sh
source "$script_dir/lib.sh"
DRY_RUN=false
while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run|-n) DRY_RUN=true; shift ;;
        --target-dir) BACKUP_ROOT="${2:?--target-dir needs a directory}"; shift 2 ;;
        -h|--help) sed -n '2,5p' "$0"; exit 0 ;;
        *) die "unknown argument: $1" ;;
    esac
done
# The recovery payload has fixed database names/containers. Never silently
# ignore an older per-database override or back up another kind cluster.
[[ "$BACKUP_DATABASES" == 'econ_graph:postgresql-0:postgres econ_graph_app:app-postgres-0:postgres keycloak:keycloak-postgres-0:keycloak' ]] ||
    die "the unified host writer requires the canonical data/app/Keycloak BACKUP_DATABASES; remove the unsupported override"
[[ "$KIND_NODE" == *-control-plane ]] || die "KIND_NODE must name a kind control-plane container"
selected_cluster="${KIND_NODE%-control-plane}"
[[ -z "${KIND_CLUSTER:-}" || "$KIND_CLUSTER" == "$selected_cluster" ]] || die "KIND_CLUSTER and KIND_NODE disagree"
KIND_CLUSTER="$selected_cluster"
export ECONGRAPH_BACKUP_ROOT="$BACKUP_ROOT"
NAMESPACE="$K8S_NAMESPACE"
# The installed timer uses reviewed copies of the same deploy libraries.
kind_lib="$script_dir/kind-lib/kind-env.sh"
[[ -f "$kind_lib" ]] || kind_lib="$script_dir/../deploy/lib/kind-env.sh"
# shellcheck source=scripts/deploy/lib/kind-env.sh
source "$kind_lib"
# Check public recipient and tmpfs before contacting the cluster.
backup_encryption_preflight
check_docker_daemon
ensure_kind_kubectl
if [[ "$DRY_RUN" != true ]]; then
    mkdir -p "$BACKUP_ROOT"
    exec 9>"$BACKUP_ROOT/.lock"
    flock -n 9 || die "another host dump is running"
fi
backup_databases "$(new_backup_dir)"
