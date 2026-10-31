#!/usr/bin/env python3
"""Render or explicitly install an accepted host store. Default: no host changes."""
import argparse
import fcntl
import grp
import hashlib
import ipaddress
import io
import json
import os
from pathlib import Path
import pwd
import re
import stat
import sys
import subprocess
import tarfile
import tempfile

IMAGE = "versity/versitygw@sha256:89cad13cc981986ee284cbafea4613f9533ce6fd5f37a2c96c4c4fb8adb3a316"
ARCHIVE_SHA = "8f9869752939d4dc9cfe1bddb935a02cc6610e95319a3a9fc2947d2a6ebd2c91"
BINARY_SHA = "11a815fbd71386318f556a0f5d735d21f12a4721d3b21a645ffc0c16278efc7d"
ARCHIVE_URL = "https://github.com/versity/versitygw/releases/download/v1.10.0/versitygw_v1.10.0_Linux_x86_64.tar.gz"
DATA = Path("/srv/econgraph/objectstore")
CONFIG = Path("/etc/econgraph/objectstore")
MARKER = CONFIG / "managed.json"
COVERAGE = Path("/etc/econgraph/objectstore-snapshots.enabled")
UNIT = Path("/etc/systemd/system/econgraph-objectstore.service")
BINARY = Path("/usr/local/libexec/econgraph-versitygw")
USER = "econgraph-objectstore"
GATES = {"crawler_sql_catalog", "independent_readers", "multipart_hash", "interrupted_put",
         "cnpg_pitr", "paginated_listing", "btrfs_restore"}


def run(*argv):
    return subprocess.run(argv, text=True, check=True, capture_output=True, timeout=120).stdout.strip()


def no_symlinks(path):
    path = Path(path)
    if not path.is_absolute() or ".." in path.parts:
        raise ValueError("An absolute normalized path is required")
    for part in (path, *path.parents):
        if part.is_symlink():
            raise ValueError(f"Refusing symlink: {part}")


def root_private(path, mode):
    no_symlinks(path)
    for parent in path.parents:
        value = parent.stat()
        if value.st_uid != 0 or value.st_mode & 0o022:
            raise ValueError(f"File has an untrusted writable parent: {path}")
    value = path.stat()
    if not stat.S_ISREG(value.st_mode) or value.st_uid != 0 or value.st_gid != 0 or stat.S_IMODE(value.st_mode) != mode:
        raise ValueError(f"Expected root:root regular file mode {mode:o}: {path}")


def public_file(path, content, mode=0o644):
    """Create once, or verify identical existing bytes; never replace a foreign file."""
    no_symlinks(path)
    if path.exists():
        root_private(path, mode)
        if path.read_bytes() != content:
            raise ValueError(f"Existing file differs; refusing overwrite: {path}")
        return
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, mode)
    with os.fdopen(descriptor, "wb") as stream:
        stream.write(content)
        stream.flush()
        os.fsync(stream.fileno())
    os.chmod(path, mode)


def validate_args(args):
    address = ipaddress.IPv4Address(args.bind_address)
    if not address.is_private or address.is_loopback or address.is_unspecified or address.is_link_local or address.is_multicast:
        raise ValueError("Bind address must be an explicit private KVM bridge IPv4 address")
    if not re.fullmatch(r"[A-Za-z0-9_-]{1,15}", args.bridge):
        raise ValueError("Invalid KVM bridge interface name")
    if not re.fullmatch(r"[0-9a-fA-F]{8}(-[0-9a-fA-F]{4}){3}-[0-9a-fA-F]{12}", args.filesystem_uuid):
        raise ValueError("Explicit btrfs filesystem UUID is required")
    args.filesystem_uuid = args.filesystem_uuid.lower()
    if not 1000 <= args.uid <= 65534 or not 1000 <= args.gid <= 65534:
        raise ValueError("Dedicated non-root service UID/GID must be between 1000 and 65534")
    if not re.fullmatch(r"/[A-Za-z0-9_./-]+", str(args.credential_file)):
        raise ValueError("Credential path must be absolute and safe for an EnvironmentFile reference")
    no_symlinks(args.credential_file)


