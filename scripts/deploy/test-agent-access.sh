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
case "$*" in
    *'config view --minify --flatten --raw'*)
        printf '[{"name":"operator-cluster","cluster":{"server":"https://example.invalid","certificate-authority-data":"Y2E="}}]' ;;
    *'config view --minify'*'.namespace}'*) printf econ-graph ;;
    *'config view --minify'*) printf '%s' "${AGENT_TEST_USER:-econgraph-agent}" ;;
    *'apply -f -'*) cat >/dev/null ;;
    *'create token econgraph-agent'*) printf 'test-bearer-token\n' ;;
    *'get namespaces -o'*) printf 'econ-graph\nkube-system\n' ;;
    *'auth can-i get pods'*) printf yes; [ "${AGENT_TEST_EXPIRED:-false}" != true ] ;;
    *'auth can-i'*secrets*)
        if [ "${AGENT_TEST_ALLOW_SECRETS:-false}" = true ]; then printf yes; else printf no; exit 1; fi ;;
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
assert resources[2]["kind"] == "RoleBinding"
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
assert c["users"] == [{"name": "econgraph-agent", "user": {"token": "test-bearer-token"}}]
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
echo 'Agent access checks passed.'
