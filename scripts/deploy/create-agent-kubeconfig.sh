#!/usr/bin/env bash
# Bootstrap the restricted agent identity. Run yourself using an operator config.
# Usage: create-agent-kubeconfig.sh [--operator-kubeconfig FILE] [--kubectl BIN]
#        [--namespace NAME] [--cluster NAME] [--output-dir DIR] [--dry-run]
# Writes a private kubeconfig with a short-lived token; never prints credentials.
# Without --kubectl/--operator-kubeconfig: KUBECTL/KUBECONFIG, else the kind
# cluster's kubectl and kubeconfig in ~/.cache/econgraph; never plain kubectl.
set -euo pipefail
set +x
umask 077
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/deploy/lib/agent-access.sh
source "$ROOT/scripts/deploy/lib/agent-access.sh"
require_operator_session
operator_config="" kubectl_bin="" namespace=econ-graph cluster=econ-graph
output_dir="" dry_run=false
while [ "$#" -gt 0 ]; do
    case "$1" in
        --operator-kubeconfig|--kubectl|--namespace|--cluster|--output-dir)
            [ "$#" -ge 2 ] || { echo "error: $1 needs a value" >&2; exit 2; }
            case "$1" in
                --operator-kubeconfig) operator_config="$2" ;;
                --kubectl) kubectl_bin="$2" ;;
                --namespace) namespace="$2" ;;
                --cluster) cluster="$2" ;;
                --output-dir) output_dir="$2" ;;
            esac
            shift 2 ;;
        --dry-run) dry_run=true; shift ;;
        -h|--help) sed -n '2,8p' "$0" | sed 's/^# //'; exit 0 ;;
        *) echo "error: unknown argument $1" >&2; exit 2 ;;
    esac