def acceptance(path, form):
    if path is None:
        raise ValueError("Execution requires the actual seven-gate smoke report")
    no_symlinks(path)
    report = json.loads(path.read_text())
    if report.get("format_version") != 1 or report.get("status") != "passed":
        raise ValueError("Host installation is blocked: all seven acceptance gates must pass")
    candidates = [run for run in report.get("runs", []) if run.get("store") == "versity"
                  and run.get("image") == IMAGE and run.get("status") == "passed"]
    for candidate in candidates:
        gates = candidate.get("gates", {})
        if set(gates) != GATES or any(value.get("status") != "passed" or not value.get("evidence") for value in gates.values()):
            continue
        crawler = gates["crawler_sql_catalog"]["evidence"]
        readers = gates["independent_readers"]["evidence"]
        multipart = gates["multipart_hash"]["evidence"]
        interrupted = gates["interrupted_put"]["evidence"]
        checks = gates["cnpg_pitr"]["evidence"]
        listing = gates["paginated_listing"]["evidence"]
        restore = gates["btrfs_restore"]["evidence"]
        checksum = crawler.get("sha256", "")
        if crawler.get("sql_catalog_commit") is not True or crawler.get("rows", 0) < 1 or not re.fullmatch(r"[0-9a-f]{64}", checksum):
            continue
        if readers.get("rows") != crawler["rows"] or readers.get("sha256") != checksum or set(readers.get("readers", [])) != {"PyIceberg", "DuckDB"}:
            continue
        if multipart.get("bytes", 0) <= 2 * 1024**3 or not re.fullmatch(r"[0-9a-f]{64}", multipart.get("sha256", "")) or multipart.get("parts", 0) < 2:
            continue
        if interrupted.get("partial_object_visible") is not False or interrupted.get("store_restarted") is not True:
            continue
        if not all(checks.get(key) for key in ("base_backup", "wal_archive", "restored_cluster", "recovery_target", "verified_rows")):
            continue
        if listing.get("objects", 0) <= 10000 or listing.get("pages", 0) <= 1 or not listing.get("disk_evidence"):
            continue
        if restore.get("readonly") is not True or restore.get("restored") is not True or restore.get("table_reread_sha256") != checksum:
            continue
        if form == "systemd" and candidate.get("native_binary_sha256") != BINARY_SHA:
            raise ValueError("Native form needs seven-gate evidence for the pinned native binary too")
        return hashlib.sha256(path.read_bytes()).hexdigest()
    raise ValueError("No complete accepted pinned Versity candidate; Garage fallback needs separate explicit configuration review")


