# Roadmap: federation (data plane and app plane; Iceberg + Arrow Flight time series)

Status: proposed (2026-09-26). Supersedes the design in the unmerged federation PRs #124/#125/#129.

## Goal

Federation here means two things:

1. **Split development.** Work on new user-facing features runs against a fresh local user
   database while reading real crawled data from a shared data service. Nobody recrawls
   anything to work on the UI, auth, plans or admin.
2. **Time series storage that scales.** Series metadata stays in Postgres; the
   observations themselves move out of Postgres into Apache Iceberg tables (Parquet files) on object storage,
   served over Arrow Flight.

Hard requirements:

- **Observation rows never touch Postgres**, not even as a staging or ingest buffer. There
  are too many rows. Postgres holds only metadata and pointers to files.
- **Writes stay transactional.** A crawl either becomes fully visible (its observations,
  the series metadata, the crawl job's completion) or not at all, and readers never see a
  half-written state.

## Two planes

The system splits into a **data plane** (everything the crawlers produce) and an **app
plane** (everything users produce). Each has its own Postgres database and migrations.

| Data plane (shared) | App plane (per environment, can be local) |
|---|---|
| `data_sources`, `economic_series`, `series_metadata` (discovery catalog) | Users, sessions, auth, plans, teams, permissions |
| Crawl queue and crawl attempts | Annotations, chart annotations, collaboration, and annotations on filings (`financial_annotations`, `annotation_assignments`, replies, templates) |
| The Iceberg catalog, and the Iceberg tables (Parquet files) on object storage | Per-user data source preferences |
| SEC/XBRL data (`companies`, `financial_statements`, `financial_line_items`, taxonomy tables): rich, relational, and staying in Postgres | Admin UI state and audit logs |
| Later, other observation tables such as `global_indicator_data` (to Parquet) | |

**Why series metadata sits in the data plane.** A crawl commits series metadata, new
Parquet files and the crawl job's completion in one Postgres transaction. Putting metadata
in the app plane would turn that into a commit across two databases. The app plane only
needs to refer to series and sources by stable ID.

**Cross-plane references.** The app plane stores data plane IDs without foreign keys.
Today that is already true for charts: `chart_annotations.series_id` is a plain `VARCHAR`
with no foreign key. The foreign keys that cross the line, and get dropped, are:

- `user_data_source_preferences.data_source_id` to `data_sources`;
- `financial_annotations.statement_id` and `.line_item_id` to `financial_statements` and
  `financial_line_items`;
- `annotation_assignments.statement_id` and `.line_item_id`, the same two tables.

In exchange:

- Series, source, statement and line item IDs are never deleted or reused; retired series
  are marked inactive, and a re-processed filing keeps its statement and line item IDs.
- Joins across the line become batched ID lookups in the app plane (the existing GraphQL
  dataloaders), for queries like "my annotated series" or "search, favorites first".

**One API for the frontend.** Each plane serves its own GraphQL subgraph (async-graphql
supports federation entities), and a gateway composes them into one supergraph. The app
subgraph extends the data plane's `Series` type with user fields (annotations, favorites).
The alternative, the app backend proxying data plane queries, avoids running a gateway but
duplicates the data API in the app backend. Arrow Flight is served by the data plane
directly.

**Auth across the line** is specified in the auth roadmap (`auth-plans-permissions.md`,
section "Multiple services and split development"). In short: user tokens carry both the
`econ-graph-app` and `econ-graph-data` audiences, and the app plane forwards the user's
token so the data plane enforces plan roles itself, which makes the API between the planes
also the paywall. Crawlers and other background jobs use their own client-credentials
client, and local and feature builds use a `dev-readonly` client with read-only data
roles.

**Development setups.**

| Setup | App plane | Data plane |
|---|---|---|
| New feature work | Fresh local Postgres, migrations only | Shared staging data plane, `dev-readonly` client |
| Offline | Fresh local Postgres | Local sample: a small catalog dump plus a few Parquet files |
| Crawler work | Any | Local data plane with its own crawl |

The staging data plane is either its own crawl or a copy of production's: Parquet files are
immutable, so copying is bucket replication plus a dump of the catalog database, with no
recrawl.

Development code never holds database or bucket credentials for the staging or production
data plane. It reaches data only through the data plane's API (GraphQL and Flight), with a
`dev-readonly` token, so the data plane stays a separate, hardened service that enforces its
own security.

## Where things stand today (main at `1f17cc8`)

- All observations live in one Postgres table, `data_points`
  (`backend/crates/econ-graph-core/src/schema.rs`), with one row per
  `(series_id, date, revision_date)` and `value numeric(20,6)`.
- Writes: the crawler upserts series and points in one transaction
  (`econ-graph-crawler/src/persist.rs`, `persist_series`).
- Reads go straight to Diesel from three places, with no shared abstraction:
  - `econ-graph-services/src/services/series_service.rs` (`get_series_data`, transforms)
  - `econ-graph-graphql/src/graphql/dataloaders.rs` (`DataPointsBySeriesBatcher`,
    `DataPointCountBatcher`)
  - `econ-graph-mcp/src/mcp_server.rs` (through the series service)
- The HTTP stack is warp 0.3 (hyper 0.14). There is no tonic, arrow, parquet or object
  storage anywhere in the workspace, and no object store in `terraform/` or `k8s/`.
- Postgres 18 is the floor (#159, merged).
- Draft PR #163 originally turned revisions into non-overlapping vintages inside
  `data_points` (a `superseded_on` column, a PG 18 `WITHOUT OVERLAPS` constraint and
  triggers). That schema is for a table this plan removes, so the API part moved to #184,
  against today's columns with no migration: an `asOf` filter
  (`DataQueryParams.as_of`, GraphQL `DataFilter.asOf`), `latestRevisionOnly` done in SQL,
  and a data points dataloader that returns current values only. That API is what the
  Iceberg store implements later (see phase 0).

## What the earlier attempts left

#124 was merged and reverted 31 seconds later (`c9b588b`, no reason recorded; it also carried
unrelated Postgres 18 and frontend changes). #125 re-opened the same branch, and #129 is
stacked on it. Findings from reading all three:

- **No Arrow Flight was ever built.** `ParquetFlightService` is wrapped in a
  `FlightServiceServer` but never implements `FlightService` (no `do_get`/`do_put`) and never
  binds a port.
- **No Iceberg was ever built.** `IcebergStorage` is a stub in #124/#125; in #129 it is
  custom Hive-style date partitioning on the local filesystem under an Iceberg name. The
  docs there concluded iceberg-rust was not ready (memory catalog only, weak time
  partitioning) and recommended custom partitioning first.
- **No link to Postgres.** The design moved series metadata into Parquet plus a JSON
  catalog, which is the opposite of the current goal.
- **It does not apply to main.** The branches predate crawler consolidation (#157), the
  dependency upgrade and the removal of `backend/src` (#174), edit files that no longer
  exist, delete ~620 lines of dataloaders to compile, and add axum 0.8 + tonic 0.13 next to
  warp 0.3 (two hyper majors). arrow 56 is several majors old.

Worth keeping as reference when rewriting:

- The Arrow schema and RecordBatch to Parquet round trip in #124's
  `econ-graph-financial-data/src/storage/parquet_storage.rs`.
- The storage trait shape (`FinancialDataStorage`: write/read points by date range).
- #129's partition path logic (`storage/iceberg_storage.rs`).
- The design docs under `docs/projects/backend-federation/` on those branches, as background.

Recommendation: close #125 and #129 as superseded by this roadmap once it is accepted, and
rewrite the useful pieces against current crates instead of rebasing them.

## Design

### File format: Parquet at rest, Arrow in memory and on the wire

Parquet and Arrow are complementary, not alternatives. Data files are **Parquet**:
columnar, heavily compressed, with row group and page statistics so a read skips everything
but the series and dates asked for; it is Iceberg's default file format and every engine
reads it (DuckDB, Polars, pandas, Spark, DataFusion). Parquet decodes into **Arrow**, which
is what DataFusion computes on and what Flight sends. Formats considered and not chosen:
Arrow IPC (Feather) files load fast but compress worse and prune poorly, so they are a cache
format, not storage; ORC has weak Rust and Iceberg-Rust support; Lance targets ML and vector
search with random access and is younger; Vortex is still experimental.

### Table format: Apache Iceberg

Observations live in Apache Iceberg tables: Parquet data files plus Iceberg's metadata
(snapshots, manifests, schema and partition specs). Iceberg supplies what a hand-rolled
catalog would otherwise have to build and get right: atomic commits, snapshot isolation
for readers, time travel, snapshot expiry, orphan file cleanup, and compaction of small
files. The catalog is an Iceberg **SQL catalog stored in the data plane's Postgres**, so
there is still exactly one database holding metadata, and other engines (PyIceberg,
DuckDB, Spark, Trino) can read the same tables for analysis and maintenance.

### Data model: datasets and series

Series are not all alike: some are one value per date, some carry several measures per
date (Census BDS has firms, establishments, job creation and more for the same
state-year), and some carry a value plus a flag (BLS footnote codes, preliminary or
estimated markers, which the crawler drops today). So the unit of storage is the
**dataset**: a group of related series that share a schema, following the SDMX model
that IMF, OECD, ECB, Eurostat and the World Bank publish in natively:

- **Dimensions** identify a series within the dataset (for Census BDS by state: `state`).
- **Measures** are the observed values (one column for FRED-style series; several for
  Census BDS).
- **Attributes** are per-observation flags and notes (footnote codes, status markers).

Each dataset is one Iceberg table whose schema is its dimensions, then `date` and
`revision_date`, then its measures and attributes. Every table also has a `series_id`
column: a series is one combination of dimension values, and gets a UUID in
`economic_series` like today. A plain single-value dataset has no dimensions besides that,
and its schema is:

| Column | Iceberg type | Notes |
|---|---|---|
| `series_id` | `uuid` | Refers to Postgres `economic_series.id`, by convention |
| `date` | `date` | Observation date |
| `revision_date` | `date` | Start of the vintage |
| `value` | `decimal(20, 6)`, optional | Matches `numeric(20,6)`; see open question 1 |

Census BDS by state would instead be `series_id, state, date, revision_date, firms,
estabs, job_creation, ...`, with one series per state, so partitioning by series is
partitioning by state. Datasets change schema through Iceberg schema evolution (adding a
measure or attribute is a metadata-only change). The set of tables is expected to be tens,
not thousands.

**Wide or long.** Wide rows do not make Parquet less columnar: each column is its own chunk
per row group, so a query reads only the measures it names, and mostly empty attributes
compress to almost nothing. The costs of width scale with the column count: every column
adds footer entries and statistics to each file (noticeable next to small per-series
files), per-column statistics in Iceberg manifests (which can be limited per column), and
schema changes when measures are added. So a dataset stays wide when it has a handful to a
few dozen measures that are published together, like Census BDS; one with hundreds of
variables, or one that keeps adding them, is stored long instead, with the measure name as
a dimension and a single value column, so each variable is its own series and the schema
stays fixed.

**Metadata in Postgres.** A new `datasets` table records each dataset: its source, its
Iceberg table, and its dimensions, measures (with units) and attributes, with types.
`economic_series` gains `dataset_id`, the series' dimension values (instead of encoding
them into external ids, as `CENSUS_BDS_{VARIABLE}_state_{FIPS}` does today), and a
default measure, which is what a chart plots when the user has not picked one.

**Crawler.** Source adapters declare the datasets they write and return observations as
rows in that dataset's schema (Arrow record batches) instead of today's single-value
`FetchedPoint` (`econ-graph-crawler/src/adapter.rs`). Discovery registers series with
their dimension values. Adapters that parse flags keep them as attributes instead of
discarding them.

**Vintages.** All tables use the #163 vintage semantics but are stored append-only.
There is deliberately no `superseded_on` column. A vintage ends where the next
`revision_date` for the same `(series_id, date)` begins, so the reader derives it
(`LEAD(revision_date)` over the sorted rows). A revision is therefore always an append,
never an update, and vintages cannot overlap by construction. The writer enforces
uniqueness of `(series_id, date, revision_date)`: a re-crawl that returns an already stored
vintage with identical measures and attributes writes nothing, and one that differs is a
correction, written with an Iceberg row-level delete plus the new row. "Latest" is the last
vintage per `(series_id, date)`, and `asOf X` is the last vintage with
`revision_date <= X`.

**Each series is its own partition.** Every table is partitioned by `identity(series_id)`
and sorted by `(date, revision_date)`, so every data file holds exactly one series, in date
order. That matches how the data is used: a series is crawled on its own and read on its
own, even when several are joined. A crawl appends a file only to that series' partition;
a single-series read opens only that series' files, which is a single file after
compaction; a join across series is still one scan of one table, pruned to the partitions
named.

Layouts considered:

| Layout | Single-series read | Crawl writes | Cost |
|---|---|---|---|
| One table, `bucket(N, series_id)` | Reads shared files, pruned by statistics | Files mix many series | Rows from other series in each row group |
| **Table per dataset, `identity(series_id)`** (chosen) | Only that series' files | Touch only that series | Many small files: at least one per series |
| Table per series | Only that series' files | Touch only that series | Per-table overhead times the series count (below) |

Per-series partitions cost file count: a source with hundreds of thousands of series has at
least that many files, most a few kilobytes. Object stores handle that, and one small file
per series is the fastest possible single-series read. The pressure is on query planning,
which walks the manifests; maintenance keeps manifests clustered by series so planning
prunes them, and phase 3 measures planning time at full size. Splitting tables by dataset
gives each schema its own table, bounds each table's size, lets datasets be maintained and
compacted independently, and means crawlers for different datasets never contend on the
same commit.

**Why not a table per series.** Each table is cheap on its own, but the costs scale with the
series count. Every commit writes a new metadata file, manifest list and manifest, and with
a table per series every series crawl is its own commit: 800,000 series crawled daily would
be about 2.4 million extra metadata objects and 800,000 catalog updates a day, where a
dataset table batches thousands of series per commit. Snapshot expiry, compaction and
orphan cleanup run per table, so they become one job per series. A cold single-series read
fetches about three metadata objects before the data (mostly hidden by caching). A query
over many series reads the same data files either way, but across tables it plans one
table at a time, reads each series at its own latest snapshot rather than one consistent
point in time, and has to be written as a union of table names instead of a
`series_id IN (...)` filter. None of this breaks, so if the
tracked series count turns out to be in the tens of thousands, a table per series is a
reasonable alternative; open question 2 decides it.

### Transactions

Iceberg commits are optimistic: the writer writes data files, manifests and a new metadata
file, then atomically swaps the table's current metadata pointer in the catalog, failing if
someone else committed first. With a SQL catalog, that swap is an `UPDATE` of one row in the
data plane's Postgres.

To keep the crawl transactional end-to-end, the commit cannot go through iceberg-rust's
`Catalog::update_table`, which runs its own transaction. The data plane keeps the standard
SQL catalog schema (so other engines still read it) and adds its own commit operation that
takes the caller's Diesel connection inside an open transaction: iceberg-rust writes the
data files, manifests and new metadata file, and the operation only performs the pointer
swap on that connection. The summary updates and crawl-job completion take the same
connection (today `CrawlQueueItem::complete` takes a `&DatabasePool`, so it gains a
connection-taking variant). The `Catalog` trait implementation, used for reads and by
maintenance, shares the same tables. The crawl's commit is then one Postgres transaction that:

- swaps the dataset table's metadata pointer (compare-and-swap on the previous location);
- updates the touched series' summary columns in `economic_series` (start and end dates,
  last updated);
- marks the crawl queue job done.

If the swap loses a race, the transaction rolls back, the writer rebases its new files
onto the latest snapshot and retries; nothing becomes visible until the transaction
commits. Files written by a writer that crashed before committing are never referenced by
any snapshot, and Iceberg's orphan file cleanup removes them. Readers always see a whole
snapshot. Snapshot expiry keeps old snapshots longer than the longest allowed read, so no
running read loses its files.

This gives the same guarantee the crawler has today with `persist_series`: series metadata,
observations and the crawl queue commit together or not at all.

### Write path

Crawls append a day or more of observations per series at a time, so a single crawl adds
only a few rows to each series. Crawl workers therefore batch: a worker accumulates many
completed series of one dataset in memory and commits them as one Iceberg append (one new
file per touched series) in one transaction, bounded by a row count and a time limit.
Workers for the same dataset commit to the same table, so commits go through a single
committer task per dataset, or retry on conflict, which is cheap at this commit rate.

Each crawl adds a small file to each series it touched. Scheduled Iceberg maintenance
compacts a series' partition back into one sorted file once it has accumulated a few
appends, rewrites manifests so they stay clustered by series, and expires old snapshots. Compaction and deletes are the
iceberg-rust features to verify first (see phase 3); if either is missing, maintenance
runs in another engine (PyIceberg or Spark) against the same catalog.

### Read path

1. A `TimeSeriesStore` trait in `econ-graph-services` with
   `read(series_ids, start, end, revisions) -> Vec<(DatasetId, RecordBatchStream)>` (plus
   count), where
   `revisions` is `Latest` (today's `latestRevisionOnly`), `AsOf(date)` or `All`. Every reader
   (series service, dataloaders, MCP) goes through it. Datasets have different schemas, so
   a read covers one dataset: the store groups the requested IDs by `dataset_id` (from
   series metadata) and returns one stream per dataset, each with that dataset's schema.
   There is no normalized cross-dataset schema; callers that want one project the default
   measure themselves.
2. Implementations: `PostgresStore` (today's queries, kept only until the cutover in phase 5)
   and `IcebergStore` (DataFusion over the observation tables via `iceberg-datafusion`,
   with partition, sort and statistics pruning).
3. An Arrow Flight service (`econ-graph-flight`, its own binary) exposing `IcebergStore`:
   `do_get` with a ticket encoding `{series_ids, start, end, revisions}`, `get_flight_info`
   for planning. A `FlightInfo` carries one schema, so a ticket covers one dataset: a request
   whose IDs span datasets is rejected with the dataset of each ID, and the client (for
   example a small Python helper) issues one request per dataset. Tokens are validated as in
   the auth roadmap. Flight SQL is a later option, not a start.
4. GraphQL and MCP keep returning JSON; they call the store in-process. A series' data
   returns its default measure as `value`, as today, and can also return its other
   measures and attributes by name. Flight returns whole dataset rows.

**Who Arrow Flight is for.** The charts (Chart.js and D3 in the frontend and admin UI,
Chart.js in `chart-api-service`) get their data as JSON through Apollo GraphQL, and
browsers cannot speak Flight directly: it is gRPC and needs HTTP/2 trailers. Flight is
for joins and ML: Python (pyarrow, pandas, Polars, DuckDB), Spark and notebooks read whole
columnar result sets with no row-by-row conversion, through the same token and plan checks
as every other data plane API. Internal jobs can also read the Iceberg tables directly
with any Iceberg engine; Flight is the governed path for users and plan-limited access.
The browser always gets plain arrays: the data plane converts Arrow results to JSON arrays
for the GraphQL API, and the charts stay on Chart.js.

tonic needs hyper 1 and the backend is on warp 0.3 (hyper 0.14), so the Flight service
starts as a separate binary and port rather than sharing the backend's listener. That
also keeps the arrow/datafusion compile cost out of the main backend build until phase 4.

### Object storage

The `object_store` crate: local filesystem for development and tests, S3-compatible
(MinIO in k8s, or a cloud bucket) in deployment. Needs new terraform and k8s resources.

## Phases

Each phase is its own PR (or small stack), per project convention.

0. **Prerequisites.** Land #184, which fixes the `asOf` /
   `latestRevisionOnly` API shape before any storage changes. Decide open questions 1
   and 2.
1. **Read and write abstraction, no behavior change.** Add a `TimeSeriesStore` trait
   (`read`, `count`, `commit`) with the current Postgres implementation, and route the
   series service, dataloaders, MCP and `persist_series` through it.
2. **Split the planes.** Two databases and two migration sets; drop the cross-plane
   foreign keys; a data plane subgraph and an app plane subgraph behind a gateway; token
   validation and role checks in the data plane per the auth roadmap; a staging data plane and a local
   sample data plane. This is what enables feature work against a fresh user database,
   and it does not wait on Parquet.
3. **Datasets, Iceberg tables and catalog.** The `datasets` table and the new
   `economic_series` columns (dataset, dimension values, default measure); adapters
   returning rows in their dataset's schema. New crate `econ-graph-timeseries`: the
   per-dataset observation tables (schema, `identity(series_id)` partition spec, sort order), the
   Diesel-backed SQL
   catalog, and `object_store` config. First, confirm what iceberg-rust supports today for
   appends, row-level deletes, compaction and snapshot expiry, and pick where maintenance
   runs. Round-trip tests against local filesystem storage, and a single-series read
   latency check against the current Postgres query.
4. **Transactional commits and `IcebergStore`.** The one-transaction crawl commit (metadata
   swap, series summaries, crawl job), batched crawl commits, scheduled compaction and
   snapshot expiry, DataFusion reads, and a test suite that runs the same queries (range,
   latest, `asOf`, transforms) against both stores, plus tests for commit conflicts and
   crashed writers. Behind a config flag.
5. **Cut over and drop `data_points`.** Iceberg becomes the only store. There is no
   production data, so nothing is migrated; the table and its indexes are dropped.
6. **Arrow Flight service.** `econ-graph-flight` binary, `do_get` / `get_flight_info`,
   auth, metrics via `econ-graph-metrics`, k8s manifest, CI job.
7. **Later.** Flight SQL for ad hoc joins; bring `global_indicator_data` onto the same format; storage tiering.

## Open questions

1. **Value type.** Keep exact `Decimal128(20,6)` to match Postgres, or switch to `Float64`,
   which is smaller and much faster for charts and correlations? (#124 used Float64.)
2. **Scale target.** How many series and observations should this be designed for? It
   decides the file count per table, how much manifest clustering and caching query
   planning needs, and whether a table per series is affordable.
3. **Object store in deployment.** MinIO in the cluster, or a managed bucket?
4. **Gateway or proxy.** Compose the two GraphQL subgraphs with a federation gateway
   (proposed), or have the app backend proxy the data plane and skip the gateway?
