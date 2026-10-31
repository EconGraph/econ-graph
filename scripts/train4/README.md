# Reversible observation-store migration and recovery

This toolbox implements ECO-189/239 against the current PostgreSQL store and the real
`econ-graph-timeseries` Iceberg adapter. It never drops `data_points`, expires a snapshot,
removes a recovery copy, or changes a backend merely because objects were prepared.
Production recovery objectives, capacity budgets and the acceptance/rollback window still
require operator agreement. The checked-in test policies are disposable fixture scenarios,
not approved production values.

## Build and verification

Build `timeseries-recovery` with `cargo build --locked -p econ-graph-timeseries
--bin timeseries-recovery` from `backend/`. This includes DataFusion/Arrow and has a substantial
first-build cost; subsequent checks reuse Cargo artifacts. The Python companion requires
only Python 3 and PostgreSQL client tools. Backups/restores need client tools matching the
server major version; `--pg-dump`/`--pg-restore` select those executables explicitly.

`python3 scripts/train4/test_recovery.py` runs offline precision, interruption, identity,
checksum, path-confinement and acceptance-gate tests. Set
`TRAIN4_RECOVERY_TEST_DATABASE_URL` to an explicitly disposable PostgreSQL 18 admin URL to
also exercise transactional fences, snapshot-isolation refusal, real SQL export and safe
missing-row replay. Tests create/drop only their generated `eg_train4_recovery_*` databases.
To add genuine Iceberg backfill and catalog/object restore, set `TRAIN4_RECOVERY_BINARY`
to the compiled binary and, when needed, `TRAIN4_PG_DUMP` and `TRAIN4_PG_RESTORE` to matching
client wrappers. Checked-in `scripts/train4/pg-dump` and `pg-restore` use the real matching
clients inside `TRAIN4_TEST_PG_CONTAINER` (for GitHub Actions, `job.services.postgres.id`);
they permit only generated recovery scratch DBs and reject URI routing overrides. They
never select a Docker context or a production database implicitly. The integration test moves its original temporary warehouse aside and
retains it, restores into a fresh directory at the original logical URI and into a distinct
fresh database, then reruns full parity. It verifies that copied objects have different
inodes, so the original object files cannot satisfy the restore reads. Disposable backend
transition tests use synthetic acceptance hashes solely to exercise transaction guards;
they do not report production E2E, load or recovery acceptance.

## Runtime and durable pause

All candidate writers must call `assert_timeseries_write_allowed` inside the same
READ COMMITTED transaction as observations, catalog publication, summaries, attempts and
queue completion. The shared advisory transaction lock drains before recovery's exclusive
lock. An established REPEATABLE READ/serializable snapshot is refused: waiting for an
advisory lock cannot refresh that snapshot. An empty recovery state retains legacy
PostgreSQL behavior; initialized state requires the exact `TIMESERIES_TARGET_ID` and selected
`TIMESERIES_BACKEND`. Old/unconfigured processes fail closed after initialization. A pause
only drains writers running this protocol; verify the candidate workload inventory before
using it against a deployed environment.

Use `recovery.py control initialize|pause|resume --target-id UUID` to inspect a plan.
Mutations additionally require `--apply-target UUID` with the same exact UUID.
Initialization needs `--label`; pause needs a saved `--pause-token UUID`; resume needs that
token and the exact `--epoch` from the durable pause receipt. Repeating a pause with the
same token is idempotent. No command silently steals another operator's pause. Use the
explicit `DATA_DATABASE_URL`; no development/production connection is inferred.

Before cutover, deploy the candidate runtime with explicit target identity and
`TIMESERIES_ICEBERG_MIRROR=true` while PostgreSQL remains authoritative. Coordinate initial
state/configuration rollout in a maintenance deployment. Do not infer that legacy workers
honor the new fence. Root runtime integration retains the PostgreSQL shadow even after
Iceberg is selected; both publications share the existing caller-owned atomic transaction.

## Inventory, backfill and catch-up

`recovery.py export-postgres --target-id UUID --output source.jsonl` captures catalog
metadata and every observation/revision in one repeatable-read SQL snapshot. The version 1
JSONL header identifies the target, checkpoint, datasets, series and write fence. Rows keep
legacy observation/series/dataset UUIDs, dates, original-release status, exact nullable
values, timestamps and attributes. Unknown historical attributes remain null; they are
never invented from missing footnotes.

`recovery.py inventory source.jsonl` computes disk-backed per-series/dataset counts,
ranges and content hashes. `parity source.jsonl iceberg.jsonl` compares all metadata and
rows, rejects duplicates, and reports missing/changed identities. Decimal canonicalization
never uses a bounded decimal arithmetic context. Numerically equivalent decimal scales and
UTC timestamp spellings compare equivalently; unknown/null, empty objects, ordered arrays,
unknown provider fields and literal footnote strings remain distinguishable. This full-content
comparison supplements, rather than replaces, the shared store/query contract and release
E2E suites for revision modes, pagination, transformations, authorization and summaries.

