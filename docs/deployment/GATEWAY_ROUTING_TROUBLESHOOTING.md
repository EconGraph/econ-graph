# Gateway routing troubleshooting

Traffic reaches the kind cluster through a Gateway API gateway run by Envoy Gateway. The objects,
versions and routes are listed in the [Gateway section of k8s/README.md](../../k8s/README.md#-gateway).
This page is about finding out why a request does not arrive where it should.

```text
client ──> host port 80/443 ──> Envoy proxy pod (envoy-gateway-system, control-plane node)
cloudflared ──> econ-graph-gateway-proxy:443 ─┘  Gateway econ-graph-gateway, listener http / https
                                  │  HTTPRoutes in econ-graph: host localhost on http; for a
                                  │  public host, <name>-public copies on https and redirects
                                  ├── /, /auth/callback, /silent-callback.html  ──> econ-graph-frontend-service:3000
                                  ├── /api, /auth, /health                      ──> econ-graph-backend-service:9876
                                  ├── /graphql                (rate-limited)    ──> econ-graph-backend-service:9876
                                  ├── /data/graphql -> /graphql (no credentials, rate-limited) ──> econ-graph-backend-service:9876
                                  └── /idp/realms/econ-graph, /idp/resources    ──> keycloak-service:8080
```

## Check everything at once

```bash
./scripts/deploy/test-gateway-routes.sh                 # http://localhost
./scripts/deploy/test-gateway-routes.sh --rate-limit    # bursts test 429s and consume this client's tokens
ROUTE_HOST=www.econgraph.com EXPECTED_ISSUER=https://www.econgraph.com/idp/realms/econ-graph \
  ./scripts/deploy/test-gateway-routes.sh https://localhost   # the public host
./scripts/deploy/check-public-site.sh --host econgraph.com                         # and its redirects, spoofing
```

Each result names the route and what was expected. The URL selects the connection address;
`ROUTE_HOST` independently sets the Host header. Set `EXPECTED_ISSUER` to Keycloak's configured
issuer, regardless of either address: a public-host deployment needs the public issuer above
even when testing localhost or the port-forward below. The example uses the default
`MAIN_HOST=www.${PUBLIC_HOST}`; use the actual served main host when it differs. See
[Main public address](MAIN_HOST.md). Without Keycloak, run
`test-gateway-routes.sh` with `SKIP_KEYCLOAK=true`.

To test the proxy without the host ports (for example while another controller still holds
them), port-forward its Service:

```bash
kubectl -n envoy-gateway-system port-forward svc/econ-graph-gateway-proxy 18080:80
./scripts/deploy/test-gateway-routes.sh http://127.0.0.1:18080
```

## Status, from the top down

```bash
# Controller and proxy pods. The proxy must run on the control-plane node.
kubectl -n envoy-gateway-system get pods -o wide

# GatewayClass accepted, Gateway programmed, listeners and their attached routes.
kubectl get gatewayclass econ-graph
kubectl describe gatewayclass econ-graph
kubectl -n econ-graph-gateway describe gateway econ-graph-gateway
kubectl -n econ-graph-gateway get gateway econ-graph-gateway \
  -o jsonpath='{range .status.listeners[*]}{.name}: {.attachedRoutes} routes{"\n"}{end}'

# Every route Accepted and ResolvedRefs (a missing Service shows here).
kubectl -n econ-graph get httproute
kubectl -n econ-graph describe httproute

# Rate-limit policies and the burst patch.
kubectl -n econ-graph describe backendtrafficpolicy
kubectl -n econ-graph-gateway describe envoypatchpolicy
```

Read the Gateway and listener conditions, then each relevant HTTPRoute parent's conditions,
including their reasons and messages. The Gateway should be `Programmed: True`; routes should
be `Accepted: True` and `ResolvedRefs: True` for the intended listener. Attached-route counts
alone do not establish readiness.

| Symptom | Likely cause |
| --- | --- |
| Connection refused or reset on `localhost:80` | The proxy is not running on the control-plane node, or it was installed without host ports. A running proxy reachable through its Service can still lack a host listener; check `GATEWAY_HOST_PORTS` and the installer warning described below. |
| Proxy pod `Pending` with host-port conflict events | The pod requests host ports already held on the target node. Inspect `kubectl describe pod` events and other pods on that node. A conflict that appears after the installer's check, or applying the host-port manifest directly, can cause this; the installer's default `auto` mode avoids detected conflicts. |
| 404 with an empty body for every path | No route matches the request Host on that listener. Compare the Host and listener with the route's `hostnames` and `parentRefs`: `localhost` (any port) uses `http`; the configured public host uses `<name>-public` routes on `https`. For missing public configuration, follow the `PUBLIC_HOST` workflow in [Public site](../../k8s/README.md#-public-site-cloudflare-tunnel) and apply routes as described in [Gateway](../../k8s/README.md#-gateway). |
| A route is `Accepted: False` | Its namespace lacks the label `econgraph.io/gateway-access: "true"` (`k8s/manifests/namespace.yaml`), or the route attaches to listener `https` from a namespace without `econgraph.io/env: production`. |
| `ResolvedRefs: False` | The backend Service or port in the route does not exist. |
| 503 `no healthy upstream` | The Service has no ready pods, or a NetworkPolicy blocks the proxy (it must allow pods `app.kubernetes.io/name: envoy` in `envoy-gateway-system`; see `k8s/manifests/network-policy.yaml`). |
| 429 on `/graphql` or `/data/graphql` | The per-client limit: 20 a second, bursts of 100. Requests from the host itself share one client address. |
| The EnvoyPatchPolicy is `Programmed: False` | Inspect the reason/message and the base `BackendTrafficPolicy`: a missing GraphQL route or rate-limit policy leaves nothing to patch. Only an active base limit without its burst patch falls back to 20 a second with bursts of 20; a missing base policy does not provide that limit. |
| `/idp/...` sign-in pages fail | Only `/idp/realms/econ-graph` and `/idp/resources` are routed. The issuer is the configured `KC_HOSTNAME` plus `/realms/econ-graph`; `MAIN_HOST` selects the public address (default `www.${PUBLIC_HOST}`) shown above. |

The [gateway installer](../../scripts/deploy/install-gateway.sh) controls host listeners with
`GATEWAY_HOST_PORTS`:

- `auto` (default) checks for other pods holding host port 80 or 443 on nodes labelled
  `ingress-ready=true`. If it finds one, it warns and installs the proxy without either
  host port. The proxy can run and serve through its ClusterIP Service in this mode.
- `true` requires host ports. A detected holder stops a normal installation before any
  manifests are applied; `--dry-run` reports the conflict and continues its preview.
- `false` always installs the proxy without host ports.

For kind's ingress-nginx, follow the [Gateway cutover procedure](../../k8s/README.md#-gateway).
For MicroK8s's ingress add-on, the installer advises disabling that add-on and rerunning
the installer. Freeing a port alone does not update an already installed proxy that lacks
host ports; rerun the installer with `auto` or `true` after resolving the conflict.

## Looking inside the proxy

The proxy's access log has one JSON line per request, with the route that matched
(`route_name`), the response flags (`RL` = rate-limited, `NR` = no route) and the client address
as the proxy saw it (`downstream_remote_address`):

```bash
kubectl -n envoy-gateway-system logs deploy/$(kubectl -n envoy-gateway-system get deploy \
  -l gateway.envoyproxy.io/owning-gateway-name=econ-graph-gateway -o name | cut -d/ -f2) -c envoy --tail=20
```

Its generated configuration, from the admin API:

```bash
kubectl -n envoy-gateway-system port-forward deploy/<proxy deployment> 19000:19000
curl -s 'localhost:19000/config_dump?resource=dynamic_route_configs'
```

The controller's log explains why a route or policy was not accepted:

```bash
kubectl -n envoy-gateway-system logs deploy/envoy-gateway --tail=50
```
