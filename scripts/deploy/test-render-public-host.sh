#!/usr/bin/env bash
# Tests for scripts/deploy/render-public-host.py against the committed manifests:
# every localhost HTTPRoute gains a separate -public copy for the public host (listener
# https only, without the admin and debug paths; localhost stays as it was), the rate
# limits cover the copies, plain http and www redirect to https://<host>, Keycloak's
# hostname and the backend's issuer agree on https://<host>/idp/realms/econ-graph, the
# JWKS stays in-cluster, the realm import allows the public origin plus localhost, CORS
# gains the origin once, and bad host names are refused.
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
def route(name):
    return docs[("HTTPRoute", name)]
def hosts(name):
    return route(name)["spec"]["hostnames"]
def sections(name):
    return [p.get("sectionName") for p in route(name)["spec"]["parentRefs"]]
def paths(name):
    return [m["path"]["value"] for r in route(name)["spec"]["rules"] for m in r.get("matches", [])]
def redirect(name):
    return route(name)["spec"]["rules"][0]["filters"][0]["requestRedirect"]
def targets(name):
    return [t["name"] for t in docs[("BackendTrafficPolicy", name)]["spec"]["targetRefs"]]
sys.exit(0 if eval(sys.argv[1]) else 1)
' "$expr" <<<"$rendered" || fail "$name"
}

rendered="$("$render" econgraph.com \
  "$manifests/gateway/routes/httproutes.yaml" "$manifests/gateway/routes/ratelimit-policies.yaml" \
  "$manifests/keycloak/deployment.yaml" "$manifests/keycloak/realm-import-job.yaml" \
  "$manifests/backend-deployment.yaml" "$manifests/configmap.yaml" "$manifests/cloudflared.yaml")"

for r in econ-graph-frontend econ-graph-backend econ-graph-graphql econ-graph-data-graphql econ-graph-idp; do
  check "$r stays localhost-only on listener http" "hosts('$r') == ['localhost'] and sections('$r') == ['http']"
  check "$r-public serves econgraph.com only, on listener https only" \
    "hosts('$r-public') == ['econgraph.com'] and sections('$r-public') == ['https']"
  check "$r-public is labelled for cleanup" \
    "route('$r-public')['metadata']['labels'] == {'econgraph.io/public-host': 'true'}"
  check "$r-public keeps the rules' filters and backends" \
    "[(r.get('filters'), r['backendRefs']) for r in route('$r-public')['spec']['rules']] == [(r.get('filters'), r['backendRefs']) for r in route('$r')['spec']['rules']]"
done
check "the rate limits apply on the public host too" \
  "targets('econ-graph-graphql-ratelimit') == ['econ-graph-graphql', 'econ-graph-graphql-public'] and targets('econ-graph-data-graphql-ratelimit') == ['econ-graph-data-graphql', 'econ-graph-data-graphql-public']"
check "the public host leaves out the admin and debug paths" \
  "not any(p == x or p.startswith(x + '/') for r in ('econ-graph-frontend-public', 'econ-graph-backend-public') for p in paths(r) for x in ('/admin', '/api/admin', '/playground')) and '/api' in paths('econ-graph-backend-public')"
check "the public Keycloak route serves only the econ-graph realm and its resources" \
  "paths('econ-graph-idp-public') == ['/idp/realms/econ-graph', '/idp/resources']"
check "plain http for the host and www redirects permanently to https://econgraph.com" \
  "hosts('econ-graph-public-redirect') == ['econgraph.com', 'www.econgraph.com'] and sections('econ-graph-public-redirect') == ['http'] and redirect('econ-graph-public-redirect') == {'scheme': 'https', 'hostname': 'econgraph.com', 'statusCode': 301}"
check "https for www redirects permanently to https://econgraph.com" \
  "hosts('econ-graph-www-redirect') == ['www.econgraph.com'] and sections('econ-graph-www-redirect') == ['https'] and redirect('econ-graph-www-redirect') == {'scheme': 'https', 'hostname': 'econgraph.com', 'statusCode': 301}"
check "every route rule sets X-Forwarded-For to the address the gateway settled on" \
  "all(any(f['type'] == 'RequestHeaderModifier' and {'name': 'X-Forwarded-For', 'value': '%DOWNSTREAM_REMOTE_ADDRESS_WITHOUT_PORT%'} in f['requestHeaderModifier'].get('set', []) for f in r.get('filters', [])) for (k, n), d in docs.items() if k == 'HTTPRoute' and not n.endswith('-redirect') for r in d['spec']['rules'])"
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
check "cloudflared is untouched (0 replicas, as committed)" \
  "docs[('Deployment', 'cloudflared')]['spec']['replicas'] == 0"

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
