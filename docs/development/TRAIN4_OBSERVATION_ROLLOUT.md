# Observation backend rollout

The default remains PostgreSQL. Enabling the Iceberg mirror publishes native
Iceberg metadata through the data database's SQL catalog, in the same transaction
as the retained PostgreSQL observations, series summary, crawl attempt and queue
completion. An unsuccessful publication or queue transition rolls those database
changes back together. Unpublished object files may remain after an interrupted
writer; they do not become visible through the catalog and are not automatically
deleted.

| Setting | Behavior |
| --- | --- |
| `TIMESERIES_BACKEND` | `postgres` by default; `iceberg` selects Iceberg observation reads |
| `TIMESERIES_TARGET_ID` | Explicit recovery target UUID; required for mirroring or Iceberg |
| `TIMESERIES_ICEBERG_MIRROR` | `true` enables the shadow while PostgreSQL is authoritative; Iceberg selection always enables it |
| `TIMESERIES_MIRROR_MAX_ROWS` | Positive per-series materialization limit, default 100000; overflow refuses the entire write |
| `TIMESERIES_MIRROR_TIMEOUT_SECS` | Mirror operation timeout, default 120 seconds; permitted range 1–3600 |
| `ICEBERG_WAREHOUSE` | Absolute local/file warehouse or an explicitly configured S3-compatible warehouse |

An empty recovery control table permits only the default PostgreSQL configuration.
Once initialized, both readers and writers require the matching target and backend.
Readers check selection inside their database snapshot. Writers acquire the
recovery shared lock in their caller's READ COMMITTED transaction; pause and
transition operations acquire the exclusive lock and drain guarded writers.
Stronger writer snapshot isolation is refused because it could retain a stale
pre-pause control row. A pause preserves read availability.

Every mirrored series includes its retained PostgreSQL history, stable observation
UUIDs and nullable attributes. PostgreSQL continues receiving the same transaction's
writes after Iceberg cutover, so post-cutover writes remain available for verified
rollback. A configuration flag alone cannot authorize a backend transition: the
durable control state must agree, and the recovery procedure requires parity and
recovery evidence before accepting it. No legacy observation table is dropped.

The initial mirror stages object I/O while holding the worker lease and series
locks. This favors an exact copy of PostgreSQL's existing revision/upsert semantics
and caller-owned atomic publication. It increases transaction duration and can
append redundant physical rows on an unchanged recrawl; bounded compaction and
measured capacity are required before production acceptance. These development
limits are safeguards, not approved production scale or latency targets.

The real-worker integration test exercises both authoritative-backend selections.
It also fails the final queue completion after the pointer write and verifies that
the pointer and PostgreSQL observations both roll back while the job retries.
Recovery selection tests separately verify target mismatch, stale configuration
and paused-write refusal. The platform's privileged migration, restore and load
gates remain necessary before an operational cutover.
