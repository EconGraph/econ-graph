#!/usr/bin/env bash
# The kind cluster's own kubectl for the cluster scenarios (scripts/qa/lib/qa.py, Kube).
#
#   kind-kubectl.sh CONTEXT
#
# CONTEXT is kind-<cluster>, e.g. kind-econ-graph. Through scripts/deploy/lib/kind-env.sh
# (never plain kubectl from PATH), prints two lines: the kubectl to run and the KUBECONFIG
# to run it with. The caller passes --context CONTEXT on every call. Reads nothing from
# the cluster beyond `kubectl version`; exits 1, saying why, when it cannot be reached.
set -euo pipefail

context="${1:-}"
if [[ ! "$context" =~ ^kind-([a-z0-9][a-z0-9.-]*)$ ]]; then
    echo "error: expected a kind context (kind-<cluster>), got '${context}'" >&2
    exit 1
fi
KIND_CLUSTER="${BASH_REMATCH[1]}"
DRY_RUN=false
ASSUME_YES=false
# shellcheck source=scripts/deploy/lib/kind-env.sh
source "$(dirname "${BASH_SOURCE[0]}")/../../deploy/lib/kind-env.sh"
ensure_kind_kubectl_quietly || {
    echo "error: cannot reach kind cluster '${KIND_CLUSTER}' on ${DOCKER_HOST}" >&2
    exit 1
}
command -v kubectl
echo "${KUBECONFIG}"
