# Changelog

All notable changes to EconGraph are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Releases are tagged `vX.Y.Z`; see
[the release process](docs/development/RELEASE_PROCESS.md) for how a release reaches a tag.

## [Unreleased]

### Added

- The frontend can send data operations (series, search, data sources, the dashboard cards and
  the world map) to a separate GraphQL URL, `VITE_DATA_GRAPHQL_URL`, and never sends them a
  sign-in token. Annotations, comments and admin operations still go to `/graphql` with the
  token. Unset, every request goes where it went before. The dev server and `vite preview`
  proxy `/data/graphql` to `DATA_BACKEND_URL` (default `BACKEND_URL`).
- The kind cluster serves `/data/graphql`: the backend's `/graphql` with `Authorization` and
  `Cookie` removed, rate-limited like `/graphql`.
- For the public host (`PUBLIC_HOST`, served through the Cloudflare Tunnel), the gateway gets an
  https listener with a cert-manager certificate, `-public` copies of the routes on it, and
  redirects of plain http and `www` to `https://<host>`.
- `scripts/deploy/test-gateway-routes.sh` checks every route on the gateway, and
  `scripts/deploy/cutover-to-gateway.sh` moves a running cluster from ingress-nginx to the
  gateway (with `--dry-run` and `--rollback`).
- `crawler migrate [--dry-run]` applies the pending data migrations to `DATA_DATABASE_URL` and
  prints each version applied; `--dry-run` prints the pending versions and changes nothing. It
  never applies or records an app migration.
- `scripts/deploy/deploy.sh` runs `crawler migrate` as the Kubernetes Job `crawler-migrate`
  (`k8s/manifests/crawler-migrate-job.yaml`) before it applies the backend and crawler-worker,
  and stops if the Job fails. It skips the Job in a namespace whose `econ-graph-secrets` has no
  `data-database-url`. `deploy.sh --dry-run` prints the deploy's commands and applies nothing.
- A backend with no `DATA_DATABASE_URL` checks a new annotation's series by asking the data
  plane's GraphQL endpoint at `DATA_GRAPHQL_URL`, with no token, sending `DATA_GRAPHQL_HOST` as
  `Host` and giving up after `DATA_GRAPHQL_TIMEOUT_MS` (default 2000). An unknown series is
  refused with "Series not found" as before. If the data plane can't be reached, times out or
  doesn't answer cleanly, nothing is written and `createAnnotation` fails with
  `extensions.code = DATA_PLANE_UNAVAILABLE` ("Couldn't check the series right now; try again");
  the annotation form stays open with what was typed and shows that message. With neither
  `DATA_DATABASE_URL` nor `DATA_GRAPHQL_URL`, the check is skipped and the backend logs that once
  at startup.
- GraphQL query field `seriesExists(id: ID!): Boolean!`: whether a series exists, counting a
  series with no observations (which `series(id)` returns as null). The remote annotation check
  above asks it, so both checks accept the same series.
- `scripts/deploy/install-kind.sh` installs kind v0.33.0 for the current user (no sudo) under
  `~/.local/share/econgraph/bin`, checked against a pinned SHA-256. The deploy scripts and
  `terraform/k8s` run that kind instead of the one on `PATH`, and stop with a pointer to the
  installer when it is missing; only read-only `kind get` falls back to the `kind` on `PATH`.
  Older kind cannot load images into Kubernetes 1.36 nodes.

### Changed

- The kind cluster's HTTP entry point is a Gateway API gateway (Envoy Gateway v1.9.2, Gateway
  API v1.6.3 standard channel) instead of ingress-nginx, which no longer gets security fixes.
  `deploy.sh` installs it with `scripts/deploy/install-gateway.sh` (pinned, checksummed release
  manifests; no Helm). The routes and paths are unchanged, except that `/playground` is no
  longer routed to the backend.
- The `/graphql` rate limit (20 requests a second per client address, bursts of 100) now
  answers 429 instead of 503 when exceeded, and counts IPv6 clients too.
- The client address is the TCP peer, or, for requests from a pod (the tunnel connector), the
  visitor in `X-Forwarded-For`, so the rate limits apply per visitor behind the tunnel. Every
  route sets `X-Forwarded-For` to that address, and a client's `X-Forwarded-Proto/-Host/-Port`,
  `Forwarded` and `X-Real-IP` are dropped.
- The tunnel's origin is the gateway:
  `https://econ-graph-gateway-proxy.envoy-gateway-system.svc.cluster.local:443` (No TLS
  Verify, as before).
