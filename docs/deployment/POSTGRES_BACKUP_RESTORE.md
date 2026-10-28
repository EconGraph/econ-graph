# PostgreSQL Backups and Restore

The `econ-graph` namespace runs two PostgreSQL databases: `econ_graph` (the
`postgresql` StatefulSet, behind `postgres-service`) and Keycloak's `keycloak`
database (the `keycloak-postgres` StatefulSet, behind
`keycloak-postgres-service`), which holds user accounts and their linked
Google identities. Both are dumped nightly to a separate volume, and can be
shipped to object storage.

The commands below run kubectl as `k`, the kind cluster's own kubectl and
kubeconfig on its context. On the production host, plain `kubectl` is the
MicroK8s snap wrapper, which reaches a different cluster. Define `k` once
per shell:

```sh
k() { KUBECONFIG=~/.cache/econgraph/kubeconfig-econ-graph ~/.cache/econgraph/bin/kubectl --context kind-econ-graph "$@"; }
```

## What's in place

- **`postgres-backup-cronjob.yaml`**: a nightly (`0 8 * * *` UTC) `CronJob`
  whose `pg-dump` container `pg_dump`s `econ_graph` and `keycloak` to
  `/backups` on the **`postgres-backup-data`** PVC:
  a separate volume from both databases' own PVCs, so losing/recreating a
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
- **`scripts/deploy/teardown.sh`**: prompts before deleting the
  `econ-graph` namespace (which owns the database and backup PVCs).
  Before removing resources, it sets both bound PVs to Retain and stops if
  that step fails.
  Pass `--yes`/`-y` to skip that prompt.
  This does **not** also delete the kind cluster: the cluster's nodes hold
  the backup volume (and Grafana's data), so deleting the cluster destroys
  the backups of both databases. That still needs its own confirmation, or an explicit
  `--delete-cluster` flag. Both databases live on host btrfs subvolumes
  outside the cluster ([HOST_STORAGE.md](HOST_STORAGE.md)), so they survive
  it. Every kubectl call goes to the kind cluster's kubectl with
  `--context kind-econ-graph`. Before any change it checks that this context
  is the kind cluster `econ-graph` on the rootful Docker daemon, whatever
  `DOCKER_HOST` the shell has (`ECONGRAPH_DOCKER_HOST` chooses another): the
  same kube-system UID inside its control-plane container. It shows the cluster and
  asks once to go ahead (`--yes` answers that; on a cluster marked production,
  or whose marker can't be read, only the first 8 characters of its
  kube-system UID do, typed or given as `--confirm-production`, which is
  refused on any cluster it doesn't match). Without a terminal it needs `--yes` and one of
  `--delete-cluster` or `--keep-cluster` (or `--dry-run`, which changes
  nothing), and otherwise stops before any change.
- **`scripts/deploy/backup-postgres-now.sh`**: triggers an ad-hoc backup
  without waiting for the nightly schedule.
- **`scripts/deploy/restore-postgres-backup.sh`**: restores `econ_graph`.
- **`scripts/deploy/test-postgres-backup-scripts.sh`**: tests the three
  scripts; see [Testing](#testing).

## What a run writes

Each run takes one UTC timestamp and writes, on the backup PVC:

| File | Contents |
|------|----------|
| `econ_graph-<YYYYMMDD-HHMMSS>.dump` | `pg_dump --format=custom` of `econ_graph` |
| `keycloak-<YYYYMMDD-HHMMSS>.dump` | `pg_dump --format=custom` of `keycloak` |
| `latest.dump` | symlink to the newest good `econ_graph` dump |
| `latest-keycloak.dump` | symlink to the newest good `keycloak` dump |

Both dumps use `--no-owner --no-privileges` and `--lock-wait-timeout=600s`:
`pg_dump` waits at most 10 minutes for its share lock on the tables (only an
exclusive lock, such as a migration's `ALTER TABLE` or a `VACUUM FULL`,
blocks it), then fails that dump instead of waiting until the Job deadline.
`PG_DUMP_LOCK_WAIT_TIMEOUT` overrides it. Each is written as
`<name>.partial` and only renamed once it passes every check: it is not
empty, `pg_restore --file=/dev/null` reads the whole archive back, and
`pg_restore --list` shows at least one table. A dump that fails a check is
removed, and the `latest` link keeps pointing at the previous good one.

The CronJob reads the Keycloak password from the Secret
`econ-graph-keycloak` (key `db-password`) with `optional: true`, so a
missing Secret doesn't stop the pod or the `econ_graph` dump. The manifest
sets `KEYCLOAK_REQUIRED=true`, which makes a missing or empty password a
failed Keycloak dump. `deploy.sh` deploys Keycloak only when that Secret
exists; on a cluster deployed without it, set the variable to `false` and the
run logs that Keycloak is skipped and backs up `econ_graph` alone:

```sh
k -n econ-graph set env cronjob/postgres-backup -c pg-dump KEYCLOAK_REQUIRED=false
```

`deploy.sh` re-applies the manifest, so this is needed again after each
deploy on such a cluster.

### When one dump fails

The two dumps are independent: each is attempted even if the other fails,
and a good dump is kept whatever happens to the other. `backup.sh` records
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
write another full `econ_graph` dump each time, which nothing prunes. Only a
pod ended by a disruption (eviction, node drain, preemption, node loss) is
retried, up to twice (`backoffLimit: 2`): a `Count` rule for the pod's
`DisruptionTarget` condition comes before the `FailJob` rule, because a
disrupted container still ends with an exit code (137). So a night where a dump
fails costs one extra dump of the other database at most, and the next try
is the next night's run.

Each run has a 4-hour deadline (`activeDeadlineSeconds: 14400`), so a hung
run can't hold the CronJob's `concurrencyPolicy: Forbid` and block later
nights. `econ_graph`'s volume is 50Gi; `pg_dump --format=custom` runs at
about 25MB/s of database on one core, so a full 50GiB database takes about
40 minutes to dump and check, and the deadline leaves room for the lock
waits and the sync.

Run outside the CronJob without `BACKUP_STATUS_FILE`, `backup.sh` itself
exits 1 when a dump fails.

### Monitoring

No Prometheus rule watches the backup Job, so a failed or missing nightly
dump of either database shows only as a failed Job (above) and in the
dumps' timestamps.

## Credentials

The `econ_graph` dump and restore read `postgres-password` from the same
`econ-graph-postgres` Secret as the PostgreSQL StatefulSet. The Keycloak dump
connects as `keycloak` with `db-password` from `econ-graph-keycloak`, the
password `keycloak-postgres` and Keycloak use. Both reach `pg_dump` only
through `PGPASSWORD` in its environment; the scripts never print them. The
`scripts/deploy/create-secrets.sh` script preserves existing values. A
password change in an existing data volume still requires an explicit
database-side change; verify with a manual backup after deployment.

## Manual backup

```sh
./scripts/deploy/backup-postgres-now.sh
```

Creates and waits on a one-off `Job` from the `postgres-backup` CronJob,
then prints its logs. It dumps both databases, like the nightly run.

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
not followed.

Before doing anything else, both print the cluster they reached:

- this host;
- the kubectl and the `KUBECONFIG`;
- the context and its API server;
- the nodes;
- the `kube-system` namespace UID, which is new for every cluster;
- whether the cluster carries the production marker that
  `mark-production-cluster.sh` writes.

The restore prints this before its confirmation prompt. A context the
kubeconfig doesn't have stops either script before any other call.

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

Each command shows the cluster's identity first; see [Which cluster](#which-cluster).
Without `--yes`, the confirmation is read from a terminal only.

This is **destructive**: `pg_restore --clean --if-exists` drops and
recreates every object the dump contains before restoring it, overwriting
whatever is currently in `econ_graph`. That's why it prompts unless `--yes`
is passed. `restore.sh` refuses a file that isn't an `econ_graph-*.dump`
(such as a Keycloak dump). It resolves `latest.dump` to the dump it points
at once, before reading anything, so a backup that finishes during the
restore and moves the link doesn't change which dump is restored.

#### Ownership

The restore job connects as `postgres`, but every object it recreates is
owned by `econgraph`, the role the backend and crawler-worker connect as
(the job's `RESTORE_OWNER`). The dumps carry no owners or grants (they are
taken with `--no-owner --no-privileges`), so a plain `pg_restore` run as
`postgres` would leave every table owned by `postgres`, and the app could
neither read nor write it. `restore.sh` runs in two steps:

1. **Extensions**, as `postgres`: the dump's `CREATE EXTENSION IF NOT EXISTS`
   statements and their comments. This only adds an extension the database
   lacks and never drops or changes one. On a newly initialised server,
   where the init script has created `uuid-ossp`, `pg_stat_statements` and
   `pg_trgm`, that adds `pgcrypto`. `econgraph` can't do this step itself:
   `pg_stat_statements` needs a superuser to create, and the extensions
   (`pg_stat_statements`, `pg_trgm`, `pgcrypto`, `uuid-ossp`) are
   owned by `postgres`, so `econgraph` can't drop them either.
2. **Everything else**, with `pg_restore --role=econgraph --clean --if-exists
   --single-transaction --exit-on-error`: after connecting, `pg_restore`
   switches to `econgraph`, which drops and recreates the tables, sequences,
   views, indexes, types, functions, triggers and `__diesel_schema_migrations`
   as their owner, as the migrations created them. A failure rolls this step
   back, leaving the database as it was.

`restore.sh` checks that `RESTORE_OWNER` exists before changing anything.
Every object in the dump must be owned by `econgraph` in the database being
restored into, since that role drops them. If one isn't, the restore stops at
`must be owner of ...` and rolls back. Give the object to `econgraph`
(`ALTER TABLE ... OWNER TO econgraph`, or the matching `ALTER` for another
kind of object), then restore again.

To check the result:

```sh
k -n econ-graph exec postgresql-0 -- psql -U postgres -d econ_graph -c \
  "SELECT tableowner, count(*) FROM pg_tables WHERE schemaname = 'public' GROUP BY 1"
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
   both kinds) and copy it off the backup PVC, while no backup Job runs:

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

2. Stop Keycloak, so nothing writes to the database during the restore:

   ```sh
   k -n econ-graph scale deployment/keycloak --replicas=0
   # Until no Keycloak pod is left (returns at once if there was none)
   while k -n econ-graph get pods -l app=keycloak -o name | grep -q .; do sleep 2; done
   ```

3. Recreate the database and restore the dump into it:

   ```sh
   k -n econ-graph exec keycloak-postgres-0 -- dropdb -U keycloak --if-exists keycloak
   k -n econ-graph exec keycloak-postgres-0 -- createdb -U keycloak keycloak
   k -n econ-graph exec -i keycloak-postgres-0 -- \
     pg_restore -U keycloak -d keycloak --no-owner --no-privileges \
     --exit-on-error --single-transaction < "./${DUMP}"
   ```

4. Start Keycloak again: `./scripts/deploy/deploy.sh`, or
   `k -n econ-graph scale deployment/keycloak --replicas=1`.

## Testing

```sh
scripts/deploy/test-postgres-backup-scripts.sh
```

Runs `backup.sh`, `sync.sh` and `restore.sh` from the ConfigMap in the
CronJob's images and security settings (uid 999, read-only root), with the
`pg-dump` container's environment read from the CronJob manifest, against
scratch `postgres:18` containers standing in for `postgres-service` and
`keycloak-postgres-service`, and an S3 mock. The `econ_graph` server runs
the init script from `postgres-init.yaml`, `pgcrypto` is created as
`postgres`, and the backend's migrations run as `econgraph`, so, as in
production, every extension is owned by `postgres` and every other object by
`econgraph`.
It checks that both dumps are
written, restore with every table's row count equal to the source (the
Keycloak one through `pg_restore` on stdin, as in step 3 above) and sync
without the `latest` links; that `restore.sh`, run with the restore job's
environment, puts `econ_graph` back both over the existing database and on a
newly initialised server, with every object owned by `econgraph`, the
extensions in place, and `econgraph` (connecting with its password, as the
app does) able to read every table and write; that it rolls back every table
when it can't drop a schema, which `--clean` drops after the tables; that it
keeps restoring the dump it listed when `latest.dump` moves to another dump
mid-restore; that it stops when `RESTORE_OWNER` doesn't exist, and refuses a
Keycloak dump; that a failure of either dump (server down, wrong
password, no tables, a table locked past the lock wait) keeps and syncs the
other and fails the Job; that a missing Keycloak password is a failure with
`KEYCLOAK_REQUIRED=true` and a skip with `false`; that the Job has a
deadline and a `podFailurePolicy` that doesn't retry a failed container;
that no run deletes a dump; and that no output contains a password. It needs Docker and
python3 with PyYAML, and runs in CI ("PostgreSQL Backup Scripts").

```sh
scripts/deploy/test-postgres-backup-cluster.sh
```

Runs `restore-postgres-backup.sh` and `backup-postgres-now.sh` against stub
kubectls, with a failing plain `kubectl` first on `PATH` and the kind
cluster's copies in a scratch `~/.cache/econgraph`. It checks that:

- the plain kubectl is never called;
- every call carries the pinned context, also when another
  `CLUSTER_ROLE_CONTEXT` is inherited;
- the identity is shown before the confirmation;
- an unconfirmed restore, or one on a missing context, applies nothing;
- an empty `--context` is refused before any call;
- the restore Job sets `RESTORE_OWNER=econgraph`.

It also runs in the "PostgreSQL Backup Scripts" CI job.

The two tests do not cover the in-cluster wiring (the PVC mounts, the Secrets, the
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

The sync uploads every dump on the PVC that isn't in the bucket yet, both
`econ_graph-*` and `keycloak-*`, under the same prefix; it skips the
`latest*.dump` links and any `*.partial` left by a killed run, and never
deletes from the bucket. A run where a dump failed still syncs before
failing the Job.

AWS credentials need to reach the `backup-sync` container (e.g. via
`envFrom.secretRef` for a Secret holding `AWS_ACCESS_KEY_ID` /
`AWS_SECRET_ACCESS_KEY`, or IAM roles for service accounts on EKS). They are
not wired up by default.

## Reattaching a Retain'd volume

The two database PVs, `econgraph-data-plane-pg` and
`econgraph-app-prod-keycloak-pg`, are re-armed by
`scripts/deploy/apply-host-storage.sh` on the next deploy, and the
StatefulSets name them in `volumeName`, so their new PVCs bind to the same
host directories ([HOST_STORAGE.md](HOST_STORAGE.md)). The steps below are for
the backup volume.

Once a PV's reclaim policy is `Retain`, deleting its PVC leaves the PV
behind in the `Released` phase; it isn't automatically reused. A plain
redeploy creates a *new*, empty PVC/PV pair, quietly giving you a fresh,
empty database instead of your old data. To bind back to the old volume:

1. `k get pv` and find the `Released` one (check its capacity/claimRef
   to confirm it's the right one).
2. `k patch pv <pv-name> -p '{"spec":{"claimRef": null}}'` to clear its
   claim so it's `Available` again.
3. Apply `postgres-deployment.yaml` as usual; if the new PVC doesn't bind to
   it automatically (StatefulSet-created PVCs don't pin a specific volume),
   create a PVC by hand requesting that exact PV via `spec.volumeName`
   before applying the StatefulSet.

## Retention

Nothing deletes backups automatically: every nightly dump of both databases
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
