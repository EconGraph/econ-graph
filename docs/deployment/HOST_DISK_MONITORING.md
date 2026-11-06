# Host disk and capacity monitoring

Everything runs on one physical host, so one full disk can take down the whole
site. This page covers how each place that fills up is watched, how to install
the host side, and what the cluster needs to scrape it.

| What fills up | Watched by |
|---|---|
| Worker data volume `/var/mnt/postgres`: every `econgraph-local` volume (CNPG app/data/Keycloak data and WAL, Prometheus, Loki, Grafana) | Chart `NodeFilesystem*` and `KubePersistentVolumeFillingUp`; `EconGraphNodeVolumeAlmostFull` (80%) and `EconGraphNodeVolumeFillingUp` (full within 7 days) |
| Each database against its volume request | `EconGraphPostgresVolumeOverRequest` (data + WAL over 80% of the PVC request) |
| Talos `/var` on each 40 GiB OS disk (images, logs, etcd) | Chart `NodeFilesystem*`; `EconGraphNodeVolume*` |
| Host libvirt pool (thin qcow2 VM disks) | `EconGraphHostFilesystemAlmostFull`/`FillingUp` with `role="libvirt_pool"`; `EconGraphLibvirtPoolOvercommitted` |
| Host object store (backups, WAL, logs, Iceberg; only grows) | `EconGraphHostFilesystem*` with `role="objectstore"` |
| Host root filesystem | `EconGraphHostFilesystem*` with `role="host_root"` |
| Host swap | `EconGraphHostSwapAlmostFull` |
| The host check itself | `EconGraphHostExporterDown`, `EconGraphHostDiskCheckStale`, `EconGraphHostDiskPathMissing`, `EconGraphLibvirtPoolsNotReported` |

The rules are in [`platform/observability/rules-disk.yaml`](../../platform/observability/rules-disk.yaml)
with promtool tests in `platform/observability/tests/disk.test.yaml`. The Grafana
dashboard **EconGraph disk and capacity** (`platform/observability/dashboard-disk.yaml`)
has a row for each of these, showing usage and growth per day or days until full.

## The requested volume sizes are not limits

The `econgraph-local` class is local-path-provisioner with `local` volumes. Every
volume on a worker is a directory on that worker's one `/var/mnt/postgres` XFS
filesystem, and nothing enforces the 10Gi/100Gi requests. So:

- kubelet reports the **whole worker filesystem** for every such PVC. The chart's
  `KubePersistentVolumeFillingUp` fires only when the shared filesystem is nearly
  full, and every PVC on that worker fires together.
- A database can grow past its request until the worker filesystem is full.
  `EconGraphPostgresVolumeOverRequest` compares CNPG's own sizes
  (`cnpg_pg_database_size_bytes` plus `cnpg_collector_pg_wal{value="size"}`) with
  the PVC request. WAL that keeps growing usually means archiving has stalled.
- To test the PVC alert on a scratch cluster, filling a "10Gi" test PVC to 10Gi
  does nothing; fill the worker's `/var/mnt/postgres` past 85% (for
  `KubePersistentVolumeFillingUp` warning, which also needs a predicted fill
  within 4 days) or 97% (critical), or past 80% for `EconGraphNodeVolumeAlmostFull`.

## Host side

[`scripts/host/install-host-disk-exporter.sh`](../../scripts/host/install-host-disk-exporter.sh)
installs:

- **node_exporter 1.12.1** (the official linux-amd64 release, SHA-256 pinned) as
  `econgraph-node-exporter.service`, listening only on the address you give, with
  just the filesystem, meminfo, vmstat (swap), btrfs and textfile collectors.
- **`econgraph-host-disk-check`** ([`host-disk-check.sh`](../../scripts/host/host-disk-check.sh)),
  run every minute by `econgraph-host-disk-check.timer`. It writes
  `/var/lib/econgraph/node-exporter-textfile/econgraph_host_disk.prom` with the
  size and free space of the host root, the object store mount and every active
  libvirt pool, plus each pool's summed virtual and allocated volume sizes. It
  only reads.

```sh
sudo scripts/host/install-host-disk-exporter.sh --listen-address 192.168.250.1:9100 --dry-run
sudo scripts/host/install-host-disk-exporter.sh --listen-address 192.168.250.1:9100
curl -s http://192.168.250.1:9100/metrics | grep '^econgraph_'
```