done
[[ "$namespace" =~ ^[a-z0-9]([a-z0-9-]*[a-z0-9])?$ && ${#namespace} -le 63 ]] || exit 2
[[ "$cluster" =~ ^[a-z0-9]([a-z0-9-]*[a-z0-9])?$ && ${#cluster} -le 63 ]] || exit 2
# The kubectl and operator config are chosen as for the other operator scripts
# (cluster_role_default_kubectl): the flags, else KUBECTL/KUBECONFIG, else the
# kind cluster's cached kubectl and kubeconfig. Plain kubectl from PATH is
# refused: on the production host it is the MicroK8s snap wrapper, which
# ignores the kubeconfig and reaches another cluster.
# shellcheck source=scripts/deploy/lib/cluster-role.sh
source "$ROOT/scripts/deploy/lib/cluster-role.sh"
[ -z "$kubectl_bin" ] || KUBECTL="$kubectl_bin"
[ -z "$operator_config" ] || KUBECONFIG="$operator_config"
unset CLUSTER_ROLE_CONTEXT
cluster_role_default_kubectl || exit 1
kubectl_bin="${KUBECTL:-}" operator_config="${KUBECONFIG:-}"
[[ "$kubectl_bin" = /* && -x "$kubectl_bin" ]] || {
    echo 'error: no kind kubectl found; pass --kubectl with an absolute executable path' >&2
    exit 2
}
case "$(readlink -f "$kubectl_bin")" in
    */snap/* | *microk8s*)
        echo "error: $kubectl_bin is the MicroK8s kubectl; use the kind cluster's kubectl" >&2
        exit 2 ;;
esac
[ -n "$operator_config" ] || {
    echo 'error: no operator kubeconfig selected; pass --operator-kubeconfig' >&2
    exit 2
}
export KUBECTL="$kubectl_bin" KUBECONFIG="$operator_config"
require_operator_session
output_dir="${output_dir:-${XDG_CACHE_HOME:-$HOME/.cache}/econgraph/agent-${cluster}}"
[[ "$output_dir" = /* ]] || { echo 'error: --output-dir must be absolute' >&2; exit 2; }
# The kubeconfig holds a bearer token: never write it where git could commit it.
probe="$output_dir"
while [ ! -e "$probe" ] && [ "$probe" != / ]; do probe="$(dirname "$probe")"; done
if git -C "$probe" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    echo 'error: --output-dir is inside a git work tree; choose a private directory outside it' >&2
    exit 2
fi
operator() { "$kubectl_bin" --kubeconfig "$operator_config" --context "kind-${cluster}" "$@"; }
manifest() {
    cat <<YAML
apiVersion: v1
kind: ServiceAccount
metadata:
  name: econgraph-agent
  namespace: ${namespace}
automountServiceAccountToken: false
---
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: econgraph-agent
  namespace: ${namespace}
rules:
- apiGroups: [""]
  resources: ["services", "configmaps", "persistentvolumeclaims", "events"]
  verbs: ["get", "list", "watch", "create", "update", "patch", "delete"]
# Pods only through their controllers: no bare pod create, update or patch.
- apiGroups: [""]
  resources: ["pods"]
  verbs: ["get", "list", "watch", "delete"]
- apiGroups: [""]
  resources: ["pods/log"]
  verbs: ["get"]
- apiGroups: ["apps"]
  resources: ["deployments", "replicasets", "statefulsets", "daemonsets"]
  verbs: ["get", "list", "watch", "create", "update", "patch", "delete"]
- apiGroups: ["batch"]
  resources: ["jobs", "cronjobs"]
  verbs: ["get", "list", "watch", "create", "update", "patch", "delete"]
- apiGroups: ["networking.k8s.io"]
  resources: ["networkpolicies", "ingresses"]
  verbs: ["get", "list", "watch", "create", "update", "patch", "delete"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata:
  name: econgraph-agent
  namespace: ${namespace}
subjects:
- kind: ServiceAccount
  name: econgraph-agent
  namespace: ${namespace}
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: Role
  name: econgraph-agent
YAML
}
if [ "$dry_run" = true ]; then
    manifest
    echo "# Would apply RBAC on context kind-${cluster} with ${kubectl_bin} and issue a 1-hour token"
    echo "# into ${output_dir}; no cluster contacted."
    exit 0
fi
# Pin kind-<cluster> in the operator config (its current context stays as it
# is) and show which cluster that is before writing to it.
cluster_role_operator_context "kind-${cluster}" || {
    echo 'Nothing was applied.' >&2
    exit 1
}
# The directory must be owned by the invoking operator, never a symlink.
[ ! -L "$output_dir" ] || { echo 'error: output directory is a symlink' >&2; exit 1; }
mkdir -p "$output_dir"
[ -O "$output_dir" ] || { echo 'error: output directory must be owned by you' >&2; exit 1; }
chmod 0700 "$output_dir"
work="$(mktemp -d "$output_dir/.bootstrap.XXXXXX")"
trap 'rm -rf "$work"' EXIT
manifest | operator apply -f - >/dev/null
# Select cluster connection metadata only; do not copy operator user credentials.
operator config view --minify --flatten --raw -o 'jsonpath={.clusters}' >"$work/clusters.json"
operator -n "$namespace" create token econgraph-agent --duration=1h >"$work/token"
python3 - "$work" "$namespace" "$cluster" <<'PY'
import json
import pathlib
import sys
work, namespace, cluster = pathlib.Path(sys.argv[1]), sys.argv[2], sys.argv[3]
clusters = json.loads((work / "clusters.json").read_text())
if len(clusters) != 1:
    raise SystemExit("error: expected exactly one cluster")
token = (work / "token").read_text().strip()
if not token:
    raise SystemExit("error: token request returned no token")
# Refuse a token that outlives the request (an API server may extend or cap
# lifetimes): the claims are read locally and never printed.
import base64
import time
try:
    payload = token.split(".")[1]
    claims = json.loads(base64.urlsafe_b64decode(payload + "=" * (-len(payload) % 4)))
    expires = int(claims["exp"])
except (IndexError, KeyError, TypeError, ValueError):
    raise SystemExit("error: the issued token has no readable expiry; nothing was written")
if expires - time.time() > 3600 + 300:
    raise SystemExit("error: the API server issued a token valid for more than 1 hour; nothing was written")
config = {
    "apiVersion": "v1", "kind": "Config",
    "clusters": [{"name": cluster, "cluster": clusters[0]["cluster"]}],
    "users": [{"name": "econgraph-agent", "user": {"token": token}}],
    "contexts": [{"name": f"kind-{cluster}", "context": {
        "cluster": cluster, "user": "econgraph-agent", "namespace": namespace}}],
    "current-context": f"kind-{cluster}",
}
(work / "kubeconfig").write_text(json.dumps(config) + "\n")
PY
# Existing bindings may grant this ServiceAccount more than the Role above.
# Verify with the actual issued token before publishing it, never by impersonation.
# Namespace names are connection metadata; no Secrets are read by these checks.
operator get namespaces -o 'jsonpath={range .items[*]}{.metadata.name}{"\n"}{end}' >"$work/namespaces"
[ -s "$work/namespaces" ] || { echo 'error: could not enumerate namespaces for access verification' >&2; exit 1; }
expect_permission() {
    local expected="$1" answer
    shift
    answer="$("$kubectl_bin" --kubeconfig "$work/kubeconfig" auth can-i "$@" 2>"$work/auth-error")" || true
    if [ "$answer" != "$expected" ]; then
        echo "error: agent access verification failed for '$*' (expected $expected)." >&2
        echo 'Review existing RoleBindings/ClusterRoleBindings before retrying; no kubeconfig published.' >&2
        exit 1
    fi
}
# Subresources go in --subresource: `can-i create pods/exec` would ask about a
# pod *named* exec, not the exec subresource.
expect_permission yes get pods -n "$namespace"
expect_permission yes get pods --subresource=log -n "$namespace"
while IFS= read -r ns; do
    [ -n "$ns" ] || continue
    for verb in get list watch; do
        expect_permission no "$verb" secrets -n "$ns"
    done
    expect_permission no create pods -n "$ns"
    for sub in exec attach portforward; do
        expect_permission no create pods --subresource="$sub" -n "$ns"
        expect_permission no get pods --subresource="$sub" -n "$ns"
    done
    expect_permission no patch pods --subresource=ephemeralcontainers -n "$ns"
    expect_permission no get services --subresource=proxy -n "$ns"
    expect_permission no create serviceaccounts --subresource=token -n "$ns"
    expect_permission no impersonate serviceaccounts -n "$ns"
    for kind in roles rolebindings; do
        for verb in create update patch delete; do
            expect_permission no "$verb" "$kind.rbac.authorization.k8s.io" -n "$ns"
        done
    done
    expect_permission no bind roles.rbac.authorization.k8s.io -n "$ns"
    expect_permission no escalate roles.rbac.authorization.k8s.io -n "$ns"
done <"$work/namespaces"
expect_permission no impersonate users
expect_permission no impersonate groups
for kind in clusterroles clusterrolebindings; do
    for verb in create update patch delete; do
        expect_permission no "$verb" "$kind.rbac.authorization.k8s.io"
    done
done
expect_permission no bind clusterroles.rbac.authorization.k8s.io
expect_permission no escalate clusterroles.rbac.authorization.k8s.io
expect_permission no get nodes --subresource=proxy
expect_permission no create certificatesigningrequests.certificates.k8s.io
[ ! -L "$output_dir/bin" ] || { echo 'error: output bin directory is a symlink' >&2; exit 1; }
mkdir -p "$output_dir/bin"
ln -sfn "$kubectl_bin" "$output_dir/bin/kubectl"
mv -f "$work/kubeconfig" "$output_dir/kubeconfig-${cluster}"
echo "Agent kubeconfig written privately to ${output_dir}/kubeconfig-${cluster}."
echo 'Token requested for 1 hour; the API server determines its actual lifetime. Re-run to renew.'
echo 'Launch agent tools through scripts/deploy/run-agent-session.sh.'
