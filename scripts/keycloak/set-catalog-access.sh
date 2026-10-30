#!/bin/bash

# Set who reads a catalog in a running cluster's Keycloak: admins only, nobody by default,
# or everyone (config/keycloak/README.md, "Catalog access").
#
# Usage:
#   scripts/keycloak/set-catalog-access.sh --context kind-econ-graph [--dry-run] \
#       (--admin-only ROLES | --withdraw ROLES | --make-public ROLES)...
#
#   ROLES is one catalog read role, catalog.<source>:read, or several separated by commas.
#
#   --context CONTEXT    required: the kind cluster's context, kind-<cluster>. The current
#                        context is not changed.
#   --admin-only ROLES   remove each role from the catalog-public composite and the
#                        anonymous reader's scope, and grant it to the admin composite
#                        (super_admin includes admin). Signed-in users and visitors lose it.
#   --withdraw ROLES     remove each role from catalog-public and the anonymous scope,
#                        granting it to nobody.
#   --make-public ROLES  the inverse of both: add each role to catalog-public and the anonymous
#                        scope, and remove a direct grant on admin (admin then reads it
#                        through catalog-public).
#   --dry-run            print what would change in Keycloak; change nothing there.
#
# The options may repeat; each role may appear once. Every role is checked before anything
# changes, and every step runs only when needed, so running the same command again changes
# nothing. It refuses, changing nothing, when a role is not a
# catalog read or doesn't exist, or when catalog-public or admin doesn't exist.
#
# How it reaches Keycloak: like the realm import's catalog-policy step
# (scripts/keycloak/preserve-catalog-grants.py), a short-lived pod in the cluster runs
# scripts/keycloak/catalog-access.py against http://keycloak-service:8080/idp with the
# admin credentials from Secret econ-graph-keycloak, passed to it by reference. The
# credentials never reach this machine and nothing prints them. The pod uses the realm
# import's Python image and is deleted when it exits, dry run included. Every kubectl call
# goes to the kind cluster's own kubectl (scripts/deploy/lib/kind-env.sh) through kc
# (scripts/deploy/lib/cluster-role.sh) on CONTEXT, and the cluster's identity is printed
# first.
#
# Deploys keep the result: the realm import preserves the live catalog-public, the catalog
# grants on admin and the anonymous scope (scripts/keycloak/preserve-catalog-grants.py).
# Visitors see a change at the backend's next anonymous-grant renewal (within about four
# minutes) and signed-in users when their access token expires (five minutes); restart the
# backend to apply it to visitors at once.
#
# Environment: NAMESPACE (default econ-graph).

set -euo pipefail

usage() {
    sed -n '3,45p' "$0" | sed 's/^# \{0,1\}//'
    exit "${1:-0}"
}

context=""
dry_run=false
helper_args=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --context)
            [[ $# -ge 2 && -n "$2" ]] || { echo "--context needs a value" >&2; usage 2; }
            context="$2"; shift ;;
        --admin-only | --withdraw | --make-public)
            [[ $# -ge 2 && -n "$2" ]] || { echo "$1 needs a role" >&2; usage 2; }
            IFS=, read -r -a roles <<<"$2"
            for role in "${roles[@]}"; do
                [[ "$role" =~ ^catalog\.[a-z0-9_]+:read$ ]] ||
                    { echo "refused: '$role' is not a catalog read role (catalog.<source>:read)" >&2; exit 2; }
                helper_args+=("$1" "$role")
            done
            shift ;;
        --dry-run) dry_run=true ;;
        -h | --help) usage 0 ;;
        *) echo "unknown argument: $1" >&2; usage 2 ;;
    esac
    shift
done
[[ -n "$context" ]] || { echo "pass --context kind-<cluster>, e.g. --context kind-econ-graph" >&2; usage 2; }
[[ "$context" =~ ^kind-[a-z0-9][a-z0-9.-]*$ ]] ||
    { echo "refused: --context must be a kind cluster's context, kind-<cluster>; got '$context'" >&2; exit 2; }
[[ ${#helper_args[@]} -gt 0 ]] || { echo "pass at least one of --admin-only, --withdraw, --make-public" >&2; usage 2; }
$dry_run && helper_args+=(--dry-run)

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HELPER="$PROJECT_ROOT/scripts/keycloak/catalog-access.py"
JOB_MANIFEST="$PROJECT_ROOT/k8s/manifests/keycloak/realm-import-job.yaml"
KIND_CLUSTER="${context#kind-}"
DRY_RUN="$dry_run"
ASSUME_YES=true
# shellcheck source=scripts/deploy/lib/kind-env.sh
source "$PROJECT_ROOT/scripts/deploy/lib/kind-env.sh"
NAMESPACE="${NAMESPACE:-econ-graph}"

image="$(grep -m1 -oE 'image: python:[^[:space:]]+' "$JOB_MANIFEST" | sed 's/^image: //')"
[[ -n "$image" ]] || die "no Python image in ${JOB_MANIFEST}"

check_docker_daemon >&2
kind_cluster_exists || die "kind cluster '${KIND_CLUSTER}' not found on ${DOCKER_HOST}"
ensure_kind_kubectl
KUBECTL="${ECONGRAPH_KUBE_DIR:-${ECONGRAPH_CACHE}}/bin/kubectl"
[ -x "$KUBECTL" ] || die "the kind cluster's kubectl is not at ${KUBECTL}"
export KUBECTL
cluster_role_operator_context "$context" || die "could not pin context ${context}; nothing changed"

pod="keycloak-catalog-access-$(date +%s)-$RANDOM"
secret_env() {
    printf '{"name":"%s","valueFrom":{"secretKeyRef":{"name":"econ-graph-keycloak","key":"%s"}}}' "$1" "$2"
}
args_json="$(printf '"%s",' python - "${helper_args[@]}")"
overrides="$(cat <<EOF
{"apiVersion":"v1","spec":{
  "restartPolicy":"Never","automountServiceAccountToken":false,
  "containers":[{
    "name":"catalog-access","image":"${image}","imagePullPolicy":"IfNotPresent",
    "command":[${args_json%,}],"stdin":true,"stdinOnce":true,
    "env":[{"name":"KEYCLOAK_URL","value":"http://keycloak-service:8080/idp"},
           $(secret_env KEYCLOAK_USER admin-username),
           $(secret_env KEYCLOAK_PASSWORD admin-password)],
    "resources":{"requests":{"memory":"32Mi","cpu":"20m"},"limits":{"memory":"128Mi","cpu":"250m"}},
    "securityContext":{"runAsNonRoot":true,"runAsUser":1000,"allowPrivilegeEscalation":false,
      "capabilities":{"drop":["ALL"]},"seccompProfile":{"type":"RuntimeDefault"}}}]}}
EOF
)"

echo "Keycloak on ${context}: ${helper_args[*]}" >&2
status=0
kc -n "$NAMESPACE" run "$pod" --rm -i --quiet --restart=Never --pod-running-timeout=3m \
    --labels=app=keycloak-catalog-access,component=auth \
    --image="$image" --overrides="$overrides" <"$HELPER" || status=$?
if [[ $status -ne 0 ]]; then
    echo "set-catalog-access: failed (exit ${status}); see above. A refusal changed nothing." >&2
    exit "$status"
fi
if ! $dry_run; then
    echo "To apply it to visitors at once: ${KUBECTL} --context ${context} -n ${NAMESPACE} rollout restart deployment/econ-graph-backend" >&2
fi
