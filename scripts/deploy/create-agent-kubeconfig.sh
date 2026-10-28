#!/usr/bin/env bash
# Bootstrap the read-only agent identity. Run yourself using an operator config.
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
[[ "$namespace" =~ ^[a-z0-9]([a-z0-9-]*[a-z0-9])?$ && ${#namespace} -le 63 ]] || {
    echo 'error: --namespace must be a DNS label' >&2; exit 2; }
[[ "$cluster" =~ ^[a-z0-9]([a-z0-9-]*[a-z0-9])?$ && ${#cluster} -le 63 ]] || {
    echo 'error: --cluster must be a DNS label' >&2; exit 2; }
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
# Every link of the chain counts, not only its end: /snap/bin/kubectl is a
# symlink to /usr/bin/snap, which re-executes as MicroK8s' kubectl.
snap_kubectl() {
    local path="$1" target hops=0
    while :; do
        case "$path" in */snap/bin/*) return 0 ;; esac
        case "$(basename "$path")" in microk8s* | snap | snapd) return 0 ;; esac
        [ -L "$path" ] || return 1
        hops=$((hops + 1))
        [ "$hops" -le 40 ] || return 0
        target="$(readlink "$path")"
        case "$target" in /*) path="$target" ;; *) path="$(dirname "$path")/$target" ;; esac
    done
}
if snap_kubectl "$kubectl_bin"; then
    echo "error: $kubectl_bin is the MicroK8s (snap) kubectl; use the kind cluster's kubectl" >&2
    exit 2
fi
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

# The Role: get, list and watch on the non-secret resources agents inspect,
# and nothing else. Any write to a workload can harvest a token, delete data
# (ownerReferences, PVC retention policy) or run a privileged pod, so every
# change, restarts included, stays with operator-run scripts. JSON, so the
# access check below reads the same rules it verifies against.
manifest() {
    python3 - "$namespace" <<'PY'
import json
import sys
ns = sys.argv[1]
read_only = ["get", "list", "watch"]
resources = {
    "": ["pods", "pods/log", "services", "endpoints", "configmaps", "events",
         "persistentvolumeclaims"],
    "apps": ["deployments", "replicasets", "statefulsets", "daemonsets"],
    "batch": ["jobs", "cronjobs"],
    "networking.k8s.io": ["networkpolicies", "ingresses"],
    "gateway.networking.k8s.io": ["httproutes"],
}
meta = {"name": "econgraph-agent", "namespace": ns}
print(json.dumps({"apiVersion": "v1", "kind": "List", "items": [
    {"apiVersion": "v1", "kind": "ServiceAccount", "metadata": meta,
     "automountServiceAccountToken": False},
    {"apiVersion": "rbac.authorization.k8s.io/v1", "kind": "Role", "metadata": meta,
     "rules": [{"apiGroups": [group], "resources": names, "verbs": read_only}
               for group, names in resources.items()]},
    {"apiVersion": "rbac.authorization.k8s.io/v1", "kind": "RoleBinding", "metadata": meta,
     "subjects": [{"kind": "ServiceAccount", "name": "econgraph-agent", "namespace": ns}],
     "roleRef": {"apiGroup": "rbac.authorization.k8s.io", "kind": "Role",
                 "name": "econgraph-agent"}},
]}, indent=2))
PY
}
if [ "$dry_run" = true ]; then
    manifest
    echo "# Would apply RBAC on context kind-${cluster} with ${kubectl_bin} and issue a 1-hour token" >&2
    echo "# into ${output_dir}; no cluster contacted." >&2
    exit 0
fi

# The output directory and every directory above it must be writable only by
# you or root (sticky directories such as /tmp aside; a group-writable one only
# when the group is your own private group), so nobody else can swap a path
# component between the checks and the writes below.
trusted_dir() {
    local owner group mode
    read -r owner group mode < <(stat -c '%u %g %a' "$1") || return 1
    [ "$owner" = 0 ] || [ "$owner" = "$(id -u)" ] || return 1
    mode=$((8#$mode))
    (( mode & 01000 )) && return 0
    (( mode & 0002 )) && return 1
    if (( mode & 0020 )); then
        [ "$group" = "$(id -g)" ] && [ "$(id -gn)" = "$(id -un)" ] || return 1
    fi
    return 0
}
check_ancestors() {
    local dir
    dir="$(realpath -e "$1")" || return 1
    while :; do
        trusted_dir "$dir" || {
            echo "error: $dir can be changed by other users; choose another --output-dir" >&2
            return 1
        }
        [ "$dir" != / ] || return 0
        dir="$(dirname "$dir")"
    done
}
parent="$(dirname "$output_dir")"
existing="$parent"
while [ ! -e "$existing" ]; do existing="$(dirname "$existing")"; done
check_ancestors "$existing" || exit 1
mkdir -p "$parent"
check_ancestors "$parent" || exit 1
# Create, then verify what is there: no check-then-create window.
mkdir "$output_dir" 2>/dev/null || true
[ ! -L "$output_dir" ] || { echo 'error: output directory is a symlink' >&2; exit 1; }
[ -d "$output_dir" ] || { echo 'error: output directory could not be created' >&2; exit 1; }
[ -O "$output_dir" ] || { echo 'error: output directory must be owned by you' >&2; exit 1; }
chmod 0700 "$output_dir"
[ "$(stat -c %a "$output_dir")" = 700 ] || { echo 'error: could not make the output directory private' >&2; exit 1; }

# Pin kind-<cluster> in the operator config (its current context stays as it
# is) and show which cluster that is before writing to it.
cluster_role_operator_context "kind-${cluster}" || {
    echo 'Nothing was applied.' >&2
    exit 1
}
work="$(mktemp -d "$output_dir/.bootstrap.XXXXXX")"
trap 'rm -rf "$work"' EXIT
manifest >"$work/manifest.json"
operator apply -f - <"$work/manifest.json" >/dev/null
# Select cluster connection metadata only; do not copy operator user credentials.
operator config view --minify --flatten --raw -o 'jsonpath={.clusters}' >"$work/clusters.json"
operator -n "$namespace" create token econgraph-agent --duration=1h >"$work/token"
python3 - "$work" "$namespace" "$cluster" <<'PY'
import base64
import json
import pathlib
import sys
import time
work, namespace, cluster = pathlib.Path(sys.argv[1]), sys.argv[2], sys.argv[3]
clusters = json.loads((work / "clusters.json").read_text())
if len(clusters) != 1:
    raise SystemExit("error: expected exactly one cluster")
token = (work / "token").read_text().strip()
if not token:
    raise SystemExit("error: token request returned no token")
# Refuse a token that outlives the request (an API server may extend or cap
# lifetimes): the claims are read locally and never printed.
try:
    payload = token.split(".")[1]
    claims = json.loads(base64.urlsafe_b64decode(payload + "=" * (-len(payload) % 4)))
    expires = int(claims["exp"])
except (IndexError, KeyError, TypeError, ValueError):
    raise SystemExit("error: the issued token has no readable expiry; nothing was written")
if expires - time.time() > 65 * 60:
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

# Existing bindings may grant this ServiceAccount more than the Role. Ask the
# API server, with the issued token, for everything the token may do in every
# namespace (a SelfSubjectRulesReview, what `kubectl auth can-i --list` sends,
# read as JSON). Cluster-wide grants appear in every namespace's answer. Refuse
# anything beyond the Role's reads in its namespace and the defaults every
# authenticated identity has. No impersonation; no Secrets are read.
agent() { "$kubectl_bin" --kubeconfig "$work/kubeconfig" "$@"; }
operator get namespaces -o 'jsonpath={range .items[*]}{.metadata.name}{"\n"}{end}' >"$work/namespaces"
[ -s "$work/namespaces" ] || { echo 'error: could not enumerate namespaces for access verification' >&2; exit 1; }
mkdir "$work/rules"
while IFS= read -r ns; do
    [ -n "$ns" ] || continue
    printf '{"apiVersion":"authorization.k8s.io/v1","kind":"SelfSubjectRulesReview","spec":{"namespace":"%s"}}\n' \
        "$ns" >"$work/rules/$ns.request"
    agent create --raw /apis/authorization.k8s.io/v1/selfsubjectrulesreviews \
        -f "$work/rules/$ns.request" >"$work/rules/$ns.json" 2>"$work/rules/$ns.error" || {
        echo "error: could not list the token's permissions in namespace $ns; no kubeconfig published" >&2
        exit 1
    }
done <"$work/namespaces"
python3 - "$work" "$namespace" <<'PY'
import json
import pathlib
import sys
work, namespace = pathlib.Path(sys.argv[1]), sys.argv[2]
manifest = json.loads((work / "manifest.json").read_text())
role = next(item for item in manifest["items"] if item["kind"] == "Role")
granted = {(group, resource, verb) for rule in role["rules"] for group in rule["apiGroups"]
           for resource in rule["resources"] for verb in rule["verbs"]}
# What every authenticated identity may do (system:basic-user, system:discovery,
# system:public-info-viewer, system:service-account-issuer-discovery).
defaults = {("authorization.k8s.io", "selfsubjectaccessreviews", "create"),
            ("authorization.k8s.io", "selfsubjectrulesreviews", "create"),
            ("authentication.k8s.io", "selfsubjectreviews", "create")}
default_urls = {"/api", "/api/*", "/apis", "/apis/*", "/healthz", "/livez", "/readyz",
                "/version", "/version/", "/openapi", "/openapi/*",
                "/.well-known/openid-configuration", "/.well-known/openid-configuration/",
                "/openid/v1/jwks", "/openid/v1/jwks/"}
extra = []
for path in sorted((work / "rules").glob("*.json")):
    ns = path.stem
    status = json.loads(path.read_text()).get("status", {})
    if status.get("incomplete") or status.get("evaluationError"):
        extra.append(f"namespace {ns}: the API server could not list every permission")
    allowed = defaults | (granted if ns == namespace else set())
    for rule in status.get("resourceRules") or []:
        for verb in rule.get("verbs") or []:
            for group in rule.get("apiGroups") or []:
                for resource in rule.get("resources") or []:
                    # Wildcards ("*", "pods/*") never match the allowed set.
                    if (group, resource, verb) not in allowed:
                        extra.append(f"namespace {ns}: {verb} {resource}"
                                     f"{'.' + group if group else ''}")
    for rule in status.get("nonResourceRules") or []:
        for verb in rule.get("verbs") or []:
            for url in rule.get("nonResourceURLs") or []:
                if verb != "get" or url not in default_urls:
                    extra.append(f"namespace {ns}: {verb} {url}")
if extra:
    print("error: the agent token can do more than read; no kubeconfig published:", file=sys.stderr)
    for line in sorted(set(extra)):
        print(f"  {line}", file=sys.stderr)
    print("Review existing RoleBindings/ClusterRoleBindings before retrying.", file=sys.stderr)
    raise SystemExit(1)
PY
# And the reads it is for must work.
for check in "get pods" "get pods --subresource=log"; do
    # shellcheck disable=SC2086 # split the fixed check into words
    [ "$(agent auth can-i $check -n "$namespace" 2>/dev/null)" = yes ] || {
        echo "error: the agent token cannot $check in $namespace; no kubeconfig published" >&2
        exit 1
    }
done
[ ! -L "$output_dir/bin" ] || { echo 'error: output bin directory is a symlink' >&2; exit 1; }
mkdir -p "$output_dir/bin"
ln -sfn "$kubectl_bin" "$output_dir/bin/kubectl"
mv -f "$work/kubeconfig" "$output_dir/kubeconfig-${cluster}"
echo "Agent kubeconfig written privately to ${output_dir}/kubeconfig-${cluster}."
echo 'Token requested for 1 hour; a longer one is refused. Re-run to renew.'
echo 'Launch agent tools through scripts/deploy/run-agent-session.sh.'
