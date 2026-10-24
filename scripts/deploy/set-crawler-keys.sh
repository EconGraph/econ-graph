#!/usr/bin/env bash
#
# Enter the crawler API keys (and, with --google, the Google sign-in client)
# into the kind cluster's Secrets without the values ever appearing on screen,
# in shell history, in process arguments, or in an AI agent's context.
#
# Usage (run it yourself, in your own terminal):
#   scripts/deploy/set-crawler-keys.sh [--google] [--no-start-crawler] [--dry-run]
#
# Each key is read with hidden input. Press Enter to keep the value already in
# the cluster. The values go to scripts/deploy/create-secrets.sh through its
# environment, which keeps every other Secret as it is and never prints values.
#
# Afterwards the crawler worker is scaled to one replica and restarted so it
# picks up the keys (skip with --no-start-crawler). With --google, Keycloak is
# restarted to pick up the new Google client.
#
# --dry-run prints which keys are currently set (names only) and what would
# happen; it prompts for nothing and writes nothing.
#
# The script refuses to run without a terminal on stdin, so values cannot be
# piped in by another program.
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
NAMESPACE="${NAMESPACE:-econ-graph}"

DRY_RUN=false
GOOGLE=false
START_CRAWLER=true
for arg in "$@"; do
    case "$arg" in
        --dry-run) DRY_RUN=true ;;
        --google) GOOGLE=true ;;
        --no-start-crawler) START_CRAWLER=false ;;
        -h | --help)
            sed -n '2,24p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "unknown argument: $arg (see --help)" >&2
            exit 2
            ;;
    esac
done

# shellcheck source=scripts/deploy/lib/kind-env.sh
source "$PROJECT_ROOT/scripts/deploy/lib/kind-env.sh"
check_docker_daemon
ensure_kind_kubectl

CRAWLER_KEYS=(FRED_API_KEY BLS_API_KEY BEA_API_KEY CENSUS_API_KEY)
GOOGLE_KEYS=(KEYCLOAK_GOOGLE_CLIENT_ID KEYCLOAK_GOOGLE_CLIENT_SECRET)

# Secret and key each variable lands in (see create-secrets.sh).
secret_of() {
    case "$1" in
        KEYCLOAK_*) echo econ-graph-keycloak ;;
        *) echo crawler-api-keys ;;
    esac
}
key_of() {
    case "$1" in
        FRED_API_KEY) echo fred-api-key ;;
        BLS_API_KEY) echo bls-api-key ;;
        BEA_API_KEY) echo bea-api-key ;;
        CENSUS_API_KEY) echo census-api-key ;;
        KEYCLOAK_GOOGLE_CLIENT_ID) echo google-idp-client-id ;;
        KEYCLOAK_GOOGLE_CLIENT_SECRET) echo google-idp-client-secret ;;
    esac
}

# Prints "set" or "not set" for a key, never its value.
key_state() {
    local secret key
    secret="$(secret_of "$1")"
    key="$(key_of "$1")"
    if kubectl -n "$NAMESPACE" get secret "$secret" --ignore-not-found \
        -o "go-template={{if index .data \"$key\"}}set{{end}}" 2>/dev/null | grep -q set; then
        echo "set"
    else
        echo "not set"
    fi
}

vars=("${CRAWLER_KEYS[@]}")
if [ "$GOOGLE" = true ]; then
    vars+=("${GOOGLE_KEYS[@]}")
fi

echo "==> Keys in namespace ${NAMESPACE}"
for v in "${vars[@]}"; do
    printf '  %-30s %s/%s: %s\n' "$v" "$(secret_of "$v")" "$(key_of "$v")" "$(key_state "$v")"
done

if [ "$DRY_RUN" = true ]; then
    echo "==> Dry run: would prompt (hidden) for each key above, then run create-secrets.sh"
    [ "$START_CRAWLER" = true ] && echo "  then: kubectl -n ${NAMESPACE} scale deployment/crawler-worker --replicas=1 and restart it"
    [ "$GOOGLE" = true ] && echo "  then: kubectl -n ${NAMESPACE} rollout restart deployment/keycloak"
    exit 0
fi

if [ ! -t 0 ]; then
    die "stdin is not a terminal; run this yourself so the keys are typed, not piped"
fi

echo "==> Enter each key (input is hidden; Enter keeps the current value)"
entered=0
for v in "${vars[@]}"; do
    value=""
    read -rs -p "  ${v}: " value
    echo
    if [ -n "$value" ]; then
        export "${v}=${value}"
        entered=$((entered + 1))
    else
        unset "$v"
    fi
done
unset value

if [ "$entered" -eq 0 ]; then
    echo "==> No keys entered; nothing changed"
else
    echo "==> Writing ${entered} key(s) through create-secrets.sh"
    "$PROJECT_ROOT/scripts/deploy/create-secrets.sh"
fi
for v in "${vars[@]}"; do
    unset "$v"
done

if [ "$START_CRAWLER" = true ] &&
    kubectl -n "$NAMESPACE" get deployment crawler-worker >/dev/null 2>&1; then
    echo "==> Starting the crawler worker with the new keys"
    kubectl -n "$NAMESPACE" scale deployment/crawler-worker --replicas=1
    kubectl -n "$NAMESPACE" rollout restart deployment/crawler-worker
fi
if [ "$GOOGLE" = true ] && [ "$entered" -gt 0 ] &&
    kubectl -n "$NAMESPACE" get deployment keycloak >/dev/null 2>&1; then
    echo "==> Restarting Keycloak for the new Google client"
    kubectl -n "$NAMESPACE" rollout restart deployment/keycloak
fi

echo "==> Keys now"
for v in "${vars[@]}"; do
    printf '  %-30s %s\n' "$v" "$(key_state "$v")"
done
