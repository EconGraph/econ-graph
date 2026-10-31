#!/usr/bin/env bash
# Tests for scripts/deploy/render-public-host.py against the committed manifests:
# every localhost HTTPRoute gains a separate -public copy for the public host (listener
# https only, without the admin and debug paths; localhost stays as it was), the rate
# limits cover the copies, plain http and the alternate name redirect to https://MAIN_HOST
# (302 to www until MAIN_HOST_PERMANENT names it, then 301; always 301 to the apex), Keycloak's
# hostname and the backend's issuer agree on https://<host>/idp/realms/econ-graph, the
# JWKS stays in-cluster, the realm import allows the public origin only (the committed
# manifests keep localhost), CORS
# allows the public and www origins and http://localhost but not :3000 (the committed ConfigMap
# keeps both localhost origins), and bad host names are refused.
#
# Needs python3 with PyYAML. Usage: scripts/deploy/test-render-public-host.sh
set -euo pipefail
unset MAIN_HOST MAIN_HOST_PERMANENT

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
    "hosts('$r-public') == ['www.econgraph.com'] and sections('$r-public') == ['https']"
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
check "plain http for the host and www redirects temporarily to https://www.econgraph.com" \
  "hosts('econ-graph-public-redirect') == ['econgraph.com', 'www.econgraph.com'] and sections('econ-graph-public-redirect') == ['http'] and redirect('econ-graph-public-redirect') == {'scheme': 'https', 'hostname': 'www.econgraph.com', 'statusCode': 302}"
check "https for the apex redirects temporarily to https://www.econgraph.com" \
  "hosts('econ-graph-www-redirect') == ['econgraph.com'] and sections('econ-graph-www-redirect') == ['https'] and redirect('econ-graph-www-redirect') == {'scheme': 'https', 'hostname': 'www.econgraph.com', 'statusCode': 302}"
check "every route rule sets X-Forwarded-For to the address the gateway settled on" \
  "all(any(f['type'] == 'RequestHeaderModifier' and {'name': 'X-Forwarded-For', 'value': '%DOWNSTREAM_REMOTE_ADDRESS_WITHOUT_PORT%'} in f['requestHeaderModifier'].get('set', []) for f in r.get('filters', [])) for (k, n), d in docs.items() if k == 'HTTPRoute' and not n.endswith('-redirect') for r in d['spec']['rules'])"
check "KC_HOSTNAME is the https public URL" \
  "env('Deployment', 'keycloak', 'keycloak', 'KC_HOSTNAME') == 'https://www.econgraph.com/idp'"
check "the admin console stays on the port-forward" \
  "env('Deployment', 'keycloak', 'keycloak', 'KC_HOSTNAME_ADMIN') == 'http://localhost:18080/idp'"
check "Keycloak's proxy headers are still trusted" \
  "env('Deployment', 'keycloak', 'keycloak', 'KC_PROXY_HEADERS') == 'xforwarded'"
check "the backend's issuer is Keycloak's" \
  "env('Deployment', 'econ-graph-backend', 'backend', 'OIDC_ISSUER') == env('Deployment', 'keycloak', 'keycloak', 'KC_HOSTNAME') + '/realms/econ-graph'"
check "the JWKS is still fetched in-cluster" \
  "env('Deployment', 'econ-graph-backend', 'backend', 'OIDC_JWKS_URL').startswith('http://keycloak-service:8080/')"
check "Keycloak's realm placeholders use the public origin" \
  "env('Deployment', 'keycloak', 'keycloak', 'KC_WEB_BASE_URL') == 'https://www.econgraph.com' and env('Deployment', 'keycloak', 'keycloak', 'KC_WEB_E2E_BASE_URL') == 'https://www.econgraph.com'"
check "the realm import resets its frontend URL to the canonical IdP" \
  "env('Job', 'keycloak-realm-import', 'render-realm', 'KC_REALM_FRONTEND_URL') == 'https://www.econgraph.com/idp'"
check "the realm import allows the public origin only" \
  "env('Job', 'keycloak-realm-import', 'render-realm', 'KC_WEB_BASE_URL') == 'https://www.econgraph.com' and env('Job', 'keycloak-realm-import', 'render-realm', 'KC_WEB_E2E_BASE_URL') == 'https://www.econgraph.com'"