- The frontend's security headers (CSP, HSTS, frame options) are sent on every frontend
  response through the gateway, on plain HTTP too, and the API responses get
  `X-Content-Type-Options`, `X-Frame-Options` and HSTS. The CSP no longer allows connections to
  `https://api.econ-graph.com`.
- The backend applies only app plane migrations at startup, and its `/readyz` gates readiness
  on those migrations. Service startup does not apply data plane migrations; run
  `crawler migrate` first on a new or older data database, or let `deploy.sh` run its migration
  Job.
- The data-only binaries (`crawler`, `crawler-worker`, `seed-fixtures`, `sec-crawler`,
  `sec-company-crawler`) read the data database from `DATA_DATABASE_URL`. They no longer read
  `DATABASE_URL`: with only `DATABASE_URL` set they exit at startup with `DATABASE_URL is no
  longer read; set APP_DATABASE_URL and/or DATA_DATABASE_URL`. `seed-fixtures` applies only the
  data migrations. The SEC crawlers no longer fall back to a local default database.
- The Kubernetes Secret `econ-graph-secrets` holds `app-database-url` and `data-database-url`
  alongside the compatibility key `database-url`, which matches `data-database-url`. The backend
  and crawler-worker manifests read the new keys. New Secrets use separate app and data
  databases; existing Secrets preserve their app database setting unless `APP_DATABASE` is
  supplied. See [database settings](k8s/README.md#databases).
- The backend reads `APP_DATABASE_URL` (required) and `DATA_DATABASE_URL` (optional) instead of
  `DATABASE_URL`, and exits at startup with the same message when only `DATABASE_URL` is set.
  Users, annotations, comments, collaborators, audit logs and security events use the app
  database; series, observations, sources, datasets, search and the crawl queue use the data
  database. Both may be the same database.
- The backend does not apply data plane migrations; operators apply them with
  `crawler migrate` (the `crawler-migrate` Job in Kubernetes, the `crawler-migrate` service in
  docker-compose). `/readyz` fails only when the app database is unreachable or its migrations
  are pending, each database check bounded on its own. It reports the data database as `ok`,
  `unreachable`, `pending_migrations` or `not_configured` in its body without failing, so a data
  outage does not take sign-in down.
- When `APP_DATABASE_URL` and `DATA_DATABASE_URL` are the same, the backend opens one pool for
  both. `APP_DB_POOL_SIZE` and `DATA_DB_POOL_SIZE` (default 10 each) size the pools; a shared
  pool gets their sum, so a backend opens at most 20 connections by default either way.
- With no `DATA_DATABASE_URL`, app operations work as before and every data field fails with
  `extensions.code = DATA_SERVED_REMOTELY` (`Data is served by the data plane at /data/graphql`);
  `createAnnotation` checks its series through `DATA_GRAPHQL_URL` (see Added),
  `systemHealth`'s `queueItems` is null, and `triggerCrawl` is refused but still audited
  (outcome `unavailable`). `queueItems` is nullable in the schema for this.
- docker-compose runs `crawler migrate` as a one-shot `crawler-migrate` service that the
  backends wait for.
- The backend pod no longer gets `DATABASE_URL`.

- New kind clusters (`recreate-kind-cluster.sh`, `terraform/k8s`) run Kubernetes 1.36
  (`kindest/node:v1.36.4`, pinned by digest in `kind-config.yaml`), which needs kind v0.33.0 or
  newer. cert-manager is v1.21.3.

### Removed

- ingress-nginx, its Ingress manifests (`ingress.yaml`, `graphql-ingress.yaml`,
  `keycloak/ingress.yaml`, `ssl-ingress.yaml`, `frontend-ingress.yaml`,
  `ingress-cloudflare-dns01.yaml`), the unused `security-configmap.yaml`, the Terraform
  `ingress` and `admin-ingress` modules and the kind ingress-nginx installers.

### Fixed

- Observation values are stored exactly, as unconstrained `NUMERIC`: data migration
  `20261025000100`, which `crawler migrate` applies (ECO-501).
- `searchSeries`'s `totalCount` is the number of series that match the query and filters, the
  same on every page, instead of the length of the page returned (ECO-508).
- A `source` on `searchSeries`, or a `filter.sourceId` on `seriesList`, that isn't a UUID
  (including an empty string) fails with a `BAD_USER_INPUT` error naming the argument, instead
  of being ignored and returning every source's series (ECO-509).

## [4.0.1] - 2026-10-30

Sign-in on https://econgraph.com and the privacy policy on phones now work, the explorer
follows the header search and other links to another `/explore` URL, and the deploy scripts
reach only the kind cluster.

### Changed

- The deploy scripts never fall back to a plain `kubectl`. Without `KUBECTL` they use the kind
  cluster's kubectl kept in `~/.cache/econgraph`, with an exported `KUBECONFIG` or else the kind
  kubeconfig kept there, and say which; when a copy is missing they stop and say how to get it.
  `deploy.sh`, `teardown.sh`, `recreate-kind-cluster.sh`, `apply-host-storage.sh` and
  `build-images.sh` run `kind` and `docker` on the rootful Docker daemon
  (`unix:///var/run/docker.sock`, or `ECONGRAPH_DOCKER_HOST`) whatever `DOCKER_HOST` is set to
  (ECO-702).
- `scripts/deploy/recreate-kind-cluster.sh` shows the cluster (nodes, `kube-system` UID and
  production marker) and asks before deleting it: the cluster name typed at a terminal, or
  `--yes`. A cluster with a production marker whose role is anything but `none`, or whose
  marker can't be read, needs the first 8 characters of its `kube-system` UID, typed or given as
  `--confirm-production`. It never writes `~/.kube/config` or switches its current context, and
  gains `--if-missing` and `--dry-run` (ECO-719, ECO-721).
- `scripts/release/tag.sh` tags patch releases: for `X.Y.Z` with `Z` greater than 0 it requires
  `train-N` on origin with its commit an ancestor of `HEAD` and the same as `vX.Y.0`'s, and
  pushes only `vX.Y.Z`.

### Removed

- The navigation sidebar's FRED Data item. Data Sources lists all six sources, each with a
  Browse Series link (ECO-751).

### Fixed

- Sign-in shows on https://econgraph.com. Release and CI frontend images were built with an
  empty OpenID Connect issuer, which hides sign-in; image builds now default it to
  `/idp/realms/econ-graph` (a `VITE_OIDC_ISSUER` repository variable or build argument still
  overrides it), and an image build with an empty issuer fails unless
  `VITE_OIDC_SIGN_IN_DISABLED=true` is also passed (ECO-728).
- The explorer follows a link to another `/explore` URL: the header search from a source's
  listing shows that search with the Source filter cleared, and back and forward restore each
  entry's search, source and frequency (ECO-511).
- The navigation drawer has a Privacy Policy item, so the privacy policy can be reached on a
  phone, where the header hides its Privacy button (ECO-730).

### Security

- admin-frontend's seroval moves from 1.6.8 to 1.7.5 (GHSA-f86p-h4j2-7qgj, GHSA-9wx9-5pv9-3349),
  the backend's zstd to 0.14.3 (RUSTSEC-2026-0391, RUSTSEC-2026-0398, RUSTSEC-2026-0403,
  RUSTSEC-2026-0410) and the frontend's qs to 6.16.2 (GHSA-6pq8-jhm2-2vcw).

