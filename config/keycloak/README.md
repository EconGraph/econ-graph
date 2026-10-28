# Keycloak realm `econ-graph`

Keycloak is EconGraph's identity provider. The realm is configuration as code:

| File | Used by | Contents |
|---|---|---|
| `econ-graph-realm.json` | docker-compose and k8s | Realm settings, clients, roles and composites, token mappers, Google identity provider |
| `dev/econ-graph-users-0.json` | docker-compose only | Seeded test users. Never deployed |
| `qa/econ-graph-users-1.json` | `docker-compose.qa-users.yml` only | QA users `qa-alice`, `qa-bob`. Never deployed |

Keycloak imports the mounted files on first start (`--import-realm`). An existing realm is
not overwritten, so after editing a file, reset the realm by dropping Keycloak's
database volume and starting again:

```bash
docker compose rm -sf keycloak keycloak-db
docker volume rm "$(basename "$PWD")_keycloak_db_data"
docker compose up -d keycloak
```

In Kubernetes the realm is re-applied on every deploy instead (see below).

## Local stack

```bash
docker compose up -d keycloak
scripts/keycloak/dev-token.sh alice   # prints iss, aud and sub of alice's access token
```

- Issuer: `http://localhost:8081/realms/econ-graph` (host port from `KEYCLOAK_PORT`)
- Admin console: <http://localhost:8081/admin>, user `admin`, password `admin`
  (override with `KEYCLOAK_ADMIN_USERNAME` / `KEYCLOAK_ADMIN_PASSWORD`)

### Test users

The release end-to-end suite depends on these. Keep the usernames, ids and
passwords stable: the ids are the tokens' `sub`, which becomes `users.id`.

| Username | Id (`sub`) | Password |
|---|---|---|
| `alice` | `0199a0e0-0000-7000-8000-00000000a11c` | `alice-dev-password` |
| `bob` | `0199a0e0-0000-7000-8000-000000000b0b` | `bob-dev-password` |
| `dave` | `0199a0e0-0000-7000-8000-00000000da7e` | `dave-dev-password` |
| `carol` | `0199a0e0-0000-7000-8000-00000000ca01` | `carol-dev-password` |
| `erin` | `0199a0e0-0000-7000-8000-0000000000e1` | `erin-dev-password` |
| `frank` | `0199a0e0-0000-7000-8000-0000000000f1` | `frank-dev-password` |
| `staff-admin` | `0199a0e0-0000-7000-8000-0000000005af` | `staff-admin-dev-password` |

Imported users get no default roles automatically, so each one lists
`default-roles-econ-graph`; `staff-admin` also has `admin`.

## Roles

