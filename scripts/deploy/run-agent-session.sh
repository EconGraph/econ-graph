#!/usr/bin/env bash
# Run an agent command with the restricted kubeconfig by default.
# Usage: run-agent-session.sh [--cluster NAME] [--config-dir DIR] -- COMMAND [ARG...]
set -euo pipefail
cluster=econ-graph
config_dir=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --cluster|--config-dir)
            [ "$#" -ge 2 ] || exit 2
            case "$1" in --cluster) cluster="$2" ;; --config-dir) config_dir="$2" ;; esac
            shift 2 ;;
        --) shift; break ;;
        -h|--help) sed -n '2,3p' "$0" | sed 's/^# //'; exit 0 ;;
        *) echo "error: unknown argument $1" >&2; exit 2 ;;
    esac
done
[ "$#" -gt 0 ] || { echo 'error: supply an agent command after --' >&2; exit 2; }
config_dir="${config_dir:-${XDG_CACHE_HOME:-$HOME/.cache}/econgraph/agent-${cluster}}"
[[ "$config_dir" = /* ]] || { echo 'error: --config-dir must be absolute' >&2; exit 2; }
export KIND_CLUSTER="$cluster" ECONGRAPH_AGENT_SESSION=1
export ECONGRAPH_KUBE_DIR="$config_dir" KUBECONFIG="$config_dir/kubeconfig-${cluster}"
export KUBECTL="$config_dir/bin/kubectl"
[ -s "$KUBECONFIG" ] && [ -x "$KUBECTL" ] || {
    echo 'error: restricted config missing; ask the operator to run create-agent-kubeconfig.sh' >&2
    exit 1
}
user="$("$KUBECTL" --kubeconfig "$KUBECONFIG" config view --minify -o 'jsonpath={.contexts[0].context.user}')"
[ "$user" = econgraph-agent ] || { echo 'error: refusing a non-agent kubeconfig' >&2; exit 1; }
NAMESPACE="$("$KUBECTL" --kubeconfig "$KUBECONFIG" config view --minify -o 'jsonpath={.contexts[0].context.namespace}')"
[ -n "$NAMESPACE" ] || { echo 'error: agent kubeconfig must select a namespace' >&2; exit 1; }
export NAMESPACE
export PATH="$config_dir/bin:$PATH"
unset CLUSTER_ROLE_CONTEXT
exec "$@"
