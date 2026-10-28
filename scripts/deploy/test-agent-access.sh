#!/usr/bin/env bash
# Offline regression checks: no Kubernetes or Docker connection is made.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
export AGENT_TEST_CALLS="$scratch/calls"
mkdir "$scratch/bin"
cat >"$scratch/bin/kubectl" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >>"$AGENT_TEST_CALLS"
printf 'KUBECONFIG=%s %s\n' "${KUBECONFIG:-}" "$*" >>"$AGENT_TEST_CALLS.env"
case "$*" in
    *'config use-context'* | *'config set-context'* | *'config set '*)
        echo "the current context must not change: $*" >&2; exit 1 ;;
    *'config view --minify -o jsonpath={.clusters[0].cluster.server}'*)
        printf 'https://example.invalid' ;;
    *'get nodes -o'*) printf 'econ-graph-control-plane' ;;
    *'get namespace kube-system -o'*) printf 'kube-system-uid' ;;
    *'get configmap econgraph-cluster-role'*) ;;
    *'config view --minify --flatten --raw'*)
        printf '[{"name":"operator-cluster","cluster":{"server":"https://example.invalid","certificate-authority-data":"Y2E="}}]' ;;
    *'config view --minify'*'.namespace}'*) printf econ-graph ;;
    *'config view --minify'*) printf '%s' "${AGENT_TEST_USER:-econgraph-agent}" ;;
    *'apply -f -'*) cat >/dev/null ;;
    *'create token econgraph-agent --duration=1h'*)
        # A JWT shaped token; its signature part carries the marker the test
        # greps for in output.
        python3 -c 'import base64, json, sys, time
claims = {"exp": int(time.time()) + int(sys.argv[1])}
enc = lambda b: base64.urlsafe_b64encode(b).rstrip(b"=").decode()
print(enc(b"{}") + "." + enc(json.dumps(claims).encode()) + ".test-bearer-token")' \
            "${AGENT_TEST_TOKEN_SECONDS:-3600}" ;;
    *'get namespaces -o'*) printf 'econ-graph\nkube-system\n' ;;
    *'auth can-i get pods -n'* | *'auth can-i get pods --subresource=log -n'*)
        printf yes; [ "${AGENT_TEST_EXPIRED:-false}" != true ] ;;
    *'auth can-i'*secrets*)
        if [ "${AGENT_TEST_ALLOW_SECRETS:-false}" = true ] && [[ "$*" == *' -n kube-system' ]]; then
            printf yes
        else
            printf no; exit 1
        fi ;;
    *'auth can-i'*) printf no; exit 1 ;;
    *) echo "unexpected kubectl call: $*" >&2; exit 1 ;;
esac
STUB
chmod +x "$scratch/bin/kubectl"
export KUBECTL="$scratch/bin/kubectl"
unset ECONGRAPH_AGENT_SESSION KUBECONFIG
"$ROOT/scripts/deploy/create-agent-kubeconfig.sh" --operator-kubeconfig "$scratch/operator" \
    --kubectl "$KUBECTL" --output-dir "$scratch/agent" --dry-run >"$scratch/rbac.yaml"
[ ! -f "$AGENT_TEST_CALLS" ]
python3 - "$scratch/rbac.yaml" <<'PY'
import sys
import yaml
resources = list(yaml.safe_load_all(open(sys.argv[1])))
assert resources[0]["automountServiceAccountToken"] is False
role = resources[1]
assert role["kind"] == "Role"
for rule in role["rules"]:
    assert "*" not in rule["resources"]
    assert "secrets" not in rule["resources"]
    assert "serviceaccounts/token" not in rule["resources"]
    assert "pods/exec" not in rule["resources"]
    assert not ({"bind", "escalate", "impersonate"} & set(rule["verbs"]))
binding = resources[2]
assert binding["kind"] == "RoleBinding"
assert binding["roleRef"] == {
    "apiGroup": "rbac.authorization.k8s.io", "kind": "Role", "name": "econgraph-agent"}
assert binding["subjects"] == [{
    "kind": "ServiceAccount", "name": "econgraph-agent",
    "namespace": binding["metadata"]["namespace"]}]
