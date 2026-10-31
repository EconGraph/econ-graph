"""Offline refusal/identity tests. Synthetic evidence fixtures never authorize a host operation."""
import contextlib
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("host_store", Path(__file__).with_name("host_store.py"))
host = importlib.util.module_from_spec(spec)
spec.loader.exec_module(host)
BASE = ["--form", "docker", "--bind-address", "192.168.250.1", "--bridge", "econgraph0",
        "--filesystem-uuid", "03ee0fcb-5f19-4f44-8596-e227f1fb647c"]


def synthetic_report():
    checksum = "a" * 64
    evidence = {
        "crawler_sql_catalog": {"sql_catalog_commit": True, "rows": 5, "sha256": checksum},
        "independent_readers": {"rows": 5, "sha256": checksum, "readers": ["PyIceberg", "DuckDB"]},
        "multipart_hash": {"bytes": 2164260864, "sha256": "b" * 64, "parts": 129},
        "interrupted_put": {"partial_object_visible": False, "store_restarted": True},
        "cnpg_pitr": {"base_backup": "synthetic-base", "wal_archive": "synthetic-wal", "restored_cluster": "synthetic-restore", "recovery_target": "synthetic-time", "verified_rows": 1},
        "paginated_listing": {"objects": 10007, "pages": 74, "disk_evidence": "synthetic"},
        "btrfs_restore": {"readonly": True, "restored": True, "table_reread_sha256": checksum},
    }
    return {"format_version": 1, "status": "passed", "runs": [{"store": "versity", "image": host.IMAGE,
            "status": "passed", "gates": {key: {"status": "passed", "evidence": value} for key, value in evidence.items()}}]}


