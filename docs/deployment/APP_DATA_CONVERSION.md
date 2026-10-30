# Convert an existing combined database

The backend and Kubernetes manifests support separate app and data databases. A database
created by the original combined baseline still contains both sets of tables. Deploying the
new manifests does not copy existing users, sessions, annotations, preferences or audit logs
into the app database. `scripts/deploy/convert-app-data.sh` does that on a kind cluster: it
copies the app records into `econ_graph_app`, verifies them, and optionally removes the old
app tables from `econ_graph`. The crawl stays in its original database; series IDs never
change and no crawl is started. The same script rolls a removal back.

The wrapper drives `scripts/deploy/split-app-data.py`, which does the database work and
can also be run on its own against any two libpq services.

## What the wrapper does

```sh
scripts/deploy/convert-app-data.sh --cluster NAME [--production] --source-map FILE \
    [--drop-source-app] [--previous-evidence FILE] [--dry-run]
scripts/deploy/convert-app-data.sh --cluster NAME [--production] \
    --restore-source-app BACKUP_DIR [--dry-run]
```

1. **Checks** (read-only, also in `--dry-run`): the named cluster through its own kubectl
   and the context `kind-NAME` (never the host's `kubectl`); `--production` must be given
   exactly when the cluster carries the production marker; `postgresql-0` and
   `app-postgres-0` run; both servers have the same major version as the `pg_dump` in
   `PG_IMAGE` (default `postgres:18`); `PYTHON` (default `python3`) imports
   `psycopg[binary]==3.3.6`; no Job is running.
2. **Full cluster backup**: `scripts/deploy/backup-kind-databases.sh` (data, app and
   Keycloak dumps plus the Secrets). The run stops if it fails, before anything changes.
3. **Writers stopped**: `deployment/econ-graph-backend` and `deployment/crawler-worker`
   scaled to 0 and `cronjob/postgres-backup` suspended. Each is put back as it was when
   the script exits, also after a failure. After a successful removal or rollback the
   backend stays at 0, because its database setting must change before it starts.
4. **Connections**: port-forwards to both databases on 127.0.0.1, and a private temporary
   directory with a mode-0600 libpq service file and password file built from the
   `postgres-password` keys of the Secrets `econ-graph-postgres` and
   `econ-graph-app-postgres`. Passwords are never printed. The directory and the
   port-forwards are removed on exit. `pg_dump` and `pg_restore` run from `PG_IMAGE`
   on the host network, not from the host's packages.
5. **Conversion**: `split-app-data.py` with `--dry-run`, then the copy, then, with
   `--drop-source-app`, the removal, which asks for a typed phrase on the terminal. With
   `--restore-source-app`, the rollback instead.

`--dry-run` performs the checks of step 1 and prints every later step. It takes no backup,
stops nothing, reads no Secret and opens no database connection. A cluster other than
`econ-graph` is a scratch cluster and needs `ECONGRAPH_SCRATCH_STORAGE` (see
[Testing on a scratch cluster](HOST_STORAGE.md#testing-on-a-scratch-cluster)).

The nightly host dump (`scripts/backup/dump-databases.sh`) can hold locks the conversion
waits for; run the conversion outside its window.

## Locks and duration

Every `split-app-data.py` step, the dry run included, locks every source table against
writes and every app table against all access, then computes row counts and SHA-256 of
the sorted contents of every table, `data_points` included, while holding those locks. On
a full crawl this takes minutes and uses temporary sort space on the database server.
Writers block for that whole time, which is why the wrapper stops them before the first
step. Lock acquisition times out after ten seconds. Each copy and each removal also writes
a full dump of the combined database under the backup root.

The dry run keeps the full content hash: it checks everything a real run checks, and its
duration measures the maintenance window the real run needs.

## Prepare the source map

The conversion checks every series ID against the crawler's UUIDv5 contract and needs to
know which crawler source each `data_sources` row is. Write a JSON object from each
`data_sources.id` to its canonical crawler code, the `SourceId::as_str()` values in
`backend/crates/econ-graph-crawler/src/source.rs`:

```json
{"<FRED source UUID>": "FRED", "<BLS source UUID>": "BLS"}
```

List the rows with the cluster's kubectl (the wrapper's check step prints which one it
uses), for example
`kubectl --context kind-NAME -n econ-graph exec postgresql-0 -c postgresql -- psql -U postgres -d econ_graph -Atc 'select id, name from data_sources'`.
Every source that has an economic series must be mapped. The script stops if any series ID
differs from UUIDv5 of `SOURCE:external_id`; do not change IDs to make it pass.

## Rehearse on a copy of production

1. Take a backup of production. It only reads the cluster:
   `scripts/deploy/backup-kind-databases.sh` writes `~/econgraph-backups/<UTC timestamp>/`.
   Note `SERIES_COUNT` and `DATA_POINTS_COUNT` from its `counts.env`.
2. Create a scratch cluster at the release production runs, as in
   [Testing on a scratch cluster](HOST_STORAGE.md#testing-on-a-scratch-cluster): export
   `KIND_CLUSTER=<name>`, `ECONGRAPH_SCRATCH_STORAGE`, `XDG_CACHE_HOME` and a scratch
   `ECONGRAPH_BACKUP_ROOT`, then `scripts/deploy/recreate-kind-cluster.sh --if-missing
   --config <copy>`.
3. Restore the production backup into it:
   `scripts/deploy/restore-kind-databases.sh --from ~/econgraph-backups/<timestamp>
   --expect-series N --expect-data-points N`. This brings over the combined `econ_graph`,
   Keycloak and the Secrets the databases were created with.
4. Deploy the release being rolled out to the scratch cluster with `scripts/deploy/deploy.sh`.
   It creates `app-postgres` with an empty `econ_graph_app` and keeps
   `APP_DATABASE=shared`, so the backend still uses `econ_graph`.
5. Run the conversion against the scratch cluster, with the same environment:

   ```sh
   scripts/deploy/convert-app-data.sh --cluster <name> --source-map /private/source-map.json --dry-run
   scripts/deploy/convert-app-data.sh --cluster <name> --source-map /private/source-map.json --drop-source-app
   ```

   The empty app database gets the app schema from the checked-in migrations, created as
   the backend's role `econgraph`, in the same transaction as the copy. Note how long the
   dry run takes.
6. Rehearse the rollback with the backup directory the removal printed, then convert again:

   ```sh
   scripts/deploy/convert-app-data.sh --cluster <name> --restore-source-app <scratch backup root>/<timestamp>
   scripts/deploy/convert-app-data.sh --cluster <name> --source-map /private/source-map.json --drop-source-app
   ```

   If the backend ran on `econ_graph` in between, the copy in `econ_graph_app` is stale; add
   `--previous-evidence <scratch backup root>/<copy timestamp>/evidence.json` to replace it.

7. Switch the backend over with `APP_DATABASE=separate scripts/deploy/create-secrets.sh`,
   start it, and check that existing users sign in and their annotations still show on
   their original series. Run the tagged crawler's `migrate --dry-run` against the data
   database to confirm nothing is pending.

The scratch backup root then holds full copies of production records; it has the same
permissions and handling as below. The synthetic test in CI does not replace this
rehearsal.

## Convert production

After a rehearsal on the same release, run the same commands with
`--cluster econ-graph --production`, a `--dry-run` first. Without `--drop-source-app` the
wrapper copies, verifies and puts every writer back: the backend keeps using `econ_graph`
and the copy in `econ_graph_app` goes stale. A later run replaces that stale copy only when
given its `--previous-evidence`; any other existing app data in `econ_graph_app` is refused.

With `--drop-source-app`, the third step asks for `DROP APP TABLES FROM econ_graph` on the
terminal. There is no bypass. While the prompt waits, the copy is committed and the source
is still locked, so the evidence can be reviewed from another terminal. The removal drops
the app tables and their five enum types without `CASCADE` (the three cross-plane foreign
keys go with their tables) and deletes only the app migration entries. Every data table
and the series-ID checksum must be unchanged before it commits. A wrong phrase, a refusal
or a failed check leaves the source as it was, with the verified copy committed and every
writer put back.

After a removal, run `APP_DATABASE=separate scripts/deploy/create-secrets.sh`, start the
backend (`scripts/deploy/deploy.sh`), and check users and annotations as in the rehearsal.

## Roll back a removal

```sh
scripts/deploy/convert-app-data.sh --cluster econ-graph --production \
    --restore-source-app ~/econgraph-backups/<timestamp of the removal run>
```

The rollback puts the app tables back into `econ_graph` from the dump the removal run took
just before it dropped them. It requires that directory's `evidence.json` to record a
removal, the dump to match its recorded SHA-256, and the source to be the same database
(by system identifier and name), holding the data tables and no app tables. From
`pg_restore --list` it selects only the app tables, their rows, the app enum types and the
indexes, constraints, triggers, comments and privileges on them, writes that list to
`restore.list` in a new directory under the backup root, and restores it in one
transaction. It then checks every app table against the row counts and content hashes
recorded before the removal and adds back the app migration entries the removal deleted.
Data tables are not touched; crawl data written after the conversion stays.

Rows written to `econ_graph_app` after the switch are not carried back. The app database is
left as it is. Afterwards run `APP_DATABASE=shared scripts/deploy/create-secrets.sh` and
start the backend.

`scripts/deploy/restore-kind-databases.sh` does not restore conversion backups: they have no
`econ_graph.dump`, so it refuses them. It compares only `economic_series` and
`data_points` counts, which the conversion never changes, so it could not tell whether the
app tables came back.

## Conversion backups

Each copying or removal run writes one new directory, by default
`$ECONGRAPH_BACKUP_ROOT/<UTC timestamp>/` (`~/econgraph-backups` when the variable is
unset); `split-app-data.py --backup-dir` names a different new directory. An existing
directory is refused. The dump is taken under the conversion's locks from the same snapshot
the copy is verified against:

```
<backup root>/<YYYYMMDDTHHMMSSZ>/
  combined.dump          pg_dump --format=custom of the whole combined source database
  SHA256SUMS             checksum of combined.dump
  pg_tools.log           stderr of pg_dump and pg_restore
  source-evidence.json   row counts and content hashes of every table before the copy
  evidence.json          the same, plus the copy and removal results
```

The dump is written as `combined.dump.partial`, read back in full with
`pg_restore --file=/dev/null`, and renamed only when every data block decodes. Any failure
stops the conversion before app rows change and reports the last lines of `pg_tools.log`.
Directories are mode 0700 and every file mode 0600. Nothing deletes or prunes these
directories, including a `.partial` dump left by a failed run; remove them by hand only
after the conversion has been verified.

The dump is not encrypted. It holds every app record (users, sessions, annotations, audit
history) next to the data tables, and the backup root beside it holds Keycloak's dumps and
the cluster Secrets. Keep it on the host: do not copy it to other machines, shared storage
or chat, and do not loosen its permissions. The directory has no `VERIFIED` marker, so
`scripts/backup/offsite-restic.sh` does not pick it up from a shared backup root.

## Running split-app-data.py directly

```sh
python scripts/deploy/split-app-data.py --source-service SRC --app-service APP \
    --source-map FILE [--init-app-schema ROLE] [--dry-run | --writers-stopped [--drop-source-app]] \
    [--previous-evidence FILE] [--backup-dir DIR]
python scripts/deploy/split-app-data.py --source-service SRC --restore-source-app DIR --writers-stopped
```

The services are names in a private libpq service file (`PGSERVICEFILE`) with passwords in
a mode-0600 password file, never on the command line; `pg_dump` and `pg_restore` on `PATH`
must match the server's major version. The script refuses unknown or missing tables, a
partial split, unknown migration versions, column differences in name, type or nullability,
unrelated rows in the app database, and a run without `--writers-stopped`. Later
migrations that add or rename tables require reviewing this tool before use.

## Synthetic engineering checks

`python scripts/deploy/test-split-app-data.py -v` creates a uniquely named disposable
PostgreSQL 18 container, publishing its port on 127.0.0.1 only, and applies the actual
migration files. With synthetic users, annotations and a stable series it checks the copy,
the removal, the rollback, evidence-backed reruns, writer locks, the backup layout and
permissions, a full restore of the backup, and the refusal cases. It needs Docker (the
daemon named by `DOCKER_HOST`, default the rootful socket) and `psycopg[binary]==3.3.6`;
its backups go to a scratch `ECONGRAPH_BACKUP_ROOT`.

`scripts/deploy/test-convert-app-data.sh` runs the wrapper against stub `kubectl`, `docker`,
`kind` and Python with `HOME`, the cache and the backup root in a scratch directory. Neither
test connects to a cluster or reads production configuration.