assert role["metadata"]["namespace"] == binding["metadata"]["namespace"]
allowed = {"pods", "pods/log", "services", "configmaps", "persistentvolumeclaims", "events",
           "deployments", "replicasets", "statefulsets", "daemonsets",
           "jobs", "cronjobs", "networkpolicies", "ingresses"}
assert {r for rule in role["rules"] for r in rule["resources"]} <= allowed
assert all("*" not in rule["verbs"] and "*" not in rule["apiGroups"] for rule in role["rules"])
permissions = {(group, resource, verb) for rule in role["rules"]
               for group in rule["apiGroups"] for resource in rule["resources"]
               for verb in rule["verbs"]}
workload_verbs = {"get", "list", "watch", "create", "update", "patch", "delete"}
required = {
    "": {"services", "configmaps", "persistentvolumeclaims", "events"},
    "apps": {"deployments", "replicasets", "statefulsets", "daemonsets"},
    "batch": {"jobs", "cronjobs"},
    "networking.k8s.io": {"networkpolicies", "ingresses"},
}
expected = {(group, resource, verb) for group, names in required.items()
            for resource in names for verb in workload_verbs}
# Nothing automated deletes data: volume claims and StatefulSets lose delete.
expected -= {("", "persistentvolumeclaims", "delete"), ("apps", "statefulsets", "delete")}
expected |= {("", "pods", verb) for verb in ("get", "list", "watch", "delete")}
expected.add(("", "pods/log", "get"))
assert permissions == expected, "Role must grant exactly the documented workload/log permissions"

# The denial matrix, evaluated as the RBAC authorizer would for this Role alone.
def allowed_by_role(group, resource, verb):
    return any(group in r["apiGroups"] and resource in r["resources"] and verb in r["verbs"]
               for r in role["rules"])
rbac = "rbac.authorization.k8s.io"
denied = [("", "secrets", v) for v in ("get", "list", "watch", "create", "update", "patch")]
denied += [("", "pods", v) for v in ("create", "update", "patch")]
denied += [("", f"pods/{s}", v) for s in ("exec", "attach", "portforward")
           for v in ("get", "create")]
denied += [("", "pods/ephemeralcontainers", "patch"), ("", "services/proxy", "get"),
           ("", "serviceaccounts/token", "create"), ("", "serviceaccounts", "impersonate"),
           ("", "users", "impersonate"), ("", "groups", "impersonate"),
           ("", "nodes/proxy", "get"), ("", "namespaces", "create")]
denied += [(rbac, k, v) for k in ("roles", "rolebindings", "clusterroles", "clusterrolebindings")
           for v in ("create", "update", "patch", "delete", "bind", "escalate")]
denied += [(g, r, v) for g, r in (("", "persistentvolumeclaims"), ("apps", "statefulsets"),
                                  ("", "persistentvolumes"))
           for v in ("delete", "deletecollection")]
denied += [("", "persistentvolumes", v) for v in ("create", "update", "patch")]
denied += [(g, r, "deletecollection") for rule in role["rules"]
           for g in rule["apiGroups"] for r in rule["resources"]]
for check in denied:
    assert not allowed_by_role(*check), f"Role must deny {check}"
PY
"$ROOT/scripts/deploy/create-agent-kubeconfig.sh" --operator-kubeconfig "$scratch/operator" \
    --kubectl "$KUBECTL" --output-dir "$scratch/agent" >"$scratch/output"
if grep -q test-bearer-token "$scratch/output"; then exit 1; fi
python3 - "$scratch/agent/kubeconfig-econ-graph" <<'PY'
import json
import pathlib
import stat
import sys
p = pathlib.Path(sys.argv[1])
c = json.loads(p.read_text())
assert stat.S_IMODE(p.stat().st_mode) == 0o600
assert stat.S_IMODE(p.parent.stat().st_mode) == 0o700
assert [u["name"] for u in c["users"]] == ["econgraph-agent"]
assert list(c["users"][0]["user"]) == ["token"]
assert c["users"][0]["user"]["token"].endswith(".test-bearer-token")
assert c["contexts"][0]["context"]["user"] == "econgraph-agent"
assert c["clusters"][0]["cluster"]["server"] == "https://example.invalid"
PY
# A preexisting broad binding must stop publication of a new kubeconfig.
if AGENT_TEST_ALLOW_SECRETS=true "$ROOT/scripts/deploy/create-agent-kubeconfig.sh" \
    --operator-kubeconfig "$scratch/operator" --kubectl "$KUBECTL" \
    --output-dir "$scratch/refused" >"$scratch/refused-output" 2>&1; then
    echo 'bootstrap accepted inherited Secret access' >&2; exit 1
