# EconGraph Administrative Interface Security

The admin frontend provides crawler monitoring and management through the same backend used by the public application. Network access to the frontend and authorization of backend operations are separate controls: reaching the UI does not grant a backend role.

## Deployment paths and reachability

The normal [deployment script](../../scripts/deploy/deploy.sh) does not deploy the admin frontend. It deletes existing standalone admin Deployment and Service resources. The [Gateway routes](../../k8s/manifests/gateway/routes/httproutes.yaml) send `/admin` through the public frontend's `/` fallback, not to the admin frontend. See the [Kubernetes deployment guide](../../k8s/README.md) for the supported deployment workflow.

The repository also contains standalone manifests and a Terraform frontend module. Their resource names, ports, and network configuration differ:

| Resource | Standalone Kubernetes manifests | Terraform frontend module |
| --- | --- | --- |
| Namespace | `econ-graph` | `econ-graph-admin` |
| Deployment | `econ-graph-admin-frontend` | `admin-frontend` |
| Service | `econ-graph-admin-frontend-service`, NodePort `30002`, Service and target port `3001` | `admin-frontend-service`, ClusterIP, Service and target port `3000` |
| Frontend exposure | NodePort; no admin frontend Gateway route | ClusterIP; no supplied admin frontend Gateway route or ingress |

The standalone definitions are the [Deployment](../../k8s/manifests/admin-frontend-deployment.yaml), [Service](../../k8s/manifests/admin-frontend-service.yaml), and [NGINX ConfigMap](../../k8s/manifests/admin-frontend-nginx-configmap.yaml). The [Terraform frontend module](../../terraform/modules/admin-frontend/main.tf) declares the namespace, frontend workload, Service, configuration, authentication Secret, service account and RBAC, and network policy. Neither path supplies a dedicated admin ingress controller or IP-filtered admin endpoint. The root Terraform `allowed_admin_ips` input does not configure an access filter in the surviving frontend module.

The shared Gateway routes `/graphql` to backend port `9876`, retaining bearer credentials. It rewrites `/data/graphql` to the backend's `/graphql` after removing `Authorization` and `Cookie`, so that data route cannot carry staff credentials. The `/graphql` route exposes administrative GraphQL operations subject to backend authorization; omitting the admin frontend does not remove this backend route. A route declaration does not create a backend endpoint: the current backend does not implement the admin REST authentication routes described by the unused frontend contexts.

**Terraform image compatibility:** the module expects its `${image_repository}/econ-graph-admin:${image_tag}` image to serve HTTP on port `3000`, with successful responses at `/health` for liveness and `/ready` for readiness. The checked-in [admin Dockerfile](../../admin-frontend/Dockerfile) and [NGINX configuration](../../admin-frontend/nginx.conf) instead use `3001`. An image built from that source does not match the module's Service targets or probes without configuration changes. A custom image may satisfy the module's `3000` contract; verify that contract before using the module or its port-forward example below.

### Network policy and Gateway controls

Terraform's `admin-network-isolation` policy selects all pods in `econ-graph-admin`. It declares TCP `3000` ingress from namespaces labeled `name=monitoring` or addresses in `10.0.0.0/8`, UDP `53` egress for DNS, and TCP `8081` egress to namespaces labeled `name=econ-graph`. These are the supplied policy rules, not proof of isolation in a running cluster; their effect depends on network-policy support, namespace labels, and the actual traffic path. The policy does not configure browser access to the backend or supply a backend Service on port `8081`.

