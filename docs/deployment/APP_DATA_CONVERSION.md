# Convert an existing combined database

The backend and Kubernetes manifests support separate app and data databases. A database
created by the original combined baseline still contains both sets of tables. Deploying the
new manifests does not copy existing users, sessions, annotations, preferences or audit logs
into the app database. `scripts/deploy/split-app-data.py` performs that one-time copy, verifies
it, and optionally removes the old app tables. The crawl stays in its original database;
series IDs never change and no crawl is started.

This is an operator maintenance procedure. Choose the environment explicitly and rehearse on
a restored copy before approving a live run. Production and UAT are separate decisions. The
script does not deploy services, fetch Kubernetes Secrets, stop writers, or choose credentials.

## Prepare a rehearsal

1. Use a restored copy of the combined database and a separate app database initialized with
   every checked-in app migration. The backend can initialize the app schema, but stop that
   backend before starting the conversion. Keep the source data database on its existing
   volume; do not initialize a replacement data database. Use a release image for the services.
2. Install Python 3.11 or newer with `psycopg[binary]==3.3.6`, `pg_dump` and `pg_restore`
   matching the server's major version. The source must already have its data
   migration baseline recorded. An unknown table, missing table, partial split or unknown
   migration version stops the script rather than guessing how to repair it. The current
   table/type inventory comes from the checked-in baselines; later migrations that add or
   rename tables require reviewing and updating this conversion tool before use.
3. Configure two private libpq service entries, such as `conversion_source` and
   `conversion_app`, with distinct databases and maintenance superuser connections. The
   identity check uses `pg_control_system()`. Use a mode-0600 service/password file owned by
   the operator; never put passwords in command-line arguments. The services may point at
   explicit local port forwards for the intended namespace. Verify those endpoints yourself.
