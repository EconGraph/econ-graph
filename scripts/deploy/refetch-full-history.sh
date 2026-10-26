#!/bin/bash

# Enqueue a forced full-history re-fetch of observation data, or report its progress.
#
# Runs the crawler CLI inside the crawler-worker pod (`crawler refetch` /
# `crawler refetch-status`), so the jobs go through the cluster's own queue, worker,
# per-source rate limits and retries. Enqueuing is idempotent: series with an active
# re-fetch job are skipped, and with --completed-since so are series whose re-fetch
# completed since then, so the same command can be re-run to resume.
#
# See docs/technical/OBSERVATION_PRECISION_REFETCH.md.
#
# Usage:
#   scripts/deploy/refetch-full-history.sh (--all | --source FRED[,BLS,...]) [--dry-run]
#       [--completed-since RFC3339] [--priority N] [--include-inactive] [--json]
#   scripts/deploy/refetch-full-history.sh --status (--all | --source ...) [--since RFC3339]
#       [--failures N] [--json]
#
# The crawler CLI in the pod reads the data database from the worker's DATA_DATABASE_URL.
# Every kubectl call goes to the kind cluster's own kubectl and kubeconfig
# (scripts/deploy/lib/kind-env.sh) on context kind-econ-graph, through kc
# (scripts/deploy/lib/cluster-role.sh), without changing the current context.
#
# Environment: KIND_CLUSTER (default econ-graph), NAMESPACE (default econ-graph),
# DEPLOYMENT (default crawler-worker).

set -euo pipefail

usage() {
    sed -n '3,25p' "$0" | sed 's/^# \{0,1\}//'
    exit "${1:-0}"
}

status=false
dry_run=false
completed_since=""
source_args=()
args=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --status) status=true ;;
        --dry-run) dry_run=true; args+=("$1") ;;
        --all) source_args+=("$1"); args+=("$1") ;;
        --source)
            [[ $# -ge 2 ]] || usage 2
            source_args+=("$1" "$2"); args+=("$1" "$2"); shift ;;
        --completed-since)
            [[ $# -ge 2 ]] || usage 2
            completed_since="$2"; args+=("$1" "$2"); shift ;;
        --priority | --since | --failures)
            [[ $# -ge 2 ]] || usage 2
            args+=("$1" "$2"); shift ;;
        --include-inactive | --json) args+=("$1") ;;
        -h | --help) usage 0 ;;
        *) echo "unknown argument: $1" >&2; usage 2 ;;
    esac
    shift
done
if [[ ${#source_args[@]} -eq 0 ]]; then
    echo "pass --all or --source SOURCE[,SOURCE...]" >&2
    usage 2
fi

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/deploy/lib/kind-env.sh
source "$PROJECT_ROOT/scripts/deploy/lib/kind-env.sh"
NAMESPACE="${NAMESPACE:-econ-graph}"
DEPLOYMENT="${DEPLOYMENT:-crawler-worker}"
check_docker_daemon >&2
kind_cluster_exists || die "kind cluster '${KIND_CLUSTER}' not found on ${DOCKER_HOST}"
ensure_kind_kubectl
KUBECTL="${ECONGRAPH_KUBE_DIR:-${ECONGRAPH_CACHE}}/bin/kubectl"
[ -x "$KUBECTL" ] || die "the kind cluster's kubectl is not at ${KUBECTL}"
command "$KUBECTL" config get-contexts -o name | grep -qxF -- "kind-${KIND_CLUSTER}" ||
    die "$KUBECTL has no context kind-${KIND_CLUSTER}"
CLUSTER_ROLE_CONTEXT="kind-${KIND_CLUSTER}"
export KUBECTL CLUSTER_ROLE_CONTEXT

if $status; then
    subcommand=refetch-status
else
    subcommand=refetch
fi

# Jobs get their created_at from the database clock, so the start used in the progress and
# resume hints is ten minutes early, to cover clock skew between this machine and the database.
started="$(date -u -d '-10 minutes' +%Y-%m-%dT%H:%M:%SZ)"
kc -n "${NAMESPACE}" exec "deploy/${DEPLOYMENT}" -- \
    /app/crawler "${subcommand}" "${args[@]}"

if ! $status && ! $dry_run; then
    # A resumed run keeps the original start, so finished series stay skipped.
    since="${completed_since:-${started}}"
    echo >&2
    echo "Progress: $0 --status ${source_args[*]} --since ${since} --failures 50" >&2
    echo "Resume:   $0 ${source_args[*]} --completed-since ${since}" >&2
fi
