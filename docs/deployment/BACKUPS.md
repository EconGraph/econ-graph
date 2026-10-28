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
| `/srv/econgraph/data-plane`  | `@econgraph-data-plane` | `pg/`: main Postgres (`econ_graph`, pod `postgresql-0`); `objects/`: no active data producer |
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
copy of `/home` (about 1.57 TiB) that the maintainer decides about separately.
The recorded setup observation was about 2.59 TiB free; use
`df -h /srv/econgraph-backups` after mounting to check current capacity.
The backup target is a new top-level subvolume
`@econgraph-backups` beside it. Nothing here ever wipes, reformats or
touches `@home` or its snapshots. Because the pool is a different filesystem
from `/home`, snapshots reach it only by `btrfs send`/`receive`
(`scripts/host/backup-snapshots.sh`, as root), never by copying files.

### Which Docker daemon

The kind cluster runs on the **rootful** Docker daemon at
`unix:///var/run/docker.sock`: kind can't start under rootless Docker on this
host, whose rootless data root is on btrfs (see
[Host Storage](HOST_STORAGE.md)). The operator's login shell exports `DOCKER_HOST`
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
manifests run as (`runAsUser: 999`). This rootful Docker setup uses no user namespace,
so UID 999 in the pod is UID 999 on the host. The operator cannot read them
directly. Any copy has to preserve that ownership and mode, or Postgres
refuses to start on it. btrfs snapshots and `btrfs send`/`receive` preserve
it. A file-level copy has to be made as root with ownership kept
(`cp -a`, `rsync -aHAX --numeric-ids`), or from inside a container on the
rootful daemon (root there is host root).