def templates(args):
    identity = {"format_version": 1, "form": args.form, "bind_address": args.bind_address,
                "bridge": args.bridge, "filesystem_uuid": args.filesystem_uuid.lower(),
                "credential_file": str(args.credential_file), "uid": args.uid, "gid": args.gid,
                "image": IMAGE, "native_binary_sha256": BINARY_SHA, "data": str(DATA)}
    owner = hashlib.sha256(json.dumps(identity, sort_keys=True).encode()).hexdigest()
    identity["owner"] = owner
    common = f'''[Unit]
Description=EconGraph accepted host object store
Wants=network-online.target
After=network-online.target
RequiresMountsFor={DATA}
AssertPathExists={COVERAGE}

[Service]
ExecStartPre=/usr/bin/mountpoint -q {DATA}
UMask=0077
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
'''
    if args.form == "systemd":
        unit = common + f'''User={USER}
Group={USER}
EnvironmentFile={args.credential_file}
ExecStart={BINARY} --port {args.bind_address}:7070 posix {DATA}
ReadWritePaths={DATA}
PrivateDevices=true
CapabilityBoundingSet=
RestrictAddressFamilies=AF_INET AF_UNIX
MemoryMax=6G
CPUQuota=200%
TasksMax=256
Restart=on-failure
RestartSec=5
TimeoutStopSec=60

[Install]
WantedBy=multi-user.target
'''
        files = {str(UNIT): unit}
    else:
        compose = {"services": {"objectstore": {
            "image": IMAGE, "container_name": "econgraph-objectstore", "restart": "unless-stopped",
            "user": f"{args.uid}:{args.gid}", "env_file": [str(args.credential_file)],
            "command": ["--port", ":7070", "posix", "/data"],
            "ports": [f"{args.bind_address}:7070:7070"],
            "volumes": [{"type": "bind", "source": str(DATA), "target": "/data"}],
            "labels": {"org.econgraph.objectstore-owner": owner}, "cap_drop": ["ALL"],
            "security_opt": ["no-new-privileges:true"], "cpus": 2, "mem_limit": "6g",
            "pids_limit": 256, "read_only": True,
            "tmpfs": ["/tmp:rw,noexec,nosuid,size=64m"],
            "logging": {"driver": "json-file", "options": {"max-size": "10m", "max-file": "3"}},
        }}}
        compose["networks"] = {"default": {"name": "econgraph-objectstore-network", "labels": {"org.econgraph.objectstore-owner": owner}}}
        compose_path = CONFIG / "compose.json"
        prefix = f"/usr/bin/docker --host=unix:///var/run/docker.sock compose --project-name econgraph-objectstore --file {compose_path}"
        unit = common.replace("After=network-online.target", "After=network-online.target docker.service\nRequires=docker.service") + f'''Type=oneshot
RemainAfterExit=true
ExecStart={prefix} up -d --no-recreate --pull never
ExecStop={prefix} stop
TimeoutStartSec=120
TimeoutStopSec=120

[Install]
WantedBy=multi-user.target
'''
        files = {str(UNIT): unit, str(compose_path): json.dumps(compose, indent=2) + "\n"}
    fstab = f"UUID={args.filesystem_uuid.lower()} {DATA} btrfs subvol=/@econgraph-objectstore,compress=zstd:3,ssd,noatime,nofail 0 0"
    files["/etc/systemd/system/econgraph-backup-snapshots.service.d/objectstore.conf"] = f"[Unit]\nRequiresMountsFor={DATA}\n"
    files[str(COVERAGE)] = "objectstore-v1\n"
    return identity, files, fstab


def credential_check(path, staged=False):
    if staged:
        no_symlinks(path)
        value = path.stat()
        allowed = {0, int(os.environ.get("SUDO_UID", os.geteuid()))}
        if not stat.S_ISREG(value.st_mode) or value.st_uid not in allowed or stat.S_IMODE(value.st_mode) != 0o600:
            raise ValueError("Staged credentials must be a private regular file owned by root or the invoking operator")
    else:
        root_private(path, 0o600)
    values = {}
    for line in path.read_text().splitlines():
        key, separator, value = line.partition("=")
        if not separator or key in values or key not in {"ROOT_ACCESS_KEY", "ROOT_SECRET_KEY"}:
            raise ValueError("Credential file must contain only the two unquoted root credential assignments")
        values[key] = value
    if set(values) != {"ROOT_ACCESS_KEY", "ROOT_SECRET_KEY"} or not re.fullmatch(r"[A-Za-z0-9_-]{16,64}", values["ROOT_ACCESS_KEY"]) or not re.fullmatch(r"[A-Za-z0-9/+_=.-]{32,256}", values["ROOT_SECRET_KEY"]):
        raise ValueError("Credential assignments do not meet the private bootstrap format")
    return values


def account_check(args, create=False):
    try:
        user = pwd.getpwnam(USER)
    except KeyError:
        user = None
    for identifier, lookup in ((args.uid, pwd.getpwuid), (args.gid, grp.getgrgid)):
        try:
            existing = lookup(identifier)
            if existing[0] != USER:
                raise ValueError("Dedicated UID/GID is already owned by another account")
        except KeyError:
            pass
    if user and (user.pw_uid != args.uid or user.pw_gid != args.gid or user.pw_dir != "/nonexistent" or user.pw_shell != "/usr/sbin/nologin"):
        raise ValueError("Existing service account is not this managed account")
    if create and user is None:
        try:
            group = grp.getgrnam(USER)
        except KeyError:
            run("groupadd", "--system", "--gid", str(args.gid), USER)
        else:
            if group.gr_gid != args.gid:
                raise ValueError("Foreign service group")
        run("useradd", "--system", "--uid", str(args.uid), "--gid", str(args.gid),
            "--no-create-home", "--home-dir", "/nonexistent", "--shell", "/usr/sbin/nologin", USER)


