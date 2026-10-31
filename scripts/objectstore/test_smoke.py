"""Offline refusal and ownership checks; compatibility evidence comes from the live runner."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import MagicMock, patch

spec = importlib.util.spec_from_file_location("objectstore_smoke", Path(__file__).with_name("smoke.py"))
smoke = importlib.util.module_from_spec(spec)
spec.loader.exec_module(smoke)


class SmokeSafetyTests(unittest.TestCase):
    def test_default_is_dry_run_without_docker_or_sdk(self):
        with patch("sys.argv", ["smoke.py"]), patch.object(smoke, "docker", side_effect=AssertionError), contextlib.redirect_stdout(io.StringIO()) as out:
            self.assertEqual(smoke.main(), 0)
        plan = json.loads(out.getvalue())
        self.assertEqual(plan["status"], "dry_run")
        self.assertEqual(len(plan["gates"]), 7)
        self.assertFalse(plan["persistent_host_service"])
        self.assertTrue(all("@sha256:" in image for image in plan["images"].values()))

    def test_cleanup_refuses_container_with_other_owner(self):
        with patch.object(smoke, "docker", return_value=json.dumps({smoke.LABEL: "someone-else"})) as call:
            with self.assertRaisesRegex(RuntimeError, "different ownership"):
                smoke.owned_action("chosen", "mine", "rm", "-f")
        self.assertEqual(call.call_count, 1)

    def test_cleanup_addresses_only_exact_owned_container(self):
        with patch.object(smoke, "docker", side_effect=[json.dumps({smoke.LABEL: "mine"}), ""]) as call:
            smoke.owned_action("chosen", "mine", "rm", "-f")
        self.assertEqual(call.call_args.args, ("rm", "-f", "chosen"))

    def test_missing_hook_is_unavailable(self):
        with self.assertRaises(smoke.Unavailable):
            smoke.hook(None, SimpleNamespace(crawler_command=None), "crawler", ())

    def test_backup_hook_retains_data_even_when_command_fails(self):
        with tempfile.TemporaryDirectory() as temp:
            store = smoke.Store("versity", "owner", Path(temp))
            store.env = lambda: {}
            store.s3 = MagicMock()
            args = SimpleNamespace(cnpg_command='["actual-cnpg"]', hook_timeout=1)
            with patch.object(smoke.subprocess, "run", return_value=SimpleNamespace(returncode=1)):
                with self.assertRaises(RuntimeError):
                    smoke.hook(store, args, "cnpg", ("base_backup",))
            self.assertTrue(store.retained_backup)

    def test_hook_cannot_pass_with_missing_evidence(self):
        with tempfile.TemporaryDirectory() as temp:
            store = smoke.Store("versity", "owner", Path(temp))
            store.env = lambda: {}
            (Path(temp) / "crawler.json").write_text('{"status":"passed"}')
            args = SimpleNamespace(crawler_command='["actual-crawler"]', hook_timeout=1)
            with patch.object(smoke.subprocess, "run", return_value=SimpleNamespace(returncode=0)):
                with self.assertRaisesRegex(RuntimeError, "complete passing evidence"):
                    smoke.hook(store, args, "crawler", ("sql_catalog_commit",))

    def test_btrfs_cleanup_refuses_unmarked_or_wrong_path(self):
        with tempfile.TemporaryDirectory() as temp:
            scratch = Path(temp)
            (scratch / ".owner").write_text("a" * 16)
            with patch.object(smoke, "docker", side_effect=AssertionError):
                with self.assertRaisesRegex(RuntimeError, "exact scratch ownership"):
                    smoke.delete_scratch_subvolumes(scratch, "a" * 16)

    def test_backup_retention_bypasses_btrfs_cleanup(self):
        with tempfile.TemporaryDirectory() as temp:
            scratch = Path(temp)
            store = smoke.Store("versity", "owner", scratch, btrfs=True)
            store.retained_backup = True
            with patch.object(smoke, "delete_scratch_subvolumes", side_effect=AssertionError):
                store.stop()
            self.assertTrue(scratch.is_dir())

    def test_garage_fsync_and_no_automatic_snapshots(self):
        config = smoke.garage_config()
        self.assertIn('db_engine = "sqlite"', config)
        self.assertIn("replication_factor = 1", config)
        self.assertIn("metadata_fsync = true", config)
        self.assertIn("data_fsync = true", config)
        self.assertNotIn("metadata_auto_snapshot_interval", config)

    def test_backup_data_never_automatically_removed(self):
        with tempfile.TemporaryDirectory() as temp:
            scratch = Path(temp) / "retained"
            scratch.mkdir()
            store = smoke.Store("versity", "owner", scratch)
            store.retained_backup = True
            store.stop()
            self.assertTrue(scratch.is_dir())


if __name__ == "__main__":
    unittest.main()
