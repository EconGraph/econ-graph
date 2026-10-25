# PostgreSQL Backups and Restore

> On threadripper (production), the primary protection is now host-side:
> btrfs snapshots, verified nightly dumps of both databases, a weekly restore
> drill and a freshness check. See [BACKUPS.md](BACKUPS.md). The in-cluster
> CronJob described here is kept as a second copy for now; BACKUPS.md says
> why and when to retire it.

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
  reads the entire dump with `pg_restore --file=/dev/null` before it's considered
  successful. It never deletes dumps.
- **`scripts/deploy/protect-postgres-pv.sh`**: sets the bound
  `PersistentVolume`s for both PostgreSQL data and backups to
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
- **`scripts/deploy/restore-postgres-backup.sh`**: runs the restore.

## Credentials

The backup and restore jobs read `postgres-password` from the same
`econ-graph-postgres` Secret as the PostgreSQL StatefulSet. The
`scripts/deploy/create-secrets.sh` script preserves existing values. A
PostgreSQL password change in an existing data volume still requires an
explicit database-side change; verify with a manual backup after deployment.

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
2. Ran an earlier version of `backup.sh` against it — produced a `.dump`
   file and a `latest.dump` symlink. The newer full-archive read check
   (`pg_restore --file=/dev/null`) was added after this test.
3. Created a second, empty database and ran `restore.sh` against it (same
   commands the restore `Job`'s container runs) — the table and rows came
   back correctly.
4. Confirmed the missing-file error path (`restore.sh` with a nonexistent
   `BACKUP_FILE`) fails loudly and lists what's actually available, rather
   than silently doing nothing.

This is *script-level* verification of the original dump/restore flow, not a
full end-to-end test of the current manifests. It does not cover the newer
archive read check or the in-cluster wiring (the PVC mounts, the `econ-graph-postgres` Secret, the
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

Nothing deletes backups automatically: every nightly dump stays on the PVC
until an operator removes it. Watch the PVC's usage (20Gi by default, see
`postgres-backup-pvc.yaml`), and when it fills, copy the dumps you want to keep
elsewhere before deleting old ones by hand. List them with:

```bash
./scripts/deploy/restore-postgres-backup.sh --list
```

The PVC is `ReadWriteOnce`, so delete from a short-lived pod that mounts it while no
backup Job is running, naming each dump explicitly:

```bash
kubectl -n econ-graph run postgres-backup-cleanup --rm -i --restart=Never --image=busybox:1.36 \
  --overrides='{"spec":{"containers":[{"name":"cleanup","image":"busybox:1.36","stdin":true,"command":["rm","-v","/backups/econ_graph-20260101-080000.dump"],"volumeMounts":[{"name":"backup-data","mountPath":"/backups"}]}],"volumes":[{"name":"backup-data","persistentVolumeClaim":{"claimName":"postgres-backup-data"}}]}}'
```
