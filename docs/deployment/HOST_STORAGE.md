# Production Postgres on host btrfs subvolumes

Production, UAT and integration testing all run on one home server
(threadripper), in the kind cluster `econ-graph` on the rootful Docker daemon
(`unix:///var/run/docker.sock`, data root on ext4). The Postgres databases live
on btrfs subvolumes on the host (the `/home` RAID10 NVMe filesystem),
bind-mounted into `econ-graph-worker`. Deleting or recreating the cluster
leaves these host directories intact. Databases on older clusters that use
node-local volumes are lost when their node containers are deleted.

Snapshots and backups of these subvolumes are covered in
[the backup guide](BACKUPS.md). This page covers the layout, ownership,
moving the live deployment onto it, and recovering production.

## Layout

| Host path | Subvolume | Contents |
|---|---|---|
| `/srv/econgraph/data-plane/pg/` | `@econgraph-data-plane` | `postgresql-0`, the data database `econ_graph` (`PGDATA` = `pg/pgdata`) |
| `/srv/econgraph/data-plane/objects/` | `@econgraph-data-plane` | reserved for Parquet/Iceberg |
| `/srv/econgraph/app-prod/app-pg/` | `@econgraph-app-prod` | `app-postgres-0`, the app database `econ_graph_app` (`PGDATA` = `app-pg/pgdata`) |
| `/srv/econgraph/app-prod/keycloak-pg/` | `@econgraph-app-prod` | `keycloak-postgres-0` (`PGDATA` = `keycloak-pg/18/docker`) |
| `/srv/econgraph/snapshots/` | `@econgraph-snapshots` | read-only snapshots |

`pg/` and `objects/` share one subvolume, so a single snapshot of it is
consistent across both. `app-pg/` and `keycloak-pg/` share the other, so the
app plane (the app database and its sign-in) snapshots as one unit.

| Database | StatefulSet | PV | Secret |
|---|---|---|---|
| data, `econ_graph` | `postgresql` | `econgraph-data-plane-pg` | `econ-graph-postgres` |
| app, `econ_graph_app` | `app-postgres` | `econgraph-app-prod-app-pg` | `econ-graph-app-postgres` |
| Keycloak, `keycloak` | `keycloak-postgres` | `econgraph-app-prod-keycloak-pg` | `econ-graph-keycloak` |

