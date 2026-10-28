# Production data: snapshots, dumps and restores

Production, UAT and integration testing all run on one host (threadripper),
on a kind cluster on the rootful Docker daemon. This runbook covers how the
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
them, adds their fstab lines and makes the operator the owner of each subvolume
root.

| Mount point                  | Subvolume               | Contents                                                        |
|------------------------------|-------------------------|-----------------------------------------------------------------|
| `/srv/econgraph/data-plane`  | `@econgraph-data-plane` | `pg/`: main Postgres (`econ_graph`, pod `postgresql-0`); `objects/`: future Parquet/Iceberg |
| `/srv/econgraph/app-prod`    | `@econgraph-app-prod`   | `app-pg/`: the app database (`econ_graph_app`, pod `app-postgres-0`); `keycloak-pg/` (pod `keycloak-postgres-0`, db `keycloak`) |
| `/srv/econgraph/snapshots`   | `@econgraph-snapshots`  | read-only snapshots of the two above                            |
| `/srv/econgraph-backups`     | `@econgraph-backups` on the 870 pool | `snapshots/` (root, 0700): copies of every snapshot, by send/receive; `dumps/`: nightly logical dumps. Created by `scripts/host/setup-backup-pool.sh` |
| `~/econgraph-backups/`       | (inside `@home`)        | nightly logical dumps until `/srv/econgraph-backups/dumps` exists |
| `~/econgraph-branches/`      | (inside `@home`)        | writable clones of a snapshot for a branch's data plane         |

The 870 pool is three Samsung 870 SATA SSDs (`/dev/sda1`, `/dev/sdb1`,
`/dev/sdc1`), one btrfs filesystem, UUID
`a1e61098-087b-4403-8765-b714bf7b240b`, label `home-temp`, data RAID1 /
metadata RAID1C3. It is **not empty**: its top level holds `@home` (ID 256)
with nested snapshots `@home/.final-snap` and `@home/.final-snap2`, an older
copy of `/home` (about 1.57 TiB) that the maintainer decides about separately; about
2.59 TiB is free. The backup target is a new top-level subvolume
`@econgraph-backups` beside it. Nothing here ever wipes, reformats or
touches `@home` or its snapshots. Because the pool is a different filesystem
from `/home`, snapshots reach it only by `btrfs send`/`receive`
(`scripts/host/backup-snapshots.sh`, as root), never by copying files.

### Which Docker daemon

The kind cluster runs on the **rootful** Docker daemon at
`unix:///var/run/docker.sock`: kind can't start under rootless Docker on this
host, whose rootless data root is on btrfs (see
`docs/deployment/HOST_STORAGE.md`). The operator's login shell exports `DOCKER_HOST`
for the rootless daemon, so the backup scripts don't use that variable:
`scripts/backup/lib.sh` sets `DOCKER_HOST=unix:///var/run/docker.sock` for
itself and everything it runs, like `scripts/deploy/lib/kind-env.sh`.
Override it with `ECONGRAPH_DOCKER_HOST` (in the environment or in
`~/.config/econgraph/backup.env`). The operator is in the `docker` group, so the
user timers reach that socket without sudo. The scripts log the daemon they
use and warn if it is a rootless one.

The `docker` commands in this runbook are meant for that daemon too. From
the operator's shell, run them with `DOCKER_HOST=unix:///var/run/docker.sock` (or
`docker -H unix:///var/run/docker.sock ...`); a plain `docker` there talks
to the rootless daemon, which has no kind nodes.

### File ownership

The Postgres data directories (`data-plane/pg/`, `app-prod/app-pg/`,
`app-prod/keycloak-pg/`)
are owned by host UID **999**, mode 0700: the `postgres` user of the
`postgres:18` image (`uid=999(postgres) gid=999(postgres)`), which the
manifests run as (`runAsUser: 999`). Rootful Docker uses no user namespace,
so UID 999 in the pod is UID 999 on the host. the operator cannot read them
directly. Any copy has to preserve that ownership and mode, or Postgres
refuses to start on it. btrfs snapshots and `btrfs send`/`receive` preserve
it. A file-level copy has to be made as root with ownership kept
(`cp -a`, `rsync -aHAX --numeric-ids`), or from inside a container on the
rootful daemon (root there is host root).