## [4.0.0] - 2026-10-27

Real data from six public sources behind a site whose pages work, served publicly at
https://econgraph.com. A database created from a tag before v4.0.0 can't be upgraded in place
(see Changed).

### Added

- Real data end to end for the core loop: search a series, open it and read a correct, current
  chart, with real revisions and vintages (FRED/ALFRED and World Bank WDI).
- Six data sources crawl real data: FRED, BLS (CPI, CES and LAUS), Census BDS, FHFA HPI (rebuilt
  on the published master CSV), BEA (real NIPA table and line IDs) and World Bank WDI (about 50
  curated indicators).
- A world map showing the latest value of a chosen indicator for every country the source
  reports it for.
- Sign-in through Keycloak, with public and private annotations on a series chart.
- A real dashboard and data-sources page, replacing hard-coded sample values. The Data Sources
  page shows each source's real series count, and browsing a source with no search text lists
  its series.
- Datasets metadata (country, indicator and table dimensions) for the new multi-dimensional
  sources.
- Labels and code lists that a source publishes (BLS, BEA, Census, FHFA and World Bank) are
  fetched by the crawl and stored in the database. Refreshes are conditional (ETag,
  Last-Modified or an unchanged body), and a recorded copy can be seeded into a new database.
- Pages fit a phone screen: the navigation drawer starts closed and the world map's controls sit
  below the map.
