# Host object-store compatibility rehearsal

The host store remains a Train 4 decision gate. `scripts/objectstore/smoke.py` exercises
Versity's POSIX backend first, then runs the identical checks against Garage when any gate
fails or is unavailable. A run passes only when **all seven** gates pass for one store.
Unavailable crawler, reader, CNPG or filesystem prerequisites never become successful checks.
This script creates disposable stores; it does not install the persistent host service or
create `/srv/econgraph/objectstore`. The persistent container-versus-systemd choice is separate.

## Run the disposable checks

From the repository root, Python 3.14 is supported:

```sh
python3 -m venv /tmp/econgraph-objectstore-venv
/tmp/econgraph-objectstore-venv/bin/pip install --only-binary=:all: -r scripts/objectstore/requirements.txt
python3 scripts/objectstore/smoke.py
python3 -m unittest discover -s scripts/objectstore -p 'test_*.py' -v
/tmp/econgraph-objectstore-venv/bin/python scripts/objectstore/smoke.py \
  --execute --report /tmp/objectstore-report.json
```

The default is a dry run with no Docker or host mutation. `--execute` addresses only the local
Docker socket, starts sequential containers with unpredictable ownership labels, publishes
random **loopback** ports, and limits each store to two CPUs, 6 GiB RAM, 256 PIDs and bounded
logs. The native object workload stores a little over 2 GiB, plus an incomplete PUT and small
listing objects; it requires 6 GiB free space before starting. The 6 GiB memory limit also
accommodates multipart assembly when `/tmp` is tmpfs. Client buffers are 16 MiB per multipart
part and one MiB per read chunk; listing concurrency is eight. External hooks have a timeout
and must impose their own resource bounds.

Each run uses a new actor-owned scratch directory. Kill/restart/removal first verifies that
container's exact ownership label; cleanup never searches for or deletes unrelated Docker
resources. Disposable generated objects are removed when the container is removed. If the
CNPG hook starts, all store data and its scratch directory are retained even on failure,
because incomplete backups are recovery artifacts too. There are no lifecycle, expiry,
retentionPolicy, backup deletion or automatic metadata-snapshot rules.

Use `--gates crawler_sql_catalog,independent_readers,btrfs_restore` to rerun dependent gates
without repeating the expensive multipart/listing workload. Excluded gates are `not_run`, so
such a focused report cannot claim complete acceptance.

Use `--store versity` or `--store garage` for diagnosis. The acceptance default is `both`;
the fallback receives the same workload and hooks. Garage uses SQLite metadata,
`replication_factor=1`, `metadata_fsync=true`, `data_fsync=true`, and leaves
`metadata_auto_snapshot_interval` unset. This single-node topology proves compatibility,
not resilience to physical host loss.

## Seven required gates

| Gate | Executable proof |
| --- | --- |
| Crawler and SQL catalog | Actual dependent crawler command writes through iceberg-rust/DataFusion and commits through Postgres; emits table metadata and exact row checksum. |
| Independent readers | PyIceberg and DuckDB read the crawler's metadata location; both row counts and canonical row hashes match the crawler. |
| Multipart and conditional creation | Upload 129 × 16 MiB parts (2,164,260,864 bytes), download and compare SHA256; probe whether an `If-None-Match: *` collision returns 412 and preserves original bytes. Record the SDK's actual checksum header names. |
| Interrupted atomic PUT | Send an incomplete 128 MiB advertised PUT, SIGKILL only the owned store, restart, then require both HEAD and listing to show no partial object. |
| CNPG recovery | Actual base backup, WAL archive, and point-in-time recovery into a new owned cluster, with recovery target and verified rows. This requires the platform lane's CNPG/barman operators and drill. |
| Pagination and disk comparison | Write 10,007 objects, list in pages of 137, compare every key, then check delimiter prefixes. Versity keys match POSIX files exactly; Garage verifies every object body and the existence of its content-addressed block files. |
| btrfs recovery | On an owned scratch subvolume, stop the store, create a read-only snapshot, restart and delete only generated data-bucket keys, restore the complete store to a new writable subvolume, and rerun the independent table checksum. |

The real Rust `object_store` S3 create-only path can depend on conditional writes. The
conditional probe is explicit rather than assuming Iceberg never needs them. The current
crawler uses iceberg-rust/OpenDAL ordinary writes to UUID-isolated object locations and commits
through SQL pointer compare-and-swap; its report declares `requires_conditional_create: false`.
An unsupported conditional probe is advisory for that actual codepath. If a future writer
requires conditional creation, its report sets that field to true and the same capability
failure blocks the gate. No object-lock claim is inferred from a passing conditional write.

## Dependent command interfaces

`--crawler-command`, `--cnpg-command` and optional `--snapshot-command` accept JSON argument
arrays, never shell strings. The commands inherit normal toolchain and database settings and
receive these per-run environment variables:

