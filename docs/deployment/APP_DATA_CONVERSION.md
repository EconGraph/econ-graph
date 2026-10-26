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
2. Install Python 3.11 or newer with `psycopg[binary]==3.2.12`, `pg_dump` and `pg_restore`
   matching the server's major version, Bash and age. The source must already have its data
   migration baseline recorded. An unknown table, missing table, partial split or unknown
   migration version stops the script rather than guessing how to repair it. The current
   table/type inventory comes from the checked-in baselines; later migrations that add or
   rename tables require reviewing and updating this conversion tool before use.
3. Configure two private libpq service entries, such as `conversion_source` and
   `conversion_app`, with distinct databases and maintenance superuser connections. The
   identity check uses `pg_control_system()`. Use a mode-0600 service/password file owned by
   the operator; never put passwords in command-line arguments. The services may point at
   explicit local port forwards for the intended namespace. Verify those endpoints yourself.
4. Configure `BACKUP_AGE_RECIPIENT` with the operator's public age recipient. Backup handling
   follows [the backup runbook](BACKUPS.md). It requires tmpfs plaintext staging
   (`BACKUP_PLAINTEXT_ROOT`, default `/dev/shm`) and sufficient space for a full custom archive.
   Disable swap or encrypt it. No private age identity is needed to write the backup. A durable
   encrypted bundle is sealed **before** app rows change; existing backups are never replaced
   or deleted. New temporary plaintext staging is cleaned up after sealing or failure.
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
acquisition times out after ten seconds. Full sorted content hashing and a full source dump
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
  --backup-dir /durable/operator-backups/new-conversion-copy \
  --writers-stopped
```

The destination must be empty except for the baseline annotation templates, or already match
the source app records exactly. Template contents are checked against the baseline before
replacing their generated IDs. Existing unrelated app records are never overwritten. App
rows are copied in one transaction, including self-referencing foreign keys, and verified by
per-table row counts and SHA-256 of sorted canonical row contents. All data tables receive
before/after fingerprints too. The target keeps its own current migration history.

This first invocation retains the source app tables. Review `evidence.json` and check the app
against the copied records while keeping writers stopped. An existing user's annotations
must still resolve their original series IDs. The encrypted bundle contains the full source
archive, its checksum and the pre-copy fingerprints. Counts and hashes in the external
evidence file contain no row contents. Restore the bundle using the operator decryption
procedure in the backup runbook if recovery is needed. To rehearse recovery, create a new,
empty scratch database and a `conversion_rehearsal` libpq service pointing only at it, then
restore the encrypted combined archive there:

```sh
scripts/backup/open-backup.sh /durable/operator-backups/new-conversion-copy -- \
  pg_restore --no-owner --no-privileges --exit-on-error \
  --dbname service=conversion_rehearsal @BACKUP@/combined.dump
```

The archive includes both schemas; do not restore it over an initialized app database.

After approving the copied records, rerun with a **new** backup directory and
`--drop-source-app`. It repeats the backup and verification and requires typing
`DROP APP TABLES FROM <database name>` at a terminal. There is no `--yes` bypass. The cleanup
transaction drops the app tables together and their five enum types without `CASCADE`; the
three old cross-plane foreign keys disappear with their app tables. Unexpected dependent
objects cause a rollback. It removes only known app migration entries from the source's
history, preserving the data baseline and common initial setup. Every data table and the
ordered series-ID checksum must remain unchanged before commit.

If confirmation is refused or cleanup fails, the verified app copy remains committed and the
source app tables remain. Keep writers stopped, inspect the evidence and rerun; do not resume
writers against both copies. If the source already contains only the expected data schema,
pass `--previous-evidence /path/to/the/previous/evidence.json`. The script requires that the
source data and target app contents, series IDs and database identities match that verified
conversion; it refuses to treat an empty target as a successful rerun. This maintenance check
will also refuse after legitimate app writes change the copied content. It does not prove the
live deployment points at the right app database.

Finally, configure the backend to use the separate app and data services, verify existing
users and annotations, and run the tagged crawler's `migrate --dry-run` against the data
database to confirm nothing is pending. Restart only the intended writers after these checks.
Retain the encrypted pre-conversion bundles and record the rehearsal/live evidence with the
operator's approval. Running the synthetic test is useful engineering validation, but is not
a rehearsal of your production database.

## Synthetic engineering check

`python scripts/deploy/test-split-app-data.py -v` creates a uniquely named disposable
PostgreSQL 18 container on the local Docker socket and applies the actual migration files.
It inserts synthetic users, annotations and a stable series, verifies copying and source
cleanup, and exercises refusal and rerun behavior. It never connects to a cluster or reads
production configuration. It requires Docker, age and the Python dependency above.