Data written while the cluster still ran under rootless Docker is owned by
host UID 100998 (999 mapped through `/etc/subuid`). The Postgres init
containers on the host-storage layout (PR #421) re-own a data directory to
999:999 when they find anything else. They convert a pre-switch snapshot’s
ownership during startup; no separate ownership conversion is required.

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
| nightly 10:00 UTC, with up to 5 minutes randomized delay | `scripts/backup/dump-databases.sh`: verified `pg_dump -Fc` of `econ_graph`, `econ_graph_app` and `keycloak` | operator | `econgraph-dump.timer` (user) |
| daily 10:45 UTC | `scripts/backup/offsite-restic.sh backup`: restic copy of the verified dumps (the unit does nothing until `restic.env` exists) | operator | `econgraph-offsite.timer` (user) |
| at least weekly, interactively | `open-backup.sh` followed by `restore-drill.sh`: decrypt and restore a bundle into a throwaway Postgres 18 | operator | manual; no private identity is installed for a timer |
| before a risky change | `scripts/backup/snapshot-now.sh <reason>` | operator | by hand |
| before a deploy that changes the Keycloak image | `scripts/deploy/backup-keycloak-db.sh`: [verified upgrade dump](#keycloak-upgrade-dumps) | operator | by hand |
| when needed | `scripts/backup/restore-dump.sh`, `scripts/backup/branch-clone.sh` | operator | by hand |
| when needed | `scripts/host/restore-snapshot.sh`, `scripts/host/delete-snapshots.sh` | root | by hand only |

`backup-snapshots.sh`, `restore-snapshot.sh`, `delete-snapshots.sh` and
`snapshot-now.sh` (when run as root) take one lock,
`/run/econgraph/backup-snapshots.lock`, before they change anything, and
hold it until they exit, so none of them overlaps another. That includes
the time `restore-snapshot.sh` and `delete-snapshots.sh` wait at their typed
confirmation; an hourly run that starts meanwhile fails and that hour is
skipped. One that finds the lock held stops without changing anything
(`restore-snapshot.sh` can wait with `--lock-wait`); dry runs and
`delete-snapshots.sh --list` take no lock.

`/run/econgraph` is root's, mode 0700, so no other user can open the lock
file to hold it; a directory or lock file that isn't as the scripts make
them is refused, never used. The hourly job is a copy installed by
`install-backup-timer.sh`, so the other three refuse to run while that copy
takes an older lock: re-run `sudo scripts/host/install-backup-timer.sh`
after pulling. It stops the timer and refuses, starting it again, while a
backup run is in progress; re-run it when the run has finished. Copies older
than this lock take `/run/econgraph-backup-snapshots.lock` instead; while a
root process holds that file (a restore from an older checkout, say) the
others wait for it too. Any user can open that old file, so a lock on it
held by anyone but root is ignored.

Everything that needs root is a script under `scripts/host/`; each refuses
to run without sudo, takes `--dry-run`, prints every action, and is safe to
re-run. They are listed in order in [Needs sudo](#needs-sudo).
The root timer runs a root-owned copy at
`/usr/local/sbin/econgraph-backup-snapshots`, so changes in the checkout do
not change the scheduled code.

The user scripts read optional overrides from
`~/.config/econgraph/backup.env` (plain `KEY=value`): `BACKUP_ROOT`, the
freshness and space limits, `FRESHNESS_SKIP_SNAPSHOTS=1` (when the hourly
timer is absent), and so on; see the [shared defaults](../../scripts/backup/lib.sh)
and [freshness checker header](../../scripts/backup/check-freshness.sh) for the full list.

Snapshots on the NVMe array share its disks: they protect against bad
migrations, mistaken recrawls and deleted rows, not against losing the
array. Until `setup-backup-pool.sh` has run, the dumps also sit on the same
array (in `/home`). Successful copies on the 870 pool (snapshots plus dumps)
cover losing the NVMe array. The pool is in the same machine; protection
against losing the machine requires an off-site target and successful copies
to it.

### Keycloak upgrade dumps

Before a deploy that changes the Keycloak image, run
[`backup-keycloak-db.sh`](../../scripts/deploy/backup-keycloak-db.sh). It
scales Keycloak to zero and waits for its Pods to disappear, dumps `keycloak`
to `~/econgraph-backups/keycloak/<UTC stamp>/` by default, and verifies a
scratch-database restore with equal row counts before marking it VERIFIED.
Directories are 0700 and files 0600. Keycloak stays stopped for the deploy.

For rollback, [`restore-keycloak-db.sh`](../../scripts/deploy/restore-keycloak-db.sh)
takes a safety dump, restores into a fresh database, verifies counts, and
swaps it in by rename, retaining the previous database. A fresh database is
required for a downgrade: `restore-dump.sh` uses `--clean`, which leaves
objects absent from its archive, including tables added by newer Keycloak.
Keycloak remains stopped; deploy a compatible image before bringing it back.
See the rollback procedure in the [v4.0.0 QA plan](../release/v4.0.0-qa.md#keycloak-rollback).

These upgrade dumps accumulate on the data disk by default, rather than on
the backup pool. `offsite-restic.sh` does not upload them. Copy each useful
upgrade dump off that disk by hand; the directory contains password hashes
and client secrets and must remain private.

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
timers need lingering to run while the operator is logged out. Check
`loginctl show-user "$USER" -p Linger` before installation; it must report
`Linger=yes`. If it does not, enable lingering with
`sudo loginctl enable-linger "$USER"`.

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

A verified bundle may contain fewer than three archives when optional databases
are not deployed; its marker establishes archive checks for the attempted dumps,
not a successful restore.

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
snapshots, the independent in-cluster backup CronJob/PVC, and the special
Keycloak-upgrade backup format remain separate copies with their existing
protection. In particular, these changes do not claim those copies are
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
(and `econ_graph_app` and `keycloak` from the same directory only if present), and checks: `economic_series`,
`data_points` and `data_sources` are non-empty, no data point references a
missing series, no data point lacks a date, the Diesel migration table is
populated (in a restored app database too, once it has tables), a restored
Keycloak database has realms, and any `--expect-*` counts match exactly.
A pass covers the databases actually restored; missing companion archives
do not cause failure.
Without `--expect-*` it compares against the counts in the dump's
`MANIFEST` and warns on a difference. It prints one `RESTORE DRILL PASS` or
`RESTORE DRILL FAIL` line, removes the container, and on a pass records
`~/.local/state/econgraph/restore-drill.last` for the freshness check. This
happens even for an explicitly selected old or unverified dump: drill
freshness measures when a drill passed, not whether today’s backups restore.

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
writes `~/.local/state/econgraph/backup-freshness.prom`). The checked-in
[monitoring setup](../../k8s/monitoring) has Prometheus but
no node_exporter or Pushgateway to collect it, and the in-cluster Prometheus
cannot see host files. These metrics do not produce alerts in that setup.

## Off-site

The checked-in setup supplies no off-site target. `offsite-restic.sh` reads
the repository and password from `~/.config/econgraph/restic.env` (outside the repo, mode
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

Only verified dump directories are uploaded, and nothing is ever forgotten
or pruned from the repository: `offsite-restic.sh` has no `forget` or
`prune`, and `offsite-restic.sh restic forget|prune ...` is refused.
These wrapper rules prevent scheduled pruning, not deletion by a compromised
operator account. The account holds the repository credentials and password
and can run restic directly. Remote deletion resistance requires controls
enforced by the backend independently of those credentials.
Snapshots are not sent off-site: logical dumps can be restored on another host
with compatible PostgreSQL tools and the required roles.

## Recovery shell and shutdown checks

Use Bash from the checkout for the restore procedures below. Obtain the
maintainer’s sign-off and take the [pre-change snapshot and dump](#pre-change-snapshot)
before changing production. Arrange a maintenance window: stop other
clients, migration/import Jobs and deploy automation that could write to
the affected databases or recreate the workloads. The selectors below cover
the checked-in Deployments and StatefulSets, not arbitrary clients.

Define these helpers once in the recovery shell:

```bash
k() { docker -H unix:///var/run/docker.sock exec econ-graph-control-plane kubectl -n econ-graph "$@"; }
declare -A recovery_replicas
record_replicas() {
    local resource count
    for resource in "$@"; do
        count="$(k get "$resource" -o jsonpath='{.spec.replicas}')" || return 1
        [[ "$count" =~ ^[0-9]+$ ]] || return 1
        recovery_replicas["$resource"]="$count"
        printf '%s %s\n' "$resource" "$count"
    done
}
wait_gone() {
    local pods
    while :; do
        pods="$(k get pods -l "$1" -o name)" || return 1
        [[ -z "$pods" ]] && return 0
        sleep 2
    done
}
resume_recorded() {
    local resource
    for resource in "$@"; do
        [[ -n "${recovery_replicas[$resource]+recorded}" ]] || return 1
        k scale "$resource" --replicas="${recovery_replicas[$resource]}" || return 1
    done
}
```

Copy the printed counts into the maintenance record before scaling. Keep
this shell open; the associative array is lost if it exits. If a command
fails, stop and diagnose it before proceeding. Do not rerun the recording
step after scaling to zero: that would overwrite the original counts.
The `|| exit 1` checks end the recovery shell on failure; retain the
maintenance record so that its counts survive that exit.
Keep writers down until recovery and schema checks succeed. After a failed
step, recover the saved counts from the maintenance record if necessary;
do not blindly run the resume commands.

## Restore from a snapshot

Use this for a whole-plane rollback, for example after a bad migration.
Everything written to that plane since the snapshot is lost; the current
data is set aside, not deleted. A `data-plane.*` snapshot replaces `pg/`
and `objects/`; an `app-prod.*` snapshot replaces both `app-pg/` and
`keycloak-pg/`. With `APP_DATABASE=separate`, an app-prod rollback replaces
app state and Keycloak accounts and sessions together. In shared mode, app
state instead follows `econ_graph` in the data-plane snapshot. Choose
compatible service images, including a Keycloak version that can read the restored schema.

1. Pick the snapshot: `ls /srv/econgraph/snapshots/`. If the local one is
   gone (or the NVMe array was lost and rebuilt), use the copy on the backup
   pool: `sudo scripts/host/delete-snapshots.sh --list --pool` lists them,
   and step 3 takes `--from-pool`, which first receives that snapshot back
   into `/srv/econgraph/snapshots`. It refuses a pool copy without a
   Received UUID, and a local snapshot of the same name that the pool copy
   was not received from.

2. Set up the [recovery shell](#recovery-shell-and-shutdown-checks). The
   replacement affects only the chosen plane, but the required storage-node
   restart interrupts **all three databases**. Record all their counts and
   those of the backend, crawler and Keycloak, then stop writers before
   stopping the databases:

   ```bash
   writers=(deployment/econ-graph-backend deployment/crawler-worker deployment/keycloak)
   databases=(statefulset/postgresql statefulset/app-postgres statefulset/keycloak-postgres)
   record_replicas "${writers[@]}" "${databases[@]}" || exit 1
   k scale "${writers[@]}" --replicas=0 || exit 1
   wait_gone 'app in (econ-graph-backend,crawler-worker,keycloak)' || exit 1
   k scale "${databases[@]}" --replicas=0 || exit 1
   wait_gone 'app in (postgresql,app-postgres,keycloak-postgres)' || exit 1
   ```

   These commands assume all three databases are deployed, as in the host
   layout above. If a resource is absent, establish the actual layout before
   adapting the lists; a failed lookup is not confirmation of shutdown.
   Proceed only after both pod checks return successfully with no matching
   Pods. Ensure other clients and Jobs are stopped too.

3. Run `sudo scripts/host/restore-snapshot.sh --dry-run [--from-pool] <snapshot>`,
   then without `--dry-run`, checking that each succeeds before continuing.
   It refuses, changing nothing, while any pod
   still uses the subvolume:
   - the plane's Postgres pods (`postgresql-0` for `data-plane`;
     `app-postgres-0` and `keycloak-postgres-0` for `app-prod`), whether the
     API server still lists them or a container of theirs still runs on
     `econ-graph-worker` (as after a force delete);
   - any other pod, in any namespace, that the API shows mounting it: a
     hostPath volume at, under or above `/srv/econgraph/<name>` (compared
     as text after resolving `//`, `.` and `..`; links are not followed, so
     any hostPath under `/proc` or not absolute counts too), or a claim
     (generic ephemeral volumes included) bound to a PersistentVolume there.

   The refusal names each pod. It also refuses when the cluster can't be
   asked, and it checks again after the confirmation. Containers started
   outside Kubernetes are not seen, and the check does not stop the writers
   for you: step 2 still comes first. Before its first check it takes the
   lock the hourly `backup-snapshots.sh` holds
   (`/run/econgraph/backup-snapshots.lock`) and keeps it until it exits, so
   no backup run or second restore overlaps it. If the lock is held, it stops
   without changing anything; `--lock-wait=SECONDS` (up to 86400) waits up
   to that long first. A dry run takes no lock. The lock is held while it
   waits at the typed confirmation too: an hourly backup run that starts
   meanwhile fails at once (`systemctl status econgraph-backup-snapshots`
   shows it failed) and that hour's snapshot is skipped. It then asks you to type the subvolume name (`@econgraph-data-plane` or
   `@econgraph-app-prod`), unmounts the subvolume, renames it to
   `@econgraph-<name>.replaced-<stamp>`, puts a writable snapshot of the
   chosen snapshot in its place under the original name, and remounts it.

4. With all databases still stopped, restart the kind node that holds the
   storage, so it sees the new subvolume:

   ```bash
   docker -H unix:///var/run/docker.sock restart econ-graph-worker || exit 1
   k wait --for=condition=Ready node/econ-graph-worker --timeout=180s || exit 1
   ```

   All three databases run on `econ-graph-worker` (labelled
   `econgraph.io/host-storage=true`), which bind-mounts
   `/srv/econgraph/data-plane` and `/srv/econgraph/app-prod`. That bind mount
   keeps pointing at the old subvolume until the node container restarts;
   skipping this starts Postgres on the old data.

5. Bring the database StatefulSets back to their recorded counts:

   ```bash
   resume_recorded "${databases[@]}" || exit 1
   for resource in "${databases[@]}"; do
       if (( recovery_replicas[$resource] > 0 )); then
           k wait --for=condition=Ready "pod/${resource#statefulset/}-0" --timeout=180s || exit 1
       fi
   done
   ```

   Check the data in each resumed database. For data-plane recovery, check
   `econ_graph` and its data plane migration history; for app-prod, check both `econ_graph_app` and
   `keycloak`, including app migration history and Keycloak schema/image
   compatibility. Reconcile the restored schemas with the deployed services
   using the [migration guidance](../development/MIGRATIONS.md) before
   resuming any writer. Starting the backend applies newer app plane
   migrations, including to `econ_graph` in shared mode; data plane
   migrations require an explicit command.

   After those checks succeed, restore Keycloak first and wait for its
   rollout if its original count was nonzero, then resume the backend and
   crawler:

   ```bash
   resume_recorded deployment/keycloak || exit 1
   if (( recovery_replicas[deployment/keycloak] > 0 )); then
       k rollout status deployment/keycloak --timeout=180s || exit 1
   fi
   resume_recorded deployment/econ-graph-backend deployment/crawler-worker || exit 1
   ```

6. The set-aside subvolume stays until the operator deletes it by hand
   (`sudo scripts/host/delete-snapshots.sh @econgraph-data-plane.replaced-<stamp>`
   or the corresponding app-prod name). To undo the restore instead, it is
   the old data.

## Restore from a dump

Use a whole-database rollback when losing all writes since the dump is
acceptable. For selective row recovery, restore into an isolated container
and copy the required rows instead. For a Keycloak downgrade, use the
[fresh-database upgrade recovery](#keycloak-upgrade-dumps), because a clean
restore can leave newer tables behind.

Set up the [recovery shell](#recovery-shell-and-shutdown-checks), including
its pre-change backup and maintenance prerequisites. Choose **one** target
and its affected writers:

For `econ_graph` (also includes app state in `APP_DATABASE=shared`):

```bash
database=econ_graph
writers=(deployment/econ-graph-backend deployment/crawler-worker)
writer_selector='app in (econ-graph-backend,crawler-worker)'
```

For the separate app database:

```bash
database=econ_graph_app
writers=(deployment/econ-graph-backend)
writer_selector='app=econ-graph-backend'
```

For Keycloak with a compatible schema and image:

```bash
database=keycloak
writers=(deployment/keycloak)
writer_selector='app=keycloak'
```

Set `dump` to the matching host-side archive, then record counts, stop the
selected writers and confirm their Pods are gone. Keep the target database
running; `restore-dump.sh` does not stop writers for you.

```bash
dump="$HOME/econgraph-backups/<stamp>/${database}.dump"   # replace <stamp> and root path
record_replicas "${writers[@]}" || exit 1
k scale "${writers[@]}" --replicas=0 || exit 1
wait_gone "$writer_selector" || exit 1
scripts/backup/restore-dump.sh --dry-run "$database" "$dump" || exit 1
scripts/backup/restore-dump.sh "$database" "$dump" || exit 1
```

`--dry-run` checks the archive and the pod and prints the plan. Without
it, the helper waits up to 2 minutes for the target database Pod to be Ready
(and stops if it is not), asks for the database name as confirmation
(`--yes` skips that), then runs
`pg_restore --clean --if-exists --single-transaction --exit-on-error`
in the pod, so a restore failure rolls back. It lists relations missing
from the dump as a warning, not a refusal. As with the in-cluster restore,
`--clean` only drops objects that are in the dump: tables added by later
migrations stay and Diesel's migration history may disagree. Check
`__diesel_schema_migrations`, ownership and the schema against the deployed
services, and resolve mismatches **before** resuming writers. The backend
applies newer app plane migrations at startup; data plane migrations are
explicit. To restore into a fresh cluster whose database does not exist yet,
create the empty database and required owner roles first (the host dump
keeps object owners; the app/data owner is `econgraph`).

Only after a successful restore and data/schema checks, run:

```bash
resume_recorded "${writers[@]}" || exit 1
```

To inspect a dump without changing production, run the restore drill on it.
The container is removed, but a pass updates the drill freshness record even
for an old dump. To keep a restored copy around for querying, run `docker run`
of `postgres:18` yourself and `pg_restore --no-owner --no-privileges` into it.

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
([manual retention instructions](POSTGRES_BACKUP_RESTORE.md#retention) show how). It is kept as an
independent second copy: the two share no files, schedule or lock, run
through different paths (the CronJob over the Services from inside the
cluster, `dump-databases.sh` with `kubectl exec` through the kind node), and
neither touches the other's dumps. The host-side dumps are the primary copy
on threadripper. The CronJob remains an independent copy, with these limits:

- Its PVC uses dynamically provisioned kind local-path storage inside a kind
  node container. Removing that node or cluster loses this storage. The three
  production database PVs instead use host directories and `Retain`; kind
  deletion does not delete those directories.
- `scripts/deploy/protect-postgres-pv.sh`, invoked by deploy and teardown,
  sets a **bound** backup PV to `Retain`. If `postgres-backup-data` is still
  Pending, that protection is skipped. After the first Job binds it, rerun
  the protection helper and verify the bound PV's reclaim policy before
  relying on preservation after PVC deletion. Source configuration alone
  does not establish the policy of the current live PV.
- `Retain` preserves the PV and its files after PVC deletion, leaving it
  Released; it does not preserve a kind container that is deleted or make
  the volume automatically reusable. See [Reattaching a retained volume](POSTGRES_BACKUP_RESTORE.md#reattaching-a-retained-volume).
- The CronJob has no restore drill or freshness check, and the checked-in
  monitoring setup has no alert for its Job.

Export useful dumps before removing cluster storage or retiring the CronJob.
Do not rely on deleting a PVC to preserve or remove its files without checking
its bound PV's policy and storage location.

## Unprivileged btrfs: what works without root

The recorded permission experiments on threadripper used kernel 7.0 and
btrfs-progs 6.17.1. They ran as the operator, first in a scratch directory on
`/home`, then on the real layout
(`snapshot-now.sh` from `/srv/econgraph/data-plane` and `app-prod`, both
owned by the operator and holding only a test directory owned by UID 100998,
the rootless-Docker mapping of UID 999 in use at the time,
into `/srv/econgraph/snapshots`; `/home` was not mounted with
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
  in this account. Root-only mode 0700 on the 870 pool’s snapshot directory
  blocks ordinary direct access, but the operator has [root-level access](https://docs.docker.com/engine/install/linux-postinstall/#manage-docker-as-a-non-root-user)
  through the rootful Docker daemon. It does not isolate those copies from
  a compromised operator account. The account also holds restic credentials;
  wrapper restrictions do not establish an off-site deletion boundary.

## Needs sudo

Installation requires the following tools and privileges. Verify them before
running the installation steps:

```sh
for tool in btrfs docker systemctl loginctl findmnt flock sha256sum python3; do
    command -v "$tool" || exit 1
done
command -v restic                  # required for off-site copies
id -nG                            # docker group grants rootful socket access
ls -l /var/run/docker.sock         # confirm socket permissions
loginctl show-user "$USER" -p Linger
```

The installers do not install packages. Supply missing tools before using
the corresponding scripts. User timers need `Linger=yes` to run after
logout; enabling it requires sudo as described in [Timers and installation](#timers-and-installation).
Group membership and socket permissions must permit the operator to use the
rootful daemon. This privilege gives root-level host access, even though
dumps, drills, freshness checks, off-site copies, branch clones and
pre-change snapshots are invoked without sudo.

Each step is one checked-in script; run with `--dry-run` first. In order:

1. **Create and mount the subvolumes**: `sudo scripts/host/setup-subvolumes.sh`. Creates
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
   `/srv/econgraph-backups` that is not this pool's `@econgraph-backups`. [`inspect-sata-pool.sh`](../../scripts/host/inspect-sata-pool.sh)
   shows the pool read-only beforehand. From the next hourly run on,
   every snapshot is copied there, and the nightly dumps go to
   `/srv/econgraph-backups/dumps`, with nothing else to change.
4. **By hand, when needed**: `sudo scripts/host/restore-snapshot.sh` (restore
   from a snapshot, local or `--from-pool`) and
   `sudo scripts/host/delete-snapshots.sh` (the only thing that deletes;
   see [Deleting by hand](#deleting-by-hand)).

Leave `user_subvol_rm_allowed` off, for the reasons above.
