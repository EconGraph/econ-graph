# Independently authorized data access

The persistent data service serves the Train 3 observation/API contract through
the working Postgres `TimeSeriesStore` adapter. It has its own data audience and a
read-only GraphQL boundary. A developer can read shared data while the app uses a
fresh local app database, without receiving a shared database credential or
starting a crawler.

The [composition decision](../technical/adr/0002-independent-data-proxy.md) selects
an explicit operation-routing HTTP proxy. App and data operations remain separate
GraphQL requests. Clients compose their results, including concurrent app/data
reads; the proxy does not rewrite queries, stitch entities or silently divide a
mixed selection. Existing operation names, exact decimal strings, revision
defaults, visibility and pagination follow the
[data access contract](../api/DATA_ACCESS_CONTRACT.md).

## Preserve public browsing and add a signed entry point

The deployed candidate keeps the current combined backend as its public facade.
Existing browser `/graphql` and `/data/graphql` routes, anonymous catalog grants
and frontend routing remain in place. Known invalid bearer tokens still get 401;
catalog denial does not become an anonymous retry. The combined endpoint retains
its existing provider-outage behavior for public browsing. It authorizes every
request with its current caller/anonymous catalog grants.

The opt-in platform overlay adds the **exact** `/independent-data/graphql` path,
rewritten to `/graphql` on `econ-graph-data-backend-service:9876`. That route
preserves Authorization, removes Cookie, and has no anonymous bypass. Direct pod
access enforces the same policy: the gateway grants nothing. Do not replace the
public route with this signed-only route or globally set frontend
`VITE_DATA_GRAPHQL_AUTH=true` as part of this addition.

| `GRAPHQL_SERVICE_PLANE` | Database configuration                    | Token and surface                                                                                                 |
| ----------------------- | ----------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| unset / `combined`      | Existing app/data configuration           | Existing default behavior, including public catalog access and role-gated control operations                      |
| `app`                   | App database only; no `DATA_DATABASE_URL` | Required `econ-graph-api` audience; authenticated app operations; local data fields remain `DATA_SERVED_REMOTELY` |
| `data`                  | Data database plus ancillary app pool     | Required `econ-graph-data` audience and at least one catalog read capability; allowlisted data query roots only   |

Independent profiles refuse startup without an issuer or with the wrong audience
or database shape. Anonymous-grant configuration is used only by the combined
profile. Independent app/data requests verify RS256, issuer, required audience,
expiry, not-before and access-token type through the existing OIDC verifier; they
also retain the existing active-account check. Missing/invalid tokens get 401,
issuer/key unavailability gets 503, and a token without catalog read capabilities
gets 403 on the data service. Catalog grants still restrict actual sources before
counts and pagination; holding one catalog capability does not grant the others.
Plan and staff composites remain flattened fine-grained roles in the token.

The data profile accepts data discovery, dataset/series/observation reads,
cross-sections and introspection. It validates the selected operation's actual
root fields, including aliases and fragments, before executing any resolver. All
mutations, crawler controls, user administration and MCP are unavailable there,
even for staff. The control/admin frontend must use the authenticated combined
control backend, whose queue/configuration operations retain their named roles.
It must never use the read-only data route.

The ancillary app pool on a data service is still required by the existing
backend for subject registration, suspension checks and audit. It does not grant
a data client an app API audience or app mutation permission. Subject registration
can create an identity row there; the data observation database is never migrated
by this backend and data requests cannot enqueue jobs or change observations.
Use separately scoped database credentials and NetworkPolicy in the platform
overlay. `crawler migrate` remains the explicit data migration owner.

## Explicit identity-provider overlay

Keep `config/keycloak/econ-graph-realm.json` unchanged for existing deployments.
The independent platform imports the result of:

```sh
python3 scripts/keycloak/independent-data-realm.py \
  config/keycloak/econ-graph-realm.json > /tmp/independent-data-realm.json
scripts/keycloak/render-realm.sh /tmp/independent-data-realm.json > /tmp/rendered-realm.json
```

