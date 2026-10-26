#!/usr/bin/env bash
# Bootstrap the restricted agent identity. Run yourself using an operator config.
# Usage: create-agent-kubeconfig.sh --operator-kubeconfig FILE --kubectl BIN
#        [--namespace NAME] [--cluster NAME] [--output-dir DIR] [--dry-run]
# Writes a private kubeconfig with a short-lived token; never prints credentials.
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
        -h|--help) sed -n '2,6p' "$0" | sed 's/^# //'; exit 0 ;;
        *) echo "error: unknown argument $1" >&2; exit 2 ;;
    esac
done
[[ "$namespace" =~ ^[a-z0-9]([a-z0-9-]*[a-z0-9])?$ && ${#namespace} -le 63 ]] || exit 2
[[ "$cluster" =~ ^[a-z0-9]([a-z0-9-]*[a-z0-9])?$ && ${#cluster} -le 63 ]] || exit 2
[ -n "$operator_config" ] && [ -n "$kubectl_bin" ] || {
    echo 'error: --operator-kubeconfig and --kubectl are required; no credentials are selected implicitly' >&2
    exit 2
}
[[ "$kubectl_bin" = /* && -x "$kubectl_bin" ]] || { echo 'error: --kubectl must be an absolute executable path' >&2; exit 2; }
KUBECONFIG="$operator_config" require_operator_session
output_dir="${output_dir:-${XDG_CACHE_HOME:-$HOME/.cache}/econgraph/agent-${cluster}}"
[[ "$output_dir" = /* ]] || { echo 'error: --output-dir must be absolute' >&2; exit 2; }
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
  resources: ["pods", "services", "configmaps", "persistentvolumeclaims", "events"]
  verbs: ["get", "list", "watch", "create", "update", "patch", "delete"]
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
    echo "# Would apply RBAC and issue a 1-hour token into ${output_dir}; no cluster contacted."
    exit 0
fi
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
[ ! -L "$output_dir/bin" ] || { echo 'error: output bin directory is a symlink' >&2; exit 1; }
mkdir -p "$output_dir/bin"
ln -sfn "$kubectl_bin" "$output_dir/bin/kubectl"
mv -f "$work/kubeconfig" "$output_dir/kubeconfig-${cluster}"
echo "Agent kubeconfig written privately to ${output_dir}/kubeconfig-${cluster}."
echo 'Token requested for 1 hour; the API server determines its actual lifetime. Re-run to renew.'
echo 'Launch agent tools through scripts/deploy/run-agent-session.sh.'