fi
[ ! -f "$scratch/refused/kubeconfig-econ-graph" ]
grep -q 'access verification failed' "$scratch/refused-output"

# Subresources are asked with --subresource, never as TYPE/NAME (which kubectl
# reads as a resource name), and every namespace is checked.
grep -q 'auth can-i create pods --subresource=exec -n kube-system' "$AGENT_TEST_CALLS"
grep -q 'auth can-i create pods --subresource=portforward -n econ-graph' "$AGENT_TEST_CALLS"
grep -q 'auth can-i create serviceaccounts --subresource=token -n kube-system' "$AGENT_TEST_CALLS"
grep -q 'auth can-i delete persistentvolumeclaims -n econ-graph' "$AGENT_TEST_CALLS"
grep -q 'auth can-i deletecollection statefulsets.apps -n kube-system' "$AGENT_TEST_CALLS"
grep -q 'auth can-i delete persistentvolumes' "$AGENT_TEST_CALLS"
if grep -Eq 'auth can-i [a-z]+ [a-z.]+/' "$AGENT_TEST_CALLS"; then
    echo 'a permission check names a subresource as TYPE/NAME' >&2; exit 1
fi
# Operator calls pin kind-econ-graph in the operator config; nothing switches
# the current context (the stub fails on use-context/set-context).
grep -q -- "--kubeconfig $scratch/operator --context kind-econ-graph apply -f -" "$AGENT_TEST_CALLS"
grep -q "KUBECONFIG=$scratch/operator --context kind-econ-graph get nodes" "$AGENT_TEST_CALLS.env"

# A token issued for longer than requested is not published.
if AGENT_TEST_TOKEN_SECONDS=31536000 "$ROOT/scripts/deploy/create-agent-kubeconfig.sh" \
    --operator-kubeconfig "$scratch/operator" --kubectl "$KUBECTL" \
    --output-dir "$scratch/long" >"$scratch/long-output" 2>&1; then
    echo 'bootstrap accepted a long-lived token' >&2; exit 1
fi
[ ! -f "$scratch/long/kubeconfig-econ-graph" ]
grep -q 'more than 1 hour' "$scratch/long-output"
if grep -q test-bearer-token "$scratch/long-output"; then exit 1; fi

# Never into a git work tree, where the token could be committed.
mkdir "$scratch/repo"
git -C "$scratch/repo" init -q
if "$ROOT/scripts/deploy/create-agent-kubeconfig.sh" --operator-kubeconfig "$scratch/operator" \
    --kubectl "$KUBECTL" --output-dir "$scratch/repo/agent" >"$scratch/repo-output" 2>&1; then
    echo 'bootstrap wrote into a git work tree' >&2; exit 1
fi
[ ! -e "$scratch/repo/agent" ]
grep -q 'inside a git work tree' "$scratch/repo-output"

# Kubectl defaulting, as cluster_role_default_kubectl: without flags or
# KUBECTL/KUBECONFIG, the kind cluster's kubectl and kubeconfig in the cache.
mkdir -p "$scratch/cache/econgraph/bin"
cp "$KUBECTL" "$scratch/cache/econgraph/bin/kubectl"
echo cache >"$scratch/cache/econgraph/kubeconfig-econ-graph"
: >"$AGENT_TEST_CALLS"
(unset KUBECTL KUBECONFIG
    XDG_CACHE_HOME="$scratch/cache" "$ROOT/scripts/deploy/create-agent-kubeconfig.sh" \
        --output-dir "$scratch/defaulted" >"$scratch/defaulted-output" 2>&1) || {
    cat "$scratch/defaulted-output" >&2; exit 1
}
[ -f "$scratch/defaulted/kubeconfig-econ-graph" ]
grep -q -- "--kubeconfig $scratch/cache/econgraph/kubeconfig-econ-graph --context kind-econ-graph apply" \
    "$AGENT_TEST_CALLS"
