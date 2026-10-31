# Iceberg observation storage

The `econ-graph-timeseries` crate writes real Apache Iceberg v2 metadata, Avro manifests and
Parquet data files. The durable catalog is in the existing data PostgreSQL database. All
objects are staged before the metadata pointer changes; its compare-and-swap borrows the
caller's transaction. The crawler's existing lease lock, Postgres shadow, summaries, attempt
and queue completion therefore remain one atomic publication unit. A crash while preparing
files leaves unreachable objects, never a partially published table.

This is a feature-branch implementation. Production capacity approval, backfill/recovery,
independent object-store tests and full release checks remain release gates. The table layout
is provisional until those gates pass; no production cutover or destructive retirement follows
from creating the crate.

## Pinned capabilities and decisions

The dependency family is Apache `iceberg`/`iceberg-datafusion`/`iceberg-storage-opendal` 0.10.1,
DataFusion 53.1.0 and Arrow/Parquet 58.3.0. Rust 1.94 is the upstream minimum; the repository
uses Rust 1.99.1. These are the official Apache crates, separate from the similarly named
unofficial `iceberg-rust` package. Version compatibility comes from the
[Apache 0.10.1 manifests](https://github.com/apache/iceberg-rust/tree/v0.10.1).

| Capability | Verified implementation and consequence |
| --- | --- |
| Appends | Apache `Transaction::fast_append` creates actual snapshots/manifests. The crate writes one sorted data file group per series partition and stages a dataset batch before SQL publication. |
| Row deletes | Apache provides equality-delete writers and readers, but fast-append validation accepts data files only. No untested row-delete transaction is assumed. |
| Corrections | Existing IDs are retained. Physical append records carry `_commit_generation`; the DataFusion logical view selects the newest generation per observation UUID. This preserves in-place Postgres corrections while keeping prior snapshots readable. |
| Compaction | A bounded native replacement-snapshot path rewrites deduplicated logical rows with Apache manifest/metadata builders. It preserves prior snapshots/objects and publishes by the same SQL CAS. Repeated mirror appends amplify physical storage between compactions. |
| Snapshot expiry | The explicit maintenance path supplies both cutoff and retain-last to Apache `expire_snapshots`; it never invokes implicit age defaults or deletes objects. Old SQL checkpoint pointers and their metadata/object chains remain recoverable. History/backup retention still requires operating policy and recovery evidence. |

The relevant upstream implementations are
[transaction actions](https://github.com/apache/iceberg-rust/tree/v0.10.1/crates/iceberg/src/transaction),
[fast-append content validation](https://github.com/apache/iceberg-rust/blob/v0.10.1/crates/iceberg/src/transaction/snapshot.rs),
[delete writer](https://github.com/apache/iceberg-rust/blob/v0.10.1/crates/iceberg/src/writer/base_writer/equality_delete_writer.rs)
and [snapshot-expiry defaults](https://github.com/apache/iceberg-rust/blob/v0.10.1/crates/iceberg/src/transaction/expire_snapshots.rs).

Iceberg DECIMAL has a maximum precision of 38, per the
[Iceberg primitive-type specification](https://iceberg.apache.org/spec/#primitive-types).
Existing `Option<BigDecimal>` values can exceed that. The physical `value` field is therefore
nullable exact decimal text, never Float64 or a narrowed Decimal128. Generic SQL consumers
must not cast it to a fixed precision without proving their value range. Service transforms
continue using exact decimal values after decoding.

Each dataset has its own table, an identity partition on `series_id`, and declared sorting on
`(date, revision_date)`. The schema contains stable observation/series UUID text, dates, exact
value, original-release status, audit timestamps, optional attribute JSON text and one nullable
`dim_<name>` column per declared dataset dimension. Dataset metadata and decimal encoding are
recorded in table properties. Attributes are lossless JSON objects with nested nulls, ordered
arrays and unknown keys; null means historically unavailable rather than invented footnotes.

## Publication and reads

`SqlCatalog::ensure_dataset_conn(conn, dataset_id)` creates an empty table/catalog pointer in
the caller transaction. `prepare_append_conn(conn, commit_id, dataset_id, rows)` stages objects
using metadata visible on that same connection, including newly persisted series. For initial
mirroring this holds the worker lease lock during bounded object I/O, trading longer transactions
for safe use of the exact persisted UUIDs and rows. The runtime integration must bound batch size
and treat object-store errors as transaction failures.

`commit_prepared_conn(conn, &prepared)` publishes with a pointer/generation compare-and-swap
and durable commit receipt. A conflict rolls back the surrounding transaction. Reusing a commit
UUID with identical content is idempotent; reusing it for different content is rejected. The
out-of-transaction `prepare_append` helper is useful for recovery/backfill staging, while the
publication method never owns a pool transaction.

The upstream memory catalog is used only as a metadata staging builder: it registers the frozen
SQL pointer, generates a new Apache snapshot and writes immutable metadata. Its temporary pointer
is never authoritative or exposed to readers. The production catalog remains the Diesel-backed
SQL pointer and receipt tables.

`IcebergStore` implements the storage-neutral core `TimeSeriesStore`. A read captures the
series/dataset identity and SQL pointer in one repeatable-read transaction. DataFusion reads
that frozen metadata; count, page and transformation context are calculated from the same view.
Identity partitions are filtered before physical scans, and date bounds are applied after UUID
upsert deduplication. Revision/synthetic-vintage/default behavior matches the Train 3 contract.
Exact revision selection currently materializes the matching series range, so production scale
approval must include memory/latency and physical-version amplification measurements.

An existing series whose dataset is not migrated fails closed instead of silently reading a
second store. An unknown series returns an empty page. An initialized zero-row table returns an
empty page. PostgreSQL shadow retention is a deliberate rollback prerequisite, not permission to
drop observation history.

## Configuration and executable evidence

`ICEBERG_WAREHOUSE` is an absolute local path, a `file://` URL or an `s3://bucket/prefix` URL.
S3-compatible deployments use `S3_ENDPOINT`, `AWS_REGION`, `AWS_ACCESS_KEY_ID`,
`AWS_SECRET_ACCESS_KEY` and optional `AWS_SESSION_TOKEN`. Path-style access is enabled for the
self-hosted endpoint. Plain HTTP requires explicit `AWS_ALLOW_HTTP=true`. Configuration does not
implement Debug, and URL credentials/query strings are rejected rather than printed.

```sh
# Explicit disposable data database; normal unit runs report the integration test ignored.
DATA_DATABASE_URL=postgres://... cargo test --manifest-path backend/Cargo.toml \
  -p econ-graph-timeseries --test iceberg_contract -- --include-ignored

# Provision an empty database named eg_smoke_<this UUID without dashes> on the test server.
smoke_target="$(uuidgen)"
smoke_database="eg_smoke_${smoke_target//-/}"
DATA_DATABASE_URL="postgres://.../$smoke_database" \
  TIMESERIES_TARGET_ID="$smoke_target" ICEBERG_WAREHOUSE=/tmp/disposable-warehouse \
  cargo run --manifest-path backend/Cargo.toml -p econ-graph-timeseries \
  --bin timeseries -- smoke --disposable-target "$smoke_target" --migrate \
  --output /tmp/iceberg-smoke.json
```

The smoke command requires `--disposable-target <UUID>` before any database migration/write.
It must match explicit `TIMESERIES_TARGET_ID`, the PostgreSQL URL path and actual SQL database
name `eg_smoke_<UUID without dashes>`. There is no `DATABASE_URL` fallback. A fresh empty target
requires `--migrate`; initialized targets refuse pending migrations. A disposable recovery label
binds repeated runs to the same target, and the object warehouse gains the same UUID-scoped
subdirectory. Normal application environments and production-labelled recovery targets are
insufficient to run this fixture writer.

Target validation and the durable recovery fence precede metadata writes. The shared recovery
lock remains held during initial empty-database migration/bootstrap. Metadata inserts, table
initialization, object preparation and catalog publication use one caller-owned READ COMMITTED
transaction under `assert_write_allowed_conn`; paused or mismatched targets fail before pending
migrations, and a publication failure rolls metadata back. Only unreferenced scoped objects may
remain after a failed writer.

The smoke command also accepts `SMOKE_OUTPUT_JSON` when `--output` is omitted. Every run creates
its own dataset/series scope. It reports the actual metadata location, catalog commit and row hash;
it never deletes the table objects. Independent smoke hashing uses compact JSON lists in
`canonical_columns` order, string values or null, lowercase boolean strings, ISO dates and the
stored attribute JSON string. Lines are sorted lexically and joined with a final newline before
SHA-256. Audit timestamps are checked by Rust roundtrip parity separately, avoiding cross-engine
timestamp rendering differences in this smoke hash.

The real integration test covers huge/tiny/null values and attributes, initialized empty tables,
revision modes/context, actual DataFusion reads, uncommitted/crashed writers, stable UUID
corrections, idempotent retries, conflicting writers, rollback of caller-side summary changes,
pinned old snapshots and catalog checkpoints. It does not establish production capacity or
complete maintenance/recovery acceptance.

## Bounded maintenance and scheduling

`timeseries maintenance --max-rows N` reports physical/logical row amplification and retained
snapshot counts without changing storage. An explicit positive row budget prevents compaction
from collecting an unapproved dataset size. `--apply` compacts amplified tables using a native
replacement snapshot and requires an exact `TIMESERIES_TARGET_ID`; publication verifies the
same durable write fence as the crawler. A conflicting writer causes CAS failure and requires a
fresh plan. Prior snapshots remain readable and no old object is removed.

Snapshot-reference expiry is opt-in with `--expire-before <RFC3339> --retain-last N`. The cutoff
must be in the past and at least one recent snapshot is retained. Every observation vintage
remains in the current logical data. Expiry changes only metadata references; the SQL receipt
history and prior metadata/object chains are retained for recovery. It is not backup deletion
or a storage reclamation policy.

`scripts/train4/maintain-timeseries.sh` is the cron/Kubernetes entrypoint. Set
`ICEBERG_COMPACTION_MAX_ROWS` to the explicitly tested budget and
`TIMESERIES_MAINTENANCE_BINARY` to the installed binary path when needed. It defaults to dry run;
`TIMESERIES_MAINTENANCE_APPLY=true` enables fenced publication. Optional
`ICEBERG_EXPIRE_BEFORE`/`ICEBERG_RETAIN_LAST` carry the explicit retention decision. Schedule it
only after the operator chooses capacity and retention; no service is installed or enabled by
the feature branch. For example, an operator-owned cron schedule can invoke this entrypoint
daily with environment credentials injected by the existing secret mechanism.

The integration suite verifies a physical 7-row correction log compacts to 6 logical rows,
rejects a 5-row budget for that dataset, preserves the exact contents/old pinned snapshot,
expires metadata snapshots to the requested retain-last value, and still traverses the old
recovery objects afterward. This bounds correctness; production throughput/capacity remains
an independent acceptance gate.

## Read and identity budgets

Every materialized Iceberg snapshot read now applies a SQL `LIMIT budget + 1` before collecting
results and fails on overflow, preserving truthful counts instead of silently truncating data.
`ICEBERG_MAX_SCAN_ROWS` defaults to a conservative development limit of 100,000 logical rows;
`ICEBERG_QUERY_MEMORY_BYTES` defaults to 256 MiB managed DataFusion memory. Query batches are
1,024 rows with two target partitions. Operators must measure and explicitly configure larger
approved workloads; the default is not a production capacity claim. Compaction, identity
validation and recovery reads use these same managed query contexts.

`SqlCatalog::read_series_at_config` reads one or several series from a frozen pointer without
acquiring a database connection, allowing callers with a size-one pool to retain their own SQL
snapshot. The pool-backed `read_series_at` delegates to it. Both enforce the smaller of the
explicit caller budget and global row budget before returning observations.

The write boundary joins candidate observation UUIDs against the current frozen table and
rejects attempts to change an existing UUID's series, observation date, revision date or
original-release kind. This preserves note targets and makes series partition pruning safe.
Concurrent writers still publish through pointer CAS, so a losing writer restages against the
new identity set. Value/attribute corrections retain their identity and advance only their
physical commit generation. Lossy sub-microsecond audit timestamps and invalid attribute shapes
are rejected before staging; their content hashes cannot acknowledge truncated storage.

The `smoke_safety` integration target provisions and drops only its owned UUID-named scratch
database. It verifies that missing disposable intent, generic database fallback, name/target
mismatches and paused or production-labelled targets cause no fixture writes; that pause refusal
precedes pending migration work; and that an injected final receipt failure rolls back source,
dataset, series and catalog metadata together.