def native_binary(args):
    if args.native_archive is None:
        raise ValueError("Native install requires the staged official --native-archive")
    no_symlinks(args.native_archive)
    info = args.native_archive.stat()
    if not stat.S_ISREG(info.st_mode) or info.st_size > 128 * 1024**2:
        raise ValueError("Native archive must be a bounded regular staged file")
    archive_bytes = args.native_archive.read_bytes()
    if hashlib.sha256(archive_bytes).hexdigest() != ARCHIVE_SHA:
        raise ValueError("Native archive does not match the official pinned release checksum")
    with tarfile.open(fileobj=io.BytesIO(archive_bytes)) as archive:
        member = archive.getmember("versitygw_v1.10.0_Linux_x86_64/versitygw")
        if not member.isfile() or member.size > 128 * 1024**2:
            raise ValueError("Unexpected native executable archive member")
        payload = archive.extractfile(member).read()
    if hashlib.sha256(payload).hexdigest() != BINARY_SHA:
        raise ValueError("Native executable digest mismatch")
    return payload


def container_matches(value, args, identity, expected_credentials=None):
    """A label alone never authorizes adoption of changed runtime configuration."""
    config = value.get("Config", {})
    host = value.get("HostConfig", {})
    mounts = [mount for mount in value.get("Mounts", []) if mount.get("Destination") == "/data"]
    environment = dict(item.split("=", 1) for item in config.get("Env", []) if "=" in item)
    return (
        (expected_credentials is None or all(environment.get(key) == data for key, data in expected_credentials.items()))
        and (config.get("Labels") or {}).get("org.econgraph.objectstore-owner") == identity["owner"]
        and config.get("Image") == IMAGE
        and config.get("User") == f"{args.uid}:{args.gid}"
        and config.get("Cmd") == ["--port", ":7070", "posix", "/data"]
        and len(mounts) == 1 and mounts[0].get("Source") == str(DATA) and mounts[0].get("RW") is True
        and not host.get("Privileged") and not host.get("CapAdd") and host.get("ReadonlyRootfs") is True
        and host.get("NetworkMode") == "econgraph-objectstore-network"
        and host.get("PortBindings", {}).get("7070/tcp") == [{"HostIp": args.bind_address, "HostPort": "7070"}]
    )


