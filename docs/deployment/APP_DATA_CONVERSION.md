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

## Prerequisites

- **Set `APP_DATABASE` only while `app-database-url` is missing, or to change it.** A
  cluster deployed from a release before the app database existed has no
  `app-database-url` in `econ-graph-secrets`, and every script that runs
  `create-secrets.sh` (`deploy.sh`, `release-deploy.sh`, `restart-k8s-rollout.sh`,
  `set-crawler-keys.sh`, `restore-kind-databases.sh`) refuses it until
  `APP_DATABASE=shared` is given. Once the key is written, leave `APP_DATABASE` unset: the
  key is kept (a value equal to it counts as kept). Only a cluster without
  `econ-graph-secrets` at all gets `separate` by default.
- **The Secret records where the app tables are.** The wrapper rewrites only
  `app-database-url`, to `econ_graph_app` as soon as a removal commits and back to
  `econ_graph` after a rollback, with `create-secrets.sh --app-database-only`. That mode
  runs the same decision and checks, builds the URL from the passwords already in the
  cluster and writes nothing else; with `--dry-run` it only checks, which is what
  `release-deploy.sh` does before its first step.
- **`create-secrets.sh` refuses a setting that points at the database without the
  accounts** (`scripts/deploy/lib/app-database-guard.sh`), before it writes anything:
  - `shared`, on every run of an existing cluster, kept or not, once the removal has left
    the app tables only in `econ_graph_app` (the backend would create empty app tables in
    `econ_graph`);
  - `separate`, when it changes, while `econ_graph` still holds `public.users`, and on a
    Secret without `app-database-url` unless `econ_graph` is running and answers that it
    holds no app tables;
  - a kept `separate` while `econ_graph` holds `public.users` again, as after a rollback
    that stopped before pointing the key back; the message gives the
    `--app-database-only` command.

  A database that can't be asked (its pod down, a query failing or not answering within
  60 seconds) refuses a change, with one exception: a kept `shared` passes when
  `econ_graph_app` answers that it holds no app tables, or does not exist, since the
  tables can't then be only there. To go ahead without the check, for example to redeploy
  while repairing a database, set `ALLOW_UNCHECKED_APP_DATABASE=1`. Two refusals stand even
  then: a database that positively holds the tables elsewhere, and a kept `shared` when
  `econ_graph` answers that it holds no app tables while `econ_graph_app` can't be asked,
  since the conversion may have moved the accounts there. For the second, start
  `app-postgres-0` and rerun, or use `separate` if the conversion finished or the cluster
  never had accounts in `econ_graph` (a lab, or a data-only dump).
- **Every deploy names its cluster**: `KIND_CLUSTER=<name> ... scripts/deploy/deploy.sh`.
  `deploy.sh` runs `create-secrets.sh` on the context `kind-<name>`, so a rehearsal
  cannot write a setting into production.
- **The checkout is the release that is deployed**, the one whose app migrations the
  source has run. The conversion refuses a source that lacks an app migration of the
  checkout, and the rollback refuses a backup whose removed tables, enum types or
  migration versions differ from the checkout.
- **Run it inside tmux or screen.** A dropped connection or a closed pipe no longer
  leaves the writers stopped, but the removal's typed confirmation needs the terminal.

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
   scaled to 0 and `cronjob/postgres-backup` suspended. On exit, also after a failure, a
   signal or a closed terminal, the crawler and the CronJob are put back as they were.
   The backend is put back only if `econ_graph` still holds the app tables (checked in
   the database, not inferred from how the run ended) and no removal or rollback
   succeeded. Otherwise it stays at 0 and the script prints the deploy that starts it on
   the right database, naming the cluster.
4. **Connections**: port-forwards to both databases on 127.0.0.1, and a private temporary
   directory with a mode-0600 libpq service file and password file built from the
   `postgres-password` keys of the Secrets `econ-graph-postgres` and
   `econ-graph-app-postgres`. Passwords are never printed. The directory and the
   port-forwards are removed on exit. `pg_dump` and `pg_restore` run from `PG_IMAGE`
   on the host network, not from the host's packages.
