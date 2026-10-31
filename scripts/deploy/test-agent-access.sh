#!/usr/bin/env bash
# Offline regression checks: no Kubernetes or Docker connection is made.
set -Eeuo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# The path checks' unit tests: the owner, private-group, ACL-error, sticky
# symlink and symlink-loop rules, independent of this host's directories.
python3 "$ROOT/scripts/deploy/lib/test_agent_paths.py"
# A private base, whatever the umask and primary group: directories made below
# are 0700 unless a check sets a mode, and the chain to the base must pass.
umask 077
scratch="$(mktemp -d)"
trap 'chmod -R u+rwx "$scratch" 2>/dev/null; rm -rf "$scratch"' EXIT
trap 'echo "test-agent-access: failed at line $LINENO" >&2' ERR
chmod 0700 "$scratch"
if ! python3 "$ROOT/scripts/deploy/lib/agent_paths.py" check-chain "$scratch"; then
    echo "test-agent-access: $scratch is not under directories only you or root can change; set TMPDIR" >&2
    exit 1
fi
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
    *'config view --minify'*'.namespace}'*) printf '%s' "${AGENT_TEST_NS-econ-graph}" ;;
    *'config view --minify'*) printf '%s' "${AGENT_TEST_USER:-econgraph-agent}" ;;
    # The applied manifest is kept: the rules review below answers from it.
    *'apply -f -'*) cat >"$AGENT_TEST_CALLS.manifest" ;;
    *'create token econgraph-agent --duration=1h'*)
        # A JWT shaped token; its signature part carries the marker the test
        # greps for in output.
        python3 -c 'import base64, json, sys, time
claims = {"exp": int(time.time()) + int(sys.argv[1])}
enc = lambda b: base64.urlsafe_b64encode(b).rstrip(b"=").decode()
print(enc(b"{}") + "." + enc(json.dumps(claims).encode()) + ".test-bearer-token")' \
            "${AGENT_TEST_TOKEN_SECONDS:-3600}" ;;
    *'get namespaces -o'*)
        [ "${AGENT_TEST_NO_NAMESPACES:-false}" = true ] || printf 'econ-graph\nkube-system\n' ;;
    *'create --raw /apis/authorization.k8s.io/v1/selfsubjectrulesreviews -f '*)
        # What the API server answers: the defaults every identity has, the
        # applied Role's rules in its namespace, and an injected extra grant.
        python3 - "${@: -1}" "$AGENT_TEST_CALLS.manifest" <<'PY'
import json
import os
import sys
ns = json.load(open(sys.argv[1]))["spec"]["namespace"]
items = json.load(open(sys.argv[2]))["items"]
role = next(i for i in items if i["kind"] == "Role")
rules = [{"verbs": ["create"], "apiGroups": ["authorization.k8s.io"],
          "resources": ["selfsubjectaccessreviews", "selfsubjectrulesreviews"]},
         {"verbs": ["create"], "apiGroups": ["authentication.k8s.io"],
          "resources": ["selfsubjectreviews"]}]
if ns == role["metadata"]["namespace"]:
    rules += role["rules"]
urls = [{"verbs": ["get"], "nonResourceURLs": [
    "/api", "/api/*", "/apis", "/apis/*", "/healthz", "/livez", "/openapi", "/openapi/*",
    "/readyz", "/version", "/version/", "/.well-known/openid-configuration",
    "/.well-known/openid-configuration/", "/openid/v1/jwks", "/openid/v1/jwks/"]}]
if ns == os.environ.get("AGENT_TEST_EXTRA_NS", "kube-system"):
    extra = json.loads(os.environ.get("AGENT_TEST_EXTRA_RULE", "null"))
    if extra and "nonResourceURLs" in extra:
        urls.append(extra)
    elif extra:
        rules.append(extra)
status = {"resourceRules": rules, "nonResourceRules": urls,
          "incomplete": os.environ.get("AGENT_TEST_INCOMPLETE") == "true"}
if os.environ.get("AGENT_TEST_EVALUATION_ERROR"):
    status["evaluationError"] = os.environ["AGENT_TEST_EVALUATION_ERROR"]
