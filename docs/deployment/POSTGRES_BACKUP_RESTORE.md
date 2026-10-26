# PostgreSQL Backups and Restore

The `econ-graph` namespace runs three PostgreSQL databases
([k8s/README.md](../../k8s/README.md#databases) has the table):

| Database | StatefulSet | Service | Holds |
|----------|-------------|---------|-------|
| `econ_graph` (data) | `postgresql` | `data-postgres-service` (alias `postgres-service`) | sources, series, observations, crawl state |
| `econ_graph_app` (app) | `app-postgres` | `app-postgres-service` | users and everything they create |
| `keycloak` | `keycloak-postgres` | `keycloak-postgres-service` | user accounts and their linked Google identities |

The in-cluster `postgres-backup` CronJob dumps them nightly to a separate
volume, and can ship them to object storage. It dumps `econ_graph_app` only
when the backend's `app-database-url` names it (`APP_DATABASE=separate`).
With `APP_DATABASE=shared`, `app-database-url` names the data database and
the app tables live in `econ_graph`, so the `econ_graph` dump holds them; see
[Which app database](#which-app-database).

On threadripper (production), the primary protection is host-side: btrfs
snapshots, verified nightly dumps of the same databases
(`scripts/backup/dump-databases.sh`), a weekly restore drill and a freshness
check; see [BACKUPS.md](BACKUPS.md). The CronJob described here is an
independent second copy: it runs inside the cluster, needs nothing on the
host, and also works on a cluster without the host tooling. The two share no
files, schedule or lock, and neither deletes the other's dumps.
BACKUPS.md says how the two fit together and when the CronJob may be retired.

## What's in place

- **`postgres-backup-cronjob.yaml`**: a nightly (`0 8 * * *` UTC) `CronJob`
  whose `pg-dump` container `pg_dump`s `econ_graph`, `econ_graph_app` and
  `keycloak` to `/backups` on the **`postgres-backup-data`** PVC:
  a separate volume from the databases' own PVCs, so losing/recreating a
  primary volume doesn't take the backups with it. Its `backup-sync`
  container then optionally syncs `/backups` to S3 (or an S3-compatible
  endpoint), controlled by env vars; see
  [Object storage](#object-storage-optional) below.
- **`postgres-backup-pvc.yaml`**: the 20Gi backup volume, default
  StorageClass. For a real cluster, point `storageClassName` at storage in a
  different failure domain than the database volumes, or rely on the S3 sync
  instead.
- **`postgres-backup-configmap.yaml`**: the `backup.sh` / `sync.sh` /
  `restore.sh` scripts the CronJob and restore job run. They never delete
  dumps.
- **`scripts/deploy/protect-postgres-pv.sh`**: sets the bound
  `PersistentVolume`s for `econ_graph`'s data and the backups to
  `persistentVolumeReclaimPolicy: Retain`. It runs after PostgreSQL comes
  up and again before teardown. A backup PVC using delayed binding may not
  have a PV until the first backup job; teardown protects it once bound.
  Deleting either PVC leaves its PV `Released` for manual reattachment.
- **`scripts/deploy/teardown.sh`**: dumps the data, app and Keycloak
  databases first and refuses when `postgresql-0` is not running unless
  `--no-backup` is given. It sets the bound PVs to Retain and stops if that
  step fails. By default it deletes only the app plane (routes, frontend,
  backend, chart API, Keycloak, and the app and Keycloak Postgres
  StatefulSets), after the namespace name is typed on a terminal or `--yes`;
  their Secrets and PVCs stay. The data plane (the `postgresql` StatefulSet,
  its Services, the crawler, this CronJob), the Secrets and the `econ-graph`
  namespace, which owns the PostgreSQL and backup PVCs, go only with
  `--include-data-plane` and `econ_graph` typed on a terminal; `--yes` never
  answers that. `--include-data-plane --delete-cluster` also deletes the kind
  cluster, whose nodes hold the in-cluster backup volume and Grafana's data,
  after the cluster name is typed. A confirmation that can't be typed stops
  the script before it changes anything. `--dry-run` prints every step and
  changes nothing.
- **`scripts/deploy/backup-postgres-now.sh`**: triggers an ad-hoc backup
  without waiting for the nightly schedule.
- **`scripts/deploy/restore-postgres-backup.sh`**: restores `econ_graph`.
- **`scripts/deploy/test-postgres-backup-scripts.sh`**: tests the three
  scripts; see [Testing](#testing).

## What a run writes

Each run takes one UTC timestamp and writes, on the backup PVC:

| File | Contents |
|------|----------|
| `econ_graph-<YYYYMMDD-HHMMSS>.dump` | `pg_dump --format=custom` of `econ_graph`, from `data-postgres-service` as `postgres` |
| `econ_graph_app-<YYYYMMDD-HHMMSS>.dump` | `pg_dump --format=custom` of `econ_graph_app`, from `app-postgres-service` as `postgres`, when `APP_DATABASE=separate` |
| `keycloak-<YYYYMMDD-HHMMSS>.dump` | `pg_dump --format=custom` of `keycloak`, from `keycloak-postgres-service` as `keycloak` |
| `latest.dump` | symlink to the newest good `econ_graph` dump |
| `latest-econ_graph_app.dump` | symlink to the newest good `econ_graph_app` dump |
| `latest-keycloak.dump` | symlink to the newest good `keycloak` dump |

Every dump uses `--no-owner --no-privileges` and `--lock-wait-timeout=600s`:
`pg_dump` waits at most 10 minutes for its share lock on the tables (only an
exclusive lock, such as a migration's `ALTER TABLE` or a `VACUUM FULL`,
blocks it), then fails that dump instead of waiting until the Job deadline.
`PG_DUMP_LOCK_WAIT_TIMEOUT` overrides it. Each is written as
`<name>.partial` and only renamed once it passes every check: it is not
empty, `pg_restore --file=/dev/null` reads the whole archive back, and
`pg_restore --list` shows at least one table. A dump that fails a check is
removed, and the `latest` link keeps pointing at the previous good one.

### Which app database

`backup.sh` decides whether there is a separate app database to dump from
the URL the backend uses for it: `app-database-url` in the Secret
`econ-graph-secrets` (read with `optional: true`). It compares that URL's
host, port and database name with the database it dumps as `econ_graph`
(`PGHOST`, `PGPORT`, `PGDATABASE`: `data-postgres-service:5432/econ_graph`)
and with the app database (`APP_PGHOST`, `APP_PGPORT`, `APP_PGDATABASE`:
`app-postgres-service:5432/econ_graph_app`), and never prints the URL. Hosts
are compared in lower case, without this namespace's Service suffix
(`.econ-graph`, `.econ-graph.svc`, `.econ-graph.svc.cluster.local`), with
`postgres-service` read as `data-postgres-service`; an IP literal is
compared whole. A URL with several hosts, without a database, or with
`host`, `hostaddr`, `port`, `dbname` or `service` in its options can't be
read this way.

| `app-database-url` | What the run does |
|--------------------|-------------------|
| names the database dumped as `econ_graph` (`APP_DATABASE=shared`) | skips the app dump with one log line; the app tables are in the `econ_graph` dump |
| names `app-postgres-service:5432/econ_graph_app` (`APP_DATABASE=separate`) | dumps `econ_graph_app` with the checks above; a missing password, an unreachable server or a database without tables fails it |
| names any other database (another host or IP, the same host with another database), or can't be read | fails the app dump: the database the backend keeps its users in would otherwise go unbacked |
| is missing (Secret or key missing) | skips the app dump with one log line, or fails it with `APP_REQUIRED=true` |

`APP_REQUIRED` is `false` in the manifest. Set it to `true` on a cluster that
must have `app-database-url`, so that losing it is noticed:

```sh
kubectl -n econ-graph set env cronjob/postgres-backup -c pg-dump APP_REQUIRED=true
```

With `APP_DATABASE=separate`, the app database has tables once the backend has
started against it (it applies the app migrations at startup), so the table
check fails only a run that comes before the backend's first start.

### Keycloak

The CronJob reads the app database's password from the Secret
`econ-graph-app-postgres` (key `postgres-password`) and Keycloak's from
`econ-graph-keycloak` (key `db-password`), both with `optional: true`, so a
missing Secret doesn't stop the pod or the other dumps. The manifest sets
`KEYCLOAK_REQUIRED=true`, which makes a missing or empty Keycloak password a
failed dump. `deploy.sh` deploys Keycloak only when its Secret exists; on a
cluster deployed without it, set `KEYCLOAK_REQUIRED` to `false` and the run
logs that Keycloak is skipped:

```sh
kubectl -n econ-graph set env cronjob/postgres-backup -c pg-dump KEYCLOAK_REQUIRED=false
```

`deploy.sh` re-applies the manifest, so `set env` is needed again after each
deploy.

### When one dump fails

The dumps are independent: each is attempted even if another fails,
and a good dump is kept whatever happens to the others. `backup.sh` records
the names of the failed dumps in `/status/failed`, an `emptyDir` shared with
`backup-sync`, and exits 0 so that `sync.sh` still runs and ships the dumps
that succeeded. `sync.sh` then exits 1 (and also if `/status/failed` is
missing), so the Job fails and its pod shows which dump failed:

```sh
kubectl -n econ-graph get jobs -l app=postgres-backup
kubectl -n econ-graph logs job/<job-name> --all-containers=true --prefix=true
```

A container that exits non-zero fails the Job at once, without a retry
(`podFailurePolicy`: `FailJob` on any non-zero exit code). A retry is a full
run with its own timestamp: it would most likely fail the same way, and would
write another full dump of every database that did succeed each time, which
nothing prunes. Only a
pod ended by a disruption (eviction, node drain, preemption, node loss) is
retried, up to twice (`backoffLimit: 2`): a `Count` rule for the pod's
`DisruptionTarget` condition comes before the `FailJob` rule, because a
disrupted container still ends with an exit code (137). So a night where a dump
fails costs one extra dump of each other database at most, and the next try
is the next night's run.

Each run has a 4-hour deadline (`activeDeadlineSeconds: 14400`), so a hung
run can't hold the CronJob's `concurrencyPolicy: Forbid` and block later
nights. The volumes are 50Gi for `econ_graph`, 10Gi for `econ_graph_app` and
5Gi for `keycloak`; `pg_dump --format=custom` runs at about 25MB/s of
database on one core, so all three, full, take about 50 minutes to dump and
check, and the deadline leaves room for the lock waits and the sync.

Run outside the CronJob without `BACKUP_STATUS_FILE`, `backup.sh` itself
exits 1 when a dump fails.

### Monitoring

No Prometheus rule watches the backup Job, so a failed or missing nightly
dump of any database shows only as a failed Job (above) and in the
dumps' timestamps.

## Credentials

The `econ_graph` dump and restore read `postgres-password` from the same
`econ-graph-postgres` Secret as the `postgresql` StatefulSet. The CronJob
also reads `app-database-url` from `econ-graph-secrets`, which holds the
app role's password; it uses only the host, port and database name from it
and never prints it. The
`econ_graph_app` dump connects as `postgres` with `postgres-password` from
`econ-graph-app-postgres`, the Secret of the `app-postgres` StatefulSet. The
Keycloak dump connects as `keycloak` with `db-password` from
`econ-graph-keycloak`, the password `keycloak-postgres` and Keycloak use.
Each reaches `pg_dump` only through `PGPASSWORD` in its environment; the
scripts never print them. The
`scripts/deploy/create-secrets.sh` script preserves existing values. A
password change in an existing data volume still requires an explicit
database-side change; verify with a manual backup after deployment.

## Manual backup

```sh
./scripts/deploy/backup-postgres-now.sh
```

Creates and waits on a one-off `Job` from the `postgres-backup` CronJob,
then prints its logs. It dumps the same databases as the nightly run.

## Restore

### econ_graph

```sh
# See what's available
./scripts/deploy/restore-postgres-backup.sh --list

# Restore the latest backup (prompts for confirmation)
./scripts/deploy/restore-postgres-backup.sh

# Restore a specific dump, without the confirmation prompt
./scripts/deploy/restore-postgres-backup.sh --file econ_graph-20260101-080000.dump --yes
```

This is **destructive**: `pg_restore --clean --if-exists` drops and
recreates every object the dump contains before restoring it, overwriting
whatever is currently in `econ_graph`. The restore Job sets
`RESTORE_ROLE=econgraph`, so `restore.sh` runs `pg_restore --role=econgraph`:
the dumps carry no owner, and `econgraph`, the role the backend and crawler
connect as and the one the data migrations created the tables as, owns
everything the restore recreates, as before. Restored as `postgres` alone,
the tables would belong to `postgres` and the services could not use them.
The extensions in the dump are left as they are, since the init script
created some of them as the superuser and `econgraph` can't drop or recreate
those; `restore.sh` refuses, before changing anything, a database that lacks
one of them. That's why it prompts unless `--yes`
is passed. `restore.sh` refuses a file that isn't an `econ_graph-*.dump`
(such as an `econ_graph_app` or Keycloak dump).

Before restoring into a real deployment, scale `backend-deployment` and
`crawler-worker` to 0 replicas first: `--clean` waits on any lock they hold
(a live connection mid-query can stall the restore indefinitely), and the
crawler writing new rows mid-restore can race with it. Also note that
`--clean` only drops objects present in the dump: tables created by
migrations that ran *after* the dump was taken are left behind, which can
disagree with Diesel's migration history after the restore; check
`diesel migration list` / `__diesel_schema_migrations` afterwards.

### keycloak

A Keycloak restore replaces every account, linked Google identity and
session with the dump's; anything written to Keycloak after the dump is lost.
Take a manual backup first (`./scripts/deploy/backup-postgres-now.sh`) so the
state being replaced is kept. The steps below drop and recreate the
`keycloak` database rather than using `pg_restore --clean`, which would leave
behind tables a newer Keycloak version added. Keycloak migrates an older
schema forward on start, but can't run on a schema a newer version wrote:
restore a dump with the Keycloak version that wrote it, or a newer one.

1. Pick the dump (`./scripts/deploy/restore-postgres-backup.sh --list` lists
   all three kinds) and copy it off the backup PVC, while no backup Job runs:

   ```sh
   DUMP=keycloak-20260101-080000.dump
   kubectl -n econ-graph run postgres-backup-copy --restart=Never --image=busybox:1.38 \
     --overrides='{"spec":{"securityContext":{"runAsNonRoot":true,"runAsUser":999},"containers":[{"name":"copy","image":"busybox:1.38","command":["sleep","600"],"volumeMounts":[{"name":"backup-data","mountPath":"/backups","readOnly":true}]}],"volumes":[{"name":"backup-data","persistentVolumeClaim":{"claimName":"postgres-backup-data"}}]}}'
   kubectl -n econ-graph wait --for=condition=Ready pod/postgres-backup-copy --timeout=120s
   kubectl -n econ-graph cp "postgres-backup-copy:/backups/${DUMP}" "./${DUMP}"
   kubectl -n econ-graph delete pod postgres-backup-copy
   ```

   The dump holds password hashes and client secrets; keep the local copy
   private and remove it when done.

2. Stop Keycloak, so nothing writes to the database during the restore:

   ```sh
   kubectl -n econ-graph scale deployment/keycloak --replicas=0
   # Until no Keycloak pod is left (returns at once if there was none)
   while kubectl -n econ-graph get pods -l app=keycloak -o name | grep -q .; do sleep 2; done
   ```

3. Recreate the database and restore the dump into it:

   ```sh
   kubectl -n econ-graph exec keycloak-postgres-0 -- dropdb -U keycloak --if-exists keycloak
   kubectl -n econ-graph exec keycloak-postgres-0 -- createdb -U keycloak keycloak
   kubectl -n econ-graph exec -i keycloak-postgres-0 -- \
     pg_restore -U keycloak -d keycloak --no-owner --no-privileges \
     --exit-on-error --single-transaction < "./${DUMP}"
   ```

4. Start Keycloak again: `./scripts/deploy/deploy.sh`, or
   `kubectl -n econ-graph scale deployment/keycloak --replicas=1`.

### econ_graph_app

An app database restore replaces every user and everything they created with
the dump's. Take a manual backup first, as for Keycloak. The backend connects
as the role `econgraph`, which owns the app database's tables (the backend
creates them, running the app migrations as `econgraph`). The CronJob's dumps
carry no owner (`--no-owner`), so the restore runs with
`--role=econgraph`, into a database `econgraph` owns, to give the tables
back to it; restored as `postgres` alone they would belong to `postgres`, and
the backend could not use them. For the same reason,
`scripts/backup/restore-dump.sh` (which restores the host-side dumps, which
do keep their owners) is not for these dumps.

1. Copy the dump off the backup PVC as in step 1 above, with
   `DUMP=econ_graph_app-20260101-080000.dump`.

2. Stop the backend, the only client of the app database:

   ```sh
   kubectl -n econ-graph scale deployment/econ-graph-backend --replicas=0
   while kubectl -n econ-graph get pods -l app=econ-graph-backend -o name | grep -q .; do sleep 2; done
   ```

3. Recreate the database, owned by `econgraph`, and restore the dump into it
   as `econgraph`:

   ```sh
   kubectl -n econ-graph exec app-postgres-0 -c postgres -- dropdb -U postgres --if-exists econ_graph_app
   kubectl -n econ-graph exec app-postgres-0 -c postgres -- createdb -U postgres -O econgraph econ_graph_app
   kubectl -n econ-graph exec -i app-postgres-0 -c postgres -- \
     pg_restore -U postgres -d econ_graph_app --role=econgraph --no-owner --no-privileges \
     --exit-on-error --single-transaction < "./${DUMP}"
   ```

4. Start the backend again: `./scripts/deploy/deploy.sh`, or scale
   `deployment/econ-graph-backend` back to its replica count. It applies any
   app migrations newer than the dump at startup.

## Testing

```sh
scripts/deploy/test-postgres-backup-scripts.sh
```

Runs `backup.sh`, `sync.sh` and `restore.sh` from the ConfigMap in the
CronJob's images and security settings (uid 999, read-only root), with the
`pg-dump` container's environment read from the CronJob manifest, against
scratch `postgres:18` containers standing in for `data-postgres-service`
(also aliased `postgres-service`), `app-postgres-service` and
`keycloak-postgres-service`, and an S3 mock. It checks that all three dumps
are written, restore with every table's row count equal to the source (the
Keycloak one through `pg_restore` on stdin, as in its step 3 above) and sync
without the `latest` links; that the app dump, restored as in
[econ_graph_app](#econ_graph_app) step 3, leaves `econgraph` owning its
tables and able to write them, with the source's schema; that `restore.sh`, as the restore Job runs it,
puts `econ_graph` back with `econgraph` still owning and able to write its
tables, the schema (tables, indexes, constraints, views, functions,
extensions) equal to the source's and the superuser's extensions untouched, refuses a database lacking
one of the dump's extensions, and refuses an app or Keycloak dump; that `backup.sh` reads
each kind of `app-database-url` as intended (case, Service suffixes, IP
literals, options, unreadable URLs); that the app database is dumped when
`app-database-url` names `app-postgres-service:5432/econ_graph_app` (also in
upper case with the full Service name and options), skipped with one log
line, the run succeeding, when it names the database dumped as `econ_graph`,
and failed when it names any other database (another host, the same host
with another database, an IP, a host in the options) or can't be read,
whatever `APP_REQUIRED` says; that a missing `app-database-url` is skipped,
or failed with `APP_REQUIRED=true`; that a failure of any dump (server down,
wrong password, no tables, a table locked past the lock wait) keeps and
syncs the others and fails the Job; that a missing app password in separate mode fails the run; that a missing
Keycloak password is a failure with `KEYCLOAK_REQUIRED=true` and a skip with
`false`; that the Job has a
deadline and a `podFailurePolicy` that doesn't retry a failed container;
that no run deletes a dump; and that no output contains a password. It needs Docker and
python3 with PyYAML, and runs in CI ("PostgreSQL Backup Scripts").

It does not cover the in-cluster wiring (the PVC mounts, the Secrets, the
CronJob schedule, the Services' selectors, network policy). Check that on a
cluster with `backup-postgres-now.sh` followed by
`restore-postgres-backup.sh --list`.

## Object storage (optional)

By default backups only live on the `postgres-backup-data` PVC. To also
push them to S3 (or an S3-compatible service like MinIO), edit the
`backup-sync` container's env in `postgres-backup-cronjob.yaml`:

| Env var              | Purpose                                          |
|-----------------------|---------------------------------------------------|
| `BACKUP_DESTINATION`  | Set to `s3` to enable the sync (default: PVC only) |
| `BACKUP_S3_BUCKET`    | Target bucket                                     |
| `BACKUP_S3_PREFIX`    | Key prefix (default `postgres/econ-graph`)        |
| `BACKUP_S3_ENDPOINT`  | Optional custom endpoint (MinIO, etc.)            |

The sync uploads every dump on the PVC that isn't in the bucket yet,
`econ_graph-*`, `econ_graph_app-*` and `keycloak-*`, under the same prefix; it skips the
`latest*.dump` links and any `*.partial` left by a killed run, and never
deletes from the bucket. A run where a dump failed still syncs before
failing the Job.

AWS credentials need to reach the `backup-sync` container (e.g. via
`envFrom.secretRef` for a Secret holding `AWS_ACCESS_KEY_ID` /
`AWS_SECRET_ACCESS_KEY`, or IAM roles for service accounts on EKS). They are
not wired up by default.

## Reattaching a Retain'd volume

Once a PV's reclaim policy is `Retain`, deleting its PVC leaves the PV
behind in the `Released` phase; it isn't automatically reused. A plain
redeploy creates a *new*, empty PVC/PV pair, quietly giving you a fresh,
empty database instead of your old data. To bind back to the old volume:

1. `kubectl get pv` and find the `Released` one (check its capacity/claimRef
   to confirm it's the right one).
2. `kubectl patch pv <pv-name> -p '{"spec":{"claimRef": null}}'` to clear its
   claim so it's `Available` again.
3. Apply `postgres-deployment.yaml` as usual; if the new PVC doesn't bind to
   it automatically (StatefulSet-created PVCs don't pin a specific volume),
   create a PVC by hand requesting that exact PV via `spec.volumeName`
   before applying the StatefulSet.

## Retention

Nothing deletes backups automatically: every nightly dump of every database
stays on the PVC until an operator removes it. Watch the PVC's usage (20Gi by
default, see `postgres-backup-pvc.yaml`), and when it fills, copy the dumps
you want to keep elsewhere before deleting old ones by hand. List them with:

```bash
./scripts/deploy/restore-postgres-backup.sh --list
```

The PVC is `ReadWriteOnce`, so delete from a short-lived pod that mounts it while no
backup Job is running, naming each dump explicitly:

```bash
kubectl -n econ-graph run postgres-backup-cleanup --rm -i --restart=Never --image=busybox:1.38 \
  --overrides='{"spec":{"containers":[{"name":"cleanup","image":"busybox:1.38","stdin":true,"command":["rm","-v","/backups/econ_graph-20260101-080000.dump"],"volumeMounts":[{"name":"backup-data","mountPath":"/backups"}]}],"volumes":[{"name":"backup-data","persistentVolumeClaim":{"claimName":"postgres-backup-data"}}]}}'
```
