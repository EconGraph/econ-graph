# Keycloak realm `econ-graph`

Keycloak is EconGraph's identity provider. The realm is configuration as code:

| File | Used by | Contents |
|---|---|---|
| [`econ-graph-realm.json`](econ-graph-realm.json) | docker-compose and k8s | Realm settings, clients, roles and composites, token mappers, Google identity provider |
| [`dev/econ-graph-users-0.json`](dev/econ-graph-users-0.json) | docker-compose only | Seeded test users. Never deployed |
| [`qa/econ-graph-users-1.json`](qa/econ-graph-users-1.json) | [`docker-compose.qa-users.yml`](../../docker-compose.qa-users.yml) only | QA users `qa-alice`, `qa-bob`. Never deployed |

## Local stack

```bash
docker compose up -d keycloak
scripts/keycloak/dev-token.sh alice   # prints selected access-token claims, including roles
```

- Issuer: `http://localhost:8081/realms/econ-graph` (host port from `KEYCLOAK_PORT`)
- Admin console: <http://localhost:8081/admin>, user `admin`, password `admin`
  (override with `KEYCLOAK_ADMIN_USERNAME` / `KEYCLOAK_ADMIN_PASSWORD`)

### Reset the local realm

In the local Compose stack, Keycloak imports the mounted files on first start
(`--import-realm`). It does not overwrite an existing realm. After editing an
imported file, reset the local realm by removing Keycloak's database volume and
starting again. This deletes all local Keycloak accounts and other state in that
database, including changes made through the admin console.

Run from the repository root:

```bash
docker compose rm -sf keycloak keycloak-db
docker volume rm "$(basename "$PWD")_keycloak_db_data"
docker compose up -d keycloak
```

The volume name above assumes Compose uses the repository directory name as its
project name. If you use `-p` or `COMPOSE_PROJECT_NAME`, use
`<project-name>_keycloak_db_data` and the same project name for the Compose commands.
In [Kubernetes](#kubernetes), the realm is re-applied on every deploy.

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
  audience and owns the fine-grained client roles.

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
is first imported; change a value later by [resetting the local realm](#reset-the-local-realm).
In Kubernetes the realm-import Job renders them on every deploy with
[`scripts/keycloak/render-realm.sh`](../../scripts/keycloak/render-realm.sh),
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
default; opt in explicitly. Users are imported only on the realm's first start.
If Keycloak has run before, follow the container and volume removal steps in
[Reset the local realm](#reset-the-local-realm), including its data-loss and
project-name guidance. Use the command below instead of that procedure's final
startup command, so the QA users are mounted when the realm is first imported:

```bash
docker compose -f docker-compose.yml -f docker-compose.qa-users.yml up -d keycloak
```

Never reference the dev or QA users, or `KC_WEB_DIRECT_GRANTS`, from k8s, deploy
scripts or terraform. `scripts/keycloak/check-no-dev-users-in-k8s.sh` (run in CI)
fails the build if they do. Real Google sign-in stays a manual QA step.

## Kubernetes

See the [Keycloak manifests](../../k8s/manifests/keycloak/) and the
[deployment guide's Keycloak section](../../k8s/README.md#-keycloak). The realm file
becomes the ConfigMap `keycloak-realm`; credentials live only in the Secret
`econ-graph-keycloak`, written by
[`scripts/deploy/create-secrets.sh`](../../scripts/deploy/create-secrets.sh).

The [`keycloak-realm-import` Job](../../k8s/manifests/keycloak/realm-import-job.yaml)
creates the realm on the first deploy and applies it on every deploy after that.
It runs [keycloak-config-cli](https://github.com/adorsys/keycloak-config-cli)
against the running server's admin API, changing only what differs from the realm
file.

Google-brokered accounts and sessions survive a redeploy. Users' role mappings
also survive as long as the roles they are mapped to remain in the file. A realm
role, client or mapper removed from the file is removed from the cluster on the
next deploy; removing a role also removes users' mappings to it.

Keycloak's native imports do not provide this reconciliation behavior.
`--import-realm` skips existing realms and creates resources outside
keycloak-config-cli's tracking, so resources created that way would remain in the
cluster after removal from the file. `kc.sh import --override true` deletes the
realm and every account in it.

The Job signs in as the admin from the Secret's `admin-username` / `admin-password`.
If that password is changed in the admin console (Keycloak asks to replace the
temporary bootstrap admin), update the Secret too, or the next deploy fails at the
realm import. Its logs: `kubectl -n econ-graph logs job/keycloak-realm-import --all-containers`.