Data written while the cluster still ran under rootless Docker is owned by
host UID 100998 (999 mapped through `/etc/subuid`). The Postgres init
containers on the host-storage layout (PR #421) re-own a data directory to
999:999 when they find anything else, so a snapshot taken before the switch
still starts.

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

## Nothing is deleted automatically

No timer, service or script that runs on a schedule deletes a snapshot, a
dump or a backup copy. Everything only accumulates:

- snapshots in `/srv/econgraph/snapshots` (hourly and pre-change),
- their copies on the backup pool in `/srv/econgraph-backups/snapshots`,
- dump directories (a failed dump run is kept too, as `<stamp>.failed`),
- the off-site restic repository (the wrapper never runs `forget` or
  `prune`, and refuses to pass them through).

Space is watched instead: see [Space alerts](#space-alerts). Deleting is a
manual step the operator takes, described in [Deleting by hand](#deleting-by-hand).

The old in-cluster CronJob, which is not part of this setup, no longer
deletes its dumps either: [#427](https://github.com/EconGraph/econ-graph/pull/427) removed its 14-day pruning (see
[below](#the-in-cluster-postgres-backup-cronjob)). Nothing deletes backups
automatically.

## What runs when

| When | What | Runs as | Unit |
|---|---|---|---|
| hourly, on the hour | `scripts/host/backup-snapshots.sh`: read-only snapshots of `data-plane` and `app-prod` into `/srv/econgraph/snapshots/<name>.<UTC stamp>`, then incremental `btrfs send`/`receive` of every snapshot not yet on the backup pool into `/srv/econgraph-backups/snapshots` (skipped with a message while the pool is not mounted) | root | `econgraph-backup-snapshots.timer` (system; installed by `scripts/host/install-backup-timer.sh`) |
| hourly, :20 | `scripts/backup/check-freshness.sh`: freshness and space checks | operator | `econgraph-backup-freshness.timer` (user) |
| nightly 10:00 UTC | `scripts/backup/dump-databases.sh`: verified `pg_dump -Fc` of `econ_graph`, `econ_graph_app` and `keycloak` | operator | `econgraph-dump.timer` (user) |
| daily 10:45 UTC | `scripts/backup/offsite-restic.sh backup`: restic copy of the verified dumps (the unit does nothing until `restic.env` exists) | operator | `econgraph-offsite.timer` (user) |
| at least weekly, interactively | `open-backup.sh` followed by `restore-drill.sh`: decrypt and restore a bundle into a throwaway Postgres 18 | operator | manual; no private identity is installed for a timer |
| before a risky change | `scripts/backup/snapshot-now.sh <reason>` | operator | by hand |
| before a deploy that changes the Keycloak image | `scripts/deploy/backup-keycloak-db.sh`: Keycloak scaled to 0, `pg_dump -Fc` of `keycloak` into `~/econgraph-backups/keycloak/<UTC stamp>/`, verified by a restore into a scratch database with equal row counts. `scripts/deploy/restore-keycloak-db.sh` restores it as a fresh database swapped in by rename, the only way to downgrade Keycloak; `restore-dump.sh`'s `--clean` would leave the newer version's tables behind. These encrypted bundles are on the data disk, not the backup pool; `offsite-restic.sh` includes their verified directories alongside nightly bundles once off-site storage is configured. See the [v4.0.0 QA plan](../release/v4.0.0-qa.md) | operator | by hand |
| when needed | `scripts/backup/restore-dump.sh`, `scripts/backup/branch-clone.sh` | operator | by hand |
| when needed | `scripts/host/restore-snapshot.sh`, `scripts/host/delete-snapshots.sh` | root | by hand only |

Everything that needs root is a script under `scripts/host/`; each refuses
to run without sudo, takes `--dry-run`, prints every action, and is safe to
re-run. They are listed in order in [Needs sudo](#needs-sudo).
The root timer runs a root-owned copy at
`/usr/local/sbin/econgraph-backup-snapshots`, never the checkout, so root
never executes a file the operator account can edit.

The user scripts read optional overrides from
`~/.config/econgraph/backup.env` (plain `KEY=value`): `BACKUP_ROOT`, the
freshness and space limits, `FRESHNESS_SKIP_SNAPSHOTS=1` (until the hourly
timer is installed), and so on; see `scripts/backup/lib.sh` and the header
of `check-freshness.sh` for the full list.

Snapshots on the NVMe array share its disks: they protect against bad
migrations, mistaken recrawls and deleted rows, not against losing the
array. Until `setup-backup-pool.sh` has run, the dumps also sit on the same
array (in `/home`). The 870 pool (snapshot copies plus the dumps) and the
off-site copy are what cover losing the NVMe array; the 870 pool is in the
same machine, so only the off-site copy covers losing the machine.

## Space alerts

Because nothing is pruned, `check-freshness.sh` also fails (exit 1, a
`LOWSPACE` line, a failed `econgraph-backup-freshness.service`) when free
space drops below a threshold, and reports how big the backups are:

| Check | Default | Variable |
|---|---|---|
| free space on the `/home` filesystem (live data and local snapshots) | at least 15% | `HOME_MIN_FREE_PCT` |
| free space on the backup pool (when mounted) | at least 15% | `POOL_MIN_FREE_PCT` |
| size of `/srv/econgraph/snapshots`, of the dumps directory and of `/srv/econgraph-backups/snapshots` | reported (`SIZE` lines and metrics), no threshold | |

Free space comes from `df` (an estimate on btrfs, fine for a threshold). The
snapshot directories' contents are not readable by the operator, so their sizes
come from `/var/lib/econgraph/backup-sizes.env`, which the root hourly job
rewrites at most once a day with `btrfs filesystem du -s` (shared extents
counted once, so the number is the space really used). The dumps directory
is measured directly with `du`.

The local snapshots grow slowly while the data changes little (a snapshot
costs only the blocks changed since the previous one), but a recrawl or a
big migration rewrites a lot, and every hourly snapshot taken during it
keeps its own copy of the old blocks.

## Deleting by hand

Only the operator deletes, only by hand, with
`scripts/host/delete-snapshots.sh` (never from a timer: it refuses to run
without a terminal):

```sh
sudo scripts/host/delete-snapshots.sh --list                     # local snapshots
sudo scripts/host/delete-snapshots.sh --list --pool              # copies on the backup pool
sudo scripts/host/delete-snapshots.sh --dry-run data-plane.20261101T030000Z ...
sudo scripts/host/delete-snapshots.sh data-plane.20261101T030000Z ...
sudo scripts/host/delete-snapshots.sh --pool data-plane.20261101T030000Z ...
sudo scripts/host/delete-snapshots.sh @econgraph-data-plane.replaced-20261102T101500Z
```

It prints every subvolume it would delete, warns when one is the newest of
its series (the next hourly copy to the pool then uses an older common
parent, or does a full send if none is left), and deletes only after you
type `delete <count>`. Keep at least one snapshot of each series on both
sides so the hourly copy stays incremental. `*.incomplete-*` leftovers on the
pool (from a receive that failed and was set aside) and `*.mismatch-*` ones
(a read-only copy whose Received UUID was not the local snapshot's, set
aside before receiving it again) can be deleted the same way with `--pool`.

Dumps are plain directories owned by the operator: remove old ones with `rm -r`
if ever needed. Never remove the newest `VERIFIED` one.

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
timers need lingering to run while the operator is logged out; on threadripper
`loginctl show-user "$USER" -p Linger` already reports `Linger=yes`.

Until the root hourly timer is installed, put `FRESHNESS_SKIP_SNAPSHOTS=1`
in `~/.config/econgraph/backup.env`, or the hourly freshness check fails on
the missing snapshots (which is accurate, but noisy).

Check on them with `systemctl --user list-timers 'econgraph-*'`,
`systemctl --user --failed` and `journalctl --user -u econgraph-dump`; the
root job with `systemctl list-timers econgraph-backup-snapshots.timer` and
`journalctl -u econgraph-backup-snapshots`.

## Pre-change snapshot

Data migrations, deliberate recrawls, schema changes and restores are
"critical" changes: they need the maintainer's sign-off and a dump first. Before one:

```sh
scripts/backup/snapshot-now.sh "recrawl BLS series after the parser fix"
scripts/backup/dump-databases.sh
```

`snapshot-now.sh` (`--dry-run` to preview) snapshots both `data-plane` and
`app-prod` (or one, with `--only data-plane`) as `<name>.pre-<UTC stamp>-<reason>`, logs it to
`~/.local/state/econgraph/snapshots.log`, and needs no sudo. Postgres keeps
running. The next hourly run copies it to the backup pool with the others.
Like every snapshot it is kept until the operator deletes it by hand.

## Nightly dumps

The nightly `dump-databases.sh` and pre-change
`backup-kind-databases.sh` share one verified recovery payload. Each dumps
`econ_graph`, `econ_graph_app` and `keycloak` from the kind cluster, reads
each archive back in full with the pod's matching `pg_restore`, and records
row counts from the same exported PostgreSQL snapshot as each archive. The
snapshot-exporting session stays alive until its dump and count queries finish,
so concurrent nightly writes cannot create a false row-count mismatch during
recovery. Each database has its own consistent snapshot; there is no atomic
snapshot spanning the app, data and Keycloak databases. Stop writers, as the
pre-change migration workflow does, when a coordinated cross-database recovery
point is required. An optional database is skipped only when neither its StatefulSet
nor its data PVC exists. A stopped or unreachable deployed database fails the
backup. The payload includes `counts.env`, `MANIFEST` and the cluster Secrets
needed to recover the databases with their original credentials.

Install `age` (with `age-keygen`) on the host. Set **only the public recipient** in
`~/.config/econgraph/backup.env` (or the environment):

```sh
BACKUP_AGE_RECIPIENT=age1... # the maintainer's actual public recipient
```

Keep the private identity outside agent access and off this host if possible.
No private identity is installed in a timer or backup configuration. Missing
or invalid recipient, missing age, or unavailable tmpfs staging fails closed:
there is no plaintext fallback. Teardown, cluster recreation and the
host-storage migration check these before they dump, stop or delete anything.
The migration, which stops the writers before its backup, also checks before
the downtime that the tmpfs staging has room for the on-disk size of every
running database plus 64 MiB.

`backup_recipient_proven` in `scripts/backup/encryption.sh` is the hook for
proving that the maintainer holds the recipient's private identity; it
currently accepts every well-formed recipient, so a backup proves that its
ciphertext decrypts, not that the maintainer's key can decrypt it.

The persistent output is:

```text
~/econgraph-backups/<YYYYMMDDTHHMMSSZ>/
  backup.tar.age  # encrypted dumps, counts, MANIFEST and Secrets together
  SHA256SUMS      # checksum of the ciphertext
  VERIFIED        # timestamp: payload read-back and the decrypt check passed
```

The payload is encrypted to the public recipient and to a throwaway age key
generated in the tmpfs staging. The published `backup.tar.age` is decrypted
with the throwaway key and the decrypted tar stream must hash the same as the
stream that was encrypted; the throwaway key is then destroyed, so it cannot
open the bundle later. Publication uses an exclusive `.partial` directory, an
fsync of the ciphertext, checksum, marker and directory, and a final rename
(itself fsynced) only after all writes and the decrypt check succeed. Failed
ciphertext remains `.partial` without a `VERIFIED` marker. Writers into one
backup root are serialized by `.lock` in that root; the nightly timer fails
rather than wait, the deployment scripts wait. Existing backups are never overwritten, deleted or
converted automatically. The files are 0600 and directories 0700.
`--target-dir` or `BACKUP_ROOT` selects the nightly output root; the default
uses `/srv/econgraph-backups/dumps` when available, otherwise
`~/econgraph-backups`. The pre-change writer uses `ECONGRAPH_BACKUP_ROOT`. The unified writer covers
the canonical data/app/Keycloak databases; an older `BACKUP_DATABASES` override
is refused rather than silently ignored. `KIND_NODE` selects its kind cluster
and must agree with an explicitly supplied `KIND_CLUSTER`.

Plaintext exists only in a private disposable directory under
`BACKUP_PLAINTEXT_ROOT` (default `/dev/shm`), which must be tmpfs. It is removed
when the writer/consumer exits, including failures. tmpfs needs enough memory
for the complete compressed payload; a full filesystem aborts the backup
before destructive steps. **Use encrypted swap or disable swap**: tmpfs alone
does not prevent plaintext pages from reaching swap. SIGKILL or a host crash
may leave temporary RAM staging until cleanup/reboot; review stale staging
manually. This does not isolate a running backup from another process using
the same Unix identity or from root.

Fresh host-storage migrations reuse their verified RAM staging until their
process exits. After an interruption, recovery requires the maintainer to
open the encrypted bundle interactively; the writer cannot decrypt its own
stored backups. To recover a downloaded off-site copy, use that downloaded
bundle directory in the same commands below. Its saved Secrets travel with
the dumps because the entire verified directory is selected by restic.

Existing plaintext backup directories retain their format and remain readable
by the restore scripts. Tighten their modes manually and migrate selected
copies using `backup_seal_dir` from `scripts/backup/encryption.sh` after
`backup_encryption_preflight`; verify an interactive restore before deciding
what to do with the originals. Never delete originals automatically.

This encrypts **host logical backup bundles**. Live database files, btrfs
snapshots, and the independent in-cluster backup CronJob/PVC remain separate copies
with their existing protection. The Keycloak-upgrade writer also encrypts its
special restore-tested payload with the same public recipient. In particular, these changes do not claim those copies are
public-key encrypted; the in-cluster backup work must be integrated separately.

The encryption suite (`scripts/backup/test-encryption.sh`) uses a disposable
synthetic identity and transport stubs. The concurrent-write regression
(`python3 scripts/backup/test-snapshot-counts.py`) starts one isolated
`postgres:18` container, inserts rows after each snapshot export, restores all
three archives, and checks that metadata and restored rows agree while the
live fixture has newer rows. It also checks that exporter sessions are closed;
it uses the daemon in `DOCKER_HOST` (for example the rootless one), else the
system daemon. `scripts/deploy/test-backup-refusals.sh` runs teardown,
recreation and both migration directions against stubs and checks that each
refuses without a recipient, with disk staging, and (the migration) with too
little tmpfs, before any dump or downtime. `scripts/backup/test-restore-drill.sh`
checks the drill's encrypted-bundle guard and tmpfs data directory with a
Docker stub. CI runs all four; the real PostgreSQL check adds a
scratch-container image pull and fixture run. None of them uses a live
cluster, backup or maintainer key.

## Restore drill

```sh
# Run at an operator terminal; the identity path is prompted, not saved.
scripts/backup/open-backup.sh ~/econgraph-backups/<stamp> -- \
    scripts/backup/restore-drill.sh @BACKUP@/econ_graph.dump
scripts/backup/open-backup.sh ~/econgraph-backups/<stamp> -- \
    scripts/deploy/restore-kind-databases.sh --from @BACKUP@ --dry-run
```

`open-backup.sh` checks the ciphertext checksum, prompts for the private
identity path, decrypts and authenticates the payload into private tmpfs,
rejects archive traversal and links, then runs the command with `@BACKUP@`
replaced by that directory. It removes only that disposable directory on exit.
It refuses unattended use. A checksum detects storage corruption; age
provides ciphertext authentication during decryption.

The installer disables the old unattended weekly restore timer: without a
private key it cannot restore an encrypted backup. Run the drill manually
at least weekly; the existing freshness check still reports overdue drills.
An unattended invocation fails clearly and records no false restore PASS.

The drill starts `postgres:18` on a random localhost port, with its data
directory on tmpfs so the restored plaintext never reaches a disk volume, restores `econ_graph`
(and `econ_graph_app` and `keycloak` from the same directory), and checks: `economic_series`,
`data_points` and `data_sources` are non-empty, no data point references a
missing series, no data point lacks a date, the Diesel migration table is
populated (in the app database too, once it has tables), Keycloak has
realms, and any `--expect-*` counts match exactly.
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
missing or stale, or if space is low ([Space alerts](#space-alerts)):

| Check | Default limit | Variable |
|---|---|---|
| newest local `data-plane` and `app-prod` snapshot (hourly or pre-change) | 3h | `SNAPSHOT_MAX_AGE_HOURS` |
| last hourly `backup-snapshots.sh` run, which must also have had no failures | 3h | `SNAPSHOT_MAX_AGE_HOURS` |
| newest snapshot copied to the backup pool (when mounted), and the last copy didn't fail | 3h | `POOL_COPY_MAX_AGE_HOURS` |
| newest `VERIFIED` dump | 26h | `DUMP_MAX_AGE_HOURS` |
| newest passing drill | 192h (8 days) | `DRILL_MAX_AGE_HOURS` |

Snapshot ages come from the snapshot names, since reading btrfs creation
times needs root. The backup pool's snapshot directory is root-only, so its
state comes from `/var/lib/econgraph/backup-snapshots.status`, written by
the root hourly job. `--textfile FILE` also writes
`econgraph_backup_last_success_timestamp_seconds`, `econgraph_backup_fresh`,
`econgraph_backup_free_bytes`, `econgraph_backup_free_ratio` and
`econgraph_backup_size_bytes` in node_exporter textfile format (the timer
writes `~/.local/state/econgraph/backup-freshness.prom`). Nothing scrapes it
yet: `k8s/monitoring` has Prometheus but no node_exporter or Pushgateway,
and the in-cluster Prometheus can't see host files. Wiring it up means
adding a node_exporter with `--collector.textfile.directory` pointed there,
which is separate work.

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

Verified Keycloak-upgrade bundle directories under `KEYCLOAK_BACKUP_ROOT`
(default `${ECONGRAPH_BACKUP_ROOT:-$HOME/econgraph-backups}/keycloak`) are included
alongside verified nightly directories, even when the nightly root is on a
different disk. Recovery prompts for an identity path before contacting the
cluster, and legacy plaintext verified directories remain supported.

Only verified dump directories are uploaded, and nothing is ever forgotten
or pruned from the repository: `offsite-restic.sh` has no `forget` or
`prune`, and `offsite-restic.sh restic forget|prune ...` is refused.
Snapshots are not sent off-site: the dumps are a fraction of their size and
restore anywhere.

## Restore from a snapshot

Use this to roll production back to a point in time (any hour since the
hourly timer was installed, or any pre-change snapshot), e.g. after a bad
migration. Everything written since the snapshot is lost; the current data
is set aside, not deleted.

1. Pick the snapshot: `ls /srv/econgraph/snapshots/`. If the local one is
   gone (or the NVMe array was lost and rebuilt), use the copy on the backup
   pool: `sudo scripts/host/delete-snapshots.sh --list --pool` lists them,
   and step 3 takes `--from-pool`, which first receives that snapshot back
   into `/srv/econgraph/snapshots`. It refuses a pool copy without a
   Received UUID, and a local snapshot of the same name that the pool copy
   was not received from.
2. Stop everything that writes, then Postgres (no sudo):
   ```sh
   k() { docker -H unix:///var/run/docker.sock exec econ-graph-control-plane kubectl -n econ-graph "$@"; }
   k scale deployment/econ-graph-backend deployment/crawler-worker --replicas=0
   k scale statefulset/postgresql --replicas=0      # data-plane
   # app-prod: k scale deployment/econ-graph-backend deployment/keycloak \
   #   statefulset/app-postgres statefulset/keycloak-postgres --replicas=0
   ```
3. `sudo scripts/host/restore-snapshot.sh --dry-run [--from-pool] <snapshot>`,
   then without `--dry-run`. It refuses while the Postgres pod still exists,
   asks you to type the subvolume name (`@econgraph-data-plane` or
   `@econgraph-app-prod`), unmounts the subvolume, renames it to
   `@econgraph-<name>.replaced-<stamp>`, puts a writable snapshot of the
   chosen snapshot in its place under the original name, and remounts it.
4. With Postgres still stopped, restart the kind node that holds the
   storage, so it sees the new subvolume:
   `docker -H unix:///var/run/docker.sock restart econ-graph-worker`.
   Postgres runs on `econ-graph-worker` (labelled
   `econgraph.io/host-storage=true`), which bind-mounts
   `/srv/econgraph/data-plane` and `/srv/econgraph/app-prod`. That bind mount
   keeps pointing at the old subvolume until the node container restarts;
   skipping this starts Postgres on the old data. Wait for the node to be
   `Ready` (`k get nodes`) before the next step.
5. Scale Postgres back to 1, check the data, then the backend and crawler.
6. The set-aside subvolume stays until the operator deletes it by hand
   (`sudo scripts/host/delete-snapshots.sh @econgraph-data-plane.replaced-<stamp>`).
   To undo the restore instead, it is the old data.

## Restore from a dump

For a database-level restore (e.g. one table's worth of damage where losing
other recent writes is unacceptable, use a drill container and copy rows
over; for a whole-database rollback, this):

```sh
scripts/backup/snapshot-now.sh "before restoring econ_graph from 20261024T0442Z"
k scale deployment/econ-graph-backend deployment/crawler-worker --replicas=0
scripts/backup/restore-dump.sh --dry-run econ_graph ~/econgraph-backups/<stamp>/econ_graph.dump
scripts/backup/restore-dump.sh econ_graph ~/econgraph-backups/<stamp>/econ_graph.dump
```

`--dry-run` checks the archive and the pod and prints the plan. Without
it, it waits up to 2 minutes for the pod to be Ready (and stops if it isn't),
asks for the database name as confirmation (`--yes` skips that), then
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
scripts/backup/branch-clone.sh create my-branch data-plane.20261024T040000Z
scripts/backup/branch-clone.sh list
scripts/backup/branch-clone.sh --dry-run delete my-branch        # shows what would go
scripts/backup/branch-clone.sh delete my-branch                  # asks you to type my-branch
```

The clone is `btrfs subvolume snapshot <read-only snapshot>
~/econgraph-branches/<branch>`, i.e. a new writable subvolume, made without
sudo, plus a `<branch>.source` file next to it naming the snapshot. `create`
only clones a read-only subvolume directly in `/srv/econgraph/snapshots`,
never a live writable one. Point
the branch's Postgres 18 at `<clone>/pg` (the files are owned by
UID 999, the image's postgres user, so run it as a container, not on the
host). Delete also needs no sudo: it empties the clone from inside a
container on the rootful daemon (root there is host root) and then `rmdir`s
the empty subvolume.

Because that container is host root, delete is fenced in. It refuses a
name that is empty, starts with `.` or contains `/`; anything that is a
symlink or does not resolve (with `realpath`) to a direct child of
`BRANCH_CLONE_DIR`; anything without the `<branch>.source` marker that
only `create` writes; and anything that is not a writable btrfs subvolume.
It then asks you to type the clone's name, and refuses without a terminal
unless given `--yes`. `--dry-run` works for `create` and `delete`. The
refusals are tested by `scripts/backup/test-branch-clone.sh` (in CI).

## The in-cluster `postgres-backup` CronJob

`k8s/manifests/postgres-backup-cronjob.yaml` (with
`postgres-backup-configmap.yaml` and `postgres-backup-pvc.yaml`) dumps the
same databases as `dump-databases.sh` (`econ_graph`, `econ_graph_app` and
`keycloak`) nightly at 08:00 UTC to the `postgres-backup-data` PVC, except
that it dumps `econ_graph_app` only when the backend's `app-database-url`
names it (`APP_DATABASE=separate`; with `shared`, the app tables are in the
`econ_graph` dump). Each
dump is read back in full and must hold at least one table, each is taken
whether or not the others succeed, and the Job fails when any of them fails.
Like everything in this runbook it deletes nothing: it used to prune its dumps after 14 days (`BACKUP_RETENTION_DAYS`), and
[#427](https://github.com/EconGraph/econ-graph/pull/427) removed that, so its dumps stay until the operator removes them by hand
(`POSTGRES_BACKUP_RESTORE.md` says how). It is kept as an
independent second copy: the two share no files, schedule or lock, run
through different paths (the CronJob over the Services from inside the
cluster, `dump-databases.sh` with `kubectl exec` through the kind node), and
neither touches the other's dumps. The host-side dumps are the primary copy
on threadripper. The CronJob is not pointed at the new layout because:

- its PVC is kind local-path storage inside the kind node container: deleting
  or recreating the kind cluster destroys it together with the database,
  which is exactly the failure backups are for, and its PV's reclaim policy
  is currently `Delete`;
- moving it onto a host path would need a static PV and kind `extraMounts`
  (owned by the cluster wiring work, not this), and the obvious host path,
  inside `data-plane`, would put backups on the subvolume they back up;
- it has no restore drill or freshness check, and no alert watches its Job.

Retire it (remove the three manifests and their references in
`scripts/deploy/`) once the host timers have run cleanly for two weeks.
Removing the PVC deletes its dumps with it (reclaim policy `Delete`), so
copy off anything worth keeping first, by hand.

## Unprivileged btrfs: what works without root

Tested on threadripper (kernel 7.0, btrfs-progs 6.17.1) as the operator, first
in a scratch directory on `/home`, then on the real layout
(`snapshot-now.sh` from `/srv/econgraph/data-plane` and `app-prod`, both
owned by the operator and holding only a test directory owned by UID 100998,
the rootless-Docker mapping of UID 999 in use at the time,
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
| `rm -rf` of a subvolume holding Postgres files (owned by another UID: 999 now, 100998 under the old rootless Docker) | fails: `Permission denied`; emptying it from inside a container works (tested rootless with 100998; rootful containers run as host root) |
| `btrfs subvolume show`, `btrfs subvolume list` | fail: `Operation not permitted` (tree search needs root) |
| `btrfs send` | fails: `Operation not permitted` (so copying to the backup pool is root-only) |

Not tested, from the kernel source: snapshotting a subvolume someone else
owns fails with `EPERM` (snapshots are limited to the caller's own
subvolumes), which is why `setup-subvolumes.sh` chowns the subvolume roots
to the operator.

The test snapshots on the real layout were removed without root (clear `ro`,
empty from a container, `rmdir`); none were left behind.

Consequences:

- `snapshot-now.sh` and `branch-clone.sh` need no sudo. The hourly job
  could take its snapshots as the operator, but the copy to the backup pool
  needs `btrfs send`/`receive`, so it runs as a root system timer.
- **`user_subvol_rm_allowed` is not needed and should stay off.** Nothing
  run as the operator needs `btrfs subvolume delete`: nothing is deleted
  automatically, branch clones are removed with `rmdir`, and manual
  deletion goes through `scripts/host/delete-snapshots.sh`. Turning it on would let any process
  running as the operator delete a whole subvolume, including a live one, with
  one command.
- Local snapshots are not protected from the operator: anything running as
  the operator can flip a snapshot writable and empty it through a container.
  They guard against bad changes, not against a hostile or runaway process
  in this account. The copies on the 870 pool
  (`/srv/econgraph-backups/snapshots`, root-only, mode 0700) and the
  off-site copy are the ones that account can't reach.

## Needs sudo

Each step is one checked-in script; run with `--dry-run` first. In order:

1. **Create and mount the subvolumes** (from the `ops/host-storage-scripts`
   branch; already done on 2026-10-24): `sudo scripts/host/setup-subvolumes.sh`. Creates
   `@econgraph-data-plane`, `@econgraph-app-prod` and
   `@econgraph-snapshots`, adds their `/etc/fstab` lines, mounts them and
   chowns each subvolume root to the operator (required for `snapshot-now.sh`).
2. **Install the hourly snapshot and copy job**:
   `sudo scripts/host/install-backup-timer.sh --dry-run`, then
   `sudo scripts/host/install-backup-timer.sh --run-now`. Installs a
   root-owned copy of `scripts/host/backup-snapshots.sh` as
   `/usr/local/sbin/econgraph-backup-snapshots`, writes
   `/etc/systemd/system/econgraph-backup-snapshots.service` and `.timer`
   (`ON_CALENDAR="hourly"` at the top of the installer), enables the timer,
   and with `--run-now` takes the first snapshots immediately. No packages
   are installed. Running the installer is the maintainer's approval of the timer.
   Re-run it after merging changes to `backup-snapshots.sh`. Needs step 1.
3. **Set up the backup subvolume on the 870 pool**:
   `sudo scripts/host/setup-backup-pool.sh --dry-run`, then without
   `--dry-run`. Creates the new top-level subvolume `@econgraph-backups`
   (never touching `@home` or its snapshots; it lists them so you can see
   they are left alone), adds its `/etc/fstab` line
   (`UUID=a1e61098-... /srv/econgraph-backups btrfs subvol=/@econgraph-backups,compress=zstd:3,ssd,discard=async,space_cache=v2,noatime,nofail,x-systemd.device-timeout=30s 0 0`;
   `nofail` so a missing disk never blocks boot), mounts it, keeps the
   subvolume root root-owned (0755), and creates `snapshots/` (root, 0700)
   and `dumps/` (the operator, 0700), resetting their owner and mode on a
   rerun. It refuses an existing `/etc/fstab` entry or mount at
   `/srv/econgraph-backups` that is not this pool's `@econgraph-backups`. `scripts/host/inspect-sata-pool.sh` (same branch as
   step 1) shows the pool read-only beforehand. From the next hourly run on,
   every snapshot is copied there, and the nightly dumps go to
   `/srv/econgraph-backups/dumps`, with nothing else to change.
4. **By hand, when needed**: `sudo scripts/host/restore-snapshot.sh` (restore
   from a snapshot, local or `--from-pool`) and
   `sudo scripts/host/delete-snapshots.sh` (the only thing that deletes;
   see [Deleting by hand](#deleting-by-hand)).

Not needed:

- `loginctl enable-linger "$USER"`: already `Linger=yes`.
- `user_subvol_rm_allowed`: see above; leave it off.
- Anything for the dumps, drill, freshness check, off-site copy or branch
  clones: they run as the operator.
- Anything for pre-change snapshots: `snapshot-now.sh` runs as the operator.
- Any package: everything uses `btrfs-progs`, Docker and restic, which are
  already installed.