- `S3_ENDPOINT`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_REGION=us-east-1`,
  `AWS_ALLOW_HTTP=true`, `S3_BUCKET`, and `ICEBERG_WAREHOUSE`.
- `SMOKE_OWNER`, `SMOKE_CONTAINER`, `SMOKE_DATA_DIRECTORY`, and `SMOKE_OUTPUT_JSON`.

The HTTP allowance is only for a disposable loopback endpoint. Credentials are random,
passed through the child environment, and omitted from reports and printed command lines.
The CNPG hook receives a separate generated `S3_BUCKET` ending in `-backups`; the data bucket
is never used for backups. The harness retains the whole store once this hook starts.

A command exits nonzero for failed/unavailable prerequisites. On success it writes JSON to
`SMOKE_OUTPUT_JSON`, including `status: "passed"` and these fields:

| Command | Required evidence fields |
| --- | --- |
| Crawler | `metadata_location`, `rows`, `sha256`, `canonical_columns`, `sql_catalog_commit`, `requires_conditional_create` |
| CNPG | `base_backup`, `wal_archive`, `restored_cluster`, `recovery_target`, `verified_rows` |
| Optional snapshot adapter | `snapshot`, `restored`, `table_reread_sha256` |

Crawler canonical columns are `id`, `series_id`, `date`, `revision_date`, `value`,
`is_original_release`, `attributes`. Each row is a compact JSON array of strings or nulls;
booleans use lower-case `true`/`false`, dates use ISO dates, and exact values and attributes
retain their string representation. Sort the JSON lines lexically, join with newlines,
append one final newline, then SHA256 the UTF-8 bytes. Physical commit-generation columns
and engine-dependent timestamp formatting do not participate.

`DUCKDB_EXTENSION_DIRECTORY` may point to a writable cache of the official signed `httpfs` and
`iceberg` extensions. The runner loads installed extensions first and otherwise uses the HTTPS
upstream repository; it never enables unsigned extensions. If this environment cannot bootstrap
HTTPS extension installation, preinstall the official extensions into that cache. The default
cache is inside the owned scratch directory, avoiding writes to the operator's home.

The crawler CLI is owned by the observation-storage lane; the harness requires its actual
implementation rather than substituting a Python-written Iceberg table. CNPG recovery likewise
requires a real operator-backed drill. Missing implementations remain visible prerequisites.

## btrfs scratch ownership and cleanup

For the native filesystem gate, choose a disposable path on btrfs, for example this worktree's
ignored `temp/` directory, and opt in explicitly:

```sh
mkdir -p temp/objectstore
/tmp/econgraph-objectstore-venv/bin/python scripts/objectstore/smoke.py \
  --execute --btrfs-scratch --scratch-parent "$PWD/temp/objectstore" \
  --crawler-command '["path/to/actual-crawler", "smoke"]' \
  --report /tmp/objectstore-btrfs-report.json
```

Use the actual compiled crawler command and its required arguments. The harness does not
access primary subvolumes, production buckets, or host devices. It preserves the pre-restore
subvolume while creating a writable snapshot from the read-only copy. If CNPG has written
backups, these copies are retained. Otherwise they contain generated test data only and are
cleaned up after the rehearsal.

Some host mounts permit unprivileged creation/snapshotting but deny subvolume deletion without
`user_subvol_rm_allowed`. The scoped cleanup helper requires a rootful Docker daemon with
`SYS_ADMIN` and `DAC_OVERRIDE` available. It checks a random ownership marker and exact scratch
basename, mounts only that scratch directory writable, has a read-only container root, no
network/devices/host PID namespace/Docker socket, and deletes only that run's `data`, `snapshot`
and `pre-restore` subvolumes. It mounts the host btrfs executable and libraries read-only; this
helper currently targets the Linux x86-64 host. Unsupported hosts must supply their own reviewed
cleanup mechanism rather than running a broad privileged container.

## Operating boundaries after selection

Production provisioning must put the selected host store on its own durable btrfs subvolume,
with separate data, raw-archive and backup buckets and separate least-privilege credentials for
each environment. The disposable root credential is not a production access policy. Back up the
Postgres SQL catalog together with all objects referenced by its checkpoint. Preserve original
observations, revisions, attributes and provenance through backfill and restore. User-authored
app data and Keycloak recovery need independent, stricter recovery objectives; no claim that
raw upstream files can rebuild them is acceptable.

Compaction may create new objects but must not expire promised vintage history or automatically
delete backups. Cutover requires the separate catalog/object restore, parity, rollback and
capacity gates. This smoke report alone does not authorize replacing production storage.

## Upstream references

- [Versity Docker configuration](https://github.com/versity/versitygw/wiki/Docker)
- [Versity POSIX backend and conditional-publish locking](https://github.com/versity/versitygw/wiki/POSIX-Backend)
- [Versity v1.10.0 release](https://github.com/versity/versitygw/releases/tag/v1.10.0)
- [Garage quick start and single-node bootstrap](https://garagehq.deuxfleurs.fr/documentation/quick-start/)
- [Garage configuration, fsync and optional metadata snapshots](https://garagehq.deuxfleurs.fr/documentation/reference-manual/configuration/)

- [Rust object_store S3 conditional-put behavior](https://docs.rs/object_store/latest/object_store/aws/enum.S3ConditionalPut.html)