- The About page shows the version the site was built from.
- Admin actions (triggering a crawl, updating, suspending, activating or deleting a user) are
  written to the audit log with their outcome, and refused permissions and requests from
  suspended accounts are recorded as security events; the admin `auditLogs` and
  `securityEvents` queries return them. A deleted user's audit history is kept.
- A forced full-history re-fetch: `crawler refetch (--source S,... | --all)` queues a complete
  re-download of every series it selects, ignoring the stored "nothing changed" validators, and
  corrects stored values in place without deleting anything; `crawler refetch-status` reports
  progress. `scripts/deploy/refetch-full-history.sh` runs both in the crawler pod of the kind
  cluster and prints resume and progress commands. `scripts/deploy/precheck-db-migration.sh`
  can be run just before a deploy that migrates the data tables (ECO-501).
- Production at https://econgraph.com, served through a Cloudflare Tunnel: an in-cluster
  `cloudflared` connector dials out to Cloudflare, so the host opens no port for it.
  `create-secrets.sh --prompt-tunnel-token` stores the tunnel token (ECO-457).
- A production marker: `scripts/deploy/mark-production-cluster.sh` marks one cluster as
  production (bound to its `kube-system` namespace UID, confirmed by typing the UID's first
  8 characters; `--unmark` scales the connector down first). The tunnel connector and its
  token run only on the marked cluster, so a test cluster given the token can't silently serve
  part of the real site.
- Release image scripts for the kind cluster (ECO-525):
  - `scripts/deploy/load-release-images.sh --tag v4.0.0-rc.N` pulls the tag's images from GHCR
    by digest, checks their digest and source commit, keeps the images now running as pinned
    `pre-<tag>` copies on every node, and loads the new ones.
  - `scripts/deploy/verify-release-images.sh` restarts the Deployments after `deploy.sh` and
    fails unless every pod runs the loaded image.
  - `scripts/deploy/rollback-release-images.sh --to pre-<tag>` puts the kept images back and
    checks the pods.
- Nightly backups of both databases: the `postgres-backup` CronJob dumps `econ_graph` and,
  new, Keycloak's database (user accounts and linked identities) as
  `keycloak-<stamp>.dump`, checks each dump is readable and has tables, and fails the Job if
  either dump fails after shipping the ones that succeeded (ECO-513). A cluster without
  Keycloak sets `KEYCLOAK_REQUIRED=false`.
- `scripts/deploy/backup-keycloak-db.sh` takes a verified Keycloak dump (restored into a scratch
  database and row-counted) under `~/econgraph-backups/keycloak/`, and
  `scripts/deploy/restore-keycloak-db.sh` restores one into a fresh database and swaps it in,
  keeping the previous database. Run the backup before deploying a Keycloak upgrade.
- Host-storage volumes: both Postgres StatefulSets keep their data on host directories under
  `/srv/econgraph` through the StorageClass `econgraph-host` and two static PersistentVolumes
  (`k8s/manifests/host-storage.yaml`), so deleting or recreating the kind cluster leaves the
  data alone. The node needs the label `econgraph.io/host-storage=true`; see
  [host storage](docs/deployment/HOST_STORAGE.md) (ECO-519).

### Changed

- The ten static development-only catalogs (BOC, BOE, BOJ, ECB, ILO, OECD, RBA, SNB, UN Stats,
  WTO) and IMF discovery are excluded from release and production builds; search in those
  builds never returns a series that can't have data.
- Copyright-restricted FRED series are left out of discovery and refused when fetched, including
  through an admin-triggered crawl.
- The database schema is one v4.0.0 baseline migration, plus the migrations listed here that
  run after it. Databases created from earlier tags can't be upgraded in place and are rebuilt
  (see [database migrations](docs/development/MIGRATIONS.md)).
- Observation values are stored as unconstrained `NUMERIC` instead of `DECIMAL(20,6)`, and are
  parsed, served and exported exactly as the source publishes them. Large World Bank aggregates
  that failed with "numeric field overflow" now load, values are no longer rounded to six
  decimal places, and the series CSV download writes the exact decimal text. Values stored
  rounded before this release are corrected by the full-history re-fetch above. The migration
  briefly locks the data tables and does not rewrite them (ECO-501).
- FRED revision history is fetched in real-time windows of at most 1,000 vintage dates. Daily
  series with long revision histories (DFF, DGS10, EFFR and others), which FRED refused when
  asked for in one request, now get data (ECO-498).