`scripts/host/setup-subvolumes.sh` (needs sudo) creates the subvolumes, adds
them to `/etc/fstab` and mounts them, with the subvolume roots owned by
the operator account (`<operator>`). The one-time
[Docker mount-order setup](#after-a-host-reboot) also needs sudo. Deployment
and backup scripts run as the operator without sudo.

### How the cluster reaches them

- `terraform/k8s/kind-config.yaml` (generated from `terraform/k8s/main.tf`)
  bind-mounts `/srv/econgraph/data-plane` and `/srv/econgraph/app-prod` into
  `econ-graph-worker` at the same paths, and labels that node
  `econgraph.io/host-storage=true`.
- `k8s/manifests/host-storage.yaml` defines the StorageClass `econgraph-host`
  (no provisioner, `Retain`) and three static `hostPath` PersistentVolumes,
  `econgraph-data-plane-pg`, `econgraph-app-prod-app-pg` and
  `econgraph-app-prod-keycloak-pg`. Each one is pinned to the labelled node and
  pre-bound by `claimRef` to its StatefulSet's PVC. The StatefulSets also set
  `nodeSelector` to that label and name their PV in `volumeClaimTemplates`.
- The `hostPath` type is `Directory`, not `DirectoryOrCreate`. If the bind mount
  is missing (the subvolume failed to mount at boot, since fstab uses
  `nofail`), the pod fails to start. It does not quietly initialise an empty
  database inside the node container.
- `scripts/deploy/apply-host-storage.sh`, which `deploy.sh` runs before
  anything touches Postgres, checks the host mounts and the node's bind mounts,
  creates `pg/`, `app-pg/` and `keycloak-pg/` if they are missing, and applies
  the PVs. A PV left `Released` by a deleted PVC (after
  `teardown.sh --include-data-plane` deletes the namespace, or after a PVC is
  deleted by hand) is re-armed so the new PVC binds to the same directory. It
  also stops if a data directory already holds a database but the Secret with
  that database's passwords is missing. Generating new passwords would lock the
  apps out of the database.
- No script deletes these PVs or their directories. `teardown.sh` takes a
  host-side dump of all three databases first and, by default, removes only
  the app plane (the app and Keycloak StatefulSets, not their PVCs or
  Secrets); the data plane goes only with `--include-data-plane` and the data
  database's name typed on a terminal. `released-pvs.sh` never deletes a PV
  labelled `econgraph.io/protected=true`, which these three are.
  `recreate-kind-cluster.sh` takes a dump and asks for a confirmation before
  deleting the cluster, and `terraform destroy` doesn't delete it.

## Which Docker daemon

The cluster runs on the **rootful** Docker daemon. kind cannot start under
rootless Docker on this host: rootless Docker keeps its data on btrfs, and kind
then bind-mounts `/dev/mapper` into its nodes, which fails (ECO-432). The operator's
login shell exports `DOCKER_HOST` for the rootless daemon, so the deploy
scripts don't use that variable: `scripts/deploy/lib/kind-env.sh` sets
`DOCKER_HOST=unix:///var/run/docker.sock` for themselves and everything they
run (kind, docker, `deploy.sh`). Override it with `ECONGRAPH_DOCKER_HOST`.
The operator account is in the `docker` group, so none of this needs sudo. Every script
prints the daemon it uses and warns if it is a rootless one.

If required images are held in another Docker daemon,
`scripts/deploy/load-kind-images.sh --from-docker-host <socket>` exports them
from that daemon (`docker save`) and loads them into the cluster
(`kind load image-archive`), checking each image ID on every node.
`build-images.sh` builds into the rootful daemon.

## Which kind

The scripts run kind v0.33.0 from
`~/.local/share/econgraph/bin/kind-v0.33.0` (under `$XDG_DATA_HOME` when it
is set), regardless of which `kind` is on `PATH`. Older kind cannot load
images into a node running Kubernetes 1.36 or newer ("unknown containerd config version:
4"). Install it once, as the operator account and without sudo:

```bash
scripts/deploy/install-kind.sh
```

It verifies the download against the SHA-256 pinned in the script and leaves
the packaged `/usr/bin/kind` alone. The scripts that create or delete the
cluster or load images into it (`recreate-kind-cluster.sh`,
`load-kind-images.sh`, `build-images.sh`, `migrate-to-host-storage.sh` and
`teardown.sh --delete-cluster`) stop before changing anything when it is
missing. Read-only `kind get` commands fall back to the `kind` on `PATH`, with
a warning.

## Ownership

Rootful Docker runs containers without a user namespace (this daemon has no
`userns-remap`: `docker info` shows only apparmor, seccomp and cgroupns), so a
UID inside a kind node or pod is the same UID on the host. The `postgres` user
in the `postgres:18` image is `uid=999(postgres) gid=999(postgres)` (checked
with `docker run --rm --entrypoint id postgres:18 postgres` on 2026-10-24):

| UID inside a container, kind node or pod | UID on the host |
|---|---|
| 0 (root) | 0 |
| 999 (`postgres` in `postgres:18`) | **999** (named `systemd-coredump` in this host's `/etc/passwd`) |
| 1000 | 1000 (the operator account, `<operator>`) |

Any data directory not owned by 999:999 (for example one written under the
earlier rootless daemon, or copied without keeping ownership) is re-chowned by
`init-data-dir` the first time it sees it.

Postgres runs as UID 999 and must create and own its data directory, mode
0700. `fsGroup` doesn't apply to `hostPath` volumes, so each StatefulSet has an
`init-data-dir` initContainer. It runs as root with only `CHOWN`, `FOWNER` and
`DAC_READ_SEARCH`, and it:

1. chowns the volume root to 999:999 with mode 0700. That root is `pg/`, `app-pg/` or
   `keycloak-pg/`, never the subvolume root: snapshots need the subvolume
   roots to stay owned by the operator account.
2. If `PGDATA` exists but isn't owned by 999 (for example after a copy that
   didn't keep ownership), chowns it recursively once. Otherwise it changes
   nothing.

What this means in practice:

- On the host, `pg/`, `app-pg/` and `keycloak-pg/` and everything in them belong to
  **999:999**, mode 0700. The operator account can't list or read them directly.
  The subvolume roots stay the operator account's, so unprivileged
  `btrfs subvolume snapshot -r` still works.
- To read the files without sudo, use `kubectl exec` into the Postgres
  container as UID 999, or `docker exec econ-graph-worker` as root. Rootful
  Docker preserves the UID mapping; it does not make pod exec run as root.
  The backup scripts use `pg_dump` through `kubectl exec`.
- Snapshots keep UID 999, so restoring a snapshot needs no ownership changes.
  A copy must keep ownership (`cp -a` as root). If ownership is lost,
  `init-data-dir` repairs it on the next pod start.
- Postgres runs on **`econ-graph-worker`**, not on the control-plane node. A
  bind mount keeps pointing at the filesystem that was mounted when the node
  container started. So after a subvolume is swapped (snapshot restore,
  remount), stop Postgres and restart `econ-graph-worker`, not
  `econ-graph-control-plane`.
- If the cluster ever runs on a daemon with a user namespace again, the host
  UID changes with it; set `ECONGRAPH_HOST_POSTGRES_UID` for the scripts'
  ownership check, and `init-data-dir` re-chowns `PGDATA` on the next start.

### Earlier ownership verification

The following historical checks establish the initContainer behavior, not the
current production state or a required recovery point. They ran under the
earlier rootless daemon, so the host-side owner they saw was a user-namespace
UID, not 999. `migrate-to-host-storage.sh`'s verify
step checks that `pg/` is owned by 999.

- With a scratch directory under `~/.cache` and then throwaway
  subdirectories on the real `/srv/econgraph/data-plane` and
  `/srv/econgraph/app-prod` (since removed), the `init-data-dir` script taken
  from the manifests ran in a rootless container with the manifest's
  capabilities and a read-only root filesystem. `postgres:18` then
  initialised and served as UID 999 in both layouts (`/data/pgdata`, and
  `/var/lib/postgresql/18/docker`), and restarted cleanly on the same data.
  Host view: `drwx------`, owned by the namespace-mapped UID of 999.
- Without the initContainer, Postgres fails on an empty operator-owned
  directory: `mkdir: cannot create directory '/data/pgdata': Permission denied`.
- A `PGDATA` copied without ownership (host UID 1000) was repaired by the
  initContainer, and Postgres started on it.
- The dumps in `~/econgraph-backups/20261024T0442Z/` restored into these
  throwaway servers with the commands the restore script uses: 10,528 series,
  503,001 data points, 2 Keycloak realms.

## Moving the live deployment

The migration scripts support `--dry-run` to check available prerequisites
and display planned actions without changing the cluster or databases. A dry
run does not perform the restore or verify its resulting row counts, PVs,
pods and ownership; those checks run during the actual migration. The scripts
may create temporary copies of `kubectl` and kubeconfig, as described in
[Other notes](#other-notes).

This migration moves a deployment from node-local storage. It does not split
app plane rows out of a shared data database; see
[the app database on an existing cluster](#the-app-database-on-an-existing-cluster).

| Step | Who | sudo | Destructive |
|---|---|---|---|
| 1. `sudo scripts/host/setup-subvolumes.sh` | the operator | **yes** | no |
| 2. `scripts/deploy/migrate-to-host-storage.sh --dry-run` | anyone | no | no |
| 3. `scripts/deploy/migrate-to-host-storage.sh` | **needs the maintainer's go-ahead** | no | **yes**: downtime, then the kind cluster is deleted and recreated |

Step 3 asks for a typed `yes` before stopping anything (`--yes` skips the
prompt). It then does the following:

1. **Preflight.** The subvolumes are mounted btrfs owned by you. `pg/` and
   `keycloak-pg/` are empty (move a leftover one aside; the script says how).
   `econ_graph` is up and has data. It prints the live row counts.
2. **Stops the writers** (`econ-graph-backend`, `crawler-worker`, `keycloak`)
   so the dump is final. Downtime starts here.
3. **Backs up** to `~/econgraph-backups/<UTC timestamp>/` with
   `backup-kind-databases.sh`: `econ_graph.dump`, `keycloak.dump`, and
   `econ_graph_app.dump` when the separate app database is deployed
   (`pg_dump -Fc`, each read back in full before it counts), `counts.env`
   (row counts) and `secrets/` (the app Secrets: the database passwords the
   data was initialised with, plus any API keys). All files are owner-only.
4. **Recreates the cluster** with `recreate-kind-cluster.sh` from the new kind
   config. This is the destructive step. If the old cluster was marked
   production, the new one is marked right after with
   `mark-production-cluster.sh`, which asks for a typed confirmation: the new
   cluster has a new kube-system UID, so the old marker does not count, and
   the saved tunnel token is only restored into a marked cluster.
5. **Bootstraps the databases.** Creates the namespace and the saved Secrets
   (`cloudflare-tunnel` only on a cluster marked production), then runs `create-secrets.sh` (which keeps them), `apply-host-storage.sh`,
   and the Postgres StatefulSets. The initContainers set ownership and
   Postgres initialises empty directories on the subvolumes.
6. **Restores** the saved dumps with `restore-kind-databases.sh`. This runs before
   any app starts, so the backend's migrations find a complete schema. Each
   restore is a single transaction and the row counts must match `counts.env`.
7. **Deploys** everything else with `deploy.sh`.
8. **Verifies.** Data database row counts are at least the backup's; the data
   and Keycloak PVs are `Bound` to the host paths; `pg/` is owned by 999; it
   lists the pods.

**Re-running is safe.** The backup directory in use is remembered in
`~/.cache/econgraph/host-storage-migration.env`. A run that stopped after the
cluster was deleted picks up from that backup: it creates the cluster if it's
missing, skips Secrets that already exist, doesn't restore twice and redeploys.
`--resume-from DIR` names the backup explicitly. Once the migration is done, a
re-run only verifies.

**If `deploy.sh` fails in step 7**, the databases are already restored on the
subvolumes. Fix the cause and re-run the migration script, or run `deploy.sh`.
Without `PUBLIC_HOST` the site is not served publicly from the new cluster; the
script says so at the end. Deploy again with `PUBLIC_HOST=econgraph.com` on the
marked production cluster (see [production marking](../../k8s/README.md#production-marker)).

### Rollback

`scripts/deploy/migrate-to-host-storage.sh --rollback` (add `--dry-run` first).
It needs **the maintainer's go-ahead**, because it deletes and recreates the cluster.

1. It stops the writers and takes a fresh backup of the current databases, so
   writes made since the migration are kept. Use `--resume-from DIR` to restore
   an older backup instead.
2. It checks out `--ref` (default `origin/release/v4.0`, i.e. the node-local
   storage configuration) into `~/.cache/econgraph/rollback-src`, and
   recreates the cluster from that ref's kind config.
3. It bootstraps the Secrets and databases from that ref's manifests, restores,
   runs that ref's `deploy.sh`, and verifies the row counts.

The host subvolumes are not touched by a rollback. Their contents are stale
from that point on. Move `pg/` and `keycloak-pg/` aside before migrating again
(the forward preflight refuses non-empty directories).

## Recovering production

Recovery and saved-Secret restoration are human operator tasks. Run these
commands yourself; agents may review the examples as text but must never
read, enter, copy or decrypt actual secret values, including saved Secret
manifests and database dumps containing credentials. See
[Secrets management](../technical/SECRETS_MANAGEMENT.md).

Use this procedure to recreate a missing cluster or replace an unreachable
cluster, then restore a chosen host-side backup. Cluster deletion loses
node-local data, including Grafana data and the in-cluster backup PVC; it
does not remove the host database directories. If those directories still
hold the required data, recreating the cluster can reuse them without a dump
restore, but the matching password Secrets must also be restored before
deployment. For snapshot recovery, see [the backup guide](BACKUPS.md).

Choose the recovery point before proceeding. For a full dump recovery, the
backup directory must contain a dump for each database being recovered,
`counts.env` with the corresponding row counts, and `secrets/` with the saved
passwords and application Secrets. Data and Keycloak dumps without their
required counts are refused. An app dump without app row counts is skipped
because the app database had no tables when backed up. Missing saved Secrets
produce a warning, but newly generated passwords will not match an existing
database. Do not proceed with a full recovery without the matching Secrets.
A backup from a shared-database deployment need not contain
`econ_graph_app.dump`.

For retained-data recovery, only the matching saved Secrets are needed; no
dumps or `counts.env` are read by that recovery branch.

Set `BACKUP_DIR` to the chosen directory. Run each script with `--dry-run`
first and inspect the plan; a dry run cannot validate a restore that has not
yet run. The steps can be re-run, subject to the restore protections below.

1. Create the cluster if it is missing and install
   [Envoy Gateway](../../k8s/README.md#-gateway). The recreation script
   installs Gateway API and Envoy Gateway resources; deployment in step 5
   applies the application routes:

   ```bash
   scripts/deploy/recreate-kind-cluster.sh --if-missing
   ```

   If a cluster of the same name exists but its API server is unreachable,
   inspect the failure first. To replace it, add `--delete-unreachable`. This
   deletes the cluster without a fresh backup, after a typed `yes`; its node
   containers may still hold the host ports needed by the replacement. The
   script checks host storage before deletion. An unreachable API alone does
   not establish that the host database files are lost.

2. If required images are held in another daemon, import them using its socket
   and the backup's image list. For example, with the operator's rootless daemon:

   ```bash
   scripts/deploy/load-kind-images.sh --from-docker-host unix:///run/user/1000/docker.sock --list "$BACKUP_DIR/local-images.txt"
   ```

   This step needs `local-images.txt` and the listed images in the source
   daemon. Otherwise, build or obtain the required images before deployment.

3. Mark a newly created production cluster before restoring Secrets. Its new
   kube-system UID invalidates the old marker. Without a current marker, the
   restore omits the saved tunnel token, `deploy.sh` refuses `PUBLIC_HOST`,
   and cloudflared stays at 0 replicas. See
   [production marking](../../k8s/README.md#production-marker).

   ```bash
   scripts/deploy/mark-production-cluster.sh
   ```

4. Prepare the databases using the appropriate recovery route.

   **To reuse surviving host data**, restore its matching saved Secrets before
   deployment. From the repository root, the operator can preview namespace
   and Secret creation with this Bash subshell, then set `DRY_RUN=false` to
   perform it:

   ```bash
   (
     set -euo pipefail
     DRY_RUN=true
     source scripts/deploy/lib/kind-env.sh
     use_kind_kubectl
     run kubectl apply -f k8s/manifests/namespace.yaml
     restore_secrets "$BACKUP_DIR"
   )
   ```

   These helpers select the cluster's kubectl and kubeconfig, create missing
   saved Secrets, and leave existing Secrets unchanged. The tunnel Secret is
   restored only when the production marker is valid. This branch does not
   restore dumps or compare database row counts. Confirm that the saved
   database password Secrets match the retained databases before continuing;
   the helper does not check password correctness or whether every required
   Secret was saved, and only warns if no saved Secrets are found. If matching
   Secrets are unavailable, stop rather than let deployment generate replacements.
   Deployment in step 5 applies the PVs and StatefulSets to the retained data.

   **To recover from dumps**, bootstrap the namespace, saved Secrets, PVs and
   Postgres StatefulSets, then restore:

   ```bash
   scripts/deploy/restore-kind-databases.sh --from "$BACKUP_DIR"
   ```

   Each database restore is a single transaction. The script skips missing
   dumps and leaves a database alone when its checked row counts already
   match the backup. It refuses a database with different data unless
   `--replace` is supplied; replacement drops and recreates objects from the
   dump after typed confirmation. Check that all intended databases were
   restored. A row-count mismatch stops with `RESTORE VERIFICATION FAILED`;
   resolve it before deploying. To guard against selecting the wrong backup,
   add `--expect-series N --expect-data-points N` using independently known
   counts for the chosen recovery point. These flags first check `counts.env`;
   the restore then checks the database against that file.

5. Deploy the remaining services on the marked production cluster:

   ```bash
   PUBLIC_HOST=econgraph.com scripts/deploy/deploy.sh
   ```

### After a host reboot

Docker needs the storage subvolumes mounted before it starts, so the worker
binds the database subvolumes rather than the underlying directories. Their
`fstab` entries use `nofail`, so those entries do not make Docker wait for the
mounts. Run `sudo scripts/host/order-docker-after-subvolumes.sh` once to add
that startup ordering. The script reloads systemd without restarting Docker;
the drop-in applies on subsequent Docker startup. It uses `Wants=` and
`After=`, so Docker can still start after a failed mount attempt. Check both
mounts after a reboot before starting the nodes.

`recreate-kind-cluster.sh` reports Docker service enablement and node restart
policies. These reports do not establish automatic recovery: kind's
`on-failure:1` policy retries a failed container once, but
[Docker does not start `on-failure` containers when the daemon restarts](https://docs.docker.com/engine/containers/start-containers-automatically/).
After a reboot, check the mounts and containers on the rootful daemon:

```bash
findmnt --mountpoint /srv/econgraph/data-plane
findmnt --mountpoint /srv/econgraph/app-prod
DOCKER_HOST=unix:///var/run/docker.sock docker ps -a --filter label=io.x-k8s.kind.cluster=econ-graph
```

If both subvolumes are mounted and the existing node containers are stopped,
start them explicitly:

```bash
DOCKER_HOST=unix:///var/run/docker.sock docker start econ-graph-control-plane econ-graph-worker
```

Check node readiness and the database pods using the cluster's kubectl and
kubeconfig described in [Other notes](#other-notes). If the worker started
before the mounts were ready, stop Postgres and restart the worker as
described under [Ownership](#ownership). If the API remains unreachable,
investigate before using the cluster replacement procedure above.

kubeadm's certificates in a kind cluster expire one year after it is created.
Renew the certificates or recreate the cluster before they expire.

## The app database on an existing cluster

`deploy.sh` creates any missing app database resources: the `app-pg/`
directory, PV `econgraph-app-prod-app-pg`, Secret `econ-graph-app-postgres`, and
StatefulSet `app-postgres`. By default, deployment keeps existing Secrets
and the selected database URL while applying the manifests. It does not move
rows.

On a cluster using `APP_DATABASE=shared`, `app-database-url` points to the data
database. Creating the separate app database does not change that setting.
The app continues to use the data database until its rows are moved and
`create-secrets.sh` is run with `APP_DATABASE=separate`. See
[database settings](../../k8s/README.md#databases) for configuration details.

Until the `app-postgres` StatefulSet exists, the nightly dump skips the app
database and records the skip in its `MANIFEST`.

## Testing on a scratch cluster

The scripts can run against a throwaway kind cluster whose storage node
bind-mounts scratch directories instead of `/srv/econgraph`:

1. Create two btrfs subvolumes you own, e.g. `btrfs subvolume create
   ~/scratch/data-plane ~/scratch/app-prod` (no sudo; they need not be mount
   points).
2. Copy `terraform/k8s/kind-config.yaml`, drop the `extraPortMappings` (or move
   them to unused loopback ports), and point the worker's `extraMounts`
   `hostPath`s at the scratch subvolumes, keeping the `containerPath`s
   `/srv/econgraph/data-plane` and `/srv/econgraph/app-prod`: the PVs name those.
3. Export `KIND_CLUSTER=<name>`, `ECONGRAPH_SCRATCH_STORAGE=~/scratch`,
   `XDG_CACHE_HOME=<scratch dir>` (the cluster's kubectl copy and kubeconfig)
   and `ECONGRAPH_BACKUP_ROOT=<scratch dir>`, then run
   `recreate-kind-cluster.sh --if-missing --config <copy>` and the other scripts
   as usual. `kind-env.sh` refuses `ECONGRAPH_SCRATCH_STORAGE` when
   `KIND_CLUSTER` is `econ-graph`.

Postgres leaves files owned by UID 999 in the scratch subvolumes; remove them
through Docker (`docker run --rm -v ~/scratch:/s postgres:18 rm -rf
/s/data-plane/pg /s/app-prod/app-pg /s/app-prod/keycloak-pg`) before deleting
the subvolumes.

## Other notes

- `kubectl` on this host is the microk8s snap wrapper, which ignores
  `~/.kube/config`. The deploy scripts (`scripts/deploy/lib/kind-env.sh`) copy
  the cluster's own `kubectl` out of `econ-graph-control-plane` into
  `~/.cache/econgraph/bin` and use `kind get kubeconfig`. They don't change
  `~/.kube`: `kind create/delete cluster` get `--kubeconfig` pointing at
  `~/.cache/econgraph/kubeconfig-econ-graph`. A dry run does not update the
  cached `kubectl` or kubeconfig. It uses the cached copies if they can reach
  the cluster; otherwise, it obtains fresh copies in a temporary directory
  and removes that directory on exit.
- The in-cluster nightly `postgres-backup` CronJob and its
  `postgres-backup-data` PVC, and Grafana's data, are still node-local. They
  are lost when the cluster is deleted, which is one reason
  `recreate-kind-cluster.sh` and `teardown.sh` take a host-side dump first.
- Hostpath volumes have no size limit. The PV capacities (50Gi and 5Gi) are
  informational; the real limit is free space on the btrfs filesystem.