print(json.dumps({"kind": "SelfSubjectRulesReview", "status": status}))
PY
        ;;
    *'auth can-i get pods -n'* | *'auth can-i get pods --subresource=log -n'*)
        if [[ "$*" == *"${AGENT_TEST_NO_READ:-no such check}"* ]]; then printf no; exit 1; fi
        printf yes; [ "${AGENT_TEST_EXPIRED:-false}" != true ] ;;
    *'auth can-i'*) printf no; exit 1 ;;
    *) echo "unexpected kubectl call: $*" >&2; exit 1 ;;
esac
STUB
chmod +x "$scratch/bin/kubectl"
export KUBECTL="$scratch/bin/kubectl"
unset ECONGRAPH_AGENT_SESSION KUBECONFIG
bootstrap() {
    "$ROOT/scripts/deploy/create-agent-kubeconfig.sh" --operator-kubeconfig "$scratch/operator" \
        --kubectl "$KUBECTL" "$@"
}
# refused NAME MESSAGE ARG...: the bootstrap fails, says MESSAGE, publishes no
# kubeconfig and never prints the token.
published() { find "$scratch" -name kubeconfig-econ-graph | wc -l; }
refused() {
    local name="$1" message="$2" before
    shift 2
    before="$(published)"
    if bootstrap "$@" >"$scratch/$name.out" 2>&1; then
        echo "bootstrap accepted: $name" >&2; exit 1
    fi
    grep -qF -- "$message" "$scratch/$name.out" || {
        echo "$name: expected '$message' in:" >&2; cat "$scratch/$name.out" >&2; exit 1
    }
    if grep -q test-bearer-token "$scratch/$name.out"; then exit 1; fi
    [ "$(published)" = "$before" ] || { echo "$name: a kubeconfig was published" >&2; exit 1; }
}

# --- The Role: read-only, exactly -------------------------------------------
bootstrap --output-dir "$scratch/agent" --dry-run >"$scratch/rbac.json" 2>"$scratch/rbac.err"
[ ! -f "$AGENT_TEST_CALLS" ]
python3 - "$scratch/rbac.json" <<'PY'
import json
import sys
items = json.load(open(sys.argv[1]))["items"]
assert [i["kind"] for i in items] == ["ServiceAccount", "Role", "RoleBinding"]
assert items[0]["automountServiceAccountToken"] is False
role, binding = items[1], items[2]
assert binding["roleRef"] == {
    "apiGroup": "rbac.authorization.k8s.io", "kind": "Role", "name": "econgraph-agent"}
assert binding["subjects"] == [{
    "kind": "ServiceAccount", "name": "econgraph-agent",
    "namespace": binding["metadata"]["namespace"]}]
assert role["metadata"]["namespace"] == binding["metadata"]["namespace"] == "econ-graph"
assert all(set(rule) == {"apiGroups", "resources", "verbs"} for rule in role["rules"]), \
    "no resourceNames or nonResourceURLs"
permissions = {(group, resource, verb) for rule in role["rules"]
               for group in rule["apiGroups"] for resource in rule["resources"]
               for verb in rule["verbs"]}
readable = {
    "": {"pods", "pods/log", "services", "endpoints", "configmaps", "events",
         "persistentvolumeclaims"},
    "apps": {"deployments", "replicasets", "statefulsets", "daemonsets"},
    "batch": {"jobs", "cronjobs"},
    "networking.k8s.io": {"networkpolicies", "ingresses"},
    "gateway.networking.k8s.io": {"httproutes"},
}
expected = {(group, resource, verb) for group, names in readable.items()
            for resource in names for verb in ("get", "list", "watch")}
assert permissions == expected, sorted(permissions ^ expected)
PY