The backend authorizes with fine-grained roles only
([design record](https://linear.app/econgraph/document/auth-plans-and-permissions-roadmap-design-record-from-pr-176-eae73ef9ee81)).
They are client roles on `econ-graph-api`, one per variant of the `Role` enum in
`econ-graph-auth`, and `scripts/check-keycloak-roles` fails CI when the two differ
(`scripts/keycloak/test-check-keycloak-roles.sh` tests it against fixtures). Realm
roles compose them:

| Realm role | Grants |
|---|---|
| `user` | `annotation:create`, `annotation:comment`, `chart:share`, `api:mcp`. In `default-roles-econ-graph`, so every account has it |
| `support` | `user` plus `admin.users:read`, `admin.sessions:read`, `admin.system:read` |
| `admin` | `support` plus every other `admin.*` role |
| `super_admin` | `admin` (nothing extra yet) |

A protocol mapper on `econ-graph-web` puts the flattened `econ-graph-api` roles in the
access token's top-level `roles` claim; `dev-token.sh` prints it. Staff get their
composite in the admin console (Users, Role mapping). Adding a role means adding the
enum variant, the client role and, if staff should have it, the composite.

## Clients

- `econ-graph-web`: public client for the browser app. Authorization code with PKCE
  (S256 required). Exact redirect URIs `<origin>/auth/callback` and
  `<origin>/silent-callback.html`, post-logout redirect `<origin>/`, web origin
  `<origin>`, for each origin below. Access tokens carry `aud: econ-graph-api` through an audience mapper.
  The password grant is enabled only where `KC_WEB_DIRECT_GRANTS=true`, which only
  docker-compose sets, so scripts and tests can get tokens without a browser.
- `econ-graph-api`: bearer-only client representing the backend. It is the token
  audience and will own the fine-grained client roles.

## Placeholders

The realm file reads these environment variables when it is imported:

| Variable | Default | Meaning |
|---|---|---|
| `KC_WEB_BASE_URL` | none (required) | Web app origin |
| `KC_WEB_E2E_BASE_URL` | none (required) | Second web origin: the release e2e frontend (`http://localhost:18081`) in compose, the same as `KC_WEB_BASE_URL` in k8s |
| `KC_WEB_DIRECT_GRANTS` | `false` | Password grant on `econ-graph-web` (dev only) |
| `KC_REALM_SSL_REQUIRED` | `external` | `none` in local compose |
| `KC_GOOGLE_ENABLED` | `false` | Set to `true` by the container command when both the Google client id and secret are configured |
| `KC_GOOGLE_CLIENT_ID`, `KC_GOOGLE_CLIENT_SECRET` | `unset` | Google OAuth client |

With `--import-realm` (docker-compose) placeholders are resolved once, when the realm
is first imported; change a value later by recreating the volume. In Kubernetes the
realm-import Job renders them on every deploy with `scripts/keycloak/render-realm.sh`,
so a rotated Google client secret in the Secret reaches Keycloak on the next deploy.

For Google sign-in locally, create an OAuth client in Google Cloud with redirect URI
`http://localhost:8081/realms/econ-graph/broker/google/endpoint`, then:

```bash
export KEYCLOAK_GOOGLE_CLIENT_ID=... KEYCLOAK_GOOGLE_CLIENT_SECRET=...
docker compose up -d keycloak
```

## QA-only users

Two more realm-local users, `qa-alice` and `qa-bob` (passwords `qa-alice-password`
and `qa-bob-password`), live in `qa/econ-graph-users-1.json` for the release QA
suite to run a two-account check against a local compose stack. They are off by
default; opt in explicitly. Users are imported only on the realm's first start, so
reset the realm first (see above) if Keycloak has run before:

```bash
docker compose -f docker-compose.yml -f docker-compose.qa-users.yml up -d keycloak
```

Never reference the dev or QA users, or `KC_WEB_DIRECT_GRANTS`, from k8s, deploy
scripts or terraform. `scripts/keycloak/check-no-dev-users-in-k8s.sh` (run in CI)
fails the build if they do. Real Google sign-in stays a manual QA step.

## Kubernetes

See `k8s/manifests/keycloak/` and the Keycloak section of `k8s/README.md`. The realm
file becomes the ConfigMap `keycloak-realm`; credentials live only in the Secret
`econ-graph-keycloak`, written by `scripts/deploy/create-secrets.sh`.

The realm is created (on the first deploy) and applied (on every deploy after) by the
Job `keycloak-realm-import` (`realm-import-job.yaml`), which runs
[keycloak-config-cli](https://github.com/adorsys/keycloak-config-cli) against the
running server. Keycloak itself does not import anything: the alternatives were
rejected because `--import-realm` creates resources directly, outside
keycloak-config-cli's tracking, so removing one from the file later would leave it
behind in the cluster; `kc.sh import --override true` deletes the realm and every
account in it. keycloak-config-cli instead changes only what differs from the realm
file, so Google-brokered accounts, sessions and every user's role mappings survive a
redeploy — as long as the roles they're mapped to stay in the file. A realm role,
client or mapper removed from the file is removed from the cluster on the next
deploy, taking any user's mapping to that role with it.

The Job signs in as the admin from the Secret's `admin-username` / `admin-password`.
If that password is changed in the admin console (Keycloak asks to replace the
temporary bootstrap admin), update the Secret too, or the next deploy fails at the
realm import. Its logs: `kubectl -n econ-graph logs job/keycloak-realm-import --all-containers`.

## Catalog access, including anonymous visitors

Each supported catalog has an `econ-graph-api` read role, such as `catalog.fred:read`,
`catalog.bea:read`, or `catalog.bls:read`. Unknown source names are denied. The backend maps
canonical data-source names to these capabilities; adding a source requires its role and mapping.
Search, suggestions, lists, counts, metadata, observations and cross-sections enforce these grants.
So do annotations: `annotationsForSeries` is empty, and `commentsForAnnotation`, `addComment` and
`updateAnnotation` answer "Annotation not found", for an annotation on a series in a catalog the
caller can't read; `createAnnotation` answers as for a missing series. Lookups by id or external id
(`series`, `seriesExists`, `seriesCatalog`, `seriesByExternalId`) return null or `false`, as for a
missing series, so none of them reveals what an unreadable catalog contains.
MCP tools use the authenticated caller's catalog roles as well as `api:mcp`.

The `catalog-public` realm composite grants the initial catalog reads. The `user` composite and
the `econgraph-anonymous` service account inherit it, so it may hold only `econ-graph-api`
`catalog.*:read` roles: a deploy stops, before importing anything, while it holds any other
composite (the error names each one to remove). Remove a catalog read role from
`catalog-public` to withdraw default access for both anonymous visitors and signed-in users.
Explicit grants to other groups or users still apply. To restrict just anonymous access, remove
`catalog-public` from the anonymous service account and assign its permitted catalog roles
individually instead. Use group/composite grants for signed-in exceptions as usual.

Visitors receive no token and no service-account identity. The backend obtains a client-credentials
token, verifies its signature, issuer and audience, and uses only its catalog-read roles for
anonymous data requests. It never turns those roles into account, annotation, or staff privileges.
Signed-in callers use only their own token's roles, even if anonymous visitors have broader access.

Configure the backend with:

| Setting | Purpose |
|---|---|
| `OIDC_ISSUER`, `OIDC_AUDIENCE`, `OIDC_JWKS_URL` | Existing token verification settings |
| `OIDC_ANONYMOUS_TOKEN_URL` | Reachable HTTPS token endpoint, or loopback HTTP for local development |
| `OIDC_ANONYMOUS_ALLOW_INSECURE_HTTP` | Explicit `true` allows private-network HTTP credential transport; enabled only for the internal Compose/Kubernetes Keycloak service |
| `OIDC_ANONYMOUS_CLIENT_ID` | Defaults to `econgraph-anonymous` |
| `OIDC_ANONYMOUS_CLIENT_SECRET` | Server-only credential matching `KC_ANONYMOUS_CLIENT_SECRET` in the realm import |
| `ANONYMOUS_GRANTS_STALE_MAX_SECS` | Seconds (0 to 86400, default 0) that the last fetched anonymous grants may still be used after they expire, only while renewal is failing; see below |

Missing anonymous configuration grants no anonymous data access, including when sign-in is off.
Partial configuration fails startup. Keycloak omits the roles claim for a reader with no grants;
that is an empty grant set. Signed-in token verification remains strict.

Anonymous grants are renewed by one background task per backend process, never by a request:
requests read the current grant set from memory and never wait for Keycloak. The task fetches at
startup (until it succeeds, anonymous visitors have no catalog access), then again when three
quarters of each grant set's lifetime has passed, and every ten seconds while renewal fails. A
grant set lasts no longer than its token's absolute expiry, `expires_in`, or five minutes,
whichever is shortest. A successful renewal replaces the grant set at once, so a revocation in
Keycloak reaches anonymous visitors at the next renewal, within about four minutes. Keycloak refusing the client (disabled, or a wrong secret) or issuing a token
that fails verification drops the grants at once.

During a Keycloak outage (no answer, a timeout, or a server error), unexpired grants stay in use.
What happens after they expire depends on `ANONYMOUS_GRANTS_STALE_MAX_SECS`:

- `0`, the default, fails closed: anonymous visitors lose catalog access when the grants expire.
- Above `0`, the last successfully fetched grants stay in use for at most that many seconds past
  their expiry, only while renewal keeps failing, and the backend logs a warning on every failed
  renewal while it serves them. The first successful renewal replaces them, so a revocation made
  during the outage takes effect as soon as Keycloak answers again.

A stale window delays only revocations that can't be fetched; it never extends grants past a
successful renewal. Disabling the anonymous client counts as a refusal, not an outage, so it
revokes anonymous access at once whatever the setting.

User role removals take effect when existing five-minute access tokens expire, plus the
verifier's 30-second clock tolerance. No permission check calls Keycloak.

Docker Compose uses a development-only client secret. Kubernetes provisions a random credential
through `scripts/deploy/create-secrets.sh`, preserving it across deployments, and supplies the
same Secret key (`econ-graph-keycloak/anonymous-client-secret`) to the backend, Keycloak and realm
import Job. Set `KEYCLOAK_ANONYMOUS_CLIENT_SECRET` to supply a managed credential instead.
`scripts/deploy/deploy.sh` stops before applying anything when that Secret exists without the
`anonymous-client-secret` key; run `scripts/deploy/create-secrets.sh` (it keeps every existing
value and adds the key), then deploy again.
Existing Compose realms need the normal development realm reset. Kubernetes reapplies structural
configuration on deployment, but `preserve-catalog-grants.py` snapshots catalog policy first.
Existing catalog-public grants, catalog edges on managed composites, anonymous-reader mappings,
reader scope restrictions and enabled states survive reapply, including empty sets. Defaults seed
only the first feature rollout. The render step records which policy resources originally needed
bootstrap, so an interrupted first import can retry without freezing partial empty mappings. A
finalizer marks initialization only after config-cli succeeds; deleting a policy role or reader
cannot restore bootstrap grants on a later deploy. The reader is deliberately excluded
from signed-in default roles, which would otherwise bypass anonymous-only restrictions.

Snapshot failures abort the import instead of falling back to defaults. Snapshot and import are
separate admin API operations, so preservation is best effort under concurrent administrator edits;
avoid editing catalog policy during a deployment. Ordinary brokered users are not imported or
modified. Role definitions and non-catalog structural settings remain configuration as code.

### Separate and shared data planes

For requests to the same GraphQL endpoint, the browser sends the signed-in user's token.
A separate `VITE_DATA_GRAPHQL_URL` stays anonymous by default, preserving branch environments
that share a data plane with a different Keycloak realm. These reads use the **data plane's
anonymous catalog grants**, rather than that branch user's roles. Set the frontend build option
`VITE_DATA_GRAPHQL_AUTH=true` only when that data plane trusts the same issuer and audience;
then signed-in catalog roles apply there. A denied or invalid token is never retried anonymously
except for the existing explicit suspended-account public-read behavior.

An app-only backend checks an annotation's series through `DATA_GRAPHQL_URL`, asking
`seriesCatalog(id)` before creating the annotation or showing it, its comments, or writes to it.
The data plane answers null for a series it doesn't have or won't let this request read; this
backend then applies its own caller's catalog grants to the catalog name. If the data plane can't
answer, these annotation requests fail with `DATA_PLANE_UNAVAILABLE` rather than skip the check.
This preflight likewise uses anonymous grants by default. For a trusted data plane, set
`DATA_GRAPHQL_FORWARD_AUTHORIZATION=true` so the preflight can read a catalog granted to the
signed-in caller even when anonymous access is disabled. Forwarding requires HTTPS by default.
For a deliberately trusted private HTTP hop, also set `DATA_GRAPHQL_ALLOW_INSECURE_AUTHORIZATION=true`;
that exception sends the bearer token in plaintext and must not be used for public network targets.
Requests follow no redirects and tokens are not logged. Cross-realm gateways must continue to strip credentials; a trusted route must
preserve them. Catalog authorization does not replace that gateway trust configuration.

For a local sign-in-off server, clear `OIDC_ISSUER`, `OIDC_ANONYMOUS_TOKEN_URL` and
`OIDC_ANONYMOUS_CLIENT_SECRET` together. Empty anonymous settings count as absent. This starts
the server without Keycloak and denies catalog reads, while keeping health and other public
non-data routes available.