5. **Conversion**: `split-app-data.py` with `--dry-run`, then the copy, then, with
   `--drop-source-app`, the removal, which asks for a typed phrase on the terminal, and the
   switch of `app-database-url` to `econ_graph_app`. With `--restore-source-app`, the
   rollback and the switch back to `econ_graph` instead. If the switch fails, the run
   fails, the backend stays stopped, and the message gives the `create-secrets.sh`
   command to run.

The check step prints the kubectl it uses: the cluster's own copy under
`${XDG_CACHE_HOME:-~/.cache}/econgraph/bin/kubectl`, so in a rehearsal with a scratch
`XDG_CACHE_HOME` it is there. `--dry-run` performs the checks of step 1 and prints
every later step. It takes no backup,
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

List the rows with that kubectl, never the host's plain `kubectl` (on this host it is
MicroK8s), with the same `XDG_CACHE_HOME` as the wrapper run:

```sh
"${XDG_CACHE_HOME:-$HOME/.cache}/econgraph/bin/kubectl" --context kind-NAME -n econ-graph \
    exec postgresql-0 -c postgresql -- psql -U postgres -d econ_graph -Atc 'select id, name from data_sources'
```

Every source that has an economic series must be mapped. The script stops if any series ID
differs from UUIDv5 of `SOURCE:external_id`; do not change IDs to make it pass.

## Rehearse on a copy of production

1. Take a backup of production, in a shell without the scratch variables below. It only
   reads the cluster: `KIND_CLUSTER=econ-graph scripts/deploy/backup-kind-databases.sh`
   writes `~/econgraph-backups/<UTC timestamp>/`.
   Note `SERIES_COUNT` and `DATA_POINTS_COUNT` from its `counts.env`.
