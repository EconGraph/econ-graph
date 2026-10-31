#!/usr/bin/env python3
"""Owned, disposable ECO-435 compatibility rehearsal. No host service installation."""

import argparse
import concurrent.futures
import hashlib
import http.client
import json
import os
from pathlib import Path
import secrets
import re
import shutil
import subprocess
import tempfile
import threading
import time
from urllib.parse import urlsplit

IMAGES = {
    "versity": "versity/versitygw@sha256:89cad13cc981986ee284cbafea4613f9533ce6fd5f37a2c96c4c4fb8adb3a316",
    "garage": "dxflrs/garage@sha256:677e5b758e4425a325b811c425c026bf35a4bf42f7ddbaa773de3f4fd779a5a5",
}
LABEL = "org.econgraph.objectstore-smoke"
GATES = ("crawler_sql_catalog", "independent_readers", "multipart_hash", "interrupted_put",
         "cnpg_pitr", "paginated_listing", "btrfs_restore")


class Unavailable(RuntimeError):
    pass


class ConditionalUnsupported(RuntimeError):
    pass


def command(argv, **kwargs):
    return subprocess.run(argv, check=True, capture_output=True, text=True, timeout=300, **kwargs).stdout.strip()


def docker(*argv, **kwargs):
    # Always use the reviewed local shared-host daemon, regardless of saved contexts.
    return command(["docker", "--host=unix:///var/run/docker.sock", *argv], **kwargs)


def owned_action(name, owner, *argv):
    labels = json.loads(docker("inspect", "--format", "{{json .Config.Labels}}", name))
    if labels.get(LABEL) != owner:
        raise RuntimeError("Refusing operation on container with a different ownership label")
    return docker(*argv, name)


def delete_scratch_subvolumes(scratch, owner):
    """Delete only this run's marked throwaway btrfs data and snapshot, never backups."""
    if not re.fullmatch(r"[0-9a-f]{16}", owner) or scratch.name not in {
        f"econgraph-objectstore-{owner}-versity", f"econgraph-objectstore-{owner}-garage"
    } or scratch.is_symlink() or (scratch / ".owner").read_text() != owner:
        raise RuntimeError("Refusing btrfs cleanup without exact scratch ownership")
    targets = [name for name in ("pre-restore", "snapshot", "data") if (scratch / name).exists()]
    if not targets:
        return
    # A rootful helper needs SYS_ADMIN for BTRFS_IOC_SNAP_DESTROY on mounts that
    # omit user_subvol_rm_allowed. It sees only this scratch directory writable,
    # no host /srv, devices, PID namespace, network, or Docker socket.
    docker("run", "--rm", "--name", f"econgraph-btrfs-clean-{owner}",
           "--label", f"{LABEL}={owner}", "--network", "none", "--read-only",
           "--cpus", "1", "--memory", "128m", "--pids-limit", "16",
           "--cap-drop", "ALL", "--cap-add", "SYS_ADMIN", "--cap-add", "DAC_OVERRIDE",
           "--security-opt", "no-new-privileges",
           "--mount", f"type=bind,src={scratch},dst=/scratch",
           "--mount", "type=bind,src=/usr/bin/btrfs,dst=/host-btrfs,readonly",
           "--mount", "type=bind,src=/usr/lib,dst=/hostlib,readonly",
           "--mount", "type=bind,src=/lib64/ld-linux-x86-64.so.2,dst=/host-ld.so,readonly",
           "--entrypoint", "/host-ld.so",
           "postgres@sha256:edb0c08dc33befd0262ab3b5cf1c4f3f8cfd8b09e88c05b45adee29d6761ddb6",
           "--library-path", "/hostlib/x86_64-linux-gnu", "/host-btrfs", "subvolume", "delete",
           *(f"/scratch/{name}" for name in targets))