check "CORS allows both public origins and on-host localhost, without :3000" \
  "docs[('ConfigMap', 'econ-graph-config')]['data']['CORS_ORIGIN'] == 'https://econgraph.com,https://www.econgraph.com,http://localhost'"
check "cloudflared is untouched (0 replicas, as committed)" \
  "docs[('Deployment', 'cloudflared')]['spec']['replicas'] == 0"

# Without a public host (the committed ConfigMap: local clusters), CORS keeps localhost.
python3 -c '
import sys, yaml
docs = [d for d in yaml.safe_load_all(open(sys.argv[1])) if d and d["metadata"]["name"] == "econ-graph-config"]
sys.exit(0 if docs[0]["data"]["CORS_ORIGIN"] == "http://localhost,http://localhost:3000" else 1)
' "$manifests/configmap.yaml" || fail "the committed CORS_ORIGIN lost localhost"

# Rendering twice changes nothing more (deploy.sh may be re-run on rendered output).
twice="$("$render" econgraph.com - <<<"$rendered")"
[ "$twice" = "$rendered" ] || fail "rendering is not idempotent"

# web_client MANIFESTS: the econ-graph-web client of the realm rendered with the realm-import
# Job's KC_WEB_BASE_URL and KC_WEB_E2E_BASE_URL from MANIFESTS (stdin), as JSON with its
# redirectUris, webOrigins and post-logout redirect URIs.
web_client() {
  local values base e2e
  values="$(python3 -c '
import sys, yaml
for d in yaml.safe_load_all(sys.stdin):
    if d and d["kind"] == "Job" and d["metadata"]["name"] == "keycloak-realm-import":
        for c in d["spec"]["template"]["spec"]["initContainers"] + d["spec"]["template"]["spec"]["containers"]:
            env = {e["name"]: e.get("value") for e in c.get("env", [])}
            if "KC_WEB_BASE_URL" in env:
                print(env["KC_WEB_BASE_URL"], env["KC_WEB_E2E_BASE_URL"])
                break
')"
  read -r base e2e <<<"$values"
  env -i PATH="$PATH" KC_WEB_BASE_URL="$base" KC_WEB_E2E_BASE_URL="$e2e" \
    "${repo_root}/scripts/keycloak/render-realm.sh" "${repo_root}/config/keycloak/econ-graph-realm.json" |
    python3 -c '
import json, sys
client = next(c for c in json.load(sys.stdin)["clients"] if c["clientId"] == "econ-graph-web")
print(json.dumps({
    "redirectUris": client["redirectUris"],
    "webOrigins": client["webOrigins"],
    "postLogout": client["attributes"]["post.logout.redirect.uris"].split("##"),
}))'
}

# With a public host, the web client accepts the public host only: no localhost redirect URI,
# web origin or post-logout URI.
public_client="$(web_client <<<"$rendered")"
for uri in https://www.econgraph.com/auth/callback https://www.econgraph.com/silent-callback.html; do
  grep -qF -- "\"$uri\"" <<<"$public_client" || fail "public realm redirect URIs lack $uri"
done
grep -qF -- '"https://www.econgraph.com"' <<<"$public_client" || fail "public realm web origins lack the host"
if grep -q localhost <<<"$public_client"; then
  fail "public realm still allows localhost: $public_client"
fi

# Every HTTPS redirect has a distinct source and target; neither public app
# routes nor IdP routes may serve the alternate host.
check "canonical HTTPS cannot redirect to itself" \
  "redirect('econ-graph-www-redirect')['hostname'] not in hosts('econ-graph-www-redirect')"

# MAIN_HOST_PERMANENT naming the main host makes both redirects permanent; naming
# another host leaves them temporary.
temporary="$rendered"
rendered="$(MAIN_HOST_PERMANENT=WWW.econgraph.com "$render" econgraph.com "$manifests/gateway/routes/httproutes.yaml")"
check "MAIN_HOST_PERMANENT=www makes both redirects to www permanent" \
  "redirect('econ-graph-public-redirect')['statusCode'] == 301 and redirect('econ-graph-www-redirect')['statusCode'] == 301"