def preflight(args, identity, files, fstab):
    if os.geteuid() != 0:
        raise ValueError("Explicit installation/start must run as root after host authorization")
    evidence_sha = acceptance(args.evidence, args.form)
    for path in (DATA, CONFIG, MARKER, UNIT, BINARY, COVERAGE, Path("/etc/fstab")):
        no_symlinks(path)
    if args.credential_file.exists():
        credentials = credential_check(args.credential_file)
        if args.credential_source:
            credential_check(args.credential_source, staged=True)
            if args.credential_source.read_bytes() != args.credential_file.read_bytes():
                raise ValueError("Existing credentials differ; key rotation requires separate explicit review")
    elif args.credential_source and args.credential_file == CONFIG / "credentials.env":
        credentials = credential_check(args.credential_source, staged=True)
    else:
        raise ValueError("Provide private staged --credential-source for first installation")
    addresses = json.loads(run("ip", "-j", "addr", "show", "dev", args.bridge))
    if not Path("/sys/class/net", args.bridge, "bridge").is_dir() or not any(info.get("local") == args.bind_address for address in addresses for info in address.get("addr_info", [])):
        raise ValueError("Bind IP must actually belong to the explicit KVM bridge")
    owned = MARKER.exists()
    if not owned:
        try:
            pwd.getpwnam(USER)
        except KeyError:
            pass
        else:
            raise ValueError("An unmanaged service account already owns the selected name")
        try:
            grp.getgrnam(USER)
        except KeyError:
            pass
        else:
            raise ValueError("An unmanaged service group already owns the selected name")
    account_check(args)
    if owned:
        root_private(MARKER, 0o600)
        if json.loads(MARKER.read_text()) != identity:
            raise ValueError("Managed deployment identity differs; refusing an in-place production change")
    loaded_unit = run("systemctl", "show", "--property=FragmentPath", "--value", UNIT.name)
    if loaded_unit and (not owned or loaded_unit != str(UNIT)):
        raise ValueError("A foreign systemd unit already owns the persistent service name")
    if CONFIG.exists() and not owned:
        if CONFIG.stat().st_uid != 0 or stat.S_IMODE(CONFIG.stat().st_mode) != 0o700:
            raise ValueError("Existing bootstrap configuration directory must be private root-owned")
        permitted = {args.credential_file.name} if args.credential_file.parent == CONFIG else set()
        if {path.name for path in CONFIG.iterdir()} - permitted:
            raise ValueError("Bootstrap configuration directory contains foreign resources")
    for path in {DATA, CONFIG / "compose.json", BINARY, *(Path(path) for path in files)}:
        if path.exists() and not owned:
            raise ValueError(f"Existing unowned resource; refusing adoption: {path}")
    for path, content in files.items():
        target = Path(path)
        if target.exists():
            root_private(target, 0o644)
            if target.read_text() != content:
                raise ValueError(f"Managed file changed; refusing overwrite: {path}")
    if subprocess.run(["mountpoint", "-q", str(DATA)], capture_output=True).returncode == 0:
        if run("findmnt", "-rn", "-o", "UUID,FSTYPE,FSROOT", "--mountpoint", str(DATA)).split() != [args.filesystem_uuid.lower(), "btrfs", "/@econgraph-objectstore"]:
            raise ValueError("A foreign filesystem already owns the store mount")
    with tempfile.TemporaryDirectory(prefix="econgraph-objectstore-check-", dir="/run") as top:
        run("mount", "-t", "btrfs", "-o", "ro,subvolid=5", "UUID=" + args.filesystem_uuid, top)
        try:
            source = Path(top) / "@econgraph-objectstore"
            if source.is_symlink():
                raise ValueError("Persistent btrfs subvolume cannot be a symlink")
            if source.exists() and not owned:
                raise ValueError("An existing foreign btrfs subvolume owns the persistent store name")
            if source.exists():
                run("btrfs", "subvolume", "show", str(source))
        finally:
            run("umount", top)
    fstab_mode = stat.S_IMODE(Path("/etc/fstab").stat().st_mode)
    if fstab_mode not in {0o600, 0o644}:
        raise ValueError("Unexpected writable fstab permissions")
    root_private(Path("/etc/fstab"), fstab_mode)
    for line in Path("/etc/fstab").read_text().splitlines():
        parts = line.split()
        if len(parts) > 1 and not line.lstrip().startswith("#") and parts[1] == str(DATA) and (not owned or line != fstab):
            raise ValueError("An existing foreign fstab entry owns the object-store mount")
    backup = Path("/usr/local/sbin/econgraph-backup-snapshots")
    root_private(backup, 0o755)
    source = Path(__file__).resolve().parents[1] / "host/backup-snapshots.sh"
    if backup.read_bytes() != source.read_bytes() or "objectstore-snapshots.enabled" not in backup.read_text():
        raise ValueError("Install the reviewed updated existing backup timer/script before enabling store coverage")
    run("systemctl", "is-enabled", "econgraph-backup-snapshots.timer")
    if args.form == "docker":
        options = json.loads(run("docker", "--host=unix:///var/run/docker.sock", "info", "--format", "{{json .SecurityOptions}}"))
        if any("rootless" in option for option in options):
            raise ValueError("Persistent container requires the rootful Docker daemon")
        run("docker", "--host=unix:///var/run/docker.sock", "image", "inspect", IMAGE)
        network = run("docker", "--host=unix:///var/run/docker.sock", "network", "ls", "--filter", "name=^econgraph-objectstore-network$", "--format", "{{.Name}}")
        if network:
            labels = json.loads(run("docker", "--host=unix:///var/run/docker.sock", "network", "inspect", "--format", "{{json .Labels}}", "econgraph-objectstore-network")) or {}
            if not owned or labels.get("org.econgraph.objectstore-owner") != identity["owner"]:
                raise ValueError("A foreign network owns the persistent store network name")
        found = run("docker", "--host=unix:///var/run/docker.sock", "container", "ls", "-a", "--filter", "name=^/econgraph-objectstore$", "--format", "{{.Names}}")
        if found:
            value = json.loads(run("docker", "--host=unix:///var/run/docker.sock", "inspect", "econgraph-objectstore"))[0]
            if not owned or not container_matches(value, args, identity, credentials):
                raise ValueError("A foreign or changed container owns the persistent store name; refusing replacement")
    elif BINARY.exists():
        root_private(BINARY, 0o755)
        if hashlib.sha256(BINARY.read_bytes()).hexdigest() != BINARY_SHA:
            raise ValueError("Existing native executable has another digest")
    else:
        native_binary(args)
    if args.action == "start" and (not DATA.is_dir() or any(not Path(path).is_file() for path in files)):
        raise ValueError("Installation is incomplete; refusing service start")
    return evidence_sha


