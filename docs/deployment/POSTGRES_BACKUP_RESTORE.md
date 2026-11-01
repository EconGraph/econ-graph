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

The commands below run kubectl as `k`, the kind cluster's own kubectl and
kubeconfig on its context. On the production host, plain `kubectl` is the
MicroK8s snap wrapper, which reaches a different cluster. Define `k` once
per shell. If restoring with `--context` or a custom kubectl/kubeconfig,
adjust `k` to use that same cluster before running any commands below:

```sh
k() { KUBECONFIG=~/.cache/econgraph/kubeconfig-econ-graph ~/.cache/econgraph/bin/kubectl --context kind-econ-graph "$@"; }
```

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
  `PersistentVolume`s for all three databases and the backups to
  `persistentVolumeReclaimPolicy: Retain`. It runs after PostgreSQL comes
  up and again before teardown. A backup PVC using delayed binding may not
  have a PV until the first backup job; teardown protects it once bound.
  Deleting a PVC leaves its PV `Released`; see
  [Reattaching a retained volume](#reattaching-a-retained-volume) for the
  storage-specific recovery path.
- **`scripts/deploy/teardown.sh`**: dumps the data, app and Keycloak
  databases first and refuses when `postgresql-0` is not running unless
  `--no-backup` is given. It sets the bound PVs to Retain and stops if that
  step fails. By default it deletes only the app plane (routes, frontend,
  backend, chart API, Keycloak, and the app and Keycloak Postgres
  StatefulSets), after the namespace name is typed on a terminal or `--yes`;
  their Secrets and PVCs stay.

  With `--include-data-plane`, it also deletes the data plane (the
  `postgresql` StatefulSet, its Services, the crawler and this CronJob), the
  Secrets and the `econ-graph` namespace, which owns the PostgreSQL and backup
  PVCs. This requires `econ_graph` typed on a terminal; `--yes` never answers
  that confirmation.

  With `--include-data-plane --delete-cluster`, it also deletes the kind
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
k -n econ-graph set env cronjob/postgres-backup -c pg-dump APP_REQUIRED=true
```

With `APP_DATABASE=separate`, the app database has tables once the backend has
started against it (it applies the app migrations at startup). A backup
before that first start fails the table check because those migrations have
not created the tables yet.

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
k -n econ-graph set env cronjob/postgres-backup -c pg-dump KEYCLOAK_REQUIRED=false
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
k -n econ-graph get jobs -l app=postgres-backup
k -n econ-graph logs job/<job-name> --all-containers=true --prefix=true
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

- It waits up to an hour (`--timeout SECONDS`, or `BACKUP_WAIT_TIMEOUT`). A Job
  that fails ends the wait at once.
- A Job still running at the timeout is left running. The script says so and
  prints the command to wait for it.
- While a Job of the CronJob (scheduled or ad hoc) is running, it starts no
  other one, names the running Job, and stops.

### Which cluster

`backup-postgres-now.sh` and `restore-postgres-backup.sh` act on one kubectl
context, `kind-econ-graph` unless `--context` names another. They don't
change the current context. Every kubectl call goes through `kc`
(`scripts/deploy/lib/cluster-role.sh`) with `--context`. With none of
`KUBECTL`, `KUBECONFIG` and `CLUSTER_ROLE_CONTEXT` set, they use the kind
cluster's own kubectl and kubeconfig kept in `~/.cache/econgraph`
(`bin/kubectl`, `kubeconfig-econ-graph`), and say so. On the production
host, plain `kubectl` is the MicroK8s snap wrapper, which reaches a different
cluster. A `CLUSTER_ROLE_CONTEXT` inherited from the environment is replaced,
not followed. An empty `--context` is refused.

Before doing anything else, both print the cluster they reached:

- this host;
- the kubectl and the `KUBECONFIG`;
- the context and its API server;
- the nodes;
- the `kube-system` namespace UID, which is new for every cluster;
- whether the cluster carries the production marker that
  `mark-production-cluster.sh` writes.

The restore prints this before its confirmation prompt, which is read from a
terminal only. A context the kubeconfig doesn't have, or an identity that can't be
read (API server, nodes, `kube-system` UID), stops either script before it
changes anything. `--dry-run` shows the cluster and what would run, and
starts nothing.

<a id="restore"></a>

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

This restore is **destructive**: `pg_restore --clean --if-exists` drops and
recreates every object the dump contains, overwriting its current data in
`econ_graph`. In shared mode this also replaces app state. The helper prompts
unless `--yes` is passed; it does not stop any clients for you.

1. List the available dumps and choose the recovery point:

   ```sh
   ./scripts/deploy/restore-postgres-backup.sh --list
   ```

2. Record the current replica counts for both writers, then stop them and
   wait until their pods are gone. Keep these counts for the restart, even
   if you leave this shell during recovery:

   ```sh
   k -n econ-graph get deployment econ-graph-backend crawler-worker \
     -o custom-columns=NAME:.metadata.name,REPLICAS:.spec.replicas || exit 1
   k -n econ-graph scale deployment/econ-graph-backend deployment/crawler-worker --replicas=0 || exit 1
   while :; do
     writer_pods="$(k -n econ-graph get pods \
       -l 'app in (econ-graph-backend,crawler-worker)' -o name)" || exit 1
     [ -z "$writer_pods" ] && break
     sleep 2
   done
   ```

   `--clean` waits on locks held by clients; a live connection mid-query can
   stall the restore indefinitely, and concurrent crawler writes can race
   with it. Stop any other clients that write to `econ_graph` too. Do not
   proceed if scaling or checking the pods fails.

3. Restore the chosen dump. Run one of these alternatives:

   ```sh
   # Latest good backup (prompts for confirmation)
   ./scripts/deploy/restore-postgres-backup.sh

   # A specific dump, without the confirmation prompt
   ./scripts/deploy/restore-postgres-backup.sh --file econ_graph-20260101-080000.dump --yes
   ```

   The helper waits up to 600 seconds. If that wait times out, it reports
   failure but leaves the Job running. Keep writers stopped, inspect the
   `postgres-restore-*` Job it named and its logs, and establish whether the
   Job completed or failed before retrying or resuming clients. A timeout
   alone does not mean the restore was rolled back.

4. After the Job succeeds, check ownership as described below and check
   `diesel migration list` / `__diesel_schema_migrations` against the schema
   and the migrations required by the deployed services. `--clean` only
   drops objects present in the dump: tables created by migrations that ran
   after the dump remain, and can disagree with the restored migration
   history. Resolve any mismatch before resuming writers.

5. Scale `deployment/econ-graph-backend` and `deployment/crawler-worker` back
   to their recorded replica counts only after recovery and schema checks
   succeed. In shared mode, starting the backend applies any newer app plane
   migrations to this database. Data plane migrations require the explicit
   [migration procedure](../development/MIGRATIONS.md#applying-the-data-set-crawler-migrate).

`restore.sh` refuses a file that isn't an `econ_graph-*.dump`
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
Every existing object that the second restore step drops must be owned by
`econgraph` so that role can drop them. If one isn't, the restore stops at
`must be owner of ...` and rolls back. Give the object to `econgraph`
(`ALTER TABLE ... OWNER TO econgraph`, or the matching `ALTER` for another
kind of object), then restore again. To check the result, as `postgres` in
`econ_graph`:

```sql
SELECT tableowner, count(*) FROM pg_tables
 WHERE schemaname NOT IN ('pg_catalog', 'information_schema') GROUP BY 1;
```

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
   k -n econ-graph run postgres-backup-copy --restart=Never --image=busybox:1.38 \
     --overrides='{"spec":{"securityContext":{"runAsNonRoot":true,"runAsUser":999},"containers":[{"name":"copy","image":"busybox:1.38","command":["sleep","600"],"volumeMounts":[{"name":"backup-data","mountPath":"/backups","readOnly":true}]}],"volumes":[{"name":"backup-data","persistentVolumeClaim":{"claimName":"postgres-backup-data"}}]}}'
   k -n econ-graph wait --for=condition=Ready pod/postgres-backup-copy --timeout=120s
   k -n econ-graph cp "postgres-backup-copy:/backups/${DUMP}" "./${DUMP}"
   k -n econ-graph delete pod postgres-backup-copy
   ```

   The dump holds password hashes and client secrets; keep the local copy
   private and remove it when done.

2. Record Keycloak's replica count, then stop it so nothing writes to the
   database during the restore. Keep the count for the restart:

   ```sh
   k -n econ-graph get deployment keycloak \
     -o custom-columns=NAME:.metadata.name,REPLICAS:.spec.replicas || exit 1
   k -n econ-graph scale deployment/keycloak --replicas=0 || exit 1
   while :; do
     writer_pods="$(k -n econ-graph get pods -l app=keycloak -o name)" || exit 1
     [ -z "$writer_pods" ] && break
     sleep 2
   done
   ```

   Do not proceed if recording the count, scaling or checking the pods fails.

3. Recreate the database and restore the dump into it:

   ```sh
   k -n econ-graph exec keycloak-postgres-0 -- dropdb -U keycloak --if-exists keycloak || exit 1
   k -n econ-graph exec keycloak-postgres-0 -- createdb -U keycloak keycloak || exit 1
   k -n econ-graph exec -i keycloak-postgres-0 -- \
     pg_restore -U keycloak -d keycloak --no-owner --no-privileges \
     --exit-on-error --single-transaction < "./${DUMP}" || exit 1
   ```

4. Confirm that database deletion, creation and restoration all exited
   successfully. If any command fails, keep Keycloak stopped and resolve the
   failure before continuing. Only after a successful restore, scale
   `deployment/keycloak` back to its recorded replica count.

### econ_graph_app

Use this procedure for `APP_DATABASE=separate`, where the backend uses
`econ_graph_app` on `app-postgres-service`. With `APP_DATABASE=shared`, app
state is in the `econ_graph` dump: follow [econ_graph](#econ_graph) instead,
which replaces both app state and data plane contents. See
[Which app database](#which-app-database) for how the layout is selected.

A separate app database restore replaces every user and everything they
created with the dump's. Take a manual backup first, as for Keycloak. The
backend connects as `econgraph`, which owns the tables it creates through
app plane migrations. The CronJob's dumps omit ownership (`--no-owner`),
so restore into
an `econgraph`-owned database with `--role=econgraph` to give the restored
tables to that role. Restoring as `postgres` alone would leave the tables
owned by `postgres`, and the backend could not use them. The host-side
`scripts/backup/restore-dump.sh` helper is for dumps that retain ownership;
do not use it for these CronJob dumps.

1. Copy the dump off the backup PVC as in step 1 above, with
   `DUMP=econ_graph_app-20260101-080000.dump`.

2. Record the replica count for the backend, the only client of the app
   database, then stop it. Keep the count for the restart:

   ```sh
   k -n econ-graph get deployment econ-graph-backend \
     -o custom-columns=NAME:.metadata.name,REPLICAS:.spec.replicas || exit 1
   k -n econ-graph scale deployment/econ-graph-backend --replicas=0 || exit 1
   while :; do
     writer_pods="$(k -n econ-graph get pods -l app=econ-graph-backend -o name)" || exit 1
     [ -z "$writer_pods" ] && break
     sleep 2
   done
   ```

   Do not proceed if recording the count, scaling or checking the pods fails.

3. Recreate the database, owned by `econgraph`, and restore the dump into it
   as `econgraph`:

   ```sh
   k -n econ-graph exec app-postgres-0 -c postgres -- dropdb -U postgres --if-exists econ_graph_app || exit 1
   k -n econ-graph exec app-postgres-0 -c postgres -- createdb -U postgres -O econgraph econ_graph_app || exit 1
   k -n econ-graph exec -i app-postgres-0 -c postgres -- \
     pg_restore -U postgres -d econ_graph_app --role=econgraph --no-owner --no-privileges \
     --exit-on-error --single-transaction < "./${DUMP}" || exit 1
   ```

4. Confirm that database deletion, creation and restoration all exited
   successfully. If any command fails, keep the backend stopped and resolve
   the failure before continuing. Only after a successful restore, scale
   `deployment/econ-graph-backend` back to its recorded replica count. It
   applies any app plane migrations newer than the dump at startup.

## Testing

```sh
scripts/deploy/test-postgres-backup-scripts.sh
```

The suite runs `backup.sh`, `sync.sh` and `restore.sh` from the ConfigMap in
the CronJob's images with a read-only root and no capabilities. The fixture
uid maps to the calling host account, including on rootless Docker. It
reads the `pg-dump` container's environment from the CronJob manifest and
uses scratch `postgres:18` containers for `data-postgres-service` (also
aliased `postgres-service`), `app-postgres-service` and
`keycloak-postgres-service`, plus an S3 mock.

The checks cover these guarantees:

- **Archive round trips and app ownership.** All three dumps are written,
  read back and restored with every table's row count equal to the source.
  The Keycloak dump is restored through `pg_restore` on stdin, as in its
  procedure above. The app dump, restored as in
  [econ_graph_app](#econ_graph_app) step 3, retains the source schema and
  leaves `econgraph` owning its tables and able to write them.
- **Data ownership, schema and extensions.** With the restore Job's
  environment, `restore.sh` leaves `econgraph` owning all restored objects
  except extensions and able to write its tables. The schema matches the
  source's tables, indexes, constraints, views, functions and extensions.
  Existing extensions keep their owners and comments, including a comment
  set only on the target. On a newly initialised server using the init
  script, with `uuid-ossp` then dropped and `RESTORE_OWNER` unset, the restore
  creates and logs the two missing extensions, `pgcrypto` and `uuid-ossp`,
  with the dump's comments. Everything else belongs to `econgraph`.
- **Restore rollback and input selection.** A restore that fails at its last
  drop because a schema belongs to `postgres` rolls back every table and row.
  Moving `latest.dump` to another dump immediately after the archive list
  does not change which dump is restored. The script refuses a dump without
  tables, a missing `RESTORE_OWNER` role, and an app or Keycloak dump.
- **App target selection and credentials.** `backup.sh` interprets URL case,
  Service suffixes, IP literals, options and unreadable URLs as specified in
  [Which app database](#which-app-database). A separate app target is dumped,
  including an uppercase full Service name with options. A shared target is
  skipped with one log line and a successful run. Other targets (another
  host, the same host with another database, an IP or a host in the options)
  and unreadable URLs fail regardless of `APP_REQUIRED`. A missing URL is
  skipped, or fails with `APP_REQUIRED=true`. A missing app password in
  separate mode fails the run. A missing Keycloak password fails with
  `KEYCLOAK_REQUIRED=true` and is skipped with `false`.
- **Independent failures and bounded runs.** When a dump fails because its
  server is down, its password is wrong, it has no tables, or a table stays
  locked past the lock wait, the other dumps are kept and synced and the Job
  fails. Without `BACKUP_STATUS_FILE`, `backup.sh` itself exits 1 on a dump
  failure. Manifest checks verify a Job deadline and a `podFailurePolicy`
  that does not retry a failed container.
- **Sync, retention and output privacy.** Completed ciphertext archives sync without the `latest`
  links. No run deletes a dump, and a dump removed from the volume by hand
  remains in S3 after the next sync. No output contains a password.

The suite needs Docker and python3 with PyYAML, and runs in CI as
“PostgreSQL Backup Scripts”. It does not cover the in-cluster wiring: PVC
mounts, Secrets, the CronJob schedule, Service selectors or network policy.
On the intended cluster, `backup-postgres-now.sh` followed by
`restore-postgres-backup.sh --list` checks that an ad-hoc Job can write dumps
and the listing pod can see them. Inspect the deployed schedule and network
configuration separately; these commands do not exercise scheduled
execution or a restore.

The encryption checks use a disposable identity only on the test host, verify
real encrypt/decrypt round trips and mode 0600, exercise failed or invalid
encryption and collisions, and check the manifest's memory-backed staging
and mount separation. A separate smoke check installs and runs encryption
in its actual image as uid 999 with a read-only root and no capabilities.
A separate backup/restore pair also runs as uid 999 on private container
tmpfs, with schema and row-count comparisons after restoration.
The S3 fixture includes legacy plaintext, unrelated
files, a partial ciphertext and a symlink, all of which must stay local.

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

## Reattaching a retained volume

`Retain` keeps a PV and its storage after PVC deletion, but leaves the PV
`Released`; it does not by itself make that PV reusable. Recovery depends on
which storage layout owns the volume.

### Static host-storage databases

The current database manifests use the `econgraph-host` StorageClass and pin
their claims to `econgraph-data-plane-pg`, `econgraph-app-prod-app-pg` and
`econgraph-app-prod-keycloak-pg`. Their storage is in the host's database
directories. The normal `deploy.sh` path runs `apply-host-storage.sh`, which
re-arms a Released database PV by clearing the old claim UID and resource
version while keeping its intended claim name and namespace. The replacement
claim then binds to the same directory. Keep that prebinding; do not clear
the entire `claimRef` using the dynamic-volume procedure below. See
[How the cluster reaches host storage](HOST_STORAGE.md#how-the-cluster-reaches-them)
for the mount checks and required Secrets.

### Dynamically provisioned backup volume

`postgres-backup-data` uses the default StorageClass and is not one of the
three database PVs re-armed by deployment. A replacement claim can therefore
provision a new empty volume rather than recover the old backups. For this
volume, or a database on a cluster deliberately configured with dynamic
storage, reattach the retained PV explicitly:

1. Stop workloads using the affected claim and keep backup Jobs from
   starting during recovery. Find the old Released PV with `k get pv`.
   Confirm its capacity and `claimRef` identify the intended volume; record
   its StorageClass and access modes too.
2. Check whether a replacement PVC already exists. If none exists, prepare
   one with the required name and namespace (`postgres-backup-data` in
   `econ-graph` for backups). If a replacement is Pending and unbound, replace
   it with the explicitly bound claim only after confirming it has no bound
   volume. If it is Bound to a different PV, preserve any data on that volume
   and stop its users before replacing the claim; protect that PV with Retain
   before deleting its PVC. Do not create a competing claim with another
   name or discard a bound replacement's data.
3. Once no live claim uses the old Released PV, clear its stale claim:
   `k patch pv <pv-name> -p '{"spec":{"claimRef": null}}'`.
4. Create the intended PVC with `spec.volumeName` set to the old PV's name,
   matching its StorageClass and access modes and requesting no more than its
   capacity. Use the workload's expected claim name; for a dynamically stored
   StatefulSet database, create that claim before applying the StatefulSet.
   Do not first apply an unpinned PVC that can provision another volume.
5. Verify that the PVC is Bound to the intended PV and that the expected
   files are present before resuming workloads. Keep the explicit volume
   selection in the claim manifest used for subsequent deployment.

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
k -n econ-graph run postgres-backup-cleanup --rm -i --restart=Never --image=busybox:1.38 \
  --overrides='{"spec":{"containers":[{"name":"cleanup","image":"busybox:1.38","stdin":true,"command":["rm","-v","/backups/econ_graph-20260101-080000.dump"],"volumeMounts":[{"name":"backup-data","mountPath":"/backups"}]}],"volumes":[{"name":"backup-data","persistentVolumeClaim":{"claimName":"postgres-backup-data"}}]}}'
```
