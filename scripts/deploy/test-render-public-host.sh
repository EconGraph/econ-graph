#!/usr/bin/env bash
# Tests for scripts/deploy/render-public-host.py against the committed manifests:
# every localhost Ingress gains a separate -public copy for the public host (https only,
# without the admin and debug routes; localhost stays as it was), the main public
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
def paths(name):
    return [p["path"] for r in docs[("Ingress", name)]["spec"]["rules"] for p in r["http"]["paths"]]
sys.exit(0 if eval(sys.argv[1]) else 1)
' "$expr" <<<"$rendered" || fail "$name"
}

rendered="$("$render" econgraph.com \
  "$manifests/ingress.yaml" "$manifests/graphql-ingress.yaml" \
  "$manifests/keycloak/ingress.yaml" "$manifests/keycloak/deployment.yaml" \
  "$manifests/keycloak/realm-import-job.yaml" "$manifests/backend-deployment.yaml" \
  "$manifests/configmap.yaml" "$manifests/cloudflared.yaml")"

A="nginx.ingress.kubernetes.io"
for ingress in econ-graph-ingress econ-graph-graphql-ingress keycloak-ingress; do
  check "$ingress stays localhost-only" "hosts('$ingress') == ['localhost']"
  check "$ingress-public serves econgraph.com only" "hosts('$ingress-public') == ['econgraph.com']"
  check "$ingress-public is labelled for cleanup" \
    "docs[('Ingress', '$ingress-public')]['metadata']['labels'] == {'econgraph.io/public-host': 'true'}"
  check "$ingress-public redirects plain http to https" \
    "docs[('Ingress', '$ingress-public')]['metadata']['annotations']['$A/force-ssl-redirect'] == 'true'"
  check "$ingress-public keeps the original annotations" \
    "all(docs[('Ingress', '$ingress-public')]['metadata']['annotations'][k] == v for k, v in docs[('Ingress', '$ingress')]['metadata'].get('annotations', {}).items())"
  check "$ingress itself does not force https (http://localhost keeps working)" \
    "'$A/force-ssl-redirect' not in docs[('Ingress', '$ingress')]['metadata'].get('annotations', {})"
done
check "the graphql rate limits apply on the public host too" \
  "docs[('Ingress', 'econ-graph-graphql-ingress-public')]['metadata']['annotations']['$A/limit-rps'] == '20'"
check "the public host leaves out the admin and debug routes" \
  "paths('econ-graph-ingress-public') == [p for p in paths('econ-graph-ingress') if p not in ('/admin', '/api/admin', '/playground')] and '/api' in paths('econ-graph-ingress-public')"
check "the public Keycloak Ingress routes only the econ-graph realm and its resources" \
  "paths('keycloak-ingress-public') == ['/idp/realms/econ-graph', '/idp/resources']"
check "www redirects on the main public Ingress only" \
  "[n for (k, n), d in docs.items() if k == 'Ingress' and d['metadata'].get('annotations', {}).get('$A/from-to-www-redirect') == 'true'] == ['econ-graph-ingress-public']"
check "nothing trusts client forwarding headers" \
  "not any('use-forwarded-headers' in str(d) or 'configuration-snippet' in str(d) or 'server-snippet' in str(d) for d in docs.values())"
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
