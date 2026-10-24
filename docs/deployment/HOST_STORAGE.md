# Production Postgres on host btrfs subvolumes

Production, UAT and integration testing all run on one home server
(threadripper), in the kind cluster `econ-graph` under rootless Docker. Until
this change both Postgres databases lived in local-path volumes inside the
`econ-graph-worker` node container, so `kind delete cluster` deleted them. They
now live on btrfs subvolumes on the host (the `/home` RAID10 NVMe filesystem),
bind-mounted into the cluster. Deleting or recreating the cluster leaves the
data alone.

Snapshots and backups of these subvolumes are covered in
[BACKUPS.md](BACKUPS.md) (PR #420). This page covers the layout, ownership,
and moving the live deployment onto it.

## Layout

| Host path | Subvolume | Contents |
|---|---|---|
| `/srv/econgraph/data-plane/pg/` | `@econgraph-data-plane` | `postgresql-0`, database `econ_graph` (`PGDATA` = `pg/pgdata`) |
| `/srv/econgraph/data-plane/objects/` | `@econgraph-data-plane` | reserved for Parquet/Iceberg |
| `/srv/econgraph/app-prod/keycloak-pg/` | `@econgraph-app-prod` | `keycloak-postgres-0` (`PGDATA` = `keycloak-pg/18/docker`); later the app database |
| `/srv/econgraph/snapshots/` | `@econgraph-snapshots` | read-only snapshots |

`pg/` and `objects/` share one subvolume, so a single snapshot of it is
consistent across both.

`scripts/host/setup-subvolumes.sh` (needs sudo) creates the subvolumes, adds
them to `/etc/fstab` and mounts them, with the subvolume roots owned by
`jmalicki`. Everything after that runs without sudo.

### How the cluster reaches them

- `terraform/k8s/kind-config.yaml` (generated from `terraform/k8s/main.tf`)
  bind-mounts `/srv/econgraph/data-plane` and `/srv/econgraph/app-prod` into
  `econ-graph-worker` at the same paths, and labels that node
  `econgraph.io/host-storage=true`.
- `k8s/manifests/host-storage.yaml` defines the StorageClass `econgraph-host`
  (no provisioner, `Retain`) and two static `hostPath` PersistentVolumes,
  `econgraph-data-plane-pg` and `econgraph-app-prod-keycloak-pg`. Each one is
  pinned to the labelled node and pre-bound by `claimRef` to its StatefulSet's
  PVC. Both StatefulSets also set `nodeSelector` to that label and name their PV
  in `volumeClaimTemplates`.
- The `hostPath` type is `Directory`, not `DirectoryOrCreate`. If the bind mount
  is missing (the subvolume failed to mount at boot, since fstab uses
  `nofail`), the pod fails to start. It does not quietly initialise an empty
  database inside the node container.
- `scripts/deploy/apply-host-storage.sh`, which `deploy.sh` runs before
  anything touches Postgres, checks the host mounts and the node's bind mounts,
  creates `pg/` and `keycloak-pg/` if they are missing, and applies the PVs. A
  PV left `Released` by a deleted PVC (after `teardown.sh` deletes the
  namespace) is re-armed so the new PVC binds to the same directory. It also
  stops if a data directory already holds a database but the Secret with that
  database's passwords is missing. Generating new passwords would lock the apps
  out of the database.
- No script deletes these PVs or their directories. `teardown.sh` leaves them
  alone and takes a host-side dump first. `recreate-kind-cluster.sh` takes a
  dump and asks for a typed confirmation before deleting the cluster.

## Ownership under rootless Docker

Rootless Docker runs every container in a user namespace. On this host
(`/etc/subuid`: `jmalicki:100000:65536`) the mapping is:

| UID inside a container, kind node or pod | UID on the host |
|---|---|
| 0 (root) | 1000 (`jmalicki`) |
| *n* for 1 ≤ *n* ≤ 65536 | 100000 + *n* − 1 |
| 999 (`postgres` in `postgres:18`) | **100998** |

The kind nodes don't add another user namespace for pods, so pod UID 999 is
node UID 999, which is host UID 100998. The live cluster's existing
local-path data directories show the same thing: they are owned by
100998:100998 on the host.

Postgres runs as UID 999 and must create and own its data directory, mode
0700. `fsGroup` doesn't apply to `hostPath` volumes, so each StatefulSet has an
`init-data-dir` initContainer. It runs as root with only `CHOWN`, `FOWNER` and
`DAC_READ_SEARCH`, and it:

1. chowns the volume root to 999:999 with mode 0700. That root is `pg/` or
   `keycloak-pg/`, never the subvolume root: snapshots need the subvolume
   roots to stay owned by `jmalicki`.
2. If `PGDATA` exists but isn't owned by 999 (for example after a copy that
   didn't keep ownership), chowns it recursively once. Otherwise it changes
   nothing.

What this means in practice:

- On the host, `pg/` and `keycloak-pg/` and everything in them belong to
  **100998:100998**, mode 0700. `jmalicki` can't list or read them directly.
  The subvolume roots stay `jmalicki`'s, so unprivileged
  `btrfs subvolume snapshot -r` still works.
- To read or copy the files without sudo, go through a rootless container.
  Its root is `jmalicki` with full capabilities inside the namespace. The
  backup scripts do this, and so did the tests below.
- Snapshots keep UID 100998, so restoring a snapshot needs no ownership
  changes. A copy must keep ownership: make it from a rootless container
  (`cp -a`) or as real root. If ownership is lost, `init-data-dir` repairs it on
  the next pod start.
- Postgres runs on **`econ-graph-worker`**, not on the control-plane node. A
  bind mount keeps pointing at the filesystem that was mounted when the node
  container started. So after a subvolume is swapped (snapshot restore,
  remount), stop Postgres and restart `econ-graph-worker`, not
  `econ-graph-control-plane`.
- If `/etc/subuid` changes (a different range for `jmalicki`), the host UID
  changes with it. Existing files then show up in the container as some
  other UID, and `init-data-dir` re-chowns `PGDATA` on the next start.

### Verified on this host

- With a scratch directory under `~/.cache` and then throwaway
  subdirectories on the real `/srv/econgraph/data-plane` and
  `/srv/econgraph/app-prod` (since removed), the `init-data-dir` script taken
  from the manifests ran in a rootless container with the manifest's
  capabilities and a read-only root filesystem. `postgres:18` then
  initialised and served as UID 999 in both layouts (`/data/pgdata`, and
  `/var/lib/postgresql/18/docker`), and restarted cleanly on the same data.
  Host view: `100998:100998 drwx------`.
- Without the initContainer, Postgres fails on an empty `jmalicki`-owned
  directory: `mkdir: cannot create directory '/data/pgdata': Permission denied`.
- A `PGDATA` copied without ownership (host UID 1000) was repaired by the
  initContainer, and Postgres started on it.
- The dumps in `~/econgraph-backups/20261024T0442Z/` restored into these
  throwaway servers with the commands the restore script uses: 10,528 series,
  503,001 data points, 2 Keycloak realms.

## Moving the live deployment

Everything is scripted. Every script supports `--dry-run`, which runs all the
checks for real and changes nothing.

| Step | Who | sudo | Destructive |
|---|---|---|---|
| 1. `sudo scripts/host/setup-subvolumes.sh` | Joe | **yes** | no |
| 2. `scripts/deploy/migrate-to-host-storage.sh --dry-run` | anyone | no | no |
| 3. `scripts/deploy/migrate-to-host-storage.sh` | **needs Joe's go-ahead** | no | **yes**: downtime, then the kind cluster is deleted and recreated |

Step 3 asks for a typed `yes` before stopping anything (`--yes` skips the
prompt). It then does the following:

1. **Preflight.** The subvolumes are mounted btrfs owned by you. `pg/` and
   `keycloak-pg/` are empty (move a leftover one aside; the script says how).
   `econ_graph` is up and has data. It prints the live row counts.
2. **Stops the writers** (`econ-graph-backend`, `crawler-worker`, `keycloak`)
   so the dump is final. Downtime starts here.
3. **Backs up** to `~/econgraph-backups/<UTC timestamp>/` with
   `backup-kind-databases.sh`: `econ_graph.dump` and `keycloak.dump`
   (`pg_dump -Fc`, each read back in full before it counts), `counts.env`
   (row counts) and `secrets/` (the app Secrets: the database passwords the
   data was initialised with, plus any API keys). All files are owner-only.
4. **Recreates the cluster** with `recreate-kind-cluster.sh` from the new kind
   config. This is the destructive step.
5. **Bootstraps the databases.** Creates the namespace and the saved Secrets,
   then runs `create-secrets.sh` (which keeps them), `apply-host-storage.sh`,
   and both Postgres StatefulSets. The initContainers set ownership and
   Postgres initialises empty directories on the subvolumes.
6. **Restores** both dumps with `restore-kind-databases.sh`. This runs before
   any app starts, so the backend's migrations find a complete schema. Each
   restore is a single transaction and the row counts must match `counts.env`.
7. **Deploys** everything else with `deploy.sh`.
8. **Verifies.** Row counts are at least the backup's; both PVs are `Bound` to
   the host paths; `pg/` is owned by 100998; it lists the pods.

As of the 2026-10-24 dump, the expected counts are 10,528 rows in
`economic_series` and 503,001 in `data_points`. The script compares against
the counts recorded in its own backup, not these numbers.

**Re-running is safe.** The backup directory in use is remembered in
`~/.cache/econgraph/host-storage-migration.env`. A run that stopped after the
cluster was deleted picks up from that backup: it creates the cluster if it's
missing, skips Secrets that already exist, doesn't restore twice and redeploys.
`--resume-from DIR` names the backup explicitly. Once the migration is done, a
re-run only verifies.

**If `deploy.sh` fails in step 7**, the databases are already restored on the
subvolumes. Fix the cause and re-run the migration script, or run `deploy.sh`.

### Rollback

`scripts/deploy/migrate-to-host-storage.sh --rollback` (add `--dry-run` first).
It needs **Joe's go-ahead**, because it deletes and recreates the cluster.

1. It stops the writers and takes a fresh backup of the current databases, so
   writes made since the migration are kept. Use `--resume-from DIR` to restore
   an older backup instead.
2. It checks out `--ref` (default `origin/release/v4.0`, i.e. node-local
   storage as before this change) into `~/.cache/econgraph/rollback-src`, and
   recreates the cluster from that ref's kind config.
3. It bootstraps the Secrets and databases from that ref's manifests, restores,
   runs that ref's `deploy.sh`, and verifies the row counts.

The host subvolumes are not touched by a rollback. Their contents are stale
from that point on. Move `pg/` and `keycloak-pg/` aside before migrating again
(the forward preflight refuses non-empty directories).

## Other notes

- `kubectl` on this host is the microk8s snap wrapper, which ignores
  `~/.kube/config`. The deploy scripts (`scripts/deploy/lib/kind-env.sh`) copy
  the cluster's own `kubectl` out of `econ-graph-control-plane` into
  `~/.cache/econgraph/bin` and use `kind get kubeconfig`. They don't change
  `~/.kube`.
- The in-cluster nightly `postgres-backup` CronJob and its
  `postgres-backup-data` PVC, and Grafana's data, are still node-local. They
  are lost when the cluster is deleted, which is one reason
  `recreate-kind-cluster.sh` and `teardown.sh` take a host-side dump first.
- Hostpath volumes have no size limit. The PV capacities (50Gi and 5Gi) are
  informational; the real limit is free space on the btrfs filesystem.
