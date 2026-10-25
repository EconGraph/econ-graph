#!/bin/bash
#
# Apply namespace econ-graph's routes and rate limits on the gateway
# (k8s/manifests/gateway/routes/). The routes match host name localhost; when
# the Gateway has an https listener (scripts/deploy/install-gateway.sh,
# GATEWAY_HTTPS_HOSTNAME), its host name is added to every route, so the
# routes attach to that listener too. Every script that applies the routes
# uses this one, after install-gateway.sh.
#
# Usage: scripts/deploy/apply-gateway-routes.sh [--dry-run] [--context NAME]
#
#   --dry-run       Print the routes' host names and what would be applied.
#   --context NAME  kubectl context to use (default: the current context).
#
# Environment:
#   GATEWAY_HTTPS_HOSTNAME  Unset or empty (the default): the host name of the
#                           Gateway's https listener, if it has one. `none`: no
#                           host name besides localhost. A host name: that one
#                           (for a dry run before the listener exists).

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ROUTES_DIR="${PROJECT_ROOT}/k8s/manifests/gateway/routes"

DRY_RUN=false
KUBE_CONTEXT=""
while [ $# -gt 0 ]; do
    case "$1" in
        --dry-run) DRY_RUN=true ;;
        --context)
            [ $# -ge 2 ] || { echo "error: --context needs a value" >&2; exit 2; }
            KUBE_CONTEXT="$2"
            shift
            ;;
        --context=*) KUBE_CONTEXT="${1#--context=}" ;;
        -h | --help)
            sed -n '2,/^$/p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "error: unknown argument: $1" >&2
            exit 2
            ;;
    esac
    shift
done

die() {
    echo "error: $*" >&2
    exit 1
}
kc() {
    if [ -n "$KUBE_CONTEXT" ]; then
        kubectl --context "$KUBE_CONTEXT" "$@"
    else
        kubectl "$@"
    fi
}

host="${GATEWAY_HTTPS_HOSTNAME:-}"
if [ -z "$host" ]; then
    host="$(kc -n econ-graph-gateway get gateway econ-graph-gateway \
        -o jsonpath='{.spec.listeners[?(@.name=="https")].hostname}' 2>/dev/null || true)"
fi
[ "$host" = none ] && host=""
if [ -n "$host" ]; then
    [[ "$host" =~ ^([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]([a-z0-9-]{0,61}[a-z0-9])?$ ]] ||
        die "'${host}' is not a DNS host name"
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
# Each route's hostnames list is "  - localhost"; add the https host after it.
awk -v host="$host" '{ print } host != "" && $0 == "  - localhost" { print "  - " host }' \
    "$ROUTES_DIR/httproutes.yaml" >"$WORK/httproutes.yaml"
[ "$(grep -c '^  - localhost$' "$WORK/httproutes.yaml")" -gt 0 ] ||
    die "no 'localhost' host names in httproutes.yaml; its layout changed"

echo "==> Routes in econ-graph for localhost${host:+ and ${host}}"
if [ "$DRY_RUN" = true ]; then
    echo "+ kubectl${KUBE_CONTEXT:+ --context ${KUBE_CONTEXT}} apply -f <httproutes.yaml with those host names>"
    echo "+ kubectl${KUBE_CONTEXT:+ --context ${KUBE_CONTEXT}} apply -f k8s/manifests/gateway/routes/ratelimit-policies.yaml"
    exit 0
fi
kc apply -f "$WORK/httproutes.yaml"
kc apply -f "$ROUTES_DIR/ratelimit-policies.yaml"