def garage_config():
    # Omit metadata_auto_snapshot_interval: unset disables automatic metadata snapshots.
    return f'''metadata_dir = "/data/meta"
data_dir = "/data/objects"
db_engine = "sqlite"
replication_factor = 1
metadata_fsync = true
data_fsync = true
rpc_bind_addr = "127.0.0.1:3901"
rpc_public_addr = "127.0.0.1:3901"
rpc_secret = "{secrets.token_hex(32)}"
[s3_api]
s3_region = "us-east-1"
api_bind_addr = "0.0.0.0:3900"
root_domain = ".s3.garage.localhost"
'''


class Store:
    def __init__(self, kind, owner, scratch, btrfs=False):
        self.btrfs = btrfs
        self.kind, self.owner, self.scratch = kind, owner, scratch
        self.name = f"econgraph-s3-{owner}-{kind}"
        self.key, self.secret = "GK" + secrets.token_hex(16), secrets.token_hex(32)
        self.bucket = f"smoke-{owner}"
        self.endpoint = None
        self.retained_backup = False
        self.created = False

    def start(self):
        self.scratch.mkdir(mode=0o700)
        (self.scratch / ".owner").write_text(self.owner)
        data = self.scratch / "data"
        if self.btrfs:
            if command(["stat", "-f", "-c", "%T", str(self.scratch)]) != "btrfs":
                raise Unavailable("--btrfs-scratch requires an owned scratch directory on btrfs")
            command(["btrfs", "subvolume", "create", str(data)])
        else:
            data.mkdir(mode=0o700)
        if shutil.disk_usage(data).free < 6 * 1024**3:
            raise Unavailable("Need 6 GiB free for the bounded multipart/interruption rehearsal")
        port = 7070 if self.kind == "versity" else 3900
        argv = ["create", "--name", self.name, "--label", f"{LABEL}={self.owner}",
                "--cpus", "2", "--memory", "6g", "--memory-swap", "6g", "--pids-limit", "256",
                "--user", f"{os.getuid()}:{os.getgid()}", "--cap-drop", "ALL",
                "--security-opt", "no-new-privileges", "--log-opt", "max-size=5m",
                "--log-opt", "max-file=2", "-p", f"127.0.0.1::{port}",
                "--mount", f"type=bind,src={data},dst=/data"]
        env = {}
        if self.kind == "versity":
            env = {"ROOT_ACCESS_KEY": self.key, "ROOT_SECRET_KEY": self.secret}
            extra = ["--port", ":7070", "posix", "/data"]
        else:
            config = self.scratch / "garage.toml"
            config.write_text(garage_config())
            config.chmod(0o600)
            argv += ["--mount", f"type=bind,src={config},dst=/etc/garage.toml,readonly"]
            env = {"GARAGE_DEFAULT_ACCESS_KEY": self.key, "GARAGE_DEFAULT_SECRET_KEY": self.secret,
                   "GARAGE_DEFAULT_BUCKET": self.bucket}
            extra = ["/garage", "server", "--single-node", "--default-bucket"]
        # Secret values are passed in environment, never printed or put in argv.
        for key in env:
            argv += ["-e", key]
        docker(*argv, IMAGES[self.kind], *extra, env={**os.environ, **env})
        self.created = True
        owned_action(self.name, self.owner, "start")
        binding = docker("port", self.name, f"{port}/tcp")
        if not binding.startswith("127.0.0.1:") or "\n" in binding:
            raise RuntimeError("Store must publish exactly one random loopback port")
        self.endpoint = "http://" + binding
        self.s3 = self.client()
        for attempt in range(60):
            try:
                self.s3.list_buckets()
                if self.kind == "versity":
                    self.s3.create_bucket(Bucket=self.bucket)
                return
            except Exception:
                if attempt == 59:
                    raise RuntimeError("Store never became ready; inspect owned container logs")
                time.sleep(0.5)

    def client(self):
        import boto3
        from botocore.config import Config
        return boto3.client("s3", endpoint_url=self.endpoint, aws_access_key_id=self.key,
                            aws_secret_access_key=self.secret, region_name="us-east-1",
                            config=Config(s3={"addressing_style": "path"},
                                          connect_timeout=5, read_timeout=15,
                                          retries={"max_attempts": 1}))

    def stop(self):
        if self.created:
            owned_action(self.name, self.owner, "rm", "-f")
        if not self.retained_backup and self.scratch.exists():
            if self.btrfs:
                delete_scratch_subvolumes(self.scratch, self.owner)
            shutil.rmtree(self.scratch, ignore_errors=False)

    def env(self):
        return {**os.environ, "S3_ENDPOINT": self.endpoint, "AWS_ACCESS_KEY_ID": self.key,
                "AWS_SECRET_ACCESS_KEY": self.secret, "AWS_REGION": "us-east-1",
                "AWS_ALLOW_HTTP": "true", "S3_BUCKET": self.bucket,
                "ICEBERG_WAREHOUSE": f"s3://{self.bucket}/iceberg",
                "SMOKE_OWNER": self.owner, "SMOKE_CONTAINER": self.name,
                "SMOKE_DATA_DIRECTORY": str(self.scratch / "data")}

    def restart(self):
        owned_action(self.name, self.owner, "start")
        port = 7070 if self.kind == "versity" else 3900
        binding = docker("port", self.name, f"{port}/tcp")
        if not binding.startswith("127.0.0.1:") or "\n" in binding:
            raise RuntimeError("Restart exposed an unexpected network binding")
        self.endpoint = "http://" + binding
        self.s3 = self.client()
        for _ in range(60):
            try:
                self.s3.list_buckets()
                return
            except Exception:
                time.sleep(0.5)
        raise RuntimeError("Store did not recover after interruption")


