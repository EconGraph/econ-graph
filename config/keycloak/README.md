# Keycloak realm `econ-graph`

Keycloak is EconGraph's identity provider. The realm is configuration as code:

| File | Used by | Contents |
|---|---|---|
| `econ-graph-realm.json` | docker-compose and k8s | Realm settings, clients, roles and composites, token mappers, Google and Facebook identity providers |
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

The release end-to-end suite depends on these. It also reads by token (password
grant) as `staff-admin` for the admin-only FRED catalog and as `grace` for an ordinary
user; the QA scenario suite signs in as `grace` against the release e2e stack. Keep the
usernames, ids and passwords stable: the ids are the tokens' `sub`, which becomes `users.id`.

| Username | Id (`sub`) | Password |
|---|---|---|
| `alice` | `0199a0e0-0000-7000-8000-00000000a11c` | `alice-dev-password` |
| `bob` | `0199a0e0-0000-7000-8000-000000000b0b` | `bob-dev-password` |
| `dave` | `0199a0e0-0000-7000-8000-00000000da7e` | `dave-dev-password` |
| `carol` | `0199a0e0-0000-7000-8000-00000000ca01` | `carol-dev-password` |
| `erin` | `0199a0e0-0000-7000-8000-0000000000e1` | `erin-dev-password` |
| `frank` | `0199a0e0-0000-7000-8000-0000000000f1` | `frank-dev-password` |
| `grace` | `0199a0e0-0000-7000-8000-0000000009a7` | `grace-dev-password` |
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
| `KC_FACEBOOK_ENABLED` | `false` | Enabled by the Kubernetes render command only when both Facebook credentials are present |
| `KC_FACEBOOK_CLIENT_ID`, `KC_FACEBOOK_CLIENT_SECRET` | `unset` | Facebook app credentials, read from the environment-specific Kubernetes Secret |

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

A maintainer enters the Google client with hidden input with
`scripts/deploy/set-crawler-keys.sh --google`, which then restarts Keycloak and reruns
the Job. Before asking for anything it checks that the cluster's `keycloak-realm`
ConfigMap holds this checkout's realm files, so run it from a checkout of the commit
the cluster runs (for production, the release tag). It renders the Job for the public
host the cluster serves, and refuses a production cluster that serves none, or a
`PUBLIC_HOST` (in the environment or `~/.config/econgraph/deploy.env`) that differs
from it.

The Job signs in as the admin from the Secret's `admin-username` / `admin-password`.
If that password is changed in the admin console (Keycloak asks to replace the
temporary bootstrap admin), update the Secret too, or the next deploy fails at the
realm import. Its logs: `kubectl -n econ-graph logs job/keycloak-realm-import --all-containers`.

## Facebook sign-in

