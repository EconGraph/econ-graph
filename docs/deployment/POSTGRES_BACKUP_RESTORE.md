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
  `keycloak` to a private memory-backed `/plaintext` volume. The `encrypt`
  stage encrypts verified archives to `/backups` on the **`postgres-backup-data`** PVC:
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
  `install-age.sh`, `encrypt.sh` and `restore.sh` scripts the CronJob and
  legacy restore job run. They never delete completed dumps.
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

## Encryption setup

Before deploying this CronJob, set `BACKUP_AGE_RECIPIENT` on its `encrypt`
container to the maintainer's native `age1...` **public recipient**. This is
public configuration and can be committed or rendered in a local overlay.
An empty, malformed or private-key value fails the job; there is no plaintext
fallback. Keep the private identity only on the operator's host, outside
backup directories. Never create a Kubernetes Secret containing that identity.

`install-age` uses a digest-pinned curl image to download official age 1.3.2
archives for amd64 or arm64 and checks the release's SHA-256 before extracting
the tools into a pod-local `emptyDir`. The encryption stage mounts those tools
read-only. A fresh pod needs outbound HTTPS to GitHub releases; a download or
checksum failure fails visibly rather than writing an unencrypted backup.

Plaintext archives exist only in a memory-backed `emptyDir`, with `umask 077`.
Its default capacity is **8Gi for the combined compressed dumps**, not a RAM
reservation. Size it and the node's available memory before enabling large
backups. A full tmpfs fails the affected dump and still permits the other
successful dumps to be encrypted and synced; it does not spill to the PVC.
Account for node swap policy when deciding whether tmpfs meets the host's
at-rest requirements. Successful encryption releases each temporary plaintext archive. Failed
encryption can retain staging until Kubernetes removes the pod; inspect
failures and clean up finished Jobs as part of operations.

## What a run writes

Each run takes one UTC timestamp and writes these ciphertext files on the PVC:

| File | Contents |
|------|----------|
| `econ_graph-<YYYYMMDD-HHMMSS>.dump.age` | encrypted custom archive of the data database |
| `econ_graph_app-<YYYYMMDD-HHMMSS>.dump.age` | encrypted custom archive of the separate app database, when selected |
| `keycloak-<YYYYMMDD-HHMMSS>.dump.age` | encrypted custom archive of Keycloak |
| `latest.dump.age` | symlink to the newest encrypted data archive |
| `latest-econ_graph_app.dump.age` | symlink to the newest encrypted app archive |
| `latest-keycloak.dump.age` | symlink to the newest encrypted Keycloak archive |

`backup.sh` first writes each custom archive on tmpfs, reads it back fully
with `pg_restore --file=/dev/null`, and requires at least one table. Dumps
keep `--no-owner --no-privileges` and the 600-second lock wait. Successful
archives are then encrypted to exclusive `.partial` outputs on the PVC and
published atomically without overwriting a prior archive, only after age
succeeds. Ciphertext files are mode 0600. An existing
output is preserved and records a failure. Encryption failures are added to
the shared status file so successful archives can still sync before the Job
fails. No private identity is available to verify decryption in the pod; the
operator must run a restore drill with their key.

Old plaintext backups on the PVC remain untouched for manual migration.
The sync stage uploads only completed `*.dump.age` files, follows no symlinks,
and excludes plaintext, partials and unrelated files. It never prunes objects.

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

## Restore encrypted archives

Recovery is an operator action. Copy the chosen `.dump.age` file off the PVC
using the read-only copy pod described below, or download it from object
storage. Keep it encrypted on persistent local storage. On the operator host,
use a private tmpfs directory and decrypt with a locally held identity:

```bash
umask 077
STAGE=$(mktemp -d /dev/shm/econgraph-restore.XXXXXX)
# The identity path is local; do not copy its contents into a pod or a Secret.
read -r -p 'Local age identity path: ' IDENTITY
age --decrypt --identity "$IDENTITY" \
  --output "$STAGE/econ_graph-20260101-080000.dump" \
  econ_graph-20260101-080000.dump.age
pg_restore --file=/dev/null "$STAGE/econ_graph-20260101-080000.dump"
```

Restore the decrypted archive from this operator host over an authenticated
PostgreSQL connection (for example, a port-forward to the selected database).
Stop writers first. Preserve the ownership and extension handling described
below: data and app objects belong to `econgraph`; Keycloak objects belong to
`keycloak`. With a data dump, pre-create its required superuser-owned extensions
or restore only the extension TOC entries as the superuser, then exclude those
entries from the transactional `--clean --if-exists --role=econgraph` restore.
A plain superuser restore loses the application role's ownership. Verify row
counts and application access before restarting writers. Remove only this
operator-created tmpfs staging directory when finished; keep the ciphertext.

The in-cluster legacy restore Job does not decrypt `.age` files and refuses
them with instructions to use operator-side recovery. Default recovery also
refuses retained plaintext when encrypted data archives exist, including when
latest metadata is dangling or missing. Do not upload the
decrypted archive back to the backup PVC to use that Job. The legacy
instructions below apply only to retained pre-encryption `.dump` backups;
their read-only PVC-copy method can also fetch `.dump.age` files.