def multipart(store):
    # Deterministic incompressible 16 MiB part, sent 129 times (>2 GiB).
    part = hashlib.shake_256(b"econgraph-eco435-v1").digest(16 * 1024**2)
    count = 129
    expected = hashlib.sha256()
    upload = store.s3.create_multipart_upload(Bucket=store.bucket, Key="multipart.bin")["UploadId"]
    parts = []
    completed = False
    try:
        for number in range(1, count + 1):
            expected.update(part)
            response = store.s3.upload_part(Bucket=store.bucket, Key="multipart.bin", UploadId=upload,
                                           PartNumber=number, Body=part)
            parts.append({"PartNumber": number, "ETag": response["ETag"]})
        store.s3.complete_multipart_upload(Bucket=store.bucket, Key="multipart.bin", UploadId=upload,
                                          MultipartUpload={"Parts": parts})
        completed = True
        response = store.s3.get_object(Bucket=store.bucket, Key="multipart.bin")
        actual, size = hashlib.sha256(), 0
        body = response["Body"]
        try:
            for chunk in body.iter_chunks(1024**2):
                actual.update(chunk)
                size += len(chunk)
        finally:
            body.close()
        if size != len(part) * count or actual.digest() != expected.digest():
            raise RuntimeError("Multipart content length/hash mismatch")
        try:
            capability = conditional_create(store)
        except ConditionalUnsupported as error:
            capability = {"supported": False, "reason": str(error)}
        return {"bytes": size, "sha256": actual.hexdigest(), "parts": count,
                "create_only": capability}
    finally:
        if not completed:
            store.s3.abort_multipart_upload(Bucket=store.bucket, Key="multipart.bin", UploadId=upload)
        # This key is disposable generated test data, never a backup.
        store.s3.delete_object(Bucket=store.bucket, Key="multipart.bin")


def conditional_create(store):
    from botocore.exceptions import ClientError
    key = "conditional-create.bin"
    checksum_headers = set()
    def observe(request, **kwargs):
        checksum_headers.update(name.lower() for name in request.headers if "checksum" in name.lower())
    store.s3.meta.events.register("before-send.s3.PutObject", observe)
    try:
        store.s3.put_object(Bucket=store.bucket, Key=key, Body=b"original", IfNoneMatch="*")
        try:
            store.s3.put_object(Bucket=store.bucket, Key=key, Body=b"replacement", IfNoneMatch="*")
        except ClientError as error:
            if error.response["ResponseMetadata"]["HTTPStatusCode"] != 412:
                raise
        else:
            raise ConditionalUnsupported("Create-only If-None-Match write overwrote an existing object")
        response = store.s3.get_object(Bucket=store.bucket, Key=key)
        try:
            if response["Body"].read() != b"original":
                raise RuntimeError("Conditional create changed original bytes")
        finally:
            response["Body"].close()
        return {"supported": True, "if_none_match": "412 and original preserved", "sdk_checksum_headers": sorted(checksum_headers)}
    finally:
        store.s3.meta.events.unregister("before-send.s3.PutObject", observe)
        store.s3.delete_object(Bucket=store.bucket, Key=key)


