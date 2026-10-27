# Ingress Routing Troubleshooting Guide

Use this runbook for the local kind deployment created by the repository's deployment scripts. Start with the request's host and path, then compare ingress routing with direct Service access. A successful direct request narrows the fault to layers that request bypassed; it does not establish that ingress works.

## Deployment and routing contract

```text
Client → ingress-nginx controller → Kubernetes Service → application Pod
             ↑
       Ingress resources
       define host/path/backend rules
```

The Ingress resources describe routing. The controller translates those resources into nginx configuration and forwards requests to the selected application's endpoints. Services select application Pods and expose their ports; ready endpoints are needed for traffic to reach those Pods.

The standard [deployment script](../../scripts/deploy/deploy.sh) applies the [main Ingress](../../k8s/manifests/ingress.yaml) with its admin frontend path removed, and also applies the separate [GraphQL Ingress](../../k8s/manifests/graphql-ingress.yaml). Both use `host: localhost` and ingress class `nginx`.

| Request path | Standard local destination |
| --- | --- |
| `/` and paths without a more specific match | `econ-graph-frontend-service:3000` (Prefix `/`) |
| `/auth/callback` | Frontend (Exact match) |
| `/api`, `/api/admin`, `/auth`, `/playground`, `/health` | `econ-graph-backend-service:9876` (Prefix matches) |
| `/graphql` | Backend, through `econ-graph-graphql-ingress` |

An ingress match only selects a backend; it does not guarantee that the application implements the requested endpoint. For example, the backend has no `/api` root handler or `/auth/*` handlers. `/playground` is conditional on the backend's playground setting.

The standard deployment removes the admin frontend Deployment and Service and omits their `/admin` ingress path. A request to `/admin` therefore falls through to the frontend catch-all; it is not an admin UI health check. The [frontend-only Ingress](../../k8s/manifests/frontend-ingress.yaml) uses `frontend.econ-graph.local` when separately applied. Use that host only when testing that resource. Public host rendering has additional restrictions; see [Kubernetes public site configuration](../../k8s/README.md).

### How a path wins