2. Create a scratch cluster at the release production runs, as in
   [Testing on a scratch cluster](HOST_STORAGE.md#testing-on-a-scratch-cluster): export
   `KIND_CLUSTER=<name>`, `ECONGRAPH_SCRATCH_STORAGE`, `XDG_CACHE_HOME` and a scratch
   `ECONGRAPH_BACKUP_ROOT`, then `scripts/deploy/recreate-kind-cluster.sh --if-missing
   --config <copy>`.
3. Restore the production backup into it:
   `KIND_CLUSTER=<name> APP_DATABASE=shared scripts/deploy/restore-kind-databases.sh
   --from ~/econgraph-backups/<timestamp> --expect-series N --expect-data-points N`. This
   brings over the combined `econ_graph`, Keycloak and the Secrets the databases were
   created with; the restored `econ-graph-secrets` has no `app-database-url`, hence
   `APP_DATABASE=shared`.
4. Deploy the release being rolled out to the scratch cluster, keeping the backend on the
   combined database: `KIND_CLUSTER=<name> APP_DATABASE=shared scripts/deploy/deploy.sh`.
   It creates `app-postgres` with an empty `econ_graph_app`. Check out that release for
   the steps below too.
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

7. Start the backend, which now finds `app-database-url` naming `econ_graph_app`, with
   `KIND_CLUSTER=<name> scripts/deploy/deploy.sh`, then check that
   existing users sign in and their annotations still show on their original series.
   Run the tagged crawler's `migrate --dry-run` against the data database to confirm
   nothing is pending.

8. Measure the first-deploy effects of the catalog lifecycle for the release ticket
   (ECO-803) on the restored copy. Forward the scratch cluster's data database to a local
   port, then run the read-only report with the database password in `PGPASSWORD`, and
   `FRED_API_KEY` set for the FRED selection check:

   ```sh
   KUBECONFIG=<scratch cache>/econgraph/kubeconfig-<name> <scratch cache>/econgraph/bin/kubectl \
     --context kind-<name> -n econ-graph port-forward svc/data-postgres-service 15432:5432 &
   PGPASSWORD=<data database password> FRED_API_KEY=<key> \
     scripts/deploy/measure-v41-first-deploy.sh --database-url postgres://postgres@127.0.0.1:15432/econ_graph
   ```

   It prints a Markdown report: inactive BEA series by catalog-selection rule, the
   (series, date) pairs ECO-807 asks about, and the FRED selection against its cap. Unless
   given `--i-know-this-is-production`, it refuses any target that may be production's data
   database: an in-cluster name, a host that doesn't resolve, or a local port served by
   anything but a local Postgres, a container port or a port-forward into another cluster or
   namespace (a port-forward into the production cluster's `econ-graph` namespace, or one it
   can't place, is refused). It runs every statement in a read-only transaction with a
   statement timeout. Without `FRED_API_KEY` the FRED check is reported as skipped.

The scratch backup root then holds full copies of production records; it has the same
permissions and handling as below. The synthetic test in CI does not replace this
rehearsal.

## Convert production

Production first gets the release with the app database while keeping the combined
database: `KIND_CLUSTER=econ-graph APP_DATABASE=shared PUBLIC_HOST=<the public host>
scripts/deploy/deploy.sh`. Then, from that release's checkout and after a rehearsal of
it, run the same conversion commands with `--cluster econ-graph --production`, a
`--dry-run` first.

Without `--drop-source-app` the wrapper copies, verifies and puts every writer back: the
backend keeps using `econ_graph` and the copy in `econ_graph_app` goes stale. A later run
replaces that stale copy only when given its `--previous-evidence`; any other existing app
data in `econ_graph_app` is refused.

With `--drop-source-app`, the third step asks for `DROP APP TABLES FROM econ_graph` on the
terminal. There is no bypass. While the prompt waits, the copy is committed and the source
is still locked, so the evidence can be reviewed from another terminal. The removal drops
the app tables and their five enum types without `CASCADE` (the three cross-plane foreign
keys go with their tables) and deletes only the app migration entries, recording which.
Every data table and the series-ID checksum must be unchanged before it commits, and the
evidence is marked `pending` before the commit so a failure right after it is still
recoverable. A wrong phrase, a refusal or a failed check leaves the source as it was, with
the verified copy committed and every writer put back.

After a removal `app-database-url` names `econ_graph_app` and the backend stays stopped.
Start it with `KIND_CLUSTER=econ-graph PUBLIC_HOST=<the public host>
scripts/deploy/deploy.sh` (`APP_DATABASE` unset, or `separate`; `create-secrets.sh` refuses
`shared` from now on) and check users and annotations as in the rehearsal.

The backup CronJob is resumed when the wrapper exits, after the switch, so its next run
reads `app-database-url` as `econ_graph_app` and dumps it next to `econ_graph`.

## Roll back a removal

```sh
scripts/deploy/convert-app-data.sh --cluster econ-graph --production \
    --restore-source-app ~/econgraph-backups/<timestamp of the removal run>
```

The rollback puts the app tables back into `econ_graph` from the dump the removal run took
just before it dropped them. It requires that directory's `evidence.json` to record a
removal (`source_app_removed` true, or `pending` when the run failed after its commit),
the dump to match its recorded SHA-256, the source to be the same database (by system
identifier and name) holding the data tables, and the checkout to match
what was removed: the app tables, enum types and migration versions are taken from the
evidence and compared with the checkout before anything is restored.

The source normally has no app tables. If it already has all of them, from an earlier
rollback that stopped before recording the migration versions or from a removal that never
committed, nothing is restored: the existing tables are verified against the fingerprints,
and only on a match are the versions recorded. Tables the rollback did not restore itself
are never dropped. Some but not all app tables is refused.

From `pg_restore --list` it selects only the app tables, their rows, the app enum types
and the indexes, constraints, triggers, comments and privileges on them, writes that list
to `restore.list` in a new directory under the backup root, and restores it in one
transaction. It then locks the restored tables, checks each against the row counts and
content hashes recorded before the removal, and adds back the migration entries the
removal deleted. If the contents differ, it drops what it just restored, so the source is
as it was before the rollback, and stops. Data tables are not touched; crawl data written
after the conversion stays.

Rows written to `econ_graph_app` after the switch are not carried back. The app database
is left as it is. `app-database-url` names `econ_graph` again and the backend stays
stopped; start it with `KIND_CLUSTER=econ-graph PUBLIC_HOST=<the public host>
scripts/deploy/deploy.sh`.

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
`kind` and Python with `HOME`, the cache and the backup root in a scratch directory,
including failures and a hangup while the writers are stopped.
`scripts/deploy/test-app-database-guard.sh` checks the `create-secrets.sh` guard. None of
these tests connects to a cluster or reads production configuration.