def interrupted_put(store):
    # Start a real incomplete single PUT with a known Content-Length. Kill only
    # this owned store after the first chunk is sent; the writer cannot finish.
    key, total = "interrupted.bin", 128 * 1024**2
    url = urlsplit(store.s3.generate_presigned_url("put_object",
                    Params={"Bucket": store.bucket, "Key": key}, ExpiresIn=120))
    started, release = threading.Event(), threading.Event()
    errors = []

    def write():
        connection = http.client.HTTPConnection(url.hostname, url.port, timeout=20)
        try:
            connection.putrequest("PUT", url.path + "?" + url.query)
            connection.putheader("Content-Length", str(total))
            connection.endheaders()
            connection.send(b"x" * 1024**2)
            started.set()
            release.wait(15)
            connection.send(b"y" * 1024**2)
            response = connection.getresponse()
            if response.status < 300:
                errors.append("Incomplete upload unexpectedly succeeded")
        except (OSError, http.client.HTTPException):
            pass
        finally:
            connection.close()

    thread = threading.Thread(target=write, daemon=True)
    thread.start()
    try:
        if not started.wait(10):
            raise RuntimeError("Upload did not start")
        time.sleep(0.2)
        owned_action(store.name, store.owner, "kill", "--signal", "KILL")
    finally:
        release.set()
        thread.join(25)
        store.restart()
    if thread.is_alive() or errors:
        raise RuntimeError("Interrupted writer did not terminate cleanly")
    from botocore.exceptions import ClientError
    try:
        store.s3.head_object(Bucket=store.bucket, Key=key)
    except ClientError as error:
        if error.response["ResponseMetadata"]["HTTPStatusCode"] != 404:
            raise
    else:
        raise RuntimeError("Partial PUT became visible after restart")
    listed = store.s3.list_objects_v2(Bucket=store.bucket, Prefix=key)
    if listed.get("Contents"):
        raise RuntimeError("Partial PUT appears in object listing")
    return {"advertised_bytes": total, "partial_object_visible": False, "store_restarted": True}


def list_keys(store, **kwargs):
    pages = list(store.s3.get_paginator("list_objects_v2").paginate(
        Bucket=store.bucket, PaginationConfig={"PageSize": 137}, **kwargs))
    keys = [item["Key"] for page in pages for item in page.get("Contents", [])]
    return keys, pages


def listing(store):
    keys = [f"listing/group-{number % 11:02}/{number:05}" for number in range(10007)]
    def put(key):
        store.s3.put_object(Bucket=store.bucket, Key=key, Body=key.encode())
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as executor:
        list(executor.map(put, keys))
    actual, pages = list_keys(store, Prefix="listing/")
    if len(actual) != len(keys) or set(actual) != set(keys) or len(pages) <= 1:
        raise RuntimeError("Paginated listing lost or duplicated objects")
    _, delimited = list_keys(store, Prefix="listing/", Delimiter="/")
    prefixes = [p["Prefix"] for page in delimited for p in page.get("CommonPrefixes", [])]
    if set(prefixes) != {f"listing/group-{i:02}/" for i in range(11)}:
        raise RuntimeError("Delimiter listing differs from known prefixes")
    if store.kind == "versity":
        root = store.scratch / "data" / store.bucket
        on_disk = {str(p.relative_to(root)) for p in (root / "listing").rglob("*") if p.is_file()}
        if on_disk != set(keys):
            raise RuntimeError("S3 listing differs from POSIX object files")
        disk_evidence = "exact POSIX filename comparison"
    else:
        # Garage uses content-addressed blocks rather than one file per key.
        # Verify every listed object's bytes; filenames cannot encode its logical keys.
        for key in keys:
            if store.s3.get_object(Bucket=store.bucket, Key=key)["Body"].read() != key.encode():
                raise RuntimeError("Garage object readback differs")
        files = [p for p in (store.scratch / "data" / "objects").rglob("*") if p.is_file()]
        if not files:
            raise RuntimeError("Garage object directory contains no block files")
        disk_evidence = f"{len(files)} block files plus all logical-object byte checks"
    return {"objects": len(keys), "pages": len(pages), "delimiter_prefixes": len(prefixes),
            "disk_evidence": disk_evidence}