# --- A clean bootstrap ------------------------------------------------------
mkdir -m 0755 "$scratch/agent"   # an existing, too-open directory is made private
bootstrap --output-dir "$scratch/agent" >"$scratch/output"
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
# The launcher's kubectl is a private copy, not a symlink out.
kubectl = p.parent / "bin" / "kubectl"
assert not kubectl.is_symlink() and kubectl.is_file()
assert stat.S_IMODE(kubectl.stat().st_mode) == 0o700
assert stat.S_IMODE(kubectl.parent.stat().st_mode) == 0o700
PY
# Every namespace's permissions were listed with the issued token.
for ns in econ-graph kube-system; do
    grep -q -- "--kubeconfig $scratch/agent/.bootstrap.*/kubeconfig create --raw /apis/authorization.k8s.io/v1/selfsubjectrulesreviews -f .*/rules/$ns.request" \
        "$AGENT_TEST_CALLS"
done
# Operator calls pin kind-econ-graph in the operator config; nothing switches
# the current context (the stub fails on use-context/set-context).
grep -q -- "--kubeconfig $scratch/operator --context kind-econ-graph apply -f -" "$AGENT_TEST_CALLS"
grep -q "KUBECONFIG=$scratch/operator --context kind-econ-graph get nodes" "$AGENT_TEST_CALLS.env"

# --- Anything beyond the reads stops publication ----------------------------
# One extra verb on a resource the Role reads.
AGENT_TEST_EXTRA_NS=econ-graph AGENT_TEST_EXTRA_RULE='{"verbs":["patch"],"apiGroups":[""],"resources":["pods"]}' \
    refused extra-verb 'namespace econ-graph: patch pods' --output-dir "$scratch/extra-verb"
# A read the Role grants, but in another namespace.
AGENT_TEST_EXTRA_RULE='{"verbs":["get"],"apiGroups":[""],"resources":["pods"]}' \
    refused other-namespace 'namespace kube-system: get pods' --output-dir "$scratch/other-namespace"
n=0
while IFS='|' read -r rule message; do
    n=$((n + 1))
    AGENT_TEST_EXTRA_RULE="$rule" refused "grant-$n" "$message" --output-dir "$scratch/grant-$n"