Facebook sign-in is disabled ([ECO-470](https://linear.app/econgraph/issue/ECO-470)).
The realm declares an optional `facebook` provider that uses the same
first-broker-login and account-linking flow as Google, requests only `email` and
`public_profile`, and stores no upstream token. Facebook emails are not trusted as
verified: Keycloak handles profile completion and existing-account verification through
its default first-broker-login flow, rather than automatically linking by email. The
realm import enables the provider only when `econ-graph-keycloak` holds both
`facebook-idp-client-id` and `facebook-idp-client-secret`; without them no Facebook
button appears.

`scripts/deploy/set-crawler-keys.sh --facebook` refuses before any prompt, cluster call
or write, and exits non-zero, with `--dry-run` too. `scripts/deploy/create-secrets.sh`
likewise refuses to run, before any cluster call and with `--dry-run` too, while
`KEYCLOAK_FACEBOOK_CLIENT_ID` or `KEYCLOAK_FACEBOOK_CLIENT_SECRET` is set and non-empty;
it keeps Facebook keys a Secret already holds (see below to remove them). Local compose
does not forward Facebook credentials.

The privacy policy at `https://econgraph.com/privacy` covers either provider, and
`https://econgraph.com/privacy#account-deletion` gives the data-deletion instructions a
Meta app links to. A Meta app for this realm uses domain `econgraph.com` and redirect
URI `https://econgraph.com/idp/realms/econ-graph/broker/facebook/endpoint`
([Keycloak Facebook provider setup](https://www.keycloak.org/docs/26.8.1/server_admin/#_facebook)).
There is no Facebook deletion callback: the instructions explain how to request
account deletion. The backend serves no `/auth/*` routes; sign-in goes through Keycloak.

### Disabling Facebook sign-in

Neither `set-crawler-keys.sh` nor `create-secrets.sh` removes a key, so remove the
two keys from the Secret by hand, with the kind cluster's kubectl, then restart
Keycloak and rerun the realm import. Use a checkout of the commit the cluster runs
(for production, the release tag). A JSON-patch `remove` fails on a key that is
absent, so remove only the keys the Secret has:

```bash
k() { KUBECONFIG=~/.cache/econgraph/kubeconfig-econ-graph ~/.cache/econgraph/bin/kubectl --context kind-econ-graph "$@"; }
k -n econ-graph patch secret econ-graph-keycloak --type=json -p='[
  {"op": "remove", "path": "/data/facebook-idp-client-id"},
  {"op": "remove", "path": "/data/facebook-idp-client-secret"}]'
k -n econ-graph rollout restart deployment/keycloak
k -n econ-graph rollout status deployment/keycloak --timeout=600s
k -n econ-graph delete job keycloak-realm-import --ignore-not-found
# On the production cluster, rendered for its public host; elsewhere apply the file as it is.
scripts/deploy/render-public-host.py econgraph.com k8s/manifests/keycloak/realm-import-job.yaml | k apply -f -
k -n econ-graph wait --for=condition=complete job/keycloak-realm-import --timeout=600s
```

Use this on a cluster whose Secret holds either key. Without both keys the realm
import disables the `facebook` provider and the Facebook button disappears. Accounts
whose only sign-in is Facebook can then no longer sign in; their data stays in place.
A later deploy keeps the provider disabled while the keys are absent.

## Catalog access, including anonymous visitors

Each supported catalog has an `econ-graph-api` read role, such as `catalog.fred:read`,
`catalog.bea:read`, or `catalog.bank_of_canada:read`. Unknown source names are denied. The
backend maps canonical data-source names to these capabilities; adding a source requires its
role and mapping. A new catalog role spells the source out (`catalog.bank_of_canada:read`, not
an acronym). The crawler's `catalog_roles` test fails when a source's name has no role, a
catalog role is missing from `catalog-public`, or a role in `catalog-public` is missing from the
`econgraph-anonymous` scope mapping, apart from the exceptions it lists.

Search, suggestions, lists, counts, metadata, observations and cross-sections enforce these grants.
So do annotations: `annotationsForSeries` is empty, and `commentsForAnnotation`, `addComment` and
`updateAnnotation` answer "Annotation not found", for an annotation on a series in a catalog the
caller can't read; `createAnnotation` answers as for a missing series. Lookups by id or external id
(`series`, `seriesExists`, `seriesCatalog`, `seriesByExternalId`) return null or `false`, as for a
missing series, so none of them reveals what an unreadable catalog contains.
MCP tools use the authenticated caller's catalog roles as well as `api:mcp`.

The `catalog-public` realm composite grants the initial catalog reads: in the realm file,
BLS, BEA, Census, FHFA and World Bank. FRED's `catalog.fred:read` is on
the `admin` composite instead (`super_admin` includes `admin`), so only admins read FRED. The
IMF, ECB, OECD, Bank of England, Bank of Japan, RBA, SNB, UN Statistics and ILO reads are
granted to nobody by default. The `user` composite and
the `econgraph-anonymous` service account inherit it, so it may hold only `econ-graph-api`
`catalog.*:read` roles: a deploy stops, before importing anything, while it holds any other
composite (the error names each one to remove). For the same reason no `catalog.*:read` role
may itself be a composite, at any depth: a role nested under one would reach every account. A
deploy stops while one is, live or in the realm file. Remove a catalog read role from
`catalog-public` to withdraw default access for both anonymous visitors and signed-in users.
Explicit grants to other groups or users still apply. To restrict just anonymous access, remove
`catalog-public` from the anonymous service account and assign its permitted catalog roles
individually instead. Use group/composite grants for signed-in exceptions as usual.

The WTO and Bank of Canada catalogs are restricted by default:
`catalog.world_trade_organization:read` (World Trade Organization) and
`catalog.bank_of_canada:read` (Bank of Canada) are defined but are in neither `catalog-public`
nor the anonymous client's scope, so only users or groups granted them can read those series.
To open one up, grant its role to a user or group, or add it to `catalog-public` (and the
anonymous scope) to make it public.
`backend/crates/econ-graph-auth/tests/restricted_catalog_roles.rs` checks that the realm file
keeps them that way.

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
whichever is shortest.

A change in Keycloak reaches anonymous visitors at the backend's next renewal, not at once: with
five-minute grants, up to about 225 seconds later (three quarters of 300 seconds). The renewal
replaces the grant set entirely, so a revocation, or a narrower grant, applies from that moment.
Disabling the anonymous client, or rotating its secret, also takes effect at the next renewal:
Keycloak's refusal then drops the grants instead of keeping them, whatever the stale setting
below. To make a revocation take effect immediately, make it in Keycloak, then restart the
backend. A new process serves no anonymous catalog data until its first fetch succeeds, so once
the rollout finishes, every backend serves the revoked grant set:

```bash
kubectl -n econ-graph rollout restart deployment/econ-graph-backend
```

During a Keycloak outage (no answer, a timeout, or a server error), unexpired grants stay in use.
What happens after they expire depends on `ANONYMOUS_GRANTS_STALE_MAX_SECS`:

- `0`, the default, fails closed: anonymous visitors lose catalog access when the grants expire.
- Above `0`, the last successfully fetched grants stay in use for at most that many seconds past
  their expiry, only while renewal keeps failing, and the backend logs a warning on every failed
  renewal while it serves them. The first successful renewal replaces them, so a revocation made
  during the outage takes effect as soon as Keycloak answers again.

A stale window delays only revocations that can't be fetched; it never extends grants past a
successful renewal. A refusal from Keycloak is not an outage, so it never opens the window.

The backend reports renewal on `/metrics` (`anonymous_grant_renewals_total{outcome}`,
`anonymous_grant_renewal_consecutive_failures`, `anonymous_catalog_grants`,
`anonymous_grants_serving_stale`, `anonymous_grant_refresh_task_running`), and
`k8s/monitoring/prometheus-rules-auth.yaml` alerts after six failed renewals in a row, after ten
minutes with no anonymous grants, and when the renewal task stops.

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

The import's keycloak-config-cli step manages the scope mappings of `econ-graph-api` roles on
every client and client scope: it sets each one's to what the realm file's
`clientScopeMappings` declares, and removes them from every client and client scope the file
doesn't list there, including mappings made in Keycloak. Catalog reads in those mappings are
operator policy and are kept like the grants above. `preserve-catalog-grants.py` writes each
client's and client scope's live `catalog.*:read` scope mappings into the import. As with the
grants above, a catalog read the file declares there seeds only a new realm (or the retry of an
interrupted first import); after that, a client or client scope the import creates, or recreates
after an operator deleted it, gets none. A client that config-cli created and the realm file no
longer lists is deleted by the import, and its grants with it.
Other `econ-graph-api` roles there, the anonymous reader's scope included, are configuration as
code. When one was mapped in Keycloak but isn't declared, the import removes it and first names it
in the Job's `render-realm` log, as in `warning: the realm import removes econ-graph-api scope
mappings the realm file doesn't declare on client partner-x: api:mcp`. Read it with
`kubectl -n econ-graph logs job/keycloak-realm-import -c render-realm` until the next deploy
replaces the Job. To keep such a mapping, declare it:

```json
"clientScopeMappings": {
  "econ-graph-api": [
    {"client": "partner-x", "roles": ["api:mcp"]},
    {"clientScope": "catalog-extra", "roles": ["api:mcp"]}
  ]
}
```

The client or client scope must exist, either in Keycloak or in the realm file. Remove its
entry before deleting it. Scope mappings of realm roles, such as `catalog-public` mapped on a
client scope, are not managed and stay as they are.

Anonymous tokens are built with the anonymous client's client scopes (`basic`, and any other
default or optional scope assigned to it) as well as its own scope mapping, with composite roles
expanded. The supported switch for anonymous access to a catalog is its read mapped directly on
the client's own scope. A catalog read admitted any other way would reach visitors whatever that
mapping says, including after `set-catalog-access.sh` withdraws it. That covers a catalog read
mapped on a client scope, a realm role such as `catalog-public` or `user` mapped on one or on the
client's own scope, a composite role holding a catalog read, and full scope (`fullScopeAllowed`)
on the anonymous client. Full scope stops even the deploy that would turn it off, so turn it off
in Keycloak first. Protocol mappers that write roles into tokens are not checked: don't add one to
the anonymous client or its client scopes. So a deploy stops, before
importing anything, while one does, and the error names each scope and the catalog reads it
admits. Keycloak is checked live, and the realm file's `clientScopeMappings` and `scopeMappings`
through its realm composites. Remove those scope mappings, and make a catalog public with
`set-catalog-access.sh --make-public` instead; `set-catalog-access.sh` names such a scope too. A
realm default client scope admitting catalog reads is named with a warning: a recreated anonymous
client would be assigned it.

When a role leaves the realm file, the import deletes it if config-cli created it. Dropping a
client's block under `roles.client` deletes the roles config-cli created for it. Dropping a client
config-cli created deletes the client with every role on it, including roles made in Keycloak;
Keycloak's own clients, such as `account`, are never deleted. config-cli
records those in the realm's `de.adorsys.keycloak.config.state-default-*` attributes, and when that
list shrinks it leaves an old chunk behind, which both it and the import ignore (the import names it
in a warning). The role's live grants are then left out of the import, each named with a warning,
because importing a grant of a deleted role fails the import part way. That covers grants on
composites, on scope mappings and on the anonymous service account (its realm and client roles
alike). A role an operator created in Keycloak isn't deleted, and its grants are kept. A state that
can't be read stops the deploy.

`scripts/keycloak/test-realm-import.sh` runs the import against a throwaway Keycloak, starting
from the last release's realm as production does, and checks what it keeps, removes and refuses.
CI runs it when the realm, its import, the Keycloak manifests or `scripts/deploy/deploy.sh` change.

Snapshot failures abort the import instead of falling back to defaults. Snapshot and import are
separate admin API operations, so preservation is best effort under concurrent administrator edits;
avoid editing catalog policy during a deployment. Ordinary brokered users are not imported or
modified. Role definitions and non-catalog structural settings remain configuration as code.

### Changing catalog access on a running cluster

The realm file's catalog grants seed only a new realm. On a running cluster, deploys keep the
live `catalog-public`, the catalog grants on other composites such as `admin`, the anonymous
reader's scope and the catalog reads in other clients' and client scopes' scope mappings
(above), so a change to them is made in Keycloak.
`scripts/keycloak/set-catalog-access.sh` makes it, on the kind cluster named by `--context`:

```bash
# Match the realm file: FRED admin-only, and the catalogs it grants to nobody withdrawn:
scripts/keycloak/set-catalog-access.sh --context kind-econ-graph --dry-run \
  --admin-only catalog.fred:read \
  --withdraw catalog.imf:read,catalog.ecb:read,catalog.oecd:read,catalog.boe:read,catalog.boj:read,catalog.rba:read,catalog.snb:read,catalog.unstats:read,catalog.ilo:read
# The same without --dry-run applies it. To make a catalog public again:
scripts/keycloak/set-catalog-access.sh --context kind-econ-graph --make-public catalog.fred:read
```

- `--admin-only` removes the role from `catalog-public` and the anonymous scope and adds it to
  `admin`; `--withdraw` removes it from both and grants it to nobody; `--make-public` adds it to
  both and removes a direct grant on `admin`, which then reads it through `catalog-public`.
- `--dry-run` prints each change it would make and changes nothing in Keycloak. Every step
  runs only when needed, so running a command again changes nothing.
- It adds grants before it removes any, so a failure part way never takes a catalog from
  admins, and it reads back every grant it set. There is no rollback: if it stops with an
  error, run the same command again to finish.
- It refuses while `job/keycloak-realm-import` is running, because that deploy step would
  write back the catalog policy it read before the change. Don't start a deploy while it runs.
- It refuses, changing nothing, a role that isn't a `catalog.<source>:read` role or doesn't
  exist, and a realm without `catalog-public` or `admin`. It checks every role before changing
  any.
- It runs a short-lived pod with the realm import's Python image, which calls the admin API
  at `keycloak-service` with the credentials from Secret `econ-graph-keycloak`, passed by
  reference; nothing prints them. The cluster's identity is printed first.
- Visitors see the change at the backend's next anonymous-grant renewal and signed-in users
  when their token expires (above); the script prints the backend restart that applies it
  to visitors at once.

When a catalog's default access changes for good, change the realm file to match, so a new
realm starts the same way.

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