## Restore legacy plaintext archives

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
whatever is currently in `econ_graph`. That's why it prompts unless `--yes`
is passed. `restore.sh` refuses a file that isn't an `econ_graph-*.dump`
(such as an `econ_graph_app` or Keycloak dump), and a dump that lists no
table. It resolves `latest.dump` to the dump it points at once, before
reading anything, so a backup that finishes during the restore and moves the
link doesn't change which dump is restored.

#### Ownership

The restore Job connects as `postgres`, but every object it recreates is
owned by `econgraph`, the role the backend and crawler-worker connect as to
`econ_graph` and the one the data migrations create the schema as (the Job's
`RESTORE_OWNER`; `restore.sh` defaults to `econgraph` when it is unset). The
dumps carry no owners or grants (they are taken with `--no-owner
--no-privileges`), so a plain `pg_restore` run as `postgres` would leave every
table owned by `postgres`, and the services could neither read nor write it.
`restore.sh` runs in two steps:

1. **Extensions**, as `postgres`: the dump's extensions that `econ_graph`
   lacks, with their comments, as `CREATE EXTENSION IF NOT EXISTS`. Each one
   created is logged (`Created extension pgcrypto`). An extension already
   there is never dropped or changed, owner and comment included. On a newly
   initialised server, where the init script has created `uuid-ossp`,
   `pg_stat_statements` and `pg_trgm`, this adds `pgcrypto`, owned by
   `postgres`. `econgraph` can't do this step itself: some extensions need a
   superuser to create, and the init script's are owned by `postgres`, so
   `econgraph` can't drop them either.
2. **Everything else**, with `pg_restore --role=econgraph --clean --if-exists
   --no-owner --no-privileges --single-transaction --exit-on-error`, the
   extension entries left out by their type in the dump's table of contents:
   after connecting, `pg_restore` switches to `econgraph`, which drops and
   recreates the tables, sequences, views, indexes, types, functions,
   schemas and `__diesel_schema_migrations` as their owner. A failure rolls
   this step back, leaving the database as it was. An extension step 1
   created stays.

`restore.sh` checks that `RESTORE_OWNER` exists before changing anything.
Every object in the dump must be owned by `econgraph` in the database being
restored into, since that role drops them. If one isn't, the restore stops at
`must be owner of ...` and rolls back. Give the object to `econgraph`
(`ALTER TABLE ... OWNER TO econgraph`, or the matching `ALTER` for another
kind of object), then restore again. To check the result, as `postgres` in
`econ_graph`:

```sql
SELECT tableowner, count(*) FROM pg_tables
 WHERE schemaname NOT IN ('pg_catalog', 'information_schema') GROUP BY 1;
```

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
CronJob's images with a read-only root and no capabilities (the fixture uid
maps to the calling host account, including on rootless Docker), with the
`pg-dump` container's environment read from the CronJob manifest, against
scratch `postgres:18` containers standing in for `data-postgres-service`
(also aliased `postgres-service`), `app-postgres-service` and
`keycloak-postgres-service`, and an S3 mock. It checks that all three dumps
are written, restore with every table's row count equal to the source (the
Keycloak one through `pg_restore` on stdin, as in its step 3 above) and sync
without the `latest` links; that the app dump, restored as in
[econ_graph_app](#econ_graph_app) step 3, leaves `econgraph` owning its
tables and able to write them, with the source's schema; that `restore.sh`,
with the restore Job's environment, puts `econ_graph` back with `econgraph`
owning everything but the extensions and able to write its tables, the
schema (tables, indexes, constraints, views, functions, extensions) equal to
the source's and every extension untouched, owner and comment included (a
comment set on the target survives); that on a newly initialised server (the
init script, with `uuid-ossp` then dropped, and `RESTORE_OWNER` unset) it
creates and logs the two extensions that server lacks, `pgcrypto` and
`uuid-ossp`, each with the dump's comment, and leaves everything else owned
by `econgraph`; that a restore failing at its last drop
(a schema owned by `postgres`) rolls every table and row back; that with
`latest.dump` moved to another dump right after the list, the listed dump is
the one restored; and that it refuses a dump with no table, a missing
`RESTORE_OWNER` role, and an app or Keycloak dump; that `backup.sh` reads
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
that no run deletes a dump, and that a dump removed from the volume by hand
stays in S3 after the next sync; and that no output contains a password. It needs Docker and
python3 with PyYAML, and runs in CI ("PostgreSQL Backup Scripts").

The encryption checks use a disposable identity only on the test host, verify
real encrypt/decrypt round trips and mode 0600, exercise failed or invalid
encryption and collisions, and check the manifest's memory-backed staging
and mount separation. A separate smoke check installs and runs encryption
in its actual image as uid 999 with a read-only root and no capabilities.
A separate backup/restore pair also runs as uid 999 on private container
tmpfs, with schema and row-count comparisons after restoration.
The S3 fixture includes legacy plaintext, unrelated
files, a partial ciphertext and a symlink, all of which must stay local.

It does not exercise live in-cluster wiring (PVC mounts, Secrets, the
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
`latest*.dump.age` links, all plaintext and any `*.partial` left by a killed run, and never
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