Pause candidate writers and capture a final source export containing the drained token and
epoch. `recovery.py backfill source.jsonl --state journal.json --destination UUID` plans
batches. With `--binary /path/to/timeseries-recovery --apply-target UUID`, it bootstraps real
Iceberg tables and imports deterministic batches. The binary checks the current pause and
that each source row still matches retained PostgreSQL history. Catalog commit IDs make
lost acknowledgments retryable; the journal records only an exact durable batch receipt.
Changing the source/checkpoint/target/batch size requires a new journal. A resumed run does
not silently reinterpret an older export. A new catch-up export/journal can reimport the
latest paused full snapshot: observation UUID upserts remain idempotent at read time.
The durable pause survives an interrupted backfill. Resume writes only after the intended
backend/runtime agrees; never bypass the token/epoch to finish a cutover.

Export the real adapter with `timeseries-recovery export-iceberg --expect-target UUID
--output iceberg.jsonl` and compare it to the final PostgreSQL export. Missing datasets do
not fall back to PostgreSQL. Corrupt/missing objects and failed commands return nonzero.

## Consistent catalog/object recovery

`recovery.py capture-bundle --output NEW_DIRECTORY --target-id UUID --pause-token UUID
--epoch N --binary /path/to/timeseries-recovery` captures the SQL database and every object
reachable from its Iceberg pointers, including retained previous metadata/snapshots.
The holder acquires the exclusive session fence **before** starting its repeatable-read
transaction, verifies the paused target, and keeps its exported SQL snapshot alive while
both `pg_dump --snapshot` and object traversal/copy run. This avoids mixing catalog pointers
with later objects or accepting a stale pause snapshot after waiting for a lock. Bytes and
directory entries are synced before the completion manifest is published. Checksums are
verified against the original checkpoint inventory, including the copied catalog. Failed
copies retain `INCOMPLETE.json`; verification refuses them. Existing backup paths are never
replaced or deleted. This procedure pauses writes during its copy; include that downtime
and temporary disk use in the agreed capacity/recovery measurements.

`verify-bundle DIRECTORY` checks every retained byte hash. `restore-objects DIRECTORY
--destination FRESH_ROOT --apply-target UUID` only copies into a new root.
`restore-catalog DIRECTORY --database-env RESTORE_DATABASE_URL --apply-target UUID` restores
into an entirely empty explicitly selected database without `--clean` or overwrites and
retains the paused state. Restore objects and catalog together, then rerun real adapter
parity before resuming anything.

Iceberg metadata/manifests contain absolute object URIs. These commands do not rewrite them.
A fresh restore must preserve the logical warehouse URI using a separate endpoint or an
isolated mount at the same path; arbitrary host-directory relocation is not transparent.
Make the original objects unavailable to the restore environment when proving recovery.
The SQL/object bundle covers the data-plane catalog and reachable observation objects.
Raw archives, app user content and Keycloak retain their separate backup namespaces,
credentials and recovery artifacts; include all of them in acceptance evidence. Raw upstream
responses alone are not evidence that observation/revision/attribute history is recoverable.

## Reversible selection and retirement boundary

`gate REQUEST.json EVIDENCE.json...` validates an exact candidate/target/checkpoint, explicit
operator-approved per-plane RPO/RTO and capacity budgets, rollback window, parity, restore,
rollback, release E2E and measured load evidence. It refuses a rollback strategy consisting
only of changing a read flag. `transition ... --binary PATH` remains a plan unless the exact
`--apply-target UUID` is provided. The real binary rechecks the pause/token/epoch, accepted
catalog pointers and live complete PostgreSQL-shadow/Iceberg parity in one exclusive
transaction before recording the selected backend. It stays paused: roll out matching
runtime configuration and explicitly resume at the returned epoch. A lost transition
acknowledgment requires inspecting the durable transition ledger; there is no force option.

`replay-postgres SOURCE.jsonl --target-id UUID --pause-token UUID --epoch N` plans recovery
of missing post-cutover shadow rows. Applying requires the same `--apply-target UUID` and
current paused fence. Replay sends its disk-backed UTF-8 CSV stream to frontend `\copy FROM pstdin` until EOF;
its SQL setup/fence, copy and validation/commit commands share one explicit READ COMMITTED
psql connection. No inline `\.` terminator is sent to the server, so replay does not depend
on a particular client's SQL COPY/CSV terminator handling ([psql documentation](https://www.postgresql.org/docs/18/app-psql.html)).
The PostgreSQL test exercises both the installed psql and the service's matching client
when `TRAIN4_TEST_PG_CONTAINER` is provided, including literal terminator/SQL-like footnotes.
It restores all exact values/attributes/provenance in one transaction,
leaves existing identical rows alone, and refuses conflicting IDs/natural keys or missing
series/dataset metadata. It never deletes/replaces old history or narrows sub-microsecond
provenance into PostgreSQL precision. Revalidate parity after replay before rollback.

Retain PostgreSQL and verified recovery copies throughout the explicitly approved rollback
window. No automatic retirement, compaction, history expiry or backup deletion is performed
by this toolbox. Destructive retirement remains a separate reviewed forward migration after
production acceptance and a recovery proof for all promised history. Actual deployed
candidate inventory, production load/capacity and per-plane restore objectives remain
external acceptance evidence; local fixture success does not authorize production cutover.