4. Make sure the backup root has room for a full custom-format archive of the source. Every
   copying run first writes a verified dump of the locked source to a new directory under
   `ECONGRAPH_BACKUP_ROOT` (default `~/econgraph-backups`), the root and layout
   `backup-kind-databases.sh` uses, and stops before any app row changes unless that
   backup succeeded. See [Pre-conversion backups](#pre-conversion-backups).
5. Prepare a JSON source map from the restored database's `data_sources.id` to the canonical
   crawler source code. For example, `{"<FRED source UUID>": "FRED"}`. Use the actual source
   UUID, not the example placeholder. Canonical codes are the `SourceId::as_str()` values in
   `econ-graph-crawler/src/source.rs`; every source containing an economic series must be
   mapped. The script checks each ID against UUIDv5 of `SOURCE:external_id` in the crawler's
   fixed namespace and stops if any differ. Do not modify IDs to make this check pass.

Run an inspection first:

```sh
python scripts/deploy/split-app-data.py \
  --source-service conversion_source --app-service conversion_app \
  --source-map /private/path/source-map.json --dry-run
```

A dry run reads and locks the databases, computes content fingerprints, and rolls back. It
creates no backup and changes no persistent rows. It can briefly block writers: the script
locks every source table against writes and every app target table against access. Lock
acquisition, including the separate dump connection, times out after ten seconds. This also
prevents a queued schema change from leaving the dump waiting behind its own source locks.
Full sorted content hashing and a full source dump
cost time, temporary database sort space and backup space proportional to the data volume.

## Copy, verify, and remove the old app tables

Stop every app writer, scheduler, crawler worker and administrative writer against both
databases. Keep them stopped through the final cleanup and backend reconfiguration. The
`--writers-stopped` flag records your confirmation; it does not stop them for you. Database
locks protect the copy and verification from concurrent row writes. They remain held on the
source until cleanup finishes; a source writer cannot slip in between copy and deletion.

```sh
python scripts/deploy/split-app-data.py \
  --source-service conversion_source --app-service conversion_app \
  --source-map /private/path/source-map.json \
  --writers-stopped
```

The destination must be empty except for the baseline annotation templates, or already match
the source app records exactly. Template contents are checked against the baseline before
replacing their generated IDs. Existing unrelated app records are never overwritten. App
rows are copied in one transaction, including self-referencing foreign keys, and verified by
per-table row counts and SHA-256 of sorted canonical row contents. All data tables receive
before/after fingerprints too. The target keeps its own current migration history.

This first invocation retains the source app tables. Review `evidence.json` in the backup
directory and check the app against the copied records while keeping writers stopped. An
existing user's annotations must still resolve their original series IDs. Counts and hashes
in `evidence.json` contain no row contents.

The archive includes both schemas; do not restore it over an initialized app database. To
rehearse recovery, create a new, empty scratch database and a `conversion_rehearsal` libpq
service pointing only at it, then restore the combined archive there:

```sh
pg_restore --no-owner --no-privileges --exit-on-error \
  --dbname service=conversion_rehearsal ~/econgraph-backups/<UTC timestamp>/econ_graph.dump
```

Because the directory has the `backup-kind-databases.sh` layout, a kind cluster's combined
data database can also be put back with
`scripts/deploy/restore-kind-databases.sh --from <dir> --only econ_graph`, which compares the
restored row counts with `counts.env`.

After approving the copied records, rerun with `--drop-source-app`. It writes a new backup
directory and repeats the verification and requires typing
`DROP APP TABLES FROM <database name>` at a terminal. There is no `--yes` bypass. The cleanup
transaction drops the app tables together and their five enum types without `CASCADE`; the
three old cross-plane foreign keys disappear with their app tables. Unexpected dependent
objects cause a rollback. It removes only known app migration entries from the source's
history, preserving the data baseline and common initial setup. Every data table and the
ordered series-ID checksum must remain unchanged before commit.

If confirmation is refused or the cleanup transaction rolls back, the verified app copy
remains committed and the source app tables remain. If the connection drops during commit,
inspect the schema and use an evidence-backed rerun to establish the outcome; do not assume
the commit failed. Keep writers stopped, inspect the evidence and rerun; do not resume
writers against both copies. If the source already contains only the expected data schema,
pass `--previous-evidence /path/to/the/previous/evidence.json`. The script requires that the
source data and target app contents, series IDs and database identities match that verified
conversion; it refuses to treat an empty target as a successful rerun. This maintenance check
will also refuse after legitimate app writes change the copied content. It does not prove the
live deployment points at the right app database.

Finally, configure the backend to use the separate app and data services, verify existing
users and annotations, and run the tagged crawler's `migrate --dry-run` against the data
database to confirm nothing is pending. Restart only the intended writers after these checks.
Keep the pre-conversion backups on the host and record the rehearsal/live evidence with the
operator's approval. Running the synthetic test is useful engineering validation, but is not
a rehearsal of your production database.

## Pre-conversion backups

Each copying run (not `--dry-run`) writes one new directory, by default
`$ECONGRAPH_BACKUP_ROOT/<UTC timestamp>/` (`~/econgraph-backups` when the variable is unset);
`--backup-dir` names a different new directory. An existing directory is refused, never
reused or overwritten. The dump is taken under the conversion's locks from the same snapshot
the copy is verified against:

```
<backup root>/<YYYYMMDDTHHMMSSZ>/
  econ_graph.dump        pg_dump --format=custom of the whole combined source database
  counts.env             SERIES_COUNT and DATA_POINTS_COUNT at backup time
  SHA256SUMS             checksum of econ_graph.dump
  source-evidence.json   row counts and content hashes of every table before the copy
  evidence.json          the same, plus the copy and cleanup results
```

The dump is written as `econ_graph.dump.partial`, read back in full with
`pg_restore --file=/dev/null`, and renamed only when every data block decodes. Any failure
stops the conversion before app rows change. The directory is mode 0700 and every file in it
mode 0600. Nothing deletes or prunes these directories, including a `.partial` dump left by a
failed run; remove them by hand only after the conversion has been verified.

The dump is not encrypted. It holds every app record (users, sessions, annotations, audit
history) next to the data tables. Keep it on the host: do not copy it to other machines,
shared storage or chat, and do not loosen its permissions. The directory has no `VERIFIED`
marker, so `scripts/backup/offsite-restic.sh` does not pick it up from a shared backup root.

## Synthetic engineering check

`python scripts/deploy/test-split-app-data.py -v` creates a uniquely named disposable
PostgreSQL 18 container, publishing its port on 127.0.0.1 only, and applies the actual migration files.
It inserts synthetic users, annotations and a stable series, verifies copying and source
cleanup, and exercises refusal and rerun behavior. It never connects to a cluster or reads
production configuration. It requires Docker (the daemon named by `DOCKER_HOST`, default the rootful socket) and the
Python dependency above. Its backups go to a scratch `ECONGRAPH_BACKUP_ROOT`.
