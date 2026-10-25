# Changelog

All notable changes to EconGraph are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Releases are tagged `vX.Y.Z`; see
[the release process](docs/development/RELEASE_PROCESS.md) for how a train reaches a tag.

## [Unreleased]

### Added

- The frontend can send data operations (series, search, data sources, the dashboard cards and
  the world map) to a separate GraphQL URL, `VITE_DATA_GRAPHQL_URL`, and never sends them a
  sign-in token. Annotations, comments and admin operations still go to `/graphql` with the
  token. Unset, every request goes where it went before. The dev server and `vite preview`
  proxy `/data/graphql` to `DATA_BACKEND_URL` (default `BACKEND_URL`).
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

- The backend applies only the app migrations at startup, and its `/readyz` checks only the app
  migrations. No service applies the data migrations at startup, so redeploying one never
  changes a data database; run `crawler migrate` first on a new or older database.

- The data-only binaries (`crawler`, `crawler-worker`, `seed-fixtures`, `sec-crawler`,
  `sec-company-crawler`) read the data database from `DATA_DATABASE_URL`. They no longer read
  `DATABASE_URL`: with only `DATABASE_URL` set they exit at startup with `DATABASE_URL is no
  longer read; set APP_DATABASE_URL and/or DATA_DATABASE_URL`. `seed-fixtures` applies only the
  data migrations. The SEC crawlers no longer fall back to a local default database.
- The Kubernetes Secret `econ-graph-secrets` holds `app-database-url` and `data-database-url`
  next to `database-url`, all three naming the one database. The backend and crawler-worker
  manifests read the new keys.
- The backend reads `APP_DATABASE_URL` (required) and `DATA_DATABASE_URL` (optional) instead of
  `DATABASE_URL`, and exits at startup with the same message when only `DATABASE_URL` is set.
  Users, annotations, comments, collaborators, audit logs and security events use the app
  database; series, observations, sources, datasets, search and the crawl queue use the data
  database. Both may be the same database.
- The backend never migrates the data database; operators apply the data set with
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

### Fixed

- `searchSeries`'s `totalCount` is the number of series that match the query and filters, the
  same on every page, instead of the length of the page returned (ECO-508).
- A `source` on `searchSeries`, or a `filter.sourceId` on `seriesList`, that isn't a UUID
  (including an empty string) fails with a `BAD_USER_INPUT` error naming the argument, instead
  of being ignored and returning every source's series (ECO-509).

## [4.0.0] - 2026-10-08

Train 1: broad real data behind pages that work. See
[the train 1 roadmap](https://linear.app/econgraph/document/release-trains-design-record-from-docsroadmapreleasesmd-9cdc19709b73) for the full scope and what was deliberately
left for later trains.

### Added

- Real data end to end for the core loop: search a series, open it and read a correct, current
  chart, with real revisions and vintages (FRED/ALFRED and World Bank WDI).
- Six data sources crawl real data: FRED, BLS (CPI, CES and LAUS), Census BDS, FHFA HPI (rebuilt
  on the published master CSV), BEA (real NIPA table and line IDs) and World Bank WDI (about 50
  curated indicators).
- A world map showing the latest value of a chosen indicator for every country the source
  reports it for.
- Sign-in through Keycloak, with public and private annotations on a series chart.
- A real dashboard and data-sources page, replacing hard-coded sample values.
- Datasets metadata (country, indicator and table dimensions) for the new multi-dimensional
  sources.
- Labels and code lists that a source publishes (BLS, BEA, Census, FHFA and World Bank) are
  fetched by the crawl and stored in the database. Refreshes are conditional (ETag,
  Last-Modified or an unchanged body), and a recorded copy can be seeded into a new database.
- Pages fit a phone screen: the navigation drawer starts closed and the world map's controls sit
  below the map.

### Changed

- The ten static development-only catalogs (BOC, BOE, BOJ, ECB, ILO, OECD, RBA, SNB, UN Stats,
  WTO) and IMF discovery are excluded from release and production builds; search in those
  builds never returns a series that can't have data.
- Copyright-restricted FRED series are left out of discovery and refused when fetched, including
  through an admin-triggered crawl.
- The database schema is one v4.0.0 baseline migration. Databases created from earlier tags
  can't be upgraded in place and are rebuilt (see
  [database migrations](docs/development/MIGRATIONS.md)).

### Removed

- `/analysis`, `ProfessionalChart` and `ChartCollaboration`, which ran on mock data and a
  made-up chart ID.
- `imf.rs` and other mock/hard-coded data paths (fake correlations, the dashboard's hard-coded
  indicator cards, `SeriesDetail`'s generated mock points).