# No kind kubectl kept and none named: refused before any cluster call, never
# plain kubectl from PATH (the MicroK8s wrapper on the production host).
mkdir "$scratch/plain-bin"
cat >"$scratch/plain-bin/kubectl" <<'PLAIN'
#!/usr/bin/env bash
echo plain-kubectl >>"$AGENT_TEST_CALLS"
exit 1
PLAIN
chmod +x "$scratch/plain-bin/kubectl"
: >"$AGENT_TEST_CALLS"
if (unset KUBECTL KUBECONFIG
    PATH="$scratch/plain-bin:$PATH" XDG_CACHE_HOME="$scratch/empty-cache" \
        "$ROOT/scripts/deploy/create-agent-kubeconfig.sh" --output-dir "$scratch/plain" \
        >"$scratch/plain-output" 2>&1); then
    echo 'bootstrap ran without a kind kubectl' >&2; exit 1
fi
[ ! -s "$AGENT_TEST_CALLS" ]
[ ! -e "$scratch/plain" ]
grep -qi 'kind.*kubectl' "$scratch/plain-output"
# A MicroK8s kubectl named explicitly is refused too.
mkdir -p "$scratch/snap/bin"
cp "$scratch/plain-bin/kubectl" "$scratch/snap/bin/microk8s.kubectl"
if "$ROOT/scripts/deploy/create-agent-kubeconfig.sh" --operator-kubeconfig "$scratch/operator" \
    --kubectl "$scratch/snap/bin/microk8s.kubectl" --output-dir "$scratch/snapped" \
    >"$scratch/snap-output" 2>&1; then
    echo 'bootstrap accepted the MicroK8s kubectl' >&2; exit 1
fi
grep -q 'MicroK8s' "$scratch/snap-output"
[ ! -s "$AGENT_TEST_CALLS" ]

# The launcher overrides inherited admin selections and the helper never calls Docker/kind.
cat >"$scratch/check-session" <<'CHECK'
#!/usr/bin/env bash
set -euo pipefail
[ "$ECONGRAPH_AGENT_SESSION" = 1 ]
[ "$KUBECONFIG" = "$ECONGRAPH_KUBE_DIR/kubeconfig-econ-graph" ]
source "$ROOT/scripts/deploy/lib/kind-env.sh"
docker() { echo 'unexpected Docker call' >&2; exit 55; }
kind() { echo 'unexpected kind call' >&2; exit 56; }
ensure_kind_kubectl
AGENT_TEST_EXPIRED=true
export AGENT_TEST_EXPIRED
if ensure_kind_kubectl_quietly; then
    echo 'expired restricted credentials accepted' >&2; exit 1
fi
CHECK
chmod +x "$scratch/check-session"
export ROOT
KUBECONFIG=admin CLUSTER_ROLE_CONTEXT=admin \
    "$ROOT/scripts/deploy/run-agent-session.sh" --config-dir "$scratch/agent" -- "$scratch/check-session"
# Credentials scripts refuse before Docker, prompts, reads or writes.
for script in create-secrets.sh set-crawler-keys.sh; do
    : >"$AGENT_TEST_CALLS"
    if ECONGRAPH_AGENT_SESSION=1 "$ROOT/scripts/deploy/$script" --dry-run >"$scratch/guard" 2>&1; then
        echo "$script accepted agent session" >&2; exit 1
    fi
    grep -q 'operator terminal' "$scratch/guard"
    [ ! -s "$AGENT_TEST_CALLS" ]
    # A copied agent kubeconfig is recognised even without the session flag.
    if KUBECONFIG="$scratch/agent/kubeconfig-econ-graph" \
        "$ROOT/scripts/deploy/$script" --dry-run >"$scratch/guard" 2>&1; then
        echo "$script accepted agent kubeconfig" >&2; exit 1
    fi
    grep -q 'operator terminal' "$scratch/guard"
done
AGENT_TEST_USER="admin" "$ROOT/scripts/deploy/run-agent-session.sh" \
    --config-dir "$scratch/agent" -- true >"$scratch/guard" 2>&1 && exit 1
grep -q 'non-agent kubeconfig' "$scratch/guard"
if "$ROOT/scripts/deploy/run-agent-session.sh" --config-dir relative -- true >"$scratch/guard" 2>&1; then
    echo 'launcher accepted relative config directory' >&2; exit 1
fi
grep -q 'must be absolute' "$scratch/guard"
echo 'Agent access checks passed.'