The first command operates on the unresolved template; the second uses the same
deployment variables as the existing realm import. The overlay adds the browser
web client's data audience without removing its app audience. It adds the
bearer-only `econ-graph-data` resource and a public PKCE
`econ-graph-dev-readonly` client. That development client has a fixed loopback
callback, no password grant, no service account, no app audience, and
`fullScopeAllowed=false`. Its role scope mappings permit only catalog read roles
the signed-in account already holds; they never assign extra grants or staff
capabilities. It has no optional offline-access scope. Anonymous service-account
grants retain the existing public facade and do not receive a data audience.

Importing this overlay into a live realm is a deployment action. Rendering and
offline shape checks do not claim that a live Keycloak import, new-platform rollout
or production authorization matrix has been completed.

## Fresh local app with shared read-only data

Use the data service's primary issuer to sign in once with the read-only client:

```sh
python3 scripts/keycloak/dev-readonly-token.py \
  --issuer https://your-host/idp/realms/econ-graph \
  --output /tmp/econgraph-data-token
python3 scripts/data-access/dev-data-proxy.py \
  --data-url https://your-host/independent-data/graphql \
  --token-file /tmp/econgraph-data-token
```

The helper uses authorization code plus PKCE and binds only
`127.0.0.1:8765/callback` for up to three minutes. It creates a new token file with
mode 0600 and refuses replacement/symlink traversal. It does not print the token
or save a refresh token. The token is deliberately short-lived: after expiry,
remove that token file and sign in again. The proxy rereads the file on each
request so explicit renewal takes effect without restart.

The proxy defaults to loopback port 9877. It accepts JSON POSTs only at `/graphql`
and `/data/graphql`, uses one fixed HTTPS data target, rejects redirects and
unexpected Host headers, bounds bodies, and forwards neither caller cookies nor
the local app's bearer token. Its own token-shape checks only restrict credential
forwarding; the data backend performs cryptographic authentication. It refuses
app-audience, staff-capability, wrong-client and expired credentials. Do not expose
this development helper through a public tunnel or bind it on another interface.

Run the app backend with its own `APP_DATABASE_URL`, unset `DATA_DATABASE_URL`,
and configure its usual local issuer/app audience. For annotation-target checking,
set `DATA_GRAPHQL_URL=http://127.0.0.1:9877/graphql`; leave authorization forwarding
off because the proxy supplies the primary realm's read-only credential. This
still intersects the remote answer with the local caller's catalog grants. The
local backend does not run data migrations or recrawl.

Run the existing frontend with:

```sh
BACKEND_URL=http://127.0.0.1:9876 \
DATA_BACKEND_URL=http://127.0.0.1:9877 \
VITE_DATA_GRAPHQL_URL=/data/graphql npm --prefix frontend run dev
```

Use the port on which the local app backend actually listens for `BACKEND_URL`.
Vite forwards the data path to the loopback proxy, which uses the primary data
token. App sign-in and annotations stay local. No shared database URL or primary
app token is placed in frontend configuration. Local HTTP is confined to loopback;
the remote data URL requires HTTPS.

## Verification and deployment boundary

With `DATABASE_URL` naming a disposable Postgres 18 server whose role can create
scratch databases:

```sh
python3 scripts/data-access/test_independent_access.py
cargo test --manifest-path backend/Cargo.toml -p econ-graph-backend \
  --bin econ-graph-backend -- independent_data_tests:: data_access::tests:: request_auth_tests:: --test-threads=1
```

The acceptance tests run real TCP backend routes, RS256 verification and Postgres
observations through the current store, plus a fresh separate app database. They
exercise concurrent authorized app/data reads, wrong/missing audiences, missing
or malformed tokens, insufficient roles, issuer outage, mutations, crawler
control, unchanged queue state, and the actual read-only development proxy. The
local test issuer supplies signing keys and claims; it is not a deployed Keycloak
realm. Existing request-auth regressions verify combined/public compatibility.
Offline proxy tests cover host/path/header confinement, expired/overprivileged
credentials and redirect refusal; realm tests verify the explicit overlay shape.

The required CI step runs both independent-service boundary suites and the Python
contracts without ignoring or silently skipping database acceptance. Final
self-hosted-platform acceptance must additionally import the realm overlay, prove
both old public paths and the new signed path, exercise real user plan grants,
and verify NetworkPolicy against the ECO-438 deployment. Local contract success
does not replace that sibling deployment evidence.
