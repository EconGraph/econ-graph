# Production data: snapshots, dumps and restores

Production, UAT and integration testing all run on one host (threadripper),
on a kind cluster under rootless Docker. This runbook covers how the
production data is protected on that host, and how to get it back.

The older in-cluster dump job is covered in
[POSTGRES_BACKUP_RESTORE.md](POSTGRES_BACKUP_RESTORE.md); see
[The in-cluster `postgres-backup` CronJob](#the-in-cluster-postgres-backup-cronjob)
for how the two fit together.

## Layout

All production data lives on btrfs subvolumes at the top level of the
`/home` filesystem (btrfs RAID10 on 4x Samsung 990 PRO NVMe, label `home`,
UUID `03ee0fcb-5f19-4f44-8596-e227f1fb647c`), next to `@home`, so a snapshot
of one never contains the other. `scripts/host/setup-subvolumes.sh` creates
them, adds their fstab lines and makes jmalicki the owner of each subvolume
root.

| Mount point                  | Subvolume               | Contents                                                        |
|------------------------------|-------------------------|-----------------------------------------------------------------|
| `/srv/econgraph/data-plane`  | `@econgraph-data-plane` | `pg/`: main Postgres (`econ_graph`, pod `postgresql-0`); `objects/`: future Parquet/Iceberg |
| `/srv/econgraph/app-prod`    | `@econgraph-app-prod`   | `keycloak-pg/` (pod `keycloak-postgres-0`, db `keycloak`); later the app DB |
| `/srv/econgraph/snapshots`   | `@econgraph-snapshots`  | read-only snapshots of the two above                            |
| `/srv/econgraph-backups`     | `@econgraph-backups` on the 870 pool | `btrbk/` (root, 0700): btrbk send/receive target; `dumps/`: nightly logical dumps. Created by `scripts/host/setup-backup-pool.sh` |
| `~/econgraph-backups/`       | (inside `@home`)        | nightly logical dumps until `/srv/econgraph-backups/dumps` exists |
| `~/econgraph-branches/`      | (inside `@home`)        | writable clones of a snapshot for a branch's data plane         |

The 870 pool is three Samsung 870 SATA SSDs (`/dev/sda1`, `/dev/sdb1`,
`/dev/sdc1`), one btrfs filesystem, UUID
`a1e61098-087b-4403-8765-b714bf7b240b`, label `home-temp`, data RAID1 /
metadata RAID1C3. It is **not empty**: its top level holds `@home` (ID 256)
with nested snapshots `@home/.final-snap` and `@home/.final-snap2`, an older
copy of `/home` (about 1.57 TiB) that Joe decides about separately; about
2.59 TiB is free. The backup target is a new top-level subvolume
`@econgraph-backups` beside it. Nothing here ever wipes, reformats or
touches `@home` or its snapshots. Because the pool is a different filesystem
from `/home`, snapshots reach it only by `btrfs send`/`receive` (btrbk, as
root), never by copying files.

### File ownership

The Postgres data directories (`data-plane/pg/`, `app-prod/keycloak-pg/`)
are owned by host UID **100998**, mode 0700. That is the container's
`postgres` user (UID 999) mapped through rootless Docker
(`/etc/subuid`: `jmalicki:100000:65536`). jmalicki cannot read them
directly. Any copy has to preserve that ownership and mode, or Postgres
refuses to start on it. btrfs snapshots and `btrfs send`/`receive` preserve
it. A file-level copy has to be made as root with ownership kept
(`cp -a`, `rsync -aHAX --numeric-ids`), or from inside a rootless container,
where UID 999 maps back to 100998.

### Why snapshots are safe for a running Postgres

Each Postgres data directory, WAL included, lives entirely inside one
subvolume, and a btrfs snapshot captures the whole subvolume atomically.
Starting Postgres on a snapshot is therefore exactly like starting it after
a power cut: it replays WAL and comes up consistent. No `pg_backup_start`,
no stopping the database.

That only holds while these rules do:

- `pg/` and `objects/` stay in the **same** subvolume, so one snapshot
  captures both at the same instant. Never split them.
- Nothing moves `pg_wal` (or a tablespace) out of the data directory's
  subvolume, by symlink or otherwise.
- No subvolume is ever created *inside* `data-plane` or `app-prod`: nested
  subvolumes are silently left out of snapshots.
- `data-plane` and `app-prod` are snapshotted separately. They are not
  consistent with each other, which is fine because nothing joins across
  them.

## What each piece does

| Piece | Runs as | When | What |
|---|---|---|---|
| btrbk (`scripts/backup/btrbk/btrbk.conf`, installed by `scripts/host/install-btrbk.sh`) | root, system timer | hourly | read-only snapshots of `data-plane` and `app-prod` into `snapshots/`, and send/receive of them to the 870 pool once that is set up |
| `scripts/backup/snapshot-now.sh <reason>` | jmalicki | before any risky change | extra read-only snapshot that btrbk never prunes |
| `scripts/backup/dump-databases.sh` | jmalicki, `econgraph-dump.timer` | nightly 10:00 UTC | `pg_dump -Fc` of `econ_graph` and `keycloak`, verified, pruned after 14 days |
| `scripts/backup/restore-drill.sh` | jmalicki, `econgraph-restore-drill.timer` | Sundays 11:00 UTC | restores the newest dump into a throwaway Postgres 18 container and checks it |
| `scripts/backup/check-freshness.sh` | jmalicki, `econgraph-backup-freshness.timer` | hourly | fails if the newest snapshot, verified dump or passing drill is too old |
| `scripts/backup/offsite-restic.sh` | jmalicki, `econgraph-offsite.timer` | daily 10:45 UTC | restic copy of the verified dumps off the machine (**not configured yet**) |
| `scripts/backup/branch-clone.sh` | jmalicki | by hand | writable clone of a snapshot for a branch |
| `scripts/backup/restore-dump.sh` | jmalicki | by hand | restore a dump into the live cluster |
| `scripts/host/restore-snapshot.sh` | root | by hand | swap a live subvolume for a snapshot |
| `scripts/host/delete-snapshots.sh` | root | by hand | delete pre-change snapshots and set-aside subvolumes |

Everything that needs root is a script under `scripts/host/`; each refuses
to run without sudo, takes `--dry-run`, prints every action, and is safe to
re-run. They are listed in order in [Needs sudo (Joe)](#needs-sudo-joe).

The user scripts read optional overrides from
`~/.config/econgraph/backup.env` (plain `KEY=value`): `BACKUP_ROOT`,
`BACKUP_RETENTION_DAYS`, the freshness limits, `FRESHNESS_SKIP_SNAPSHOTS=1`
(until btrbk is running), and so on; see `scripts/backup/lib.sh` for the
full list.

## Retention

| What | Kept |
|---|---|
| btrbk snapshots on the NVMe array | every hourly one for 48h, then one a day for 30 days, then one a month for 12 months (`snapshot_preserve 48h 30d 12m`) |
| pre-change snapshots (`<name>.pre-<stamp>-<reason>`) | until deleted with `scripts/host/delete-snapshots.sh`; btrbk ignores them |
| btrbk copies on the 870 pool (once enabled) | 14 daily, 8 weekly, 24 monthly |
| logical dumps | 14 days (`BACKUP_RETENTION_DAYS`); the newest verified dump is never pruned |
| off-site restic (once configured) | 14 daily, 8 weekly, 12 monthly |

Snapshots on the NVMe array share its disks: they protect against bad
migrations, mistaken recrawls and deleted rows, not against losing the
array. Until `setup-backup-pool.sh` has run, the dumps also sit on the same
array (in `/home`). The 870 pool (btrbk send/receive plus the dumps) and the
off-site copy are what cover losing the NVMe array; the 870 pool is in the
same machine, so only the off-site copy covers losing the machine.

## Timers and installation

The user units live in `scripts/backup/systemd/`. Install (or update after a
merge) with:

```sh
scripts/backup/install-user-units.sh          # --dry-run to preview
```

It copies the scripts to `~/.local/lib/econgraph-backup/` (the timers run
that reviewed copy, not whatever branch the checkout has), installs the
units into `~/.config/systemd/user/`, and enables the dump, drill and
freshness timers; the off-site timer only once `restic.env` exists. User
timers need lingering to run while jmalicki is logged out; on threadripper
`loginctl show-user jmalicki -p Linger` already reports `Linger=yes`.

Until btrbk is running, put `FRESHNESS_SKIP_SNAPSHOTS=1` in
`~/.config/econgraph/backup.env`, or the hourly freshness check fails on the
missing snapshots (which is accurate, but noisy).

Check on them with `systemctl --user list-timers 'econgraph-*'`,
`systemctl --user --failed` and `journalctl --user -u econgraph-dump`.

## Pre-change snapshot

Data migrations, deliberate recrawls, schema changes and restores are
"critical" changes: they need Joe's sign-off and a dump first. Before one:

```sh
scripts/backup/snapshot-now.sh "recrawl BLS series after the parser fix"
scripts/backup/dump-databases.sh
```

`snapshot-now.sh` snapshots both `data-plane` and `app-prod` (or one, with
`--only data-plane`) as `<name>.pre-<UTC stamp>-<reason>`, logs it to
`~/.local/state/econgraph/snapshots.log`, and needs no sudo. Postgres keeps
running. When the change has been accepted, delete the snapshot:
`sudo scripts/host/delete-snapshots.sh data-plane.pre-...`.

## Nightly dumps

`dump-databases.sh` runs `pg_dump --format=custom` inside each Postgres pod
(through `docker exec econ-graph-control-plane kubectl ...`, since the
host's `kubectl` is a microk8s wrapper) and writes:

```
~/econgraph-backups/<YYYYMMDDTHHMMZ>/
  econ_graph.dump  econ_graph.dump.toc
  keycloak.dump    keycloak.dump.toc
  MANIFEST         sizes, TOC entry counts, row counts at dump time
  SHA256SUMS
  VERIFIED         written last, only when every dump passed
```

Each archive is checked with `pg_restore --list` and a full read-through
(`pg_restore --file=/dev/null`), using the `postgres:18` image because the
host's `pg_restore` is older than the server. The directory is built as
`<stamp>.partial` and renamed only when everything passed; any failure
removes it and exits non-zero. `--target-dir` (or `BACKUP_ROOT`) moves the
output. By default the dumps go to `/srv/econgraph-backups/dumps` as soon as
that directory exists and is writable, and to `~/econgraph-backups`
before that; older dumps left in `~/econgraph-backups` are not pruned
automatically after the switch.

## Restore drill

```sh
scripts/backup/restore-drill.sh                       # newest verified dump
scripts/backup/restore-drill.sh --expect-series 10528 --expect-data-points 503001 \
    ~/econgraph-backups/20261024T0442Z/econ_graph.dump
```

It starts `postgres:18` on a random localhost port, restores `econ_graph`
(and `keycloak` from the same directory), and checks: `economic_series`,
`data_points` and `data_sources` are non-empty, no data point references a
missing series, no data point lacks a date, the Diesel migration table is
populated, Keycloak has realms, and any `--expect-*` counts match exactly.
Without `--expect-*` it compares against the counts in the dump's
`MANIFEST` and warns on a difference. It prints one `RESTORE DRILL PASS` or
`RESTORE DRILL FAIL` line, removes the container, and on a pass records
`~/.local/state/econgraph/restore-drill.last` for the freshness check.

First run, 2026-10-24, against `~/econgraph-backups/20261024T0442Z`:

```
drill: economic_series=10528 data_points=503001 data_sources=8
drill: series_with_points=10528 orphan_points=0 null_dates=0
drill: diesel migrations=2 latest=20261021000100 data_points.date=1960-01-01..2026-09-01
RESTORE DRILL PASS: series=10528 data_points=503001 sources=8 keycloak: realms=2 users=1 (... 15s)
```

## Freshness check

`check-freshness.sh` prints one line per check and exits 1 if any is
missing or stale:

| Check | Default limit | Variable |
|---|---|---|
| newest `data-plane` and `app-prod` snapshot (btrbk or pre-change) | 3h | `SNAPSHOT_MAX_AGE_HOURS` |
| newest `VERIFIED` dump | 26h | `DUMP_MAX_AGE_HOURS` |
| newest passing drill | 192h (8 days) | `DRILL_MAX_AGE_HOURS` |

Snapshot ages come from the snapshot names, since reading btrfs creation
times needs root. `--textfile FILE` also writes
`econgraph_backup_last_success_timestamp_seconds{kind=...}` and
`econgraph_backup_fresh{kind=...}` in node_exporter textfile format (the
timer writes `~/.local/state/econgraph/backup-freshness.prom`). Nothing
scrapes it yet: `k8s/monitoring` has Prometheus but no node_exporter or
Pushgateway, and the in-cluster Prometheus can't see host files. Wiring it
up means adding a node_exporter with `--collector.textfile.directory`
pointed there, which is separate work.

## Off-site

No off-site target has been chosen. `offsite-restic.sh` reads the repository
and password from `~/.config/econgraph/restic.env` (outside the repo, mode
600) and exits 2 with setup instructions until that file exists. To set it
up once a target is chosen (any restic backend: SFTP to another machine,
Backblaze B2, S3-compatible storage, a restic REST server):

1. Create `~/.config/econgraph/restic.password` (random, mode 600) and keep
   a copy somewhere other than this machine: without it the off-site copy is
   unreadable.
2. Create `~/.config/econgraph/restic.env` with `RESTIC_REPOSITORY=...`,
   `RESTIC_PASSWORD_FILE=~/.config/econgraph/restic.password` and the
   backend's credentials (running the script with no `restic.env` prints a
   template).
3. `scripts/backup/offsite-restic.sh init`, then `... backup`, then
   `... check`.
4. Re-run `scripts/backup/install-user-units.sh` to enable the daily timer.

Only verified dump directories are uploaded. Snapshots are not sent off-site:
the dumps are a fraction of their size and restore anywhere.

## Restore from a snapshot

Use this to roll production back to a point in time (hourly granularity
for two days, daily for a month, monthly for a year), e.g. after a bad
migration. Everything written since the snapshot is lost; the current data
is set aside, not deleted.

1. Pick the snapshot: `ls /srv/econgraph/snapshots/`.
2. Stop everything that writes, then Postgres (no sudo):
   ```sh
   k() { docker exec econ-graph-control-plane kubectl -n econ-graph "$@"; }
   k scale deployment/econ-graph-backend deployment/crawler-worker --replicas=0
   k scale statefulset/postgresql --replicas=0      # data-plane
   # app-prod: k scale deployment/keycloak statefulset/keycloak-postgres --replicas=0
   ```
3. `sudo scripts/host/restore-snapshot.sh --dry-run <snapshot>`, then
   without `--dry-run`. It refuses while the Postgres pod still exists,
   unmounts the subvolume, renames it to
   `@econgraph-<name>.replaced-<stamp>`, puts a writable snapshot of the
   chosen snapshot in its place under the original name, and remounts it.
4. With Postgres still stopped, restart the kind node that holds the
   storage, so it sees the new subvolume: `docker restart econ-graph-worker`.
   Postgres runs on `econ-graph-worker` (labelled
   `econgraph.io/host-storage=true`), which bind-mounts
   `/srv/econgraph/data-plane` and `/srv/econgraph/app-prod`. That bind mount
   keeps pointing at the old subvolume until the node container restarts;
   skipping this starts Postgres on the old data. Wait for the node to be
   `Ready` (`k get nodes`) before the next step.
5. Scale Postgres back to 1, check the data, then the backend and crawler.
6. Once satisfied, delete the set-aside subvolume:
   `sudo scripts/host/delete-snapshots.sh @econgraph-data-plane.replaced-<stamp>`.
   To undo the restore instead, the set-aside subvolume is the old data.

## Restore from a dump

For a database-level restore (e.g. one table's worth of damage where losing
other recent writes is unacceptable, use a drill container and copy rows
over; for a whole-database rollback, this):

```sh
scripts/backup/snapshot-now.sh "before restoring econ_graph from 20261024T0442Z"
k scale deployment/econ-graph-backend deployment/crawler-worker --replicas=0
scripts/backup/restore-dump.sh econ_graph ~/econgraph-backups/<stamp>/econ_graph.dump
```

It asks for the database name as confirmation (`--yes` skips that), then
runs `pg_restore --clean --if-exists --single-transaction --exit-on-error`
in the pod, so a failure rolls back. As with the in-cluster restore,
`--clean` only drops objects that are in the dump: tables added by later
migrations stay and Diesel's migration history may disagree; check
`__diesel_schema_migrations` afterwards. To restore into a fresh cluster
whose database doesn't exist yet, create the empty database and the
`econgraph` role first (the dump keeps object owners).

To inspect a dump without touching anything, run the restore drill on it:
it leaves nothing behind except its pass/fail line. To keep a restored copy
around for querying, run `docker run` of `postgres:18` yourself and
`pg_restore --no-owner --no-privileges` into it.

## Clone a snapshot for a branch

A branch that needs real data gets its own writable copy of a snapshot. It
takes no space until written to and never touches production:

```sh
scripts/backup/branch-clone.sh create my-branch                  # newest data-plane snapshot
scripts/backup/branch-clone.sh create my-branch data-plane.20261024T040000+0000
scripts/backup/branch-clone.sh list
scripts/backup/branch-clone.sh delete my-branch
```

The clone is `btrfs subvolume snapshot <read-only snapshot>
~/econgraph-branches/<branch>`, i.e. a new writable subvolume, made without
sudo. Point the branch's Postgres 18 at `<clone>/pg` (the files are owned by
the rootless-Docker mapping of the container's postgres user, so run it as a
container, not on the host). Delete also needs no sudo: it empties the clone
from inside a container and then `rmdir`s the empty subvolume.

## The in-cluster `postgres-backup` CronJob

`k8s/manifests/postgres-backup-cronjob.yaml` (with
`postgres-backup-configmap.yaml` and `postgres-backup-pvc.yaml`) dumps
`econ_graph` nightly at 08:00 UTC to the `postgres-backup-data` PVC, and
verifies and prunes like `dump-databases.sh` does. It is **kept for now**,
unchanged, as an independent second copy while the host-side dumps prove
themselves. It is not pointed at the new layout because:

- its PVC is kind local-path storage inside the kind node container: deleting
  or recreating the kind cluster destroys it together with the database,
  which is exactly the failure backups are for, and its PV's reclaim policy
  is currently `Delete`;
- moving it onto a host path would need a static PV and kind `extraMounts`
  (owned by the cluster wiring work, not this), and the obvious host path,
  inside `data-plane`, would put backups on the subvolume they back up;
- it covers only `econ_graph`, not Keycloak, and has no restore drill or
  freshness check.

Retire it (remove the three manifests and their references in
`scripts/deploy/`) once the host timers have run cleanly for two weeks.

## Unprivileged btrfs: what works without root

Tested on threadripper (kernel 7.0, btrfs-progs 6.17.1) as jmalicki, first
in a scratch directory on `/home`, then on the real layout
(`snapshot-now.sh` from `/srv/econgraph/data-plane` and `app-prod`, both
owned by jmalicki and holding only a test directory owned by UID 100998,
into `/srv/econgraph/snapshots`; `/home` is not mounted with
`user_subvol_rm_allowed`):

| Operation | Without root |
|---|---|
| `btrfs subvolume create` | works |
| `btrfs subvolume snapshot -r` of a subvolume you own | works, including from `/srv/econgraph/data-plane` into `/srv/econgraph/snapshots` (two mounts of one filesystem) |
| `btrfs subvolume snapshot` (writable) of a read-only snapshot you own | works |
| writing into a read-only snapshot | fails: `Read-only file system`, also for root inside a container |
| `btrfs property get <snap> ro` | works |
| `btrfs property set <snap> ro false` on a snapshot you own | **works**: the owner can make a snapshot writable again |
| `btrfs subvolume delete` | fails: `Operation not permitted` (needs `user_subvol_rm_allowed`) |
| `rmdir` of an empty subvolume you own | works, and deletes the subvolume |
| `rm -rf` of a subvolume holding Postgres files (owned by the rootless-Docker mapped UID 100998) | fails: `Permission denied`; emptying it from inside a rootless container works |
| `btrfs subvolume show`, `btrfs subvolume list` | fail: `Operation not permitted` (tree search needs root) |
| `btrfs send` | fails: `Operation not permitted` (so `btrfs receive` and btrbk's targets are root-only too) |

Not tested, from the kernel source: snapshotting a subvolume someone else
owns fails with `EPERM` (snapshots are limited to the caller's own
subvolumes), which is why `setup-subvolumes.sh` chowns the subvolume roots
to jmalicki.

The test snapshots on the real layout were removed without root (clear `ro`,
empty from a rootless container, `rmdir`); none were left behind.

Consequences:

- `snapshot-now.sh` and `branch-clone.sh` need no sudo; btrbk does (it uses
  `subvolume show`/`list` and `send`), so it runs as a root system timer.
- **`user_subvol_rm_allowed` is not needed and should stay off.** Nothing
  run as jmalicki needs `btrfs subvolume delete`: btrbk prunes as root,
  branch clones are removed with `rmdir`, and the rest goes through
  `scripts/host/delete-snapshots.sh`. Turning it on would let any process
  running as jmalicki delete a whole subvolume, including a live one, with
  one command.
- Local snapshots are not protected from jmalicki: anything running as
  jmalicki can flip a snapshot writable and empty it through a container.
  They guard against bad changes, not against a hostile or runaway process
  in this account. The 870 pool target (`/srv/econgraph-backups/btrbk`,
  root-only, mode 0700) and the off-site copy are the copies that account
  can't reach.

## Needs sudo (Joe)

Each step is one checked-in script; run with `--dry-run` first. In order:

1. **Create and mount the subvolumes** (from the `ops/host-storage-scripts`
   branch; already done on 2026-10-24): `sudo scripts/host/setup-subvolumes.sh`. Creates
   `@econgraph-data-plane`, `@econgraph-app-prod` and
   `@econgraph-snapshots`, adds their `/etc/fstab` lines, mounts them and
   chowns each subvolume root to jmalicki (required for `snapshot-now.sh`).
2. **Install btrbk and run it hourly**: `sudo scripts/host/install-btrbk.sh`.
   Runs `apt-get install btrbk`, installs `scripts/backup/btrbk/btrbk.conf`
   as `/etc/btrbk/btrbk.conf` (keeping any different existing file), adds
   `/etc/systemd/system/btrbk.timer.d/econgraph-hourly.conf` (the package's
   timer is daily), validates with `btrbk dryrun`, enables `btrbk.timer`
   and takes the first snapshots. Needs step 1 done.
3. **Set up the backup subvolume on the 870 pool**:
   `sudo scripts/host/setup-backup-pool.sh`. Creates the new top-level
   subvolume `@econgraph-backups` (never touching `@home` or its
   snapshots; it lists them so you can see they are left alone), adds its
   `/etc/fstab` line
   (`UUID=a1e61098-... /srv/econgraph-backups btrfs subvol=/@econgraph-backups,compress=zstd:3,ssd,discard=async,space_cache=v2,noatime,nofail,x-systemd.device-timeout=30s 0 0`;
   `nofail` so a missing disk never blocks boot), mounts it, chowns the
   subvolume root to jmalicki, and creates `btrbk/` (root, 0700) and
   `dumps/` (jmalicki). `scripts/host/inspect-sata-pool.sh` (same branch as
   step 1) shows the pool read-only beforehand.
4. **Turn on send/receive to the pool**: re-run
   `sudo scripts/host/install-btrbk.sh`. It enables the target whenever
   `/srv/econgraph-backups/btrbk` is mounted (`--no-backup-pool` keeps it
   off), so steps 2 and 4 can also be one run after step 3. The nightly
   dumps move to `/srv/econgraph-backups/dumps` by themselves.
5. **By hand, when needed**: `sudo scripts/host/restore-snapshot.sh` (restore
   from a snapshot) and `sudo scripts/host/delete-snapshots.sh` (remove
   pre-change snapshots and set-aside subvolumes).

Not needed:

- `loginctl enable-linger jmalicki`: already `Linger=yes`.
- `user_subvol_rm_allowed`: see above; leave it off.
- Anything for the dumps, drill, freshness check, off-site copy or branch
  clones: they run as jmalicki.
- Anything for pre-change snapshots: `snapshot-now.sh` runs as jmalicki
  (only deleting them later goes through `delete-snapshots.sh`).
