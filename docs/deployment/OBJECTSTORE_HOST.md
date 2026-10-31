# Reviewable host object-store setup

`scripts/objectstore/host_store.py` provides actual systemd/Compose templates and an idempotent
installer for the accepted Versity host store. The persistent form is a required explicit
`--form systemd|docker` choice. No form is selected automatically, and no persistent service has
been installed by the implementation task. The current missing CNPG/PITR acceptance gate blocks
installation of either form.

## Render both candidates without host changes

Supply the host's actual KVM bridge IP/interface and durable btrfs filesystem UUID. These example
values are review inputs, not authority to create a bridge or modify storage:

```sh
python3 scripts/objectstore/host_store.py \
  --form systemd --bind-address 192.168.250.1 --bridge econgraph0 \
  --filesystem-uuid 03ee0fcb-5f19-4f44-8596-e227f1fb647c \
  --render /tmp/econgraph-host-store-systemd
python3 scripts/objectstore/host_store.py \
  --form docker --bind-address 192.168.250.1 --bridge econgraph0 \
  --filesystem-uuid 03ee0fcb-5f19-4f44-8596-e227f1fb647c \
  --render /tmp/econgraph-host-store-docker
```

The default only prints a plan. An explicit `--render` writes candidate files exclusively under
`/tmp` or this source worktree, without reading credentials or invoking host tools. Identical
renders are unchanged; different existing files, symlinks and foreign output directories are
refused. Candidate output is never installed into `/etc` by a render operation.

The systemd candidate runs the checksum-pinned official Linux x86-64 Versity v1.10.0 binary as a
dedicated non-login account. The Docker candidate uses the same digest-pinned release image as
the compatibility runner, under the rootful daemon. Its Compose service runs with a dedicated
non-root UID/GID, drops capabilities, uses a read-only root and bounded temporary space, and
publishes only the explicit private bridge address. Neither candidate binds all interfaces.
Both use systemd mount/network dependencies; the Docker unit additionally requires Docker and
uses `--no-recreate --pull never` so startup cannot silently replace the existing container.
Docker's own restart policy is `no`: a foreground Compose process is supervised by systemd,
with daemon restart propagation and `Restart=on-failure`. Every start/restart therefore checks
the exact object-store UUID/btrfs/subvolume, matching mounted snapshot filesystem and coverage
marker before starting a container. Dockerd cannot auto-start it ahead of those mount checks.

Both Versity forms use POSIX `--fsync`, which flushes object data, metadata and publication
directories before acknowledging writes. This adds storage flush latency; capacity and recovery
acceptance must measure that configuration. The installer requires smoke evidence with
`posix_fsync: true` and refuses to adopt an existing container without the flag. A process-kill
test alone does not prove power-loss durability, and it does not replace the separate recovery
acceptance gates.

The default dedicated UID/GID is 61100. The installer refuses an existing foreign account or an
occupied numeric ID. Choose an explicit unoccupied `--uid`/`--gid` if necessary. The native unit
limits memory/CPU/tasks; the container has equivalent limits and bounded logs. Stop preserves
all object data; there is no container `down --volumes`, lifecycle policy or expiry operation.

## Installation gates and artifacts

Installation and startup are separate explicit operations, each requiring `--execute`. The
installer is checked in for review and later environment-authorized operation; rendering a
candidate does not authorize either operation.

Before any privileged operation, the installer requires:

- An actual full `smoke.py` report whose seven gates passed for the pinned Versity image,
  including matching crawler/client/restore checksums, multi-GB multipart evidence,
  10,000+ listing evidence, and actual CNPG base-backup/WAL/PITR evidence. Summarized partial
  proof artifacts do not qualify. A Garage-only success never changes production configuration.
- For native systemd execution, the report must also identify the exact accepted native binary
  digest. Image-only acceptance cannot silently authorize an untested native artifact.
- The selected bind IP present on the named Linux bridge, the explicit filesystem UUID matching
  the already-mounted btrfs snapshot staging filesystem, and root-owned non-writable configuration paths. Existing unrelated units, users, containers,
  mounts, subvolumes, credential files or changed managed files are refused.
- The updated existing snapshot script installed by the reviewed
  `scripts/host/install-backup-timer.sh` workflow, and its existing hourly timer enabled.
  The object-store installer checks the installed script against the checked-in source;
  it does not overwrite an unknown host backup program.