class HostStoreGuards(unittest.TestCase):
    def test_form_is_required(self):
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            host.parser().parse_args(BASE[2:])

    def test_default_dryrun_never_calls_host_tools(self):
        with patch("sys.argv", ["host_store.py", *BASE]), patch.object(host, "run", side_effect=AssertionError), patch.object(host.subprocess, "run", side_effect=AssertionError), contextlib.redirect_stdout(io.StringIO()) as output:
            self.assertEqual(host.main(), 0)
        plan = json.loads(output.getvalue())
        self.assertEqual(plan["status"], "dry_run")
        self.assertIn("separate", plan["start"])
        self.assertNotIn("ROOT_SECRET_KEY=", output.getvalue())

    def test_bind_address_must_be_private_bridge_address(self):
        for address in ("0.0.0.0", "127.0.0.1", "8.8.8.8", "169.254.1.2", "224.1.2.3"):
            args = host.parser().parse_args(BASE)
            args.bind_address = address
            with self.subTest(address=address), self.assertRaises(ValueError):
                host.validate_args(args)

    def test_credential_symlink_refused_without_reading_contents(self):
        with tempfile.TemporaryDirectory() as temp:
            target = Path(temp) / "private"
            target.write_text("not printed")
            link = Path(temp) / "link"
            link.symlink_to(target)
            args = host.parser().parse_args(BASE + ["--credential-file", str(link)])
            with self.assertRaisesRegex(ValueError, "symlink"):
                host.validate_args(args)

    def test_staged_credentials_are_private_and_assign_only_expected_keys(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "credentials"
            path.write_text("ROOT_ACCESS_KEY=fixtureaccess123456789\nROOT_SECRET_KEY=" + "a" * 64 + "\n")
            path.chmod(0o600)
            host.credential_check(path, staged=True)
            path.chmod(0o644)
            with self.assertRaisesRegex(ValueError, "private regular file"):
                host.credential_check(path, staged=True)
            path.chmod(0o600)
            path.write_text(path.read_text() + "UNRELATED=value\n")
            with self.assertRaisesRegex(ValueError, "only the two"):
                host.credential_check(path, staged=True)

    def test_incomplete_pitr_blocks_before_any_privileged_command(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "incomplete.json"
            report = synthetic_report()
            report["runs"][0]["gates"]["cnpg_pitr"]["status"] = "unavailable"
            path.write_text(json.dumps(report))
            args = host.parser().parse_args(BASE + ["--evidence", str(path)])
            identity, files, fstab = host.templates(args)
            with patch.object(host.os, "geteuid", return_value=0), patch.object(host, "run", side_effect=AssertionError), self.assertRaises(ValueError):
                host.preflight(args, identity, files, fstab)

    def test_accepted_report_fields_must_agree(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "synthetic.json"
            report = synthetic_report()
            path.write_text(json.dumps(report))
            self.assertEqual(host.acceptance(path, "docker"), hashlib.sha256(path.read_bytes()).hexdigest())
            report["runs"][0]["gates"]["btrfs_restore"]["evidence"]["table_reread_sha256"] = "c" * 64
            path.write_text(json.dumps(report))
            with self.assertRaises(ValueError):
                host.acceptance(path, "docker")

    def test_native_needs_exact_native_artifact_evidence(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "synthetic.json"
            report = synthetic_report()
            path.write_text(json.dumps(report))
            with self.assertRaisesRegex(ValueError, "native binary"):
                host.acceptance(path, "systemd")
            report["runs"][0]["native_binary_sha256"] = host.BINARY_SHA
            path.write_text(json.dumps(report))
            self.assertTrue(host.acceptance(path, "systemd"))

    def test_garage_pass_never_automatically_reconfigures_host(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "synthetic.json"
            report = synthetic_report()
            report["runs"][0]["store"] = "garage"
            path.write_text(json.dumps(report))
            with self.assertRaisesRegex(ValueError, "explicit configuration review"):
                host.acceptance(path, "docker")

    def test_templates_pin_image_scope_bind_and_keep_credentials_out_of_argv(self):
        args = host.parser().parse_args(BASE)
        _, files, fstab = host.templates(args)
        compose = json.loads(files[str(host.CONFIG / "compose.json")])["services"]["objectstore"]
        self.assertEqual(compose["image"], host.IMAGE)
        self.assertEqual(compose["ports"], ["192.168.250.1:7070:7070"])
        self.assertNotIn("ROOT_SECRET_KEY", " ".join(compose["command"]))
        self.assertIn("subvol=/@econgraph-objectstore", fstab)
        args.form = "systemd"
        _, files, _ = host.templates(args)
        unit = files[str(host.UNIT)]
        self.assertIn("EnvironmentFile=", unit)
        self.assertIn("User=econgraph-objectstore", unit)
        self.assertNotIn("--secret", unit)
        self.assertNotIn("--nometa", unit)

    def test_candidate_render_is_idempotent_and_refuses_different_existing_file(self):
        with tempfile.TemporaryDirectory() as temp:
            destination = Path(temp) / "render"
            argv = ["host_store.py", *BASE, "--render", str(destination)]
            with patch("sys.argv", argv), contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(host.main(), 0)
                before = {file.name: file.read_bytes() for file in destination.iterdir()}
                self.assertEqual(host.main(), 0)
                self.assertEqual(before, {file.name: file.read_bytes() for file in destination.iterdir()})
            (destination / "compose.json").write_text("foreign")
            with patch("sys.argv", argv), contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(host.main(), 1)
            self.assertEqual((destination / "compose.json").read_text(), "foreign")

    def test_render_cannot_install_into_etc(self):
        with patch("sys.argv", ["host_store.py", *BASE, "--render", "/etc/systemd/system"]), contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(host.main(), 1)

    def test_install_file_keeps_identical_and_refuses_overwrite(self):
        with tempfile.TemporaryDirectory() as temp:
            target = Path(temp) / "candidate"
            host.public_file(target, b"first")
            with patch.object(host, "root_private"):
                host.public_file(target, b"first")
                with self.assertRaisesRegex(ValueError, "refusing overwrite"):
                    host.public_file(target, b"second")
            self.assertEqual(target.read_bytes(), b"first")

    def test_container_identity_includes_actual_mount_bind_and_metadata_mode(self):
        args = host.parser().parse_args(BASE)
        identity, _, _ = host.templates(args)
        value = {"Config": {"Labels": {"org.econgraph.objectstore-owner": identity["owner"]}, "Image": host.IMAGE,
                 "User": f"{args.uid}:{args.gid}", "Cmd": ["--port", ":7070", "posix", "/data"]},
                 "Mounts": [{"Destination": "/data", "Source": str(host.DATA), "RW": True}],
                 "HostConfig": {"ReadonlyRootfs": True, "NetworkMode": "econgraph-objectstore-network",
                 "PortBindings": {"7070/tcp": [{"HostIp": args.bind_address, "HostPort": "7070"}]}}}
        self.assertTrue(host.container_matches(value, args, identity))
        value["Config"]["Cmd"] = ["--port", ":7070", "posix", "--nometa", "/data"]
        self.assertFalse(host.container_matches(value, args, identity))
        value["Config"]["Cmd"] = ["--port", ":7070", "posix", "/data"]
        value["HostConfig"]["PortBindings"]["7070/tcp"][0]["HostIp"] = "0.0.0.0"
        self.assertFalse(host.container_matches(value, args, identity))

    def test_native_archive_digest_checked_before_extraction(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "tampered.tar.gz"
            path.write_bytes(b"not official")
            args = host.parser().parse_args(BASE + ["--native-archive", str(path)])
            with self.assertRaisesRegex(ValueError, "pinned release checksum"):
                host.native_binary(args)


if __name__ == "__main__":
    unittest.main()
