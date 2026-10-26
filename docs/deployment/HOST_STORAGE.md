# Postgres on host btrfs subvolumes

Production runs in the kind cluster `econ-graph` on the host's rootful Docker
daemon (`unix:///var/run/docker.sock`). Both Postgres databases keep their data
on btrfs subvolumes on the host, bind-mounted into the cluster, so deleting or
recreating the cluster leaves the data alone.

## Layout

| Host path | Subvolume | Contents |
|---|---|---|
| `/srv/econgraph/data-plane/pg/` | `@econgraph-data-plane` | `postgresql-0`, database `econ_graph` (`PGDATA` = `pg/pgdata`) |
| `/srv/econgraph/app-prod/keycloak-pg/` | `@econgraph-app-prod` | `keycloak-postgres-0` (`PGDATA` = `keycloak-pg/18/docker`) |
| `/srv/econgraph/snapshots/` | `@econgraph-snapshots` | read-only snapshots |

The subvolume roots belong to the operator account (so it can take snapshots
without sudo). `pg/` and `keycloak-pg/` and everything in them belong to
uid/gid 999, mode 0700 (see [Ownership](#ownership)).

## How the cluster reaches them

- `terraform/k8s/kind-config.yaml` (generated from `terraform/k8s/main.tf`)
  bind-mounts `/srv/econgraph/data-plane` and `/srv/econgraph/app-prod` into
  `econ-graph-worker` at the same paths, and labels that node
  `econgraph.io/host-storage=true`.
- `k8s/manifests/host-storage.yaml` defines the StorageClass `econgraph-host`
  (no provisioner, `Retain`) and two static `hostPath` PersistentVolumes:

  | PV | Path | Size | Pre-bound to PVC |
  |---|---|---|---|
  | `econgraph-data-plane-pg` | `/srv/econgraph/data-plane/pg` | 50Gi | `econ-graph/postgresql-data-postgresql-0` |
  | `econgraph-app-prod-keycloak-pg` | `/srv/econgraph/app-prod/keycloak-pg` | 5Gi | `econ-graph/keycloak-postgres-data-keycloak-postgres-0` |

  Both are pinned to the labelled node. The `hostPath` type is `Directory`,
  not `DirectoryOrCreate`: if the bind mount is missing, the pod fails to
  start instead of initialising an empty database inside the node container.
  The sizes are informational; the real limit is the free space on the btrfs
  filesystem.
- The StatefulSets `postgresql` (`k8s/manifests/postgres-deployment.yaml`)
  and `keycloak-postgres` (`k8s/manifests/keycloak/postgres.yaml`) set
  `storageClassName: econgraph-host` and `volumeName` to their PV in
  `volumeClaimTemplates`, and a `nodeSelector` for the labelled node.
  `volumeClaimTemplates` can't be changed on an existing StatefulSet, so these
  manifests must keep matching what the cluster runs (see
  [Changing the storage manifests](#changing-the-storage-manifests)).

## Deploying

`scripts/deploy/deploy.sh` runs `scripts/deploy/apply-host-storage.sh` right
after the namespace and before the Secrets and both StatefulSets, on the same
kubectl and context as the rest of the deploy. It:

1. checks that `/srv/econgraph/data-plane` and `/srv/econgraph/app-prod` are
   mounted btrfs filesystems owned by the account running the deploy;
2. checks that `pg/` and `keycloak-pg/` exist, and stops if one is missing
   (it creates them only with `--create-dirs`);
3. checks that exactly `econ-graph-worker` carries the label and that it
   bind-mounts both subvolumes (`docker exec econ-graph-worker mountpoint`,
   through the caller's `DOCKER_HOST`, as `kind` in `deploy.sh` uses);
4. stops if a data directory already holds a database while the Secret with
   its passwords (`econ-graph-postgres`, `econ-graph-keycloak`) is missing:
   `create-secrets.sh` would generate new passwords that don't match it;
5. `kubectl apply`s `host-storage.yaml`, and re-arms a PV left `Released` by
   a deleted PVC (clears the old PVC's uid in its `claimRef`), so the new PVC
   binds to the same directory.

On a cluster that already runs these objects, the apply changes nothing.
`scripts/deploy/apply-host-storage.sh --dry-run` runs the same checks and
shows `kubectl diff` for `host-storage.yaml` (no diff output means no change)
and what it would create or re-arm, without changing anything.

It never deletes a PV, a PVC or a directory and never changes ownership on
the host. `teardown.sh` does not delete these PVs; deleting the namespace
leaves them `Released`, and the next deploy re-arms them.

Run the deploy scripts against the rootful daemon
(`DOCKER_HOST=unix:///var/run/docker.sock`) so that `kind` and `docker` see
the cluster.

## A new host

1. Create and mount the subvolumes (needs sudo; adds them to `/etc/fstab` with
   `nofail`, and gives the subvolume roots to the account that ran sudo):

   ```bash
   sudo ECONGRAPH_BTRFS_UUID=<uuid> scripts/host/setup-subvolumes.sh --dry-run
   sudo ECONGRAPH_BTRFS_UUID=<uuid> scripts/host/setup-subvolumes.sh
   ```

   `<uuid>` is the btrfs filesystem to put them on (`findmnt -no UUID /home`);
   the default is the production host's.
2. Create the kind cluster from `terraform/k8s/kind-config.yaml`
   (`terraform apply` in `terraform/k8s`, which uses the rootful daemon).
   `scripts/deploy/recreate-kind-cluster.sh` uses the same config; it deletes
   any existing `econ-graph` cluster first, and refuses to start unless both
   subvolumes are mounted. Docker would otherwise create a missing bind-mount
   source as an empty root-owned directory.
3. Create the data directories, then deploy:

   ```bash
   scripts/deploy/apply-host-storage.sh --create-dirs --dry-run
   scripts/deploy/apply-host-storage.sh --create-dirs
   scripts/deploy/deploy.sh
   ```

A cluster created without the bind mounts (from an older kind config) can't
be deployed to: `apply-host-storage.sh` stops at the node check. Its data is
in the node container, so move it with a `pg_dump` of both databases and a
restore into a cluster created as above; there is no script for that here.

## Ownership

The rootful Docker daemon has no user namespace, so a UID inside a kind node
or pod is the same UID on the host. `postgres` in the `postgres:18` image is
uid/gid 999, so on the host `pg/` and `keycloak-pg/` belong to 999:999, mode
0700, and the operator account can't read them directly. `fsGroup` doesn't
apply to `hostPath` volumes, so each StatefulSet has an `init-data-dir`
initContainer (root, with only `CHOWN`, `FOWNER` and `DAC_READ_SEARCH`) that
chowns the data directory to 999:999 with mode 0700, and re-chowns `PGDATA`
once if a copy lost its ownership. It never touches the subvolume roots.

To read the files, go through the cluster (`kubectl exec` into the Postgres
pod) or `docker exec econ-graph-worker`, both of which run as root.

## Changing the storage manifests

`scripts/deploy/testdata/production-host-storage.json` holds, for the
StorageClass, both PVs and both StatefulSets, the last-applied configuration
and the fields Kubernetes won't change in place, captured read-only from the
production cluster. `scripts/deploy/check-host-storage-fixture.py` (run by
`scripts/deploy/test-host-storage.sh` in CI) fails when the manifests, as
committed or rendered for `PUBLIC_HOST=econgraph.com`, differ from it. A
difference means a deploy would change production's objects; a difference in
an immutable field means the apply would be refused. The immutable fields are
always compared exactly.

The one difference it allows, and reports as a note, is in the comment lines
of the `init-data-dir` script. The script, comments included, is part of the
pod template, so such a difference restarts that database pod once on the
next deploy. The fixture's scripts carry an older wording of one comment, so
the first deploy of these manifests restarts `postgresql-0` and
`keycloak-postgres-0` once each.

After a deliberate change has been deployed, capture the fixture again:

```bash
kubectl get storageclass econgraph-host -o yaml > sc.yaml
kubectl get pv econgraph-data-plane-pg econgraph-app-prod-keycloak-pg -o yaml > pv.yaml
kubectl -n econ-graph get sts postgresql keycloak-postgres -o yaml > sts.yaml
scripts/deploy/check-host-storage-fixture.py --capture sc.yaml pv.yaml sts.yaml \
  > scripts/deploy/testdata/production-host-storage.json
```
