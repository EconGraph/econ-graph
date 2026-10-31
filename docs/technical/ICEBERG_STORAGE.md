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
| Compaction | The selected transaction API has no complete data-file rewrite/compaction operation. It is not silently replaced by deletion or an unverified maintenance job. Retained physical versions and repeated mirrored series cause storage/read amplification until a measured rewrite path passes parity and recovery checks. |
| Snapshot expiry | Apache exposes `expire_snapshots`, but its age defaults can expire history even when explicit IDs are supplied. This implementation does not call it or delete objects automatically. History/backup retention requires explicit operating policy and recovery evidence. |

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

# Genuine table usable by independent PyIceberg/DuckDB checks. --migrate is explicit.
DATA_DATABASE_URL=postgres://... ICEBERG_WAREHOUSE=/tmp/disposable-warehouse \
  cargo run --manifest-path backend/Cargo.toml -p econ-graph-timeseries \
  --bin timeseries -- smoke --migrate --output /tmp/iceberg-smoke.json
```

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
