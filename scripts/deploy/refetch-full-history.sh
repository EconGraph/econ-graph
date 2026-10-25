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
# Environment: NAMESPACE (default econ-graph), DEPLOYMENT (default crawler-worker),
# KUBECTL (default kubectl; e.g. "microk8s kubectl").

set -euo pipefail

NAMESPACE="${NAMESPACE:-econ-graph}"
DEPLOYMENT="${DEPLOYMENT:-crawler-worker}"
read -r -a KUBECTL_CMD <<<"${KUBECTL:-kubectl}"

usage() {
    sed -n '3,21p' "$0" | sed 's/^# \{0,1\}//'
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

if $status; then
    subcommand=refetch-status
else
    subcommand=refetch
fi

started="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
"${KUBECTL_CMD[@]}" -n "${NAMESPACE}" exec "deploy/${DEPLOYMENT}" -- \
    /app/crawler "${subcommand}" "${args[@]}"

if ! $status && ! $dry_run; then
    # A resumed run keeps the original start, so finished series stay skipped.
    since="${completed_since:-${started}}"
    echo >&2
    echo "Progress: $0 --status ${source_args[*]} --since ${since} --failures 50" >&2
    echo "Resume:   $0 ${source_args[*]} --completed-since ${since}" >&2
fi