- Keycloak is upgraded to 26.8.1 (26.7 reached end of life). Keycloak migrates its database on
  first start and the older version can't run on the result, so the Deployment uses a
  Recreate rollout, `deploy.sh` waits for Keycloak before importing the realm, and the way back
  is the restore script above, not `kubectl rollout undo` (ECO-472).
- Restoring an `econ_graph` backup leaves every object owned by `econgraph`
  (`RESTORE_OWNER`), so the backend and crawler can read and write it; before, a restore left
  the tables owned by `postgres` (ECO-523).
- Backups are never deleted automatically; the nightly job no longer removes dumps older than
  14 days, and old dumps are removed by hand.
- `scripts/deploy/teardown.sh` runs every call against the kind cluster's context, checks that
  it is the cluster `kind` would delete, shows it and asks once before any change; a cluster
  marked production needs its `kube-system` UID prefix (ECO-524). The mark script,
  `create-secrets.sh`, `refetch-full-history.sh` and the restore scripts likewise use the kind
  cluster's kubectl and context by default and show which cluster they act on, so a host whose
  plain `kubectl` reaches another cluster can't be changed by mistake.
- The project's contact address is support@econgraph.com, in the crawlers' User-Agent
  (`EconGraph/<version> (+https://econgraph.com; support@econgraph.com)`), the Let's Encrypt
  account, the licence and the privacy and security documents. The User-Agent links
  econgraph.com rather than GitHub, which download.bls.gov refuses (ECO-456).
- The monitoring images (Grafana, Loki, Prometheus, Promtail) are pinned by version and digest
  instead of `:latest`, and nginx, busybox, aws-cli and cloudflared move to current releases.
- The backend's Kubernetes probes use `/livez` (the process is up) and `/readyz` (the database
  is reachable and its schema is current) instead of `/health`.
- Search finds series by common abbreviations, ranks an exact series id first, and treats a
  quoted or punctuated one-word search (`"gdp"`, `GDP.`) as the word itself.
- A chart offers only the transformations its series supports, and series reads transform the
  latest revisions by default.
- GraphQL schema changes an API client must handle: `AuditLogType.userId` is now nullable (was
  `ID!`) and is null once the user is deleted, while `userName` is kept and the new
  `userDeleted: Boolean!` field says so; and `SeriesFrequency` has a new `SEMI_ANNUAL` value.
- Annotations are refused on an unknown series or an out-of-range date or value, annotation and
  comment length is capped. An annotation create that carries a client-chosen
  `idempotencyKey` stores one annotation however often it is retried.
- The explorer's results grid is even, and its CSV and JSON exports include each series'
  source id. Signing out returns to the page you were on.
- An admin can't suspend or delete their own account, so the last active admin can't be
  removed. A suspended user sees a message and can still read public pages.
- Scheduled re-crawls skip unchanged FHFA, FRED and World Bank data.
- BEA's regional table SAGDP2N is tracked under its new name SAGDP2 (series ids change from
  `bea_regional/SAGDP2N.*` to `bea_regional/SAGDP2.*`), and a curated table BEA no longer lists
  no longer stops BEA discovery.

### Fixed

- PERCENT_CHANGE no longer blanks a series whose first value is null or zero.
- The search frequency filter matches each source's own frequency text.
- World Bank aggregates are named from the World Bank's own country list.
- The `CrawlerCuratedEntriesMissing` alert no longer resolves when a crawler worker restarts.

### Security

- Database and other server-side error details are no longer sent to GraphQL clients; they
  get "Internal server error".
- Oversize `/graphql` request bodies are refused, and a malformed body gets a 400 JSON error.
- Grafana is no longer reachable through the public ingress, and only `/graphql`, `/api`,
  `/auth`, `/playground` and `/health` reach the backend through it, so `/metrics` is not
  public.
- The admin crawler status and queue statistics queries require `admin.system:read`.
- rustls, rustls-webpki and webpki-root-certs are updated past the October 2026 advisories,
  and the frontend and admin-frontend npm dependencies past their open advisories.

### Removed

- `/analysis`, `ProfessionalChart` and `ChartCollaboration`, which ran on mock data and a
  made-up chart ID.
- `imf.rs` and other mock/hard-coded data paths (fake correlations, the dashboard's hard-coded
  indicator cards, `SeriesDetail`'s generated mock points).
- `create-secrets.sh` no longer generates the secrets of the retired in-house login.
