# PostgreSQL Backups and Restore

The `econ-graph` namespace's PostgreSQL StatefulSet had no backups and its
data could be deleted in one step (teardown, or the wrong `kubectl delete`).
This document covers what was added to fix that, and how to use it.

## What's in place

- **`postgres-backup-cronjob.yaml`**: a nightly (`0 8 * * *` UTC) `CronJob`
  that `pg_dump`s `econ_graph` to `/backups` on the **`postgres-backup-data`**
  PVC — a separate volume from PostgreSQL's own `postgresql-data` PVC, so
  losing/recreating the primary volume doesn't take the backups with it.
  A second container in the same job optionally syncs `/backups` to S3 (or
  an S3-compatible endpoint), controlled by env vars — see
  [Object storage](#object-storage-optional) below.
- **`postgres-backup-pvc.yaml`**: the 20Gi backup volume, default
  StorageClass. For a real cluster, point `storageClassName` at storage in a
  different failure domain than the primary database volume, or rely on the
  S3 sync instead.
- **`postgres-backup-configmap.yaml`**: the `backup.sh` / `sync.sh` /
  `restore.sh` scripts the CronJob and restore job run. `backup.sh` also
  verifies the dump with `pg_restore --list` before it's considered
  successful, and prunes dumps older than `BACKUP_RETENTION_DAYS` (default
  14).
- **`scripts/deploy/protect-postgres-pv.sh`**: sets the `PersistentVolume`
  backing PostgreSQL's data PVC to `persistentVolumeReclaimPolicy: Retain`.
  Run automatically by `setup-local-k8s.sh` and `deploy.sh` after PostgreSQL
  comes up. With this set, deleting the PVC (or the namespace) leaves the
  underlying volume (now `Released`, not `Bound`) and its data intact
  instead of deleting it; a human then decides whether to reclaim or delete
  it.
- **`scripts/deploy/teardown.sh`**: now prompts before deleting the
  `econ-graph` namespace (which owns both the PostgreSQL and backup PVCs).
  Pass `--yes`/`-y` to skip that prompt non-interactively (e.g. in CI).
  This does **not** also delete the kind cluster: on kind, the cluster's
  node is what actually stores every PV's data (Retain policy included), so
  deleting it destroys the database and its backups regardless. That still
  needs its own confirmation, or an explicit `--delete-cluster` flag.
- **`scripts/deploy/backup-postgres-now.sh`**: triggers an ad-hoc backup
  without waiting for the nightly schedule.
- **`scripts/deploy/restore-postgres-backup.sh`**: runs the restore.

## Manual backup

```sh
./scripts/deploy/backup-postgres-now.sh
```

Creates and waits on a one-off `Job` from the `postgres-backup` CronJob,
then prints its logs.

## Restore

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
is passed.

Before restoring into a real deployment, scale `backend-deployment` and
`crawler-worker` to 0 replicas first: `--clean` waits on any lock they hold
(a live connection mid-query can stall the restore indefinitely), and the
crawler writing new rows mid-restore can race with it. Also note that
`--clean` only drops objects present in the dump — tables created by
migrations that ran *after* the dump was taken are left behind, which can
disagree with Diesel's migration history after the restore; check
`diesel migration list` / `__diesel_schema_migrations` afterwards.

### Script-level verification

The dump/restore logic (`backup.sh` and `restore.sh` in
`postgres-backup-configmap.yaml`) was exercised directly against a local
PostgreSQL server in this environment, since the sandbox has no running k8s
cluster to apply the CronJob/Job manifests to:

1. Created a test database with a table and rows.
2. Ran `backup.sh` against it (same commands the CronJob's `pg-dump`
   container runs) — produced a verified `.dump` file and a `latest.dump`
   symlink.
3. Created a second, empty database and ran `restore.sh` against it (same
   commands the restore `Job`'s container runs) — the table and rows came
   back correctly.
4. Confirmed the missing-file error path (`restore.sh` with a nonexistent
   `BACKUP_FILE`) fails loudly and lists what's actually available, rather
   than silently doing nothing.

This is *script-level* verification, not a full end-to-end test: it exercises
the same commands the Job/CronJob containers run, but not the in-cluster
wiring around them (the PVC mounts, the `econ-graph-secrets` Secret, the
CronJob schedule, or the actual `postgres-service` label selectors), and the
restore ran into an empty database rather than over live data — the
destructive `--clean` path the confirmation prompt exists for. That
in-cluster path needs a real cluster to verify; do that once one's
available with `backup-postgres-now.sh` followed by
`restore-postgres-backup.sh --list` and a real restore.

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

AWS credentials need to reach the `backup-sync` container (e.g. via
`envFrom.secretRef` for a Secret holding `AWS_ACCESS_KEY_ID` /
`AWS_SECRET_ACCESS_KEY`, or IAM roles for service accounts on EKS). Not
wired up by default since there's no object storage available in this
project's environments yet.

## Reattaching a Retain'd volume

Once a PV's reclaim policy is `Retain`, deleting its PVC leaves the PV
behind in the `Released` phase — it isn't automatically reused. A plain
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

`BACKUP_RETENTION_DAYS` on the `pg-dump` container (default 14) controls how
many days of dumps are kept on the PVC. Raise the PVC size in
`postgres-backup-pvc.yaml` if you raise retention.