Use the host's address on the bridge the Talos VMs use (the inventory's
`bridge_address`, without the prefix length). The installer refuses `0.0.0.0`,
loopback and addresses not on the host. Until the object store is installed and
mounted at `/srv/econgraph/objectstore`, either pass `--without-objectstore` or
expect `EconGraphHostDiskPathMissing`; re-run the installer once it is mounted.
Re-run it after merging changes to `host-disk-check.sh`. It never touches the
firewall and deletes nothing.

### Why the check reports roles, not mount points

The libvirt pool is a directory inside a btrfs subvolume, not its own mount, and
the object store is a subvolume mounted on a filesystem it may share with other
data. node_exporter's `node_filesystem_*` series are per mount point, so they
cannot say which one is the pool. The check resolves each role to its filesystem
and reports `econgraph_host_filesystem_{size,avail}_bytes{role=...}`; for pools it
uses libvirt's own `pool-info`.

### Thin-provisioning overcommit

VM disks are thin qcow2 files and the fit gate checks space only when a VM is
created. `econgraph_libvirt_pool_volumes_capacity_bytes` minus
`econgraph_libvirt_pool_volumes_allocation_bytes` is how much the disks can still
grow. When that is more than the pool filesystem's free space,
`EconGraphLibvirtPoolOvercommitted` fires: if the guests fill their disks, libvirt
pauses the VMs. Pools that share one filesystem are each compared with the same
free space, so check the dashboard's headroom panel for the total.

### btrfs free space

df on btrfs is an estimate, and btrfs can run out of metadata space while data
space remains. The dashboard shows `node_btrfs_*` allocation by block group.

## Cluster side

`platform/observability/scrape-host.yaml` is a `ScrapeConfig` (operator
`monitoring.coreos.com/v1alpha1`, CRD shipped in kube-prometheus-stack 92.16.1)
with job `econgraph-host`. The chart's Prometheus selects ScrapeConfigs labelled
`release: monitoring` by default, so the HelmRelease needs no change.

The operator must set `HOST_EXPORTER_TARGET` in the `platform-settings` ConfigMap
to the same `IP:port` as `--listen-address`. Flux substitutes it like the other
settings; `bootstrap.py reconcile` refuses settings without it, because an empty
target would fail the whole `platform-observability` Kustomization.

Prometheus reaches the host over the VM bridge. The `monitoring` namespace has no
NetworkPolicy, so Cilium allows the egress, and pod traffic leaving a node is
masqueraded to the node's address. **If the host runs a firewall**, allow TCP to
the exporter port on the bridge interface from the Talos node addresses only,
for example:

```sh
sudo ufw allow in on econgraph0 from 192.168.250.0/24 to 192.168.250.1 port 9100 proto tcp
```

Anything else on that network can also read these metrics unless the firewall
restricts it; they reveal mount points, sizes and pool names, not data.

## Checks that need a live cluster

Confirmed from the chart: `KubePersistentVolumeFillingUp` and `NodeFilesystem*`
are enabled by default (`defaultRules.rules.kubernetesStorage`,
`nodeExporterAlerting`), the node-exporter DaemonSet runs on every node
(control planes included) with `hostNetwork`, `hostPID` and the host root
mounted, and the `monitoring` namespace is labelled
`pod-security.kubernetes.io/enforce: privileged`, so Talos's default baseline
PodSecurity does not block it. Confirm on the scratch cluster:

1. `kubectl -n monitoring get ds monitoring-prometheus-node-exporter` has a ready
   pod on every node, and Prometheus shows `node_filesystem_size_bytes` for
   `mountpoint="/var"` on every node and `/var/mnt/postgres` on workers. If Talos
   names them differently, update the `mountpoint` regex in the
   `EconGraphNodeVolume*` rules and the dashboard.
2. The kubelet target (`job="kubelet"`) is up: Talos's kubelet must accept
   webhook token authentication and authorization for Prometheus's service
   account over HTTPS. `kubelet_volume_stats_capacity_bytes` should exist for
   every CNPG, Prometheus and Loki PVC.
3. `up{job="econgraph-host"} == 1` and the host series for `libvirt_pool`,
   `objectstore` and `host_root` are present.
4. Fill a worker's `/var/mnt/postgres` with a test file in a test PVC past 80%
   and see `EconGraphNodeVolumeAlmostFull` fire after 15 minutes, and past 97%
   for `KubePersistentVolumeFillingUp` (critical, 1 minute).

## Local verification

```sh
bash scripts/host/test-host-disk-check.sh
shellcheck -x scripts/host/host-disk-check.sh scripts/host/install-host-disk-exporter.sh scripts/host/test-host-disk-check.sh
kubectl kustomize platform/observability
```

The promtool rule tests run with the other platform rule tests. None of this
proves the exporter on the real host or the alerts on a running cluster.
