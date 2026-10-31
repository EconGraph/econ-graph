# Production Postgres on host btrfs subvolumes

Production, UAT and integration testing all run on one home server
(threadripper), in the kind cluster `econ-graph` on the rootful Docker daemon
(`unix:///var/run/docker.sock`, data root on ext4). Until
this change both Postgres databases lived in local-path volumes inside the
`econ-graph-worker` node container, so `kind delete cluster` deleted them. They
now live on btrfs subvolumes on the host (the `/home` RAID10 NVMe filesystem),
bind-mounted into the cluster. Deleting or recreating the cluster leaves the
data alone.

Snapshots and backups of these subvolumes are covered in
[BACKUPS.md](BACKUPS.md) (PR #420). This page covers the layout, ownership,
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

A database created by the original combined baseline needs a separate
[app/data conversion](APP_DATA_CONVERSION.md) to move existing app records.
Deploying these StatefulSets alone does not perform that copy;
`scripts/deploy/convert-app-data.sh` does, and also rolls it back. The
conversion first writes a verified plaintext dump of the combined database to a
new directory under the backup root (`ECONGRAPH_BACKUP_ROOT`, default
`~/econgraph-backups`), mode 0700 with 0600 files, and never deletes it. Like
the Keycloak and app dumps beside it, it holds sensitive records (users,
sessions, annotations, audit history): keep it on this host and do not copy it
off.

`scripts/host/setup-subvolumes.sh` (needs sudo) creates the subvolumes, adds
them to `/etc/fstab` and mounts them, with the subvolume roots owned by
the operator account (`<operator>`). Everything after that runs without sudo.

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
  `recreate-kind-cluster.sh` asks for a confirmation and takes a dump before
  deleting the cluster; on a cluster with a production marker, or one whose
  marker can't be read, the confirmation is the first 8 characters of its
  kube-system UID (typed, or `--confirm-production`), never `--yes`.
  `terraform destroy` doesn't delete it.

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

Images built before the switch exist only in the rootless daemon.
`scripts/deploy/load-kind-images.sh --from-docker-host <socket>` exports them
from there (`docker save`) and loads them into the cluster
(`kind load image-archive`), checking each image ID on every node.
`build-images.sh` now builds into the rootful daemon.

## Which kind

The scripts run kind v0.33.0 from
`~/.local/share/econgraph/bin/kind-v0.33.0` (under `$XDG_DATA_HOME` when it
is set), whatever `kind` is on `PATH`. Older kind cannot load images into a
node running Kubernetes 1.36 or newer ("unknown containerd config version:
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
- To read the files without sudo, go through the cluster (`kubectl exec` into
  the Postgres pod) or `docker exec econ-graph-worker`; both run as root on
  the rootful daemon. The backup scripts use `pg_dump` through `kubectl exec`.
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

### Verified on this host

These checks ran under the earlier rootless daemon, so the host-side owner they
saw was a user-namespace UID, not 999. `migrate-to-host-storage.sh`'s verify
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

Everything is scripted. Every script supports `--dry-run`, which runs all the
checks for real and changes nothing.

| Step | Who | sudo | Destructive |
|---|---|---|---|
| 1. `sudo scripts/host/setup-subvolumes.sh` | the operator | **yes** | no |
| 2. `scripts/deploy/migrate-to-host-storage.sh --dry-run` | anyone | no | no |
| 3. `scripts/deploy/migrate-to-host-storage.sh` | **needs the maintainer's go-ahead** | no | **yes**: downtime, then the kind cluster is deleted and recreated |

Step 3 asks for a typed `yes` before stopping anything (`--yes` skips the
prompt). If the cluster has a production marker, or its marker can't be read,
recreating it also needs the first 8 characters of its kube-system UID, given
as `--confirm-production` or typed at the terminal; `--yes` does not answer it.
Both confirmations are asked before anything is stopped, and without a
terminal and without `--confirm-production` the script stops there. It then does the following:

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
   config. This is the destructive step. If the old cluster was marked
   production, the new one is marked right after with
   `mark-production-cluster.sh`, which asks for a typed confirmation: the new
   cluster has a new kube-system UID, so the old marker does not count, and
   the saved tunnel token is only restored into a marked cluster.
5. **Bootstraps the databases.** Creates the namespace and the saved Secrets
   (`cloudflare-tunnel` only on a cluster marked production), then runs `create-secrets.sh` (which keeps them), `apply-host-storage.sh`,
   and both Postgres StatefulSets. The initContainers set ownership and
   Postgres initialises empty directories on the subvolumes.
6. **Restores** both dumps with `restore-kind-databases.sh`. This runs before
   any app starts, so the backend's migrations find a complete schema. Each
   restore is a single transaction and the row counts must match `counts.env`.
7. **Deploys** everything else with `deploy.sh`.
8. **Verifies.** Row counts are at least the backup's; both PVs are `Bound` to
   the host paths; `pg/` is owned by 999; it lists the pods.

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
Without `PUBLIC_HOST` the site is not served publicly from the new cluster; the
script says so at the end. Deploy again with `PUBLIC_HOST=econgraph.com` on the
marked production cluster (see "Production marker" in `k8s/README.md`).

### Rollback

`scripts/deploy/migrate-to-host-storage.sh --rollback` (add `--dry-run` first).
It needs **the maintainer's go-ahead**, because it deletes and recreates the cluster.

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

## Recovering the cluster

Every command here runs on the rootful daemon, so `DOCKER_HOST` is set
explicitly (the operator's shell exports the rootless one). Run each script
step with `--dry-run` first. Every step can be re-run.

### A cluster whose API server doesn't answer

`recreate-kind-cluster.sh` deletes a cluster only after reading its
kube-system UID inside the control-plane container. When the API server doesn't
answer, that read fails, and the script refuses even with
`--delete-unreachable`, because nothing can show that the cluster is not
production. Bring it back first, so it goes through the normal backup and
confirmation:

1. **Stopped node containers.** Start them, then check that the API server
   answers:

   ```bash
   DOCKER_HOST=unix:///var/run/docker.sock docker ps -a --filter label=io.x-k8s.kind.cluster=econ-graph
   DOCKER_HOST=unix:///var/run/docker.sock docker start econ-graph-control-plane econ-graph-worker
   DOCKER_HOST=unix:///var/run/docker.sock docker exec econ-graph-control-plane \
     kubectl --kubeconfig /etc/kubernetes/admin.conf get nodes
   ```

2. **Expired certificates.** kubeadm's certificates expire one year after the
   cluster is created; `kubeadm certs check-expiration` shows them. Renew them
   inside the control-plane node, which also renews the client certificates in
   `admin.conf`, `controller-manager.conf` and `scheduler.conf`. The control
   plane picks up the new certificates only when its static Pods restart
   ([Certificate Management with kubeadm, "Manual certificate renewal"](https://kubernetes.io/docs/tasks/administer-cluster/kubeadm/kubeadm-certs/#manual-certificate-renewal)),
   which restarting the node container does:

   ```bash
   DOCKER_HOST=unix:///var/run/docker.sock docker exec econ-graph-control-plane kubeadm certs check-expiration
   DOCKER_HOST=unix:///var/run/docker.sock docker exec econ-graph-control-plane kubeadm certs renew all
   DOCKER_HOST=unix:///var/run/docker.sock docker restart econ-graph-control-plane
   DOCKER_HOST=unix:///var/run/docker.sock docker exec econ-graph-control-plane \
     kubectl --kubeconfig /etc/kubernetes/admin.conf get nodes
   ```

   `kubeadm certs renew` does not cover a kubelet's own client certificate
   (`/var/lib/kubelet/pki/kubelet-client-current.pem`), which the kubelet
   rotates itself. If a node stays `NotReady` and its kubelet log
   (`docker exec <node> journalctl -u kubelet`) says its certificate has
   expired, repair it as in
   [Kubelet client certificate rotation fails](https://kubernetes.io/docs/setup/production-environment/tools/kubeadm/troubleshooting-kubeadm/#kubelet-client-cert).
   Once the API server answers, the scripts fetch a fresh kubeconfig
   themselves (`kind get kubeconfig`), and `recreate-kind-cluster.sh` backs
   the cluster up and asks for the confirmation as for any other cluster.

3. **Last resort: delete it by hand.** Only when it can't be brought back and
   you are sure it is the cluster to lose. This skips the backup and the
   production check: everything in its nodes is lost (the host subvolumes are
   not). It uses the pinned kind (`scripts/deploy/lib/kind-version.sh`) and
   the kubeconfig the scripts keep, so `~/.kube/config` is not touched:

   ```bash
   DOCKER_HOST=unix:///var/run/docker.sock ~/.local/share/econgraph/bin/kind-v0.33.0 \
     delete cluster --name econ-graph --kubeconfig ~/.cache/econgraph/kubeconfig-econ-graph
   ```

### Restoring production from a backup

Into a new cluster, from a backup directory taken by
`backup-kind-databases.sh`, `recreate-kind-cluster.sh` or `teardown.sh`
(dumps, `counts.env`, `secrets/` and, from a migration, `local-images.txt`):

1. Cluster (creates it when there is none; installs the gateway):

   ```bash
   scripts/deploy/recreate-kind-cluster.sh --if-missing
   ```

2. Images, from the rootless daemon where they were built:

   ```bash
   scripts/deploy/load-kind-images.sh --from-docker-host unix:///run/user/1000/docker.sock --list ~/econgraph-backups/<ts>/local-images.txt
   ```

3. Mark the new cluster as production, before anything is restored into it.
   It has a new kube-system UID, so no earlier marker counts; without one the
   restore leaves the saved tunnel token out, `deploy.sh` refuses `PUBLIC_HOST`
   and cloudflared stays at 0 replicas (see "Production marker" in
   `k8s/README.md`):

   ```bash
   scripts/deploy/mark-production-cluster.sh
   ```

4. Databases: namespace, the saved Secrets, PVs and the Postgres
   StatefulSets, then the restores. It stops with
   `RESTORE VERIFICATION FAILED` unless the restored database has exactly the
   counts in `counts.env`; `--expect-series` and `--expect-data-points` also
   require `counts.env` to hold the counts you expect:

   ```bash
   scripts/deploy/restore-kind-databases.sh --from ~/econgraph-backups/<ts> --expect-series N --expect-data-points N
   ```

5. Everything else:

   ```bash
   PUBLIC_HOST=econgraph.com scripts/deploy/deploy.sh
   ```

### After a host reboot

The cluster comes back by itself: `docker.service` is enabled at boot, and
kind gives its node containers the restart policy `on-failure:1`, which
dockerd honours when it starts. `recreate-kind-cluster.sh` prints both. The
subvolumes' fstab entries use `nofail`, which doesn't make dockerd wait for
them; `sudo scripts/host/order-docker-after-subvolumes.sh` adds a drop-in that
orders dockerd after the two mounts.

kubeadm's certificates in a kind cluster expire one year after it is created.
Renew them, or recreate the cluster, before then (see "A cluster whose API
server doesn't answer" above).

## The app database on an existing cluster

`deploy.sh` creates `app-pg/`, the PV `econgraph-app-prod-app-pg`, the Secret
`econ-graph-app-postgres` and the StatefulSet `app-postgres` wherever they are
missing, and leaves everything else as it is. It moves no rows: on a cluster
whose `app-database-url` names the data database (`APP_DATABASE=shared`, see
`k8s/README.md`), the app keeps using the data database until its rows are
moved and `create-secrets.sh` runs with `APP_DATABASE=separate`. Until the
StatefulSet exists, the nightly dump skips the app database and notes it in
its `MANIFEST`.

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
  `~/.cache/econgraph/kubeconfig-econ-graph`. A `--dry-run` writes neither:
  it uses the cached pair if it still reaches the cluster, and otherwise
  copies fresh ones into a temporary directory that is removed on exit.
- The in-cluster nightly `postgres-backup` CronJob and its
  `postgres-backup-data` PVC, and Grafana's data, are still node-local. They
  are lost when the cluster is deleted, which is one reason
  `recreate-kind-cluster.sh` and `teardown.sh` take a host-side dump first.
- Hostpath volumes have no size limit. The PV capacities (50Gi and 5Gi) are
  informational; the real limit is free space on the btrfs filesystem.