def hook(store, args, name, minimum):
    argv = getattr(args, name + "_command")
    if not argv:
        raise Unavailable(f"Missing --{name.replace('_', '-')}-command JSON argv; actual dependent lane required")
    report = store.scratch / (name + ".json")
    if name == "cnpg":
        # Preserve all objects if a backup command even starts: partial backups count too.
        store.retained_backup = True
        store.s3.create_bucket(Bucket=store.bucket + "-backups")
    env = {**store.env(), "SMOKE_OUTPUT_JSON": str(report)}
    if name == "cnpg":
        env["S3_BUCKET"] = store.bucket + "-backups"
    result = subprocess.run(json.loads(argv), env=env, timeout=args.hook_timeout,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if result.returncode:
        raise RuntimeError(f"{name} command exited {result.returncode}; its detailed report must explain failure")
    evidence = json.loads(report.read_text())
    if evidence.get("status") != "passed" or any(not evidence.get(key) for key in minimum):
        raise RuntimeError(f"{name} command did not produce complete passing evidence")
    if name == "crawler":
        if type(evidence.get("requires_conditional_create")) is not bool:
            raise RuntimeError("Crawler must declare its actual conditional-create requirement")
        if evidence["sql_catalog_commit"] is not True or not isinstance(evidence["rows"], int) or evidence["rows"] < 1:
            raise RuntimeError("Crawler evidence requires a real SQL catalog commit and nonempty rows")
        if not evidence["metadata_location"].startswith(f"s3://{store.bucket}/") or not re.fullmatch(r"[0-9a-f]{64}", evidence["sha256"]):
            raise RuntimeError("Crawler metadata/hash does not identify this run's generated bucket")
    if name == "snapshot":
        crawler_report = json.loads((store.scratch / "crawler.json").read_text())
        if evidence["restored"] is not True or evidence["table_reread_sha256"] != crawler_report["sha256"]:
            raise RuntimeError("Snapshot restore does not match the actual crawler table checksum")
    return evidence


def independent_readers(store, crawler):
    if not crawler:
        raise Unavailable("Actual crawler SQL-catalog gate has no passing metadata reference")
    import pyiceberg.table
    import duckdb
    properties = {"s3.endpoint": store.endpoint, "s3.access-key-id": store.key,
                  "s3.secret-access-key": store.secret, "s3.region": "us-east-1"}
    table = pyiceberg.table.StaticTable.from_metadata(crawler["metadata_location"], properties)
    arrow = table.scan().to_arrow()
    db = duckdb.connect(config={"extension_directory": os.environ.get("DUCKDB_EXTENSION_DIRECTORY", str(store.scratch / "duckdb-extensions"))})
    try:
        db.execute("SET custom_extension_repository='https://extensions.duckdb.org'")
        for extension in ("httpfs", "iceberg"):
            try:
                db.execute(f"LOAD {extension}")
            except duckdb.Error:
                db.execute(f"INSTALL {extension}; LOAD {extension}")
        host = urlsplit(store.endpoint).netloc
        # Values originate in random generated credentials, not shell/user SQL.
        db.execute(f"CREATE SECRET smoke (TYPE s3, KEY_ID '{store.key}', SECRET '{store.secret}', REGION 'us-east-1', ENDPOINT '{host}', URL_STYLE 'path', USE_SSL false)")
        duck = db.execute("SELECT * FROM iceberg_scan(?)", [crawler["metadata_location"]]).fetch_arrow_table()
        # Canonical hash is specified by the real crawler gate, shared between clients.
        columns = crawler["canonical_columns"]
        def canonical(data):
            def value(item):
                if isinstance(item, bool):
                    return "true" if item else "false"
                return str(item) if item is not None else None
            rows = [[value(row[col]) for col in columns]
                    for row in data.to_pylist()]
            lines = sorted(json.dumps(row, separators=(",", ":"), ensure_ascii=False) for row in rows)
            return hashlib.sha256(("\n".join(lines) + "\n").encode()).hexdigest()
        hashes = [canonical(data) for data in (arrow, duck)]
        if arrow.num_rows != crawler["rows"] or duck.num_rows != crawler["rows"] or hashes != [crawler["sha256"]] * 2:
            raise RuntimeError("PyIceberg/DuckDB row count or canonical hash differs from crawler")
        return {"rows": arrow.num_rows, "sha256": hashes[0], "readers": ["PyIceberg", "DuckDB"]}
    finally:
        db.close()


def snapshot_restore(store, crawler):
    if not store.btrfs:
        raise Unavailable("Use --btrfs-scratch with --scratch-parent in owned btrfs worktree temp/")
    if not crawler:
        raise Unavailable("Crawler table and independent-reader dependencies must pass before snapshot restore")
    evidence = restore_scratch_snapshot(store)
    readback = independent_readers(store, crawler)
    return {**evidence, "table_reread_sha256": readback["sha256"]}


def restore_scratch_snapshot(store):
    """Actual filesystem recovery; table validation stays in the caller's gate."""
    data, snapshot = store.scratch / "data", store.scratch / "snapshot"
    owned_action(store.name, store.owner, "stop", "--time", "15")
    try:
        command(["btrfs", "subvolume", "snapshot", "-r", str(data), str(snapshot)])
        readonly = command(["btrfs", "property", "get", "-t", "s", str(snapshot), "ro"])
        if readonly != "ro=true":
            raise RuntimeError("Snapshot is not read-only")
    finally:
        store.restart()
    # Exercise loss of the generated data bucket through S3 on both physical
    # formats. CNPG backups use another bucket and are never deleted here.
    keys, _ = list_keys(store)
    for offset in range(0, len(keys), 1000):
        result = store.s3.delete_objects(Bucket=store.bucket,
                    Delete={"Objects": [{"Key": key} for key in keys[offset:offset + 1000]]})
        if result.get("Errors"):
            raise RuntimeError("Could not remove generated table objects for recovery rehearsal")
    if list_keys(store)[0]:
        raise RuntimeError("Generated table bucket is not empty before restore")
    owned_action(store.name, store.owner, "stop", "--time", "15")
    try:
        # Retain the pre-restore store too: no backup object or recovery copy is
        # automatically deleted, even when the CNPG hook wrote partial backups.
        data.rename(store.scratch / "pre-restore")
        command(["btrfs", "subvolume", "snapshot", str(snapshot), str(data)])
    finally:
        store.restart()
    return {"snapshot": str(snapshot), "readonly": True, "restored": True}


def parser():
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--execute", action="store_true", help="Actually create owned throwaway stores")
    result.add_argument("--store", choices=("versity", "garage", "both"), default="both")
    result.add_argument("--btrfs-scratch", action="store_true",
                        help="Use a scratch subvolume and owned cleanup helper; scratch-parent must be btrfs")
    result.add_argument("--gates", default=",".join(GATES),
                        help="Comma-separated gates for dependency-focused reruns; omitted gates never pass")
    result.add_argument("--report", type=Path)
    result.add_argument("--scratch-parent", type=Path, default=Path(tempfile.gettempdir()))
    result.add_argument("--hook-timeout", type=int, default=1800)
    for name in ("crawler", "cnpg", "snapshot"):
        result.add_argument(f"--{name}-command", help="JSON argv for actual dependent implementation, never a shell command")
    return result


def main():
    cli = parser()
    args = cli.parse_args()
    selected = set(args.gates.split(","))
    if not selected or not selected <= set(GATES):
        cli.error("--gates must select named acceptance gates")
    if not args.execute:
        print(json.dumps({"status": "dry_run", "images": IMAGES, "gates": GATES,
                          "resources": "unique ownership labels; 2 CPU/6GiB per store; random loopback ports",
                          "persistent_host_service": False, "required": "boto3; actual crawler; PyIceberg; DuckDB; CNPG PITR; disposable btrfs snapshot hook"}, indent=2))
        return 0
    try:
        import boto3  # noqa: F401
    except ImportError:
        raise SystemExit("Install scripts/objectstore/requirements.txt in a virtual environment first")
    owner = secrets.token_hex(8)
    kinds = ("versity", "garage") if args.store == "both" else (args.store,)
    runs = []
    for kind in kinds:
        scratch = args.scratch_parent.resolve() / f"econgraph-objectstore-{owner}-{kind}"
        store = Store(kind, owner, scratch, args.btrfs_scratch)
        run = {"store": kind, "image": IMAGES[kind], "owner": owner, "gates": {}}
        runs.append(run)
        crawler = None
        try:
            store.start()
            checks = {
                "crawler_sql_catalog": lambda: hook(store, args, "crawler", ("metadata_location", "rows", "sha256", "canonical_columns", "sql_catalog_commit")),
                "independent_readers": lambda: independent_readers(store, crawler),
                "multipart_hash": lambda: multipart(store),
                "interrupted_put": lambda: interrupted_put(store),
                "cnpg_pitr": lambda: hook(store, args, "cnpg", ("base_backup", "wal_archive", "restored_cluster", "recovery_target", "verified_rows")),
                "paginated_listing": lambda: listing(store),
                "btrfs_restore": lambda: hook(store, args, "snapshot", ("snapshot", "restored", "table_reread_sha256")) if args.snapshot_command else snapshot_restore(store, crawler),
            }
            for name, check in checks.items():
                if name not in selected:
                    run["gates"][name] = {"status": "not_run", "reason": "Excluded by --gates; no acceptance claim"}
                    continue
                started = time.monotonic()
                try:
                    evidence = check()
                    if name == "multipart_hash" and crawler and crawler.get("requires_conditional_create") is True and not evidence["create_only"]["supported"]:
                        raise RuntimeError("Actual crawler requires conditional creation but store ignores it")
                    run["gates"][name] = {"status": "passed", "evidence": evidence}
                    if name == "crawler_sql_catalog":
                        crawler = evidence
                except Exception as error:
                    status = "unavailable" if isinstance(error, (Unavailable, ImportError)) else "failed"
                    run["gates"][name] = {"status": status, "reason": str(error)}
                run["gates"][name]["seconds"] = round(time.monotonic() - started, 3)
                print(f"{kind} {name}: {run['gates'][name]['status']} {run['gates'][name].get('reason', '')}", flush=True)
        except Exception as error:
            run["startup"] = {"status": "failed", "reason": str(error)}
            for name in GATES:
                run["gates"].setdefault(name, {"status": "unavailable", "reason": "Store startup failed"})
        finally:
            try:
                store.stop()
            except Exception as error:
                run["cleanup"] = {"status": "failed", "reason": str(error), "scratch_directory": str(scratch)}
            if store.retained_backup:
                run["retained_backup_directory"] = str(scratch)
        run["status"] = "passed" if "cleanup" not in run and len(run["gates"]) == 7 and all(v["status"] == "passed" for v in run["gates"].values()) else "incomplete"
        # Any Versity failure/unavailable gate runs the identical matrix on Garage.
        if kind == "versity" and run["status"] == "passed":
            break
    result = {"format_version": 1, "owner": owner, "runs": runs,
              "status": "passed" if any(run["status"] == "passed" for run in runs) else "incomplete"}
    if args.report:
        args.report.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
