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
# Every kubectl call goes to context kind-econ-graph, the cluster deploy.sh deploys to,
# without changing the current context. The cluster's context, kube-system UID and
# production marker are printed before anything runs in it. --dry-run changes nothing.
#
# Environment: NAMESPACE (default econ-graph), DEPLOYMENT (default crawler-worker),
# KUBECTL (default kubectl) and its KUBECONFIG. With neither set, it uses the kind
# cluster's own kubectl and kubeconfig when copies are kept in ~/.cache/econgraph
# (bin/kubectl, kubeconfig-econ-graph), and says so.

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/deploy/lib/cluster-role.sh
source "$PROJECT_ROOT/scripts/deploy/lib/cluster-role.sh"

NAMESPACE="${NAMESPACE:-econ-graph}"
DEPLOYMENT="${DEPLOYMENT:-crawler-worker}"
# The context deploy.sh deploys to.
CONTEXT="kind-econ-graph"

usage() {
    sed -n '3,26p' "$0" | sed 's/^# \{0,1\}//'
    exit "${1:-0}"
}

status=false
dry_run=false
completed_since=""
source_args=()
# Options a resumed run must repeat to enqueue the same set of series at the same priority.
resume_extra=()
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
        --priority)
            [[ $# -ge 2 ]] || usage 2
            resume_extra+=("$1" "$2"); args+=("$1" "$2"); shift ;;
        --since | --failures)
            [[ $# -ge 2 ]] || usage 2
            args+=("$1" "$2"); shift ;;
        --include-inactive) resume_extra+=("$1"); args+=("$1") ;;
        --json) args+=("$1") ;;
        -h | --help) usage 0 ;;
        *) echo "unknown argument: $1" >&2; usage 2 ;;
    esac
    shift
done
if [[ ${#source_args[@]} -eq 0 ]]; then
    echo "pass --all or --source SOURCE[,SOURCE...]" >&2
    usage 2
fi

if $status; then
    subcommand=refetch-status
else
    subcommand=refetch
fi

die() {
    echo "refetch-full-history: error: $*" >&2
    exit 1
}

# One kubectl and one context for every call (kc), without changing the current context:
# plain kubectl may reach another cluster (the microk8s snap wrapper ignores ~/.kube/config).
unset CLUSTER_ROLE_CONTEXT
cluster_role_default_kubectl
KUBECTL="${KUBECTL:-kubectl}"
command -v "$KUBECTL" >/dev/null 2>&1 || die "$KUBECTL not found"
contexts="$(command "$KUBECTL" config get-contexts -o name)" || die "could not list the contexts of $KUBECTL"
grep -qxF -- "$CONTEXT" <<<"$contexts" || die "$KUBECTL has no context $CONTEXT"
CLUSTER_ROLE_CONTEXT="$CONTEXT"
export CLUSTER_ROLE_CONTEXT

# Say which cluster this is before running anything in it.
role_status=0
cluster_is_marked_production || role_status=$?
case "$role_status" in
    0) cluster_role="marked production" ;;
    1) cluster_role="not marked production" ;;
    *) cluster_role="production marker unknown: ${CLUSTER_ROLE_REASON}" ;;
esac
cluster_uid="$(kube_system_uid)" || cluster_uid="unknown"
echo "Cluster: context ${CONTEXT}, kube-system UID ${cluster_uid}, ${cluster_role}" \
    "(kubectl ${KUBECTL}, KUBECONFIG=${KUBECONFIG:-default})" >&2

# Jobs get their created_at from the database clock, so the start used in the progress and
# resume hints is ten minutes early, to cover clock skew between this machine and the database.
started="$(date -u -d '-10 minutes' +%Y-%m-%dT%H:%M:%SZ)"
kc -n "${NAMESPACE}" exec "deploy/${DEPLOYMENT}" -- \
    /app/crawler "${subcommand}" "${args[@]}"

if ! $status && ! $dry_run; then
    # A resumed run keeps the original start, so finished series stay skipped, and the same
    # --include-inactive and --priority (refetch-status takes neither, so Progress omits them).
    since="${completed_since:-${started}}"
    echo >&2
    echo "Progress: $0 --status ${source_args[*]} --since ${since} --failures 50" >&2
    echo "Resume:   $0 ${source_args[*]}${resume_extra[*]:+ ${resume_extra[*]}} --completed-since ${since}" >&2
fi