[ "$("$render" econgraph.com - <<<"$rendered" | grep -c 'statusCode: 302')" = 2 ] ||
  fail "rerendering finalized routes without MAIN_HOST_PERMANENT does not return to 302"
rendered="$(MAIN_HOST_PERMANENT=econgraph.com "$render" econgraph.com "$manifests/gateway/routes/httproutes.yaml")"
check "MAIN_HOST_PERMANENT naming another host keeps the redirects to www temporary" \
  "redirect('econ-graph-public-redirect')['statusCode'] == 302 and redirect('econ-graph-www-redirect')['statusCode'] == 302"
rendered="$temporary"

# Explicit rollback restores the apex consistently, preserving dual-origin CORS.
forward="$rendered"
rendered="$(MAIN_HOST=econgraph.com "$render" econgraph.com \
  "$manifests/gateway/routes/httproutes.yaml" "$manifests/keycloak/deployment.yaml" \
  "$manifests/keycloak/realm-import-job.yaml" "$manifests/backend-deployment.yaml" "$manifests/configmap.yaml")"
check "rollback serves the apex and redirects www permanently" \
  "hosts('econ-graph-frontend-public') == ['econgraph.com'] and hosts('econ-graph-www-redirect') == ['www.econgraph.com'] and redirect('econ-graph-www-redirect') == {'scheme': 'https', 'hostname': 'econgraph.com', 'statusCode': 301} and redirect('econ-graph-public-redirect') == {'scheme': 'https', 'hostname': 'econgraph.com', 'statusCode': 301}"
check "rollback restores the issuer and sign-in client together" \
  "env('Deployment', 'keycloak', 'keycloak', 'KC_HOSTNAME') == 'https://econgraph.com/idp' and env('Deployment', 'econ-graph-backend', 'backend', 'OIDC_ISSUER') == 'https://econgraph.com/idp/realms/econ-graph' and env('Job', 'keycloak-realm-import', 'render-realm', 'KC_WEB_BASE_URL') == 'https://econgraph.com'"
check "rollback retains both CORS origins" \
  "docs[('ConfigMap', 'econ-graph-config')]['data']['CORS_ORIGIN'] == 'https://econgraph.com,https://www.econgraph.com,http://localhost'"
[ "$(MAIN_HOST=econgraph.com "$render" econgraph.com - <<<"$rendered")" = "$rendered" ] || fail "rollback is not idempotent"
# The same generated manifests can also be switched back and forward safely.
rendered="$(MAIN_HOST=econgraph.com "$render" econgraph.com - <<<"$forward")"
check "rerendering to the apex keeps routes and issuer together" \
  "hosts('econ-graph-idp-public') == ['econgraph.com'] and hosts('econ-graph-www-redirect') == ['www.econgraph.com'] and env('Deployment', 'keycloak', 'keycloak', 'KC_HOSTNAME') == 'https://econgraph.com/idp'"
[ "$("$render" econgraph.com - <<<"$rendered")" = "$forward" ] || fail "forward/rollback rendered manifests did not round-trip"
rendered="$forward"

# Another base name works without hardcoded production host names.
rendered="$("$render" example.org "$manifests/gateway/routes/httproutes.yaml")"
check "alternate public domains use their www name" \
  "hosts('econ-graph-frontend-public') == ['www.example.org'] and hosts('econ-graph-www-redirect') == ['example.org'] and redirect('econ-graph-public-redirect')['hostname'] == 'www.example.org'"
for bad_main in '' other.example.org https://www.econgraph.com www.www.econgraph.com; do
  if MAIN_HOST="$bad_main" "$render" econgraph.com /dev/null >/dev/null 2>&1; then
    fail "MAIN_HOST '$bad_main' was accepted"
  fi
done

# Without one (the committed manifests: local clusters), it keeps localhost.
local_client="$(web_client <"$manifests/keycloak/realm-import-job.yaml")"
for uri in http://localhost/auth/callback http://localhost/silent-callback.html; do
  grep -qF -- "\"$uri\"" <<<"$local_client" || fail "local realm redirect URIs lack $uri"
done
grep -qF -- '"http://localhost"' <<<"$local_client" || fail "local realm web origins lack localhost"

for bad in localhost "" "econgraph" "https://www.econgraph.com" "econgraph.com/" "www.econgraph.com" \
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
