# PostgreSQL Backups and Restore

The `econ-graph` namespace runs two PostgreSQL databases: `econ_graph` (the
`postgres` StatefulSet, behind `postgres-service`) and Keycloak's `keycloak`
database (the `keycloak-postgres` StatefulSet, behind
`keycloak-postgres-service`), which holds user accounts and their linked
Google identities. Both are dumped nightly to a separate volume, and can be
shipped to object storage.

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
  it. Every kubectl call goes to the kind cluster's kubectl on context
  `kind-econ-graph`; it shows that cluster first and stops before any change
  if the context doesn't reach the kind cluster. Without a terminal it needs
  both `--yes` and `--delete-cluster` (or `--dry-run`, which changes nothing),
  and otherwise stops before any change.
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
kubectl -n econ-graph set env cronjob/postgres-backup -c pg-dump KEYCLOAK_REQUIRED=false
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
kubectl -n econ-graph get jobs -l app=postgres-backup
kubectl -n econ-graph logs job/<job-name> --all-containers=true --prefix=true
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
whatever is currently in `econ_graph`. That's why it prompts unless `--yes`
is passed. `restore.sh` refuses a file that isn't an `econ_graph-*.dump`
(such as a Keycloak dump).

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

## Testing

```sh
scripts/deploy/test-postgres-backup-scripts.sh
```

Runs `backup.sh`, `sync.sh` and `restore.sh` from the ConfigMap in the
CronJob's images and security settings (uid 999, read-only root), with the
`pg-dump` container's environment read from the CronJob manifest, against
scratch `postgres:18` containers standing in for `postgres-service` and
`keycloak-postgres-service`, and an S3 mock. It checks that both dumps are
written, restore with every table's row count equal to the source (the
Keycloak one through `pg_restore` on stdin, as in step 3 above) and sync
without the `latest` links; that `restore.sh` puts `econ_graph` back and
refuses a Keycloak dump; that a failure of either dump (server down, wrong
password, no tables, a table locked past the lock wait) keeps and syncs the
other and fails the Job; that a missing Keycloak password is a failure with
`KEYCLOAK_REQUIRED=true` and a skip with `false`; that the Job has a
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

1. `kubectl get pv` and find the `Released` one (check its capacity/claimRef
   to confirm it's the right one).
2. `kubectl patch pv <pv-name> -p '{"spec":{"claimRef": null}}'` to clear its
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
kubectl -n econ-graph run postgres-backup-cleanup --rm -i --restart=Never --image=busybox:1.38 \
  --overrides='{"spec":{"containers":[{"name":"cleanup","image":"busybox:1.38","stdin":true,"command":["rm","-v","/backups/econ_graph-20260101-080000.dump"],"volumeMounts":[{"name":"backup-data","mountPath":"/backups"}]}],"volumes":[{"name":"backup-data","persistentVolumeClaim":{"claimName":"postgres-backup-data"}}]}}'
```