The official native release archive is
[Versity v1.10.0 Linux x86-64](https://github.com/versity/versitygw/releases/tag/v1.10.0).
The staged archive SHA256 is
`8f9869752939d4dc9cfe1bddb935a02cc6610e95319a3a9fc2947d2a6ebd2c91`;
the extracted executable SHA256 is
`11a815fbd71386318f556a0f5d735d21f12a4721d3b21a645ffc0c16278efc7d`.
The archive is bounded, checked before extraction, and only its exact executable member is
accepted. The installer does not fetch arbitrary executable content as root. Docker execution
requires the already-cached exact image digest from the smoke runner.

## Credentials and idempotence

Prepare a private mode 0600 regular file containing exactly two unquoted assignments,
`ROOT_ACCESS_KEY` and `ROOT_SECRET_KEY`, through the operator's existing secret workflow. Supply
its path with `--credential-source` for initial installation. After all acceptance checks pass,
the installer creates the private managed directory and copies those bytes to the default
`/etc/econgraph/objectstore/credentials.env` as root:root 0600. The source must belong to root or
the invoking operator; source contents are never printed. Existing credentials are not rotated
or overwritten by rerunning installation.

The native service loads the root-only `EnvironmentFile`; Compose uses its private `env_file`.
Credentials are never embedded in units, Compose commands, plans or process arguments. These
are host bootstrap credentials, not application/crawler credentials. Environment/bucket access
policies and separate least-privilege data/raw/backup credentials remain required before clients
are configured.

The installer records a private immutable deployment identity, takes a scoped lock, creates the
new `@econgraph-objectstore` btrfs subvolume at the selected filesystem's top level, appends only
its own unique fstab entry, mounts it at `/srv/econgraph/objectstore`, and assigns only its empty
root to the dedicated account. It never recursively changes ownership of existing objects.
Before enabling coverage it verifies a real `user.*` xattr roundtrip; Versity retains its default
POSIX xattr metadata, without `--nometa`, sidecar storage or nodatacow. The object subvolume keeps
normal copy-on-write snapshot behavior.

Existing container adoption checks every mount, the pinned entrypoint and command, private
credential bindings, security/resource bounds, network scope and disabled Docker restart policy.
Unexpected writable mounts, host namespaces, entrypoints or unlimited resources are refused even
when an owner label matches. Identical existing managed files are verified without replacement. A partially completed
installation can rerun with the same identity; changed bind/form/artifact settings are refused
as a production configuration change requiring separate review. Installation ends with
`installed_not_started`. A later explicitly authorized `--action start --execute` repeats the
acceptance/environment checks before enabling and starting the unit.

## Existing backup pipeline coverage

`scripts/host/backup-snapshots.sh` continues covering its original two sources on existing hosts.
Only a successfully installed store adds the public, root-managed
`/etc/econgraph/objectstore-snapshots.enabled` marker. Then that same hourly no-deletion pipeline
snapshots the independent objectstore subvolume read-only and sends/receives it onto the existing
backup pool, preserving object files and xattrs. Invalid or symlinked markers are rejected.
There is no competing backup schedule or automatic deletion of snapshots/backups.

The installer leaves the shared backup unit and timer unchanged. Only the object-store service
requires the new mount at startup, so an unavailable object-store mount cannot prevent the
data-plane and app-prod backups from running. The backup script records that source as a failure,
continues the other snapshots and pool copies, and publishes fresh status. It never snapshots
the empty underlying object-store mount point as successful coverage. Restore/cutover still
requires the separate SQL catalog/object consistency and CNPG recovery gates.

## Local verification

```sh
python3 -m unittest discover -s scripts/objectstore -p 'test_*.py' -v
bash scripts/host/test-objectstore-snapshot-coverage.sh
bash scripts/host/test-backup-scripts.sh
shellcheck -x scripts/host/backup-snapshots.sh scripts/host/test-objectstore-snapshot-coverage.sh
```

These verify dry-run behavior, mandatory choices/evidence, artifact integrity, immutable renders,
credential/symlink guards and optional snapshot coverage. They do not claim that a persistent
service was installed or that a full host installation/PITR rehearsal has passed.