For Kubernetes Exact and Prefix rules, the longest matching path wins; Exact wins over Prefix when their matching paths have equal length. Prefix matches use path elements, so `/api` matches `/api/admin` but not `/apix`. This is why `/auth/callback` reaches the frontend while `/auth` reaches the backend. Moving `/` earlier in YAML is not a routing repair. See [Kubernetes path matching](https://kubernetes.io/docs/concepts/services-networking/ingress/#multiple-matches).

In generated nginx configuration, an exact location terminates the search; ordinary prefixes are selected by longest match. Regex locations have separate precedence rules, so inspect the full server when another Ingress introduces regex or rewrite annotations. See [nginx location selection](https://nginx.org/en/docs/http/ngx_http_core_module.html#location). A server-level `set` followed by a location-level `set` is not evidence that the former overrides the latter: [rewrite-module processing](https://nginx.org/en/docs/http/ngx_http_rewrite_module.html) runs server directives before directives inside the selected location.

## Diagnose from the outside inward

Run these commands from a workstation with `kubectl` access to the intended cluster. The examples select `kind-econ-graph` explicitly. For a different deployment, substitute its context and rule host throughout.

### 1. Check the controller and both Ingress resources

```bash
kubectl --context kind-econ-graph -n ingress-nginx get pods,services
kubectl --context kind-econ-graph -n econ-graph get ingress
kubectl --context kind-econ-graph -n econ-graph describe ingress econ-graph-ingress
kubectl --context kind-econ-graph -n econ-graph describe ingress econ-graph-graphql-ingress
```

Check controller readiness, ingress class, host, paths, backend ports and reported events. A missing GraphQL Ingress leaves `/graphql` matching the frontend's `/` catch-all. A request with an unmatched Host can reach the controller's default server instead of either application route.

To isolate host networking from controller routing, start this forward in a dedicated terminal:

```bash
kubectl --context kind-econ-graph -n ingress-nginx port-forward service/ingress-nginx-controller 8080:80
```

Wait for `Forwarding from 127.0.0.1:8080` before making requests in another terminal. If the forward exits or cannot bind the port, resolve that failure first. Keep it running during the checks below, then stop it with Ctrl-C. Stop each forward before reusing its port; do not leave background forwarding processes behind.

### 2. Check status, content type and body together

The explicit Host below selects the local rule even though the TCP connection uses a diagnostic port. Do not use `curl -f` for these checks: error responses contain useful diagnostic evidence.

```bash
curl --max-time 10 -sS -i -H 'Host: localhost' http://127.0.0.1:8080/
curl --max-time 10 -sS -i -H 'Host: localhost' http://127.0.0.1:8080/health
curl --max-time 10 -sS -i -H 'Host: localhost' \
  -H 'Content-Type: application/json' \
  --data '{"query":"{ __typename }"}' http://127.0.0.1:8080/graphql
```

With the corresponding application ready, these checks should produce:

| Check | Expected response | What it establishes |
| --- | --- | --- |
| `GET /` | HTTP 200, `text/html`, title `EconGraph - Economic Data Visualization` | The frontend HTML is served; this does not check JavaScript loading or browser API calls. |
| `GET /health` | HTTP 200, `application/json`, fields `service: "econ-graph-backend"` and `status: "healthy"` | The backend health handler answered. It is not a database readiness check. |
| GraphQL POST above, without credentials | HTTP 200, JSON with `data.__typename` equal to `Query` | A small anonymous query reached GraphQL; it does not verify data queries or authenticated operations. |

Whitespace and JSON field order are immaterial. HTML from `/graphql`, or plain `healthy` text from `/health`, is not the backend response described here. The [frontend nginx configuration](../../frontend/nginx.conf) has its own plain-text health handler, distinct from the backend JSON handler.

A GraphQL JSON `errors` response shows application processing but still needs investigation; “anything except 404” is not a passing check. Malformed GraphQL bodies can return JSON 400, and oversized bodies JSON 413. A supplied invalid bearer token can cause 401, so keep this anonymous diagnostic free of credentials. Ingress rate or connection limits can return 503 under load; compare the controller logs and direct backend response before attributing every 503 to missing endpoints.

These are targeted checks for the standard deployment. The [routing smoke script](../../scripts/test-ingress-routing.sh) also expects an admin UI, a 500 from `/api`, an introspection response and a Grafana port-forward. Those assumptions do not define this deployment's routing contract, and its aggregate result cannot replace these checks.

### 3. Check Services, endpoints and Pods

```bash
kubectl --context kind-econ-graph -n econ-graph get services
kubectl --context kind-econ-graph -n econ-graph get endpointslices -l kubernetes.io/service-name=econ-graph-frontend-service
kubectl --context kind-econ-graph -n econ-graph get endpointslices -l kubernetes.io/service-name=econ-graph-backend-service
kubectl --context kind-econ-graph -n econ-graph get pods -o wide
```

Compare Service selectors and ports with the selected Pods and EndpointSlice readiness. Missing ready addresses call for Pod readiness and selector investigation before changing ingress. A 502 or 503 alone cannot identify the failing layer; use controller events and logs to distinguish connection failures, unavailable upstreams and configured limits.

Then compare each application's response while bypassing ingress. In a dedicated terminal, run one forward at a time, waiting for its forwarding-ready message:

```bash
kubectl --context kind-econ-graph -n econ-graph port-forward service/econ-graph-frontend-service 8081:3000
```

In another terminal, request `http://127.0.0.1:8081/` using the same curl options as above. Stop that forward, then start the backend forward:

```bash
kubectl --context kind-econ-graph -n econ-graph port-forward service/econ-graph-backend-service 8082:9876
```

Repeat the health and GraphQL checks at `http://127.0.0.1:8082/health` and `http://127.0.0.1:8082/graphql`, then stop the forward. If direct responses are wrong too, investigate the selected Pods, application configuration and deployed image. If direct responses are correct but ingress responses differ, inspect host/path selection and controller configuration next. A Service port-forward selects a Pod through the Service; it bypasses ingress and does not prove that the cluster's Service network or DNS works.

### 4. Compare controller configuration with the rules

```bash
kubectl --context kind-econ-graph -n ingress-nginx logs deployment/ingress-nginx-controller --tail=100
kubectl --context kind-econ-graph -n ingress-nginx exec deployment/ingress-nginx-controller -- cat /etc/nginx/nginx.conf
```

Find the `localhost` server, its relevant locations and their selected upstreams. Compare those upstreams with the backend Service names and ports in both Ingress resources. Read complete locations rather than a few matching lines: redirects, regex locations and rewrites can change the request's destination. If multiple controller replicas exist, inspect the Pod that handled the request as well.

If a resource change is absent from generated configuration, check events and controller logs for rejection or reload errors and confirm that the resource's ingress class belongs to this controller. A controller restart does not correct an invalid rule, wrong host or missing Service. Likewise, changing the frontend image tag does not repair host/path selection; use the supported deployment process if the deployed application itself is wrong.

### 5. Separate DNS and in-cluster connectivity

If port-forwarding works but in-cluster clients fail, check Service DNS from an approved diagnostic Pod equipped with DNS tools, in the affected namespace. Resolve `econ-graph-frontend-service.econ-graph.svc.cluster.local` and `econ-graph-backend-service.econ-graph.svc.cluster.local`, then compare HTTP connectivity to the resolved Services with the direct responses above. Use the cluster's actual DNS suffix if it differs.

The [backend runtime image](../../backend/Dockerfile) installs curl but does not install `nslookup`. Do not treat a missing diagnostic executable as a DNS failure. DNS resolution alone also does not establish that the application port is reachable.

## When localhost or NodePort is unreachable

kind nodes are containers. A NodePort on a node is not automatically published on the host's localhost. The [frontend Service](../../k8s/manifests/frontend-service.yaml) uses NodePort 30000 and the [backend Service](../../k8s/manifests/backend-service.yaml) uses 30080, but the maintained [kind configuration](../../terraform/k8s/kind-config.yaml) publishes host ports 80, 443, 3000 and 8080, not those NodePorts. A refused `localhost:30000` connection therefore does not prove ingress failure. Grafana is ClusterIP-only and is not a NodePort or application-ingress check.

If the controller port-forward works while `http://localhost/` fails, inspect the host's published port 80 and the ingress controller's binding. Port-forwarding to 8080 above creates a temporary listener independently of kind's configured port mappings.

If cluster recreation is necessary, follow the supported [cluster removal and replacement guidance](../../k8s/README.md#remove-the-cluster), [recreation script](../../scripts/deploy/recreate-kind-cluster.sh) and [production recovery procedure](HOST_STORAGE.md#recovering-production-eco-432). The maintained topology includes a worker with host database mounts. Recreation checks those mounts, backs up reachable databases before deletion and requires confirmation; an unreachable cluster cannot supply that backup. Node-only contents, including in-cluster backups and Grafana data, are lost when nodes are deleted. Recreating nodes does not restore application resources or data: follow the recovery procedure's image loading, restore and deployment steps. Do not replace this topology with an inline single-node configuration to troubleshoot a refused port.

## When `/mcp` returns 405

The standard main and GraphQL Ingresses have no backend `/mcp` path. That request matches the frontend catch-all, where a POST can receive an nginx 405. This does not establish that the ingress controller blocks POST.

Check the selected host, path and upstream first. Then check the intended backend build profile: the [release flags](../../backend/flags/release.json) disable MCP, and the [backend Dockerfile](../../backend/Dockerfile) defaults to that profile. With MCP disabled, the backend has no MCP handler. A working release deployment is not expected to serve MCP JSON-RPC.

An explicitly enabled development setup needs both a backend built with the `mcp` flag and an explicit Ingress path pointing `/mcp` to `econ-graph-backend-service:9876`. The enabled [MCP handler](../../backend/crates/econ-graph-backend/src/mcp_routes.rs) accepts POST with a JSON-RPC body and requires an active signed-in user's bearer token granting `api:mcp`. Missing or invalid credentials return 401 with a bearer challenge; an inactive account or missing role returns 403. Those responses differ from a frontend 405 and still do not establish a successful MCP operation. A `proxy_method POST` override or nested location annotation cannot supply a missing route, compiled handler or authorization.

## Browser CORS failures

The backend owns CORS through `CORS_ALLOWED_ORIGINS`, supplied from `CORS_ORIGIN` in the [ConfigMap](../../k8s/manifests/configmap.yaml) by the [backend Deployment](../../k8s/manifests/backend-deployment.yaml). Configure the frontend's actual origin there. Do not also add ingress `cors-*` or `enable-cors` annotations: duplicate `Access-Control-Allow-Origin` response headers cause browser rejection.

The curl checks above do not exercise browser CORS enforcement. If they succeed while a browser request fails, inspect the browser's Origin, preflight request and response headers separately from host/path routing.
