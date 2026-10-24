#!/usr/bin/env bash
# Tests for scripts/deploy/render-public-host.py against the committed manifests:
# every localhost Ingress rule gains the public host (and localhost stays), the main
# Ingress redirects www, Keycloak's hostname and the backend's issuer agree on
# https://<host>/idp/realms/econ-graph, the JWKS stays in-cluster, the realm import
# allows the public origin plus localhost, CORS gains the origin once, and bad host
# names are refused.
#
# Needs python3 with PyYAML. Usage: scripts/deploy/test-render-public-host.sh
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
render="${repo_root}/scripts/deploy/render-public-host.py"
manifests="${repo_root}/k8s/manifests"
failures=0

fail() {
  echo "FAIL $1"
  failures=$((failures + 1))
}

# check NAME PYTHON-EXPR: evaluates EXPR with `docs` (the rendered documents, by
# kind/name) and fails NAME unless it is true. The expressions are the literals in
# this file, never input from anywhere else, so eval is safe here.
check() {
  local name="$1" expr="$2"
  python3 -c '
import sys, yaml
docs = {(d["kind"], d["metadata"]["name"]): d for d in yaml.safe_load_all(sys.stdin) if d}
def env(kind, name, container, var):
    spec = docs[(kind, name)]["spec"]["template"]["spec"]
    for c in spec.get("containers", []) + spec.get("initContainers", []):
        if c["name"] == container:
            return {e["name"]: e.get("value") for e in c.get("env", [])}[var]
def hosts(name):
    return [r.get("host") for r in docs[("Ingress", name)]["spec"]["rules"]]
sys.exit(0 if eval(sys.argv[1]) else 1)
' "$expr" <<<"$rendered" || fail "$name"
}

rendered="$("$render" econgraph.com \
  "$manifests/ingress.yaml" "$manifests/graphql-ingress.yaml" \
  "$manifests/keycloak/ingress.yaml" "$manifests/keycloak/deployment.yaml" \
  "$manifests/keycloak/realm-import-job.yaml" "$manifests/backend-deployment.yaml" \
  "$manifests/configmap.yaml" "$manifests/cloudflared.yaml")"

for ingress in econ-graph-ingress econ-graph-graphql-ingress keycloak-ingress; do
  check "$ingress serves localhost and econgraph.com" \
    "hosts('$ingress') == ['localhost', 'econgraph.com']"
done
check "the public rules route the same paths as localhost" \
  "[r['http'] for r in docs[('Ingress', 'econ-graph-ingress')]['spec']['rules']][0] == [r['http'] for r in docs[('Ingress', 'econ-graph-ingress')]['spec']['rules']][1]"
check "www redirects on the main Ingress" \
  "docs[('Ingress', 'econ-graph-ingress')]['metadata']['annotations'].get('nginx.ingress.kubernetes.io/from-to-www-redirect') == 'true'"
check "only the main Ingress redirects www" \
  "'nginx.ingress.kubernetes.io/from-to-www-redirect' not in docs[('Ingress', 'keycloak-ingress')]['metadata']['annotations']"
check "KC_HOSTNAME is the https public URL" \
  "env('Deployment', 'keycloak', 'keycloak', 'KC_HOSTNAME') == 'https://econgraph.com/idp'"
check "the admin console stays on the port-forward" \
  "env('Deployment', 'keycloak', 'keycloak', 'KC_HOSTNAME_ADMIN') == 'http://localhost:18080/idp'"
check "Keycloak's proxy headers are still trusted" \
  "env('Deployment', 'keycloak', 'keycloak', 'KC_PROXY_HEADERS') == 'xforwarded'"
check "the backend's issuer is Keycloak's" \
  "env('Deployment', 'econ-graph-backend', 'backend', 'OIDC_ISSUER') == env('Deployment', 'keycloak', 'keycloak', 'KC_HOSTNAME') + '/realms/econ-graph'"
check "the JWKS is still fetched in-cluster" \
  "env('Deployment', 'econ-graph-backend', 'backend', 'OIDC_JWKS_URL').startswith('http://keycloak-service:8080/')"
check "Keycloak's realm placeholders use the public origin" \
  "env('Deployment', 'keycloak', 'keycloak', 'KC_WEB_BASE_URL') == 'https://econgraph.com' and env('Deployment', 'keycloak', 'keycloak', 'KC_WEB_E2E_BASE_URL') == 'http://localhost'"
check "the realm import allows the public origin and localhost" \
  "env('Job', 'keycloak-realm-import', 'render-realm', 'KC_WEB_BASE_URL') == 'https://econgraph.com' and env('Job', 'keycloak-realm-import', 'render-realm', 'KC_WEB_E2E_BASE_URL') == 'http://localhost'"
check "CORS allows the public origin first and keeps the local one" \
  "docs[('ConfigMap', 'econ-graph-config')]['data']['CORS_ORIGIN'] == 'https://econgraph.com,http://localhost,http://localhost:3000'"
check "cloudflared is untouched" \
  "docs[('Deployment', 'cloudflared')]['spec']['replicas'] == 1"

# Rendering twice changes nothing more (deploy.sh may be re-run on rendered output).
twice="$("$render" econgraph.com - <<<"$rendered")"
[ "$twice" = "$rendered" ] || fail "rendering is not idempotent"

# The rendered realm-import values give the redirect URIs the realm needs.
realm="$(env -i PATH="$PATH" KC_WEB_BASE_URL=https://econgraph.com KC_WEB_E2E_BASE_URL=http://localhost \
  "${repo_root}/scripts/keycloak/render-realm.sh" "${repo_root}/config/keycloak/econ-graph-realm.json")"
for uri in https://econgraph.com/auth/callback https://econgraph.com/silent-callback.html \
  http://localhost/auth/callback; do
  grep -qF -- "\"$uri\"" <<<"$realm" || fail "realm redirect URIs lack $uri"
done

for bad in localhost "" "econgraph" "https://econgraph.com" "econgraph.com/" "www.econgraph.com" \
  "econ graph.com" "-econgraph.com"; do
  if "$render" "$bad" /dev/null >/dev/null 2>&1; then
    fail "host '$bad' was accepted"
  fi
done
"$render" EconGraph.com /dev/null >/dev/null || fail "a mixed-case host was refused"

if [ "$failures" -gt 0 ]; then
  echo "$failures check(s) failed"
  exit 1
fi
echo "render-public-host: all checks passed"