done <<'GRANTS'
{"verbs":["get"],"apiGroups":[""],"resources":["secrets"]}|namespace kube-system: get secrets
{"verbs":["*"],"apiGroups":["*"],"resources":["*"]}|namespace kube-system: * *.*
{"verbs":["get"],"apiGroups":[""],"resources":["pods/*"]}|namespace kube-system: get pods/*
{"verbs":["create"],"apiGroups":["admissionregistration.k8s.io"],"resources":["mutatingwebhookconfigurations"]}|create mutatingwebhookconfigurations.admissionregistration.k8s.io
{"verbs":["create"],"apiGroups":["apiextensions.k8s.io"],"resources":["customresourcedefinitions"]}|create customresourcedefinitions.apiextensions.k8s.io
{"verbs":["approve"],"apiGroups":["certificates.k8s.io"],"resources":["signers"]}|approve signers.certificates.k8s.io
{"verbs":["create"],"apiGroups":["certificates.k8s.io"],"resources":["certificatesigningrequests"]}|create certificatesigningrequests.certificates.k8s.io
{"verbs":["impersonate"],"apiGroups":["authentication.k8s.io"],"resources":["uids","userextras/scopes"]}|impersonate userextras/scopes.authentication.k8s.io
{"verbs":["create"],"apiGroups":[""],"resources":["nodes/proxy"]}|create nodes/proxy
{"verbs":["delete"],"apiGroups":[""],"resources":["namespaces"]}|delete namespaces
{"verbs":["patch"],"apiGroups":[""],"resources":["persistentvolumes"]}|patch persistentvolumes
{"verbs":["update"],"apiGroups":[""],"resources":["pods/ephemeralcontainers"]}|update pods/ephemeralcontainers
{"verbs":["update","patch"],"apiGroups":[""],"resources":["pods"]}|update pods
{"verbs":["create"],"apiGroups":[""],"resources":["serviceaccounts/token"]}|create serviceaccounts/token
{"verbs":["get"],"nonResourceURLs":["/metrics"]}|namespace kube-system: get /metrics
{"verbs":["post"],"nonResourceURLs":["/api/*"]}|namespace kube-system: post /api/*
GRANTS
AGENT_TEST_INCOMPLETE=true refused incomplete 'could not list every permission' \
    --output-dir "$scratch/incomplete"
AGENT_TEST_EVALUATION_ERROR='webhook authorizer unavailable' refused evaluation-error \
    'could not list every permission' --output-dir "$scratch/evaluation-error"
AGENT_TEST_NO_NAMESPACES=true refused no-namespaces 'could not enumerate namespaces' \
    --output-dir "$scratch/no-namespaces"
# The reads it is for must work, logs included.
AGENT_TEST_NO_READ='get pods -n' refused no-pods 'cannot get pods in econ-graph' \
    --output-dir "$scratch/no-pods"
AGENT_TEST_NO_READ='--subresource=log' refused no-logs 'cannot get pods --subresource=log' \
    --output-dir "$scratch/no-logs"
refused bad-namespace 'must be a DNS label' --namespace '' --output-dir "$scratch/bad-namespace"

# --- Token lifetime: at most 65 minutes -------------------------------------
AGENT_TEST_TOKEN_SECONDS=3840 bootstrap --output-dir "$scratch/64min" >/dev/null   # 64 minutes
for seconds in 4200 7200 86400 31536000; do   # 70 minutes, 2 hours, 1 day, 1 year
    AGENT_TEST_TOKEN_SECONDS=$seconds refused "token-$seconds" 'more than 1 hour' \
        --output-dir "$scratch/token-$seconds"
done

# --- Where the kubeconfig may go --------------------------------------------
mkdir "$scratch/repo"
git -C "$scratch/repo" init -q
refused repo 'inside a git work tree' --output-dir "$scratch/repo/agent"
[ ! -e "$scratch/repo/agent" ]
mkdir "$scratch/target"
ln -s "$scratch/target" "$scratch/linked"
refused symlink 'output directory is a symlink' --output-dir "$scratch/linked"
[ -z "$(ls -A "$scratch/target")" ]
if [ "$(id -u)" != 0 ]; then
    : >"$AGENT_TEST_CALLS"
    refused not-owned 'output directory must be owned by you' --output-dir /usr
    [ ! -s "$AGENT_TEST_CALLS" ]
fi
# A directory above it that others can write: refused before anything is created.
mkdir -m 0777 "$scratch/open"
chmod 0777 "$scratch/open"
refused open-parent "$scratch/open can be changed by other users" --output-dir "$scratch/open/a/agent"
[ ! -e "$scratch/open/a" ]
# A symlink inside an open directory: the directory holding the link counts,
# not only where it points.
mkdir -m 0777 "$scratch/ww"
chmod 0777 "$scratch/ww"
mkdir "$scratch/priv"
ln -s ../priv "$scratch/ww/link"
refused symlinked-ancestor "$scratch/ww can be changed by other users" --output-dir "$scratch/ww/link/agent"
[ ! -e "$scratch/priv/agent" ]
# ... also when the open directory is only on the way to the link's target.
ln -s "$scratch/ww/link" "$scratch/via-ww"
refused symlink-target "$scratch/ww can be changed by other users" --output-dir "$scratch/via-ww/agent"
# A trusted symlink is fine.
ln -s priv "$scratch/priv-link"
bootstrap --output-dir "$scratch/priv-link/agent" >/dev/null
[ -f "$scratch/priv/agent/kubeconfig-econ-graph" ]
# A directory above it owned by another user: test_agent_paths.py, above.
# Sticky directories (like /tmp) are fine.
mkdir "$scratch/sticky"
chmod 1777 "$scratch/sticky"
bootstrap --output-dir "$scratch/sticky/agent" >/dev/null
# Group-writable only for your own private group.
other_group="$(id -G | tr ' ' '\n' | grep -vx "$(id -g)" | head -n1 || true)"
if [ -n "$other_group" ]; then
    mkdir "$scratch/shared"
    chgrp "$other_group" "$scratch/shared"
    chmod 0770 "$scratch/shared"
    refused group-parent "$scratch/shared can be changed by other users" --output-dir "$scratch/shared/agent"
fi
if python3 -c 'import os, sys; sys.path.insert(0, sys.argv[1]); import agent_paths
sys.exit(not agent_paths.own_private_group(os.getgid()))' "$ROOT/scripts/deploy/lib"; then
    mkdir "$scratch/private-group"
    chmod 0770 "$scratch/private-group"
    bootstrap --output-dir "$scratch/private-group/agent" >/dev/null
fi
# The bin directory: never a symlink, made private if it is not.
mkdir -p "$scratch/binlink/agent" "$scratch/elsewhere"
chmod 0700 "$scratch/binlink/agent"
chmod 0755 "$scratch/elsewhere"
ln -s "$scratch/elsewhere" "$scratch/binlink/agent/bin"
refused bin-symlink 'output bin directory is a symlink' --output-dir "$scratch/binlink/agent"
[ -z "$(ls -A "$scratch/elsewhere")" ]
[ "$(stat -c %a "$scratch/elsewhere")" = 755 ]   # nothing was chmod-ed through the link
# A path component that is a file, not a directory.
: >"$scratch/a-file"
refused file-component "$scratch/a-file is not a directory" --output-dir "$scratch/a-file/agent"
mkdir -p "$scratch/binmode/agent/bin"
chmod 0777 "$scratch/binmode/agent/bin"
bootstrap --output-dir "$scratch/binmode/agent" >/dev/null
[ "$(stat -c %a "$scratch/binmode/agent/bin")" = 700 ]
# The helper's ownership check, on a directory that is root's.
if [ "$(id -u)" != 0 ]; then
    if python3 "$ROOT/scripts/deploy/lib/agent_paths.py" check-private 'test dir' dir 755 /usr \
        2>"$scratch/owner.out"; then
        echo 'check-private accepted a directory owned by root' >&2; exit 1
    fi
    grep -q 'test dir must be owned by you' "$scratch/owner.out"
fi

# --- POSIX ACLs --------------------------------------------------------------
mkdir "$scratch/acl-probe"
if command -v setfacl >/dev/null 2>&1 && setfacl -m u:65534:r-x "$scratch/acl-probe" 2>/dev/null; then
    # On the output directory: any extended entry, or a default ACL.
    mkdir -p "$scratch/acl-out/agent" "$scratch/acl-default/agent"
    setfacl -m u:65534:r-x "$scratch/acl-out/agent"
    refused acl-out 'output directory has an extended ACL' --output-dir "$scratch/acl-out/agent"
    setfacl -d -m u:65534:r-x "$scratch/acl-default/agent"
    refused acl-default 'output directory has an extended ACL' --output-dir "$scratch/acl-default/agent"
    mkdir -p "$scratch/acl-bin/agent/bin"
    chmod 0700 "$scratch/acl-bin/agent"
    setfacl -m u:65534:r-x "$scratch/acl-bin/agent/bin"
    refused acl-bin 'output bin directory has an extended ACL' --output-dir "$scratch/acl-bin/agent"
    # On a directory above it: write for another user or group is refused,
    # read is fine.
    mkdir "$scratch/acl-write" "$scratch/acl-group-write" "$scratch/acl-read"
    setfacl -m u:65534:rwx "$scratch/acl-write"
    refused acl-write "$scratch/acl-write can be changed by other users" --output-dir "$scratch/acl-write/agent"
    setfacl -m g:65534:rwx "$scratch/acl-group-write"
    refused acl-group-write "$scratch/acl-group-write can be changed by other users" \
        --output-dir "$scratch/acl-group-write/agent"
    setfacl -m u:65534:r-x "$scratch/acl-read"
    bootstrap --output-dir "$scratch/acl-read/agent" >/dev/null
    # A mask without write disables a named entry's write.
    mkdir "$scratch/acl-masked"
    setfacl -m u:65534:rwx,m::r-x "$scratch/acl-masked"
    bootstrap --output-dir "$scratch/acl-masked/agent" >/dev/null
else
    echo 'note: setfacl unavailable or ACLs unsupported here; skipping the ACL checks'
fi

# --- Which kubectl ----------------------------------------------------------
# Without flags or KUBECTL/KUBECONFIG: the kind cluster's kubectl and
# kubeconfig in the cache (cluster_role_default_kubectl).
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
# MicroK8s kubectl, at any link of the chain. The real one:
# /snap/bin/kubectl -> microk8s.kubectl -> /usr/bin/snap -> ../lib/snapd/snapd.
fake="$scratch/fake"
mkdir -p "$fake/snap/bin" "$fake/usr/bin" "$fake/usr/lib/snapd" "$fake/links"
cp "$scratch/plain-bin/kubectl" "$fake/usr/lib/snapd/snapd"
cp "$scratch/plain-bin/kubectl" "$fake/snap/bin/microk8s"
cp "$scratch/plain-bin/kubectl" "$fake/snap/bin/plain-file"
cp "$scratch/plain-bin/kubectl" "$fake/microk8s-kubectl"
ln -s ../lib/snapd/snapd "$fake/usr/bin/snap"
ln -s "$fake/usr/bin/snap" "$fake/snap/bin/microk8s.kubectl"
ln -s microk8s.kubectl "$fake/snap/bin/kubectl"
ln -s ../usr/bin/snap "$fake/links/via-snap"           # relative, outside /snap/bin
ln -s "$fake/usr/lib/snapd/snapd" "$fake/links/via-snapd"
ln -s "$fake/microk8s-kubectl" "$fake/links/via-microk8s"
ln -s "$KUBECTL" "$fake/links/kind-kubectl"
# Ends at a file named snap; passes through /snap/bin to an innocent end.
mkdir -p "$fake/other"
cp "$scratch/plain-bin/kubectl" "$fake/other/snap"
ln -s "$fake/other/snap" "$fake/links/to-snap-file"
ln -s "$KUBECTL" "$fake/snap/bin/hop"
ln -s "$fake/snap/bin/hop" "$fake/links/via-snap-bin"
: >"$AGENT_TEST_CALLS"
for kubectl in "$fake/snap/bin/kubectl" "$fake/snap/bin/plain-file" "$fake/snap/bin/microk8s" \
    "$fake/links/via-snap" "$fake/links/via-snapd" "$fake/links/via-microk8s" \
    "$fake/links/to-snap-file" "$fake/links/via-snap-bin"; do
    if "$ROOT/scripts/deploy/create-agent-kubeconfig.sh" --operator-kubeconfig "$scratch/operator" \
        --kubectl "$kubectl" --output-dir "$scratch/snapped" --dry-run >"$scratch/snap-output" 2>&1; then
        echo "bootstrap accepted the MicroK8s kubectl $kubectl" >&2; exit 1
    fi
    grep -q 'MicroK8s' "$scratch/snap-output"
done
[ ! -s "$AGENT_TEST_CALLS" ]
# A wrapper script that runs it, and a binary a snap's bin entry resolves to
# or that lives inside a snap (a fake snap root).
mkdir -p "$fake/wrap" "$fake/snaproot/bin" "$fake/snaproot/microk8s/current" "$fake/opt"
printf '#!/bin/sh\nexec /snap/bin/microk8s kubectl "$@"\n' >"$fake/wrap/kubectl"
printf '#!/bin/sh\nexec snap run kubectl "$@"\n' >"$fake/wrap/kubectl-snap"
cp "$KUBECTL" "$fake/opt/kubectl"
ln -s "$fake/opt/kubectl" "$fake/snaproot/bin/kubectl"
cp "$KUBECTL" "$fake/snaproot/microk8s/current/kubectl"
chmod +x "$fake/wrap/kubectl" "$fake/wrap/kubectl-snap"
for kubectl in "$fake/wrap/kubectl" "$fake/wrap/kubectl-snap" "$fake/opt/kubectl" \
    "$fake/snaproot/microk8s/current/kubectl"; do
    if ECONGRAPH_SNAP_ROOT="$fake/snaproot" "$ROOT/scripts/deploy/create-agent-kubeconfig.sh" \
        --operator-kubeconfig "$scratch/operator" --kubectl "$kubectl" --output-dir "$scratch/snapped" \
        --dry-run >"$scratch/snap-output" 2>&1; then
        echo "bootstrap accepted the MicroK8s kubectl $kubectl" >&2; exit 1
    fi
    grep -q 'MicroK8s' "$scratch/snap-output"
done
[ ! -s "$AGENT_TEST_CALLS" ]
# A script that does not mention snap or MicroK8s (like these stubs) is fine.
ECONGRAPH_SNAP_ROOT="$fake/snaproot" "$ROOT/scripts/deploy/create-agent-kubeconfig.sh" \
    --operator-kubeconfig "$scratch/operator" --kubectl "$KUBECTL" --output-dir "$scratch/snapped" \
    --dry-run >/dev/null 2>&1
# A symlink chain that ends at a real kubectl is fine.
"$ROOT/scripts/deploy/create-agent-kubeconfig.sh" --operator-kubeconfig "$scratch/operator" \
    --kubectl "$fake/links/kind-kubectl" --output-dir "$scratch/snapped" --dry-run >/dev/null 2>&1

# --- The launcher and the credential guards ---------------------------------
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
AGENT_TEST_NS="" "$ROOT/scripts/deploy/run-agent-session.sh" \
    --config-dir "$scratch/agent" -- true >"$scratch/guard" 2>&1 && exit 1
grep -q 'must select a namespace' "$scratch/guard"
# The launcher runs nothing from a config directory that is no longer private.
launcher_refuses() {
    local name="$1" message="$2" dir="$scratch/session-$1"
    shift 2
    cp -a "$scratch/agent" "$dir"
    (cd "$dir" && "$@")
    : >"$AGENT_TEST_CALLS"
    if "$ROOT/scripts/deploy/run-agent-session.sh" --config-dir "$dir" -- true >"$scratch/guard" 2>&1; then
        echo "launcher accepted: $name" >&2; exit 1
    fi
    grep -qF -- "$message" "$scratch/guard" || {
        echo "$name: expected '$message' in:" >&2; cat "$scratch/guard" >&2; exit 1
    }
    [ ! -s "$AGENT_TEST_CALLS" ] || { echo "$name: kubectl ran" >&2; exit 1; }
}
launcher_refuses kubectl-symlink 'agent kubectl is a symlink' ln -sf "$KUBECTL" bin/kubectl
launcher_refuses kubectl-mode 'agent kubectl has mode 777' chmod 0777 bin/kubectl
launcher_refuses bin-symlink 'agent bin directory is a symlink' \
    sh -c 'mv bin real-bin && ln -s real-bin bin'
launcher_refuses bin-mode 'agent bin directory has mode 755' chmod 0755 bin
launcher_refuses dir-mode 'agent config directory has mode 755' chmod 0755 .
launcher_refuses kubeconfig-mode 'agent kubeconfig has mode 644' chmod 0644 kubeconfig-econ-graph
mkdir -m 0777 "$scratch/open-sessions"
chmod 0777 "$scratch/open-sessions"
cp -a "$scratch/agent" "$scratch/open-sessions/agent"
: >"$AGENT_TEST_CALLS"
if "$ROOT/scripts/deploy/run-agent-session.sh" --config-dir "$scratch/open-sessions/agent" -- true \
    >"$scratch/guard" 2>&1; then
    echo 'launcher accepted a config directory under an open one' >&2; exit 1
fi
grep -qF "$scratch/open-sessions can be changed by other users" "$scratch/guard"
[ ! -s "$AGENT_TEST_CALLS" ]
if command -v setfacl >/dev/null 2>&1 && setfacl -m u:65534:r-x "$scratch/acl-probe" 2>/dev/null; then
    launcher_refuses kubeconfig-acl 'agent kubeconfig has an extended ACL' \
        setfacl -m u:65534:r-- kubeconfig-econ-graph
    launcher_refuses kubectl-acl 'agent kubectl has an extended ACL' setfacl -m u:65534:r-x bin/kubectl
fi
if "$ROOT/scripts/deploy/run-agent-session.sh" --config-dir relative -- true >"$scratch/guard" 2>&1; then
    echo 'launcher accepted relative config directory' >&2; exit 1
fi
grep -q 'must be absolute' "$scratch/guard"
echo 'Agent access checks passed.'