def install(args, identity, files, fstab):
    # The default command never enters this function. No mount is even probed via
    # a temporary mount until all evidence/credentials/foreign-resource checks pass.
    evidence_sha = preflight(args, identity, files, fstab)
    lock = Path("/run/econgraph-objectstore-install.lock")
    no_symlinks(lock)
    if lock.exists():
        root_private(lock, 0o600)
    descriptor = os.open(lock, os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
    with os.fdopen(descriptor, "w") as locked:
        fcntl.flock(locked, fcntl.LOCK_EX | fcntl.LOCK_NB)
        preflight(args, identity, files, fstab)
        if args.action == "start":
            if not MARKER.exists() or not COVERAGE.exists():
                raise ValueError("Install must complete before explicitly starting")
            run("systemctl", "enable", "--now", UNIT.name)
            return
        for directory in (CONFIG.parent, CONFIG):
            if directory.exists():
                if directory.stat().st_uid != 0 or directory.stat().st_mode & 0o022:
                    raise ValueError("Configuration directory is writable by another principal")
            else:
                directory.mkdir(mode=0o700 if directory == CONFIG else 0o755)
        if not args.credential_file.exists():
            credential_check(args.credential_source, staged=True)
            public_file(args.credential_file, args.credential_source.read_bytes(), 0o600)
        public_file(MARKER, (json.dumps(identity, sort_keys=True) + "\n").encode(), 0o600)
        account_check(args, create=True)
        with tempfile.TemporaryDirectory(prefix="econgraph-objectstore-top-", dir="/run") as top:
            run("mount", "-t", "btrfs", "-o", "subvolid=5", "UUID=" + args.filesystem_uuid, top)
            try:
                source = Path(top) / "@econgraph-objectstore"
                if source.is_symlink():
                    raise ValueError("Persistent btrfs subvolume cannot be a symlink")
                if source.exists():
                    run("btrfs", "subvolume", "show", str(source))
                else:
                    run("btrfs", "subvolume", "create", str(source))
                if not DATA.exists():
                    DATA.mkdir(parents=False, mode=0o750)
                mounted = subprocess.run(["mountpoint", "-q", str(DATA)], capture_output=True).returncode == 0
                if not mounted:
                    if any(DATA.iterdir()):
                        raise ValueError("Nonempty mount target cannot be covered by a new mount")
                    text = Path("/etc/fstab").read_text()
                    if fstab not in text.splitlines():
                        with Path("/etc/fstab").open("a") as stream:
                            stream.write("\n" + fstab + "\n")
                    run("systemctl", "daemon-reload")
                    run("mount", str(DATA))
                actual = run("findmnt", "-rn", "-o", "UUID,FSTYPE,FSROOT", "--mountpoint", str(DATA)).split()
                if actual != [args.filesystem_uuid.lower(), "btrfs", "/@econgraph-objectstore"]:
                    raise ValueError("Object-store path is not the exact managed btrfs mount")
                info = DATA.stat()
                if (info.st_uid, info.st_gid, stat.S_IMODE(info.st_mode)) != (args.uid, args.gid, 0o750):
                    if any(DATA.iterdir()):
                        raise ValueError("Refusing ownership/permission changes to nonempty existing object storage")
                    os.chown(DATA, args.uid, args.gid)
                    DATA.chmod(0o750)
            finally:
                run("umount", top)
        probe = DATA / ".econgraph-xattr-probe"
        fd = os.open(probe, os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_WRONLY, 0o600)
        try:
            os.setxattr(fd, b"user.econgraph-probe", b"xattrs-preserved")
            if os.getxattr(fd, b"user.econgraph-probe") != b"xattrs-preserved":
                raise ValueError("POSIX metadata xattrs did not roundtrip")
        finally:
            os.close(fd)
            probe.unlink()
        if args.form == "systemd":
            BINARY.parent.mkdir(parents=True, exist_ok=True)
            public_file(BINARY, BINARY.read_bytes() if BINARY.exists() else native_binary(args), 0o755)
        for path, content in files.items():
            parent = Path(path).parent
            no_symlinks(parent)
            if not parent.exists():
                parent.mkdir(parents=True, mode=0o755)
            if parent.stat().st_uid != 0 or parent.stat().st_mode & 0o022:
                raise ValueError("Managed unit/config parent is writable by another principal")
            public_file(Path(path), content.encode())
        run("systemctl", "daemon-reload")
        print(json.dumps({"status": "installed_not_started", "evidence_sha256": evidence_sha}))


def parser():
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--form", choices=("systemd", "docker"), required=True)
    cli.add_argument("--bind-address", required=True)
    cli.add_argument("--bridge", required=True)
    cli.add_argument("--filesystem-uuid", required=True)
    cli.add_argument("--credential-file", type=Path, default=CONFIG / "credentials.env")
    cli.add_argument("--credential-source", type=Path, help="Private operator-owned bootstrap assignments; copied root:root0600 only after acceptance")
    cli.add_argument("--uid", type=int, default=61100)
    cli.add_argument("--gid", type=int, default=61100)
    cli.add_argument("--evidence", type=Path)
    cli.add_argument("--native-archive", type=Path)
    cli.add_argument("--action", choices=("install", "start"), default="install")
    cli.add_argument("--render", type=Path)
    cli.add_argument("--execute", action="store_true")
    return cli


def main():
    cli = parser()
    args = cli.parse_args()
    try:
        validate_args(args)
        identity, files, fstab = templates(args)
        if args.execute:
            install(args, identity, files, fstab)
        else:
            if args.render:
                destination = args.render.absolute()
                no_symlinks(destination)
                repository = Path(__file__).resolve().parents[2]
                if not destination.is_relative_to(Path("/tmp")) and not destination.is_relative_to(repository):
                    raise ValueError("Candidate rendering is restricted to /tmp or this source worktree")
                args.render.mkdir(parents=True, exist_ok=True, mode=0o700)
                if args.render.stat().st_uid != os.geteuid() or args.render.stat().st_mode & 0o077:
                    raise ValueError("Candidate directory must be private and owned by the rendering actor")
                for path, content in {**files, "fstab.fragment": fstab + "\n", "plan.json": json.dumps(identity, indent=2) + "\n"}.items():
                    target = args.render / Path(path).name
                    if target.exists() or target.is_symlink():
                        if target.is_symlink() or target.stat().st_uid != os.geteuid() or target.read_text() != content:
                            raise ValueError("Refusing to replace an existing different candidate file")
                    else:
                        with target.open("x") as stream:
                            stream.write(content)
            print(json.dumps({"status": "dry_run", "identity": identity, "files": files, "fstab": fstab,
                              "native_archive": {"url": ARCHIVE_URL, "sha256": ARCHIVE_SHA},
                              "execution_prerequisites": ["explicit host authorization and chosen form", "all seven pinned-store gates including actual CNPG PITR", "native artifact evidence for native form", "private root credentials", "existing updated no-deletion snapshot timer", "matching bridge IP and btrfs filesystem"],
                              "start": "separate --action start --execute after install; no automatic service start"}, indent=2))
        return 0
    except (ValueError, OSError, KeyError, subprocess.CalledProcessError) as error:
        print(f"Refused: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