The shared [GatewayClass](../../k8s/manifests/gateway/gatewayclass.yaml) is `econ-graph`, served by Envoy Gateway. The [HTTP listener](../../k8s/manifests/gateway/gateway.yaml) admits routes from namespaces labeled `econgraph.io/gateway-access: "true"`; the [public HTTPS listener](../../k8s/manifests/gateway/gateway-https.yaml) admits routes from namespaces labeled `econgraph.io/env: production`. These control which namespaces can attach routes, not which users may perform admin operations. See the [Gateway guidance](../../k8s/README.md#-gateway) for deployment and status checks.

The [GraphQL rate-limit policies](../../k8s/manifests/gateway/routes/ratelimit-policies.yaml) declare 20 requests per second per client address on each GraphQL route, with separate buckets for `/graphql` and `/data/graphql`. The [HTTP burst patch](../../k8s/manifests/gateway/ratelimit-burst.yaml) and HTTPS listener configuration raise the burst capacity to 100 when applied. Limits are local to each proxy replica; the checked-in proxy configuration runs one replica. They limit requests through those routes, not login attempts or frontend connections.

The [client traffic policy](../../k8s/manifests/gateway/client-traffic-policy.yaml) derives the client address from the TCP peer, trusting forwarded addresses from pod CIDRs for tunnel traffic. The routes replace `X-Forwarded-For` with that derived address before forwarding to services. Callers sharing a NAT can share a rate bucket, and an in-cluster pod can claim a forwarded address; do not treat these client addresses as authoritative audit identity.

## Authentication and backend roles

The active [entry point](../../admin-frontend/src/main.tsx) renders [App](../../admin-frontend/src/App.tsx) directly, without the `AuthProvider` or `SecurityProvider`. The crawler tabs therefore have no sign-in gate from those providers. The [Apollo client](../../admin-frontend/src/services/graphqlClient.ts) uses `VITE_GRAPHQL_ENDPOINT`, defaulting to same-origin `/graphql`, and sends `localStorage.auth_token` as a bearer token when present. This entry point does not provide a Keycloak login flow to obtain that token.

The backend's [GraphQL caller handling](../../backend/crates/econ-graph-backend/src/main.rs) authenticates bearer tokens through the [OIDC verifier](../../backend/crates/econ-graph-auth/src/oidc.rs). `OIDC_ISSUER` configures the issuer, `OIDC_AUDIENCE` defaults to `econ-graph-api`, and `OIDC_JWKS_URL` can override key discovery. `OIDC_REQUIRED=true` makes startup fail when the issuer is absent; without an issuer, callers are anonymous. Protected admin operations require fine-grained roles such as `admin.crawlers:manage` and `admin.system:read` from the [backend role catalog](../../backend/crates/econ-graph-auth/src/roles.rs).

Staff role assignments belong in Keycloak. The [Keycloak roles reference](../../config/keycloak/README.md#roles) explains how realm composites grant the API client roles carried in the token. The `read_only`, `admin`, and `super_admin` hierarchy in the unused frontend `SecurityContext` is not the backend authorization model.

The unused [AuthContext](../../admin-frontend/src/contexts/AuthContext.tsx) expects `/api/admin/auth/*` REST endpoints. The unused [SecurityContext](../../admin-frontend/src/contexts/SecurityContext.tsx) keeps browser events in React state and attempts to post high/critical events to `/api/admin/security/events`. Those endpoints are not in the current backend route composition, so these contexts do not establish a working authentication or durable event path.

Terraform declares `REACT_APP_MFA_REQUIRED=true`, `REACT_APP_SESSION_TIMEOUT=1800`, and `REACT_APP_AUDIT_LOGGING=true` in `admin-frontend-config`. These labels do not enforce MFA, a 30-minute session timeout, or audit capture in the active UI. Likewise, `admin_jwt_secret`, `admin_session_key`, and `admin_encryption_key` populate `admin-auth-secret`, which the module does not inject into the frontend container. They do not configure backend OIDC verification. Configure identity-provider authentication and session policy explicitly; do not rely on these frontend declarations as defaults or treat their rotation as OIDC session revocation.

## Response headers and access logs

The [image NGINX configuration](../../admin-frontend/nginx.conf) and [standalone NGINX ConfigMap](../../k8s/manifests/admin-frontend-nginx-configmap.yaml) declare server-level `X-Frame-Options: SAMEORIGIN`, `X-Content-Type-Options: nosniff`, `X-XSS-Protection: 1; mode=block`, `Referrer-Policy: no-referrer-when-downgrade`, and CSP `default-src 'self' http: https: data: blob: 'unsafe-inline'`. The standalone Deployment mounts its ConfigMap over the image configuration. Static-asset and health locations declare their own `add_header` directives, so the server-level list does not establish the headers on every response. Assess the configuration and response location used by the installed frontend.

The Gateway's backend and GraphQL routes separately declare response filters setting `X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff`, and HSTS `max-age=31536000; includeSubDomains; preload`. Browsers apply HSTS only over HTTPS. The public frontend route has its own CSP and other headers; those filters do not apply to a separately installed admin frontend without a matching route.

The frontend NGINX files declare access and error logs at `/var/log/nginx/access.log` and `/var/log/nginx/error.log`; `/health` disables access logging. Collect the logs actually produced by the installed components and arrange their storage separately from backend audit records. These file declarations do not provide centralized or immutable storage.

## Audit records and monitoring

The backend writes `audit_logs` and `security_events` to the app database. Its [audit implementation](../../backend/crates/econ-graph-graphql/src/graphql/audit.rs) records admin mutation attempts by callers holding the required role, including an outcome, and records signed-in callers' permission denials as security events. Writes are best-effort: an insert failure leaves the operation's reply unchanged, and process shutdown can interrupt a write. The [audit models](../../backend/crates/econ-graph-core/src/models/admin.rs) count write failures, exported by the [backend metrics](../../backend/crates/econ-graph-backend/src/metrics.rs) as `audit_write_failures_total`. These records are not a guaranteed complete or immutable trail.

The [GraphQL queries](../../backend/crates/econ-graph-graphql/src/graphql/query.rs) require `admin.audit:read` for `auditLogs` and `admin.security:read` for `securityEvents` and `securityEventsPage`. Operators must arrange retention, backups, and any export to a SIEM for backend records and infrastructure logs.

The supplied admin frontend module does not configure security alert rules or delivery. Operators must select the records and metrics relevant to their installation and arrange alert routing; permission denials and audit-write failures do not by themselves produce a delivered incident alert.

## Diagnosis and recovery prerequisites

Maintain cluster-operator access and an identity-provider recovery procedure independently of the admin UI. The [Keycloak Kubernetes reference](../../config/keycloak/README.md#kubernetes) describes the realm import and its administrator credential dependency; the [roles reference](../../config/keycloak/README.md#roles) describes staff mappings. The repository supplies no emergency-user executable or application break-glass account procedure.

For a reachability failure, first identify the deployment path above. With the normal deploy, missing admin resources are expected. For a separately installed frontend, check its pods, Service ports, installed exposure path, DNS, TLS configuration, and network policies. If a custom Gateway route is used, check its parent listener and route status as well. For backend authorization failures, check OIDC issuer/audience configuration and staff role mappings. Bypassing the frontend exposure path changes reachability; it does not grant a role or repair the missing UI login flow.

For standalone manifests, these diagnostic commands target the declared resources:

```bash
kubectl get pods -n econ-graph -l app=econ-graph-admin-frontend
kubectl logs -n econ-graph deployment/econ-graph-admin-frontend
kubectl port-forward -n econ-graph service/econ-graph-admin-frontend-service 8080:3001
```

For Terraform, use its namespace and Deployment. Use the port-forward only with an image/configuration that satisfies the module's `3000` port contract:

```bash
kubectl get pods -n econ-graph-admin -l app=admin-frontend
kubectl logs -n econ-graph-admin deployment/admin-frontend
kubectl port-forward -n econ-graph-admin service/admin-frontend-service 8080:3000
```

A frontend port-forward alone does not supply the same-origin backend `/graphql` route. It is useful for diagnosing the frontend, not as a complete alternative access path.

Operators own the incident response procedure for their installation, including restricting frontend and backend reachability, preserving available records, and recovering identity-provider administrator access. Review staff assignments and the identity provider's session/token controls as part of that procedure. Do not assume changing a frontend Secret invalidates already issued bearer tokens.
