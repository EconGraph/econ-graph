# Gateway routing troubleshooting

Traffic reaches the kind cluster through a Gateway API gateway run by Envoy Gateway. The objects,
versions and routes are listed in the "Gateway" section of [k8s/README.md](../../k8s/README.md).
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
./scripts/deploy/test-gateway-routes.sh --rate-limit    # also the 429 behaviour
ROUTE_HOST=econgraph.com ./scripts/deploy/test-gateway-routes.sh https://localhost   # the public host
./scripts/deploy/check-public-site.sh --host econgraph.com                         # and its redirects, spoofing
```

Each line names the route and what was expected. To test the proxy without the host ports (for
example while another controller still holds them), port-forward its Service:

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
kubectl -n econ-graph-gateway get gateway econ-graph-gateway \
  -o jsonpath='{range .status.listeners[*]}{.name}: {.attachedRoutes} routes{"\n"}{end}'

# Every route Accepted and ResolvedRefs (a missing Service shows here).
kubectl -n econ-graph get httproute
kubectl -n econ-graph describe httproute econ-graph-frontend

# Rate-limit policies and the burst patch.
kubectl -n econ-graph get backendtrafficpolicy
kubectl -n econ-graph-gateway get envoypatchpolicy
```

| Symptom | Likely cause |
| --- | --- |
| Connection refused or reset on `localhost:80` | The proxy pod is not running on the control-plane node, or another pod holds host port 80 (`kubectl get pods -A -o wide` on that node). The proxy pod stays `Pending` while the port is taken. |
| 404 with an empty body for every path | No route matches the `Host` header. The routes match `localhost` (any port); other host names need to be added to the routes' `hostnames`. |
| A route is `Accepted: False` | Its namespace lacks the label `econgraph.io/gateway-access: "true"` (`k8s/manifests/namespace.yaml`), or the route attaches to listener `https` from a namespace without `econgraph.io/env: production`. |
| `ResolvedRefs: False` | The backend Service or port in the route does not exist. |
| 503 `no healthy upstream` | The Service has no ready pods, or a NetworkPolicy blocks the proxy (it must allow pods `app.kubernetes.io/name: envoy` in `envoy-gateway-system`; see `k8s/manifests/network-policy.yaml`). |
| 429 on `/graphql` or `/data/graphql` | The per-client limit: 20 a second, bursts of 100. Requests from the host itself share one client address. |
| The EnvoyPatchPolicy is `Programmed: False` | No `econ-graph-graphql` or `econ-graph-data-graphql` route exists yet, so the bursts have nothing to patch; the limit is then 20 a second with bursts of 20. |
| `/idp/...` sign-in pages fail | Only `/idp/realms/econ-graph` and `/idp/resources` are routed. The issuer is `KC_HOSTNAME` in `k8s/manifests/keycloak/deployment.yaml`. |

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
