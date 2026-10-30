#!/usr/bin/env python3
"""Exercise target-tip coordination in real scratch Git repositories, without network I/O."""

import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    "migration_order", Path(__file__).with_name("check_migration_order.py")
)
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


class TargetCoordinationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = Path(self.temp.name)
        self.git("init", "-b", "main")
        self.git("config", "user.name", "Migration Test")
        self.git("config", "user.email", "migration@example.invalid")
        for plane in checker.MIGRATION_SETS:
            self.migration(plane, "00000000000000_diesel_initial_setup", "SELECT 1;")
        self.data = self.migration("data", "2026-10-21-000100_v4_0_baseline", "CREATE TABLE observations (id INTEGER);")
        self.app = self.migration("app", "2026-10-21-000200_v4_0_app_baseline", "CREATE TABLE notes (id INTEGER);")
        self.commit()
        self.git("tag", "v4.0.0")
        self.baseline = self.git("rev-parse", "HEAD").strip()

    def git(self, *args):
        return subprocess.run(["git", "-C", str(self.repo), *args], check=True,
                              text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout

    def commit(self):
        self.git("add", ".")
        self.git("commit", "-m", "Scratch migration fixture")

    def migration(self, plane, name, sql="SELECT 1;"):
        path = self.repo / "backend/migrations" / plane / name
        path.mkdir(parents=True)
        (path / "up.sql").write_text(sql)
        (path / "down.sql").write_text("SELECT 1;")
        return path

    def remove(self, path):
        for file in path.iterdir():
            file.unlink()
        path.rmdir()

    def check(self, base="main"):
        with contextlib.redirect_stdout(io.StringIO()) as output:
            result = checker.check_against_target(self.repo, base)
        return result, output.getvalue()

    def test_unchanged_target(self):
        self.assertTrue(self.check()[0])

    def test_target_tip_not_fork_point_and_repair_two_branches(self):
        # Two workstreams independently add different versions. The earlier one
        # must be renumbered after the later migration lands, even without a collision.
        self.git("checkout", "-b", "workspace")
        workspace = self.migration("app", "2026-10-22-000100_workspace", "CREATE TABLE charts (id INTEGER);")
        self.commit()
        self.git("checkout", "main")
        self.git("checkout", "-b", "storage")
        self.migration("data", "2026-10-23-000100_storage", "CREATE TABLE attributes (id INTEGER);")
        self.commit()
        self.git("checkout", "main")
        self.git("merge", "--ff-only", "storage")
        self.git("checkout", "workspace")
        self.assertFalse(self.check()[0])
        self.git("rebase", "main")
        self.assertFalse(self.check()[0])
        workspace.rename(workspace.with_name("2026-10-24-000100_workspace"))
        self.commit()
        self.assertTrue(self.check()[0])
        # Both forward changes are present in the two-plane generated schema.
        # This uses portable DDL in SQLite; the existing schema_validation_test
        # remains the PostgreSQL/Diesel comparison of the repository's real schema.
        self.git("checkout", "main")
        self.git("merge", "--ff-only", "workspace")
        db = sqlite3.connect(":memory:")
        self.addCleanup(db.close)
        for plane in checker.MIGRATION_SETS:
            for path in sorted((self.repo / "backend/migrations" / plane).iterdir()):
                db.executescript((path / "up.sql").read_text())
        tables = {row[0] for row in db.execute("SELECT name FROM sqlite_master WHERE type = 'table'")}
        self.assertEqual(tables, {"observations", "attributes", "notes", "charts"})

    @unittest.skipUnless(os.environ.get("MIGRATION_TEST_DATABASE_URL"), "requires disposable PostgreSQL")
    def test_two_branch_rehearsal_on_postgres(self):
        self.test_target_tip_not_fork_point_and_repair_two_branches()
        # All objects are transaction-local to two randomly named schemas. Exercise
        # both a new two-plane database and a database upgraded as branches land.
        prefix = "migration_" + self.repo.name.replace("-", "_")
        fresh, upgraded = prefix + "_fresh", prefix + "_upgraded"
        statements = ["BEGIN;", f"CREATE SCHEMA {fresh};", f"CREATE SCHEMA {upgraded};"]
        paths = [path for plane in checker.MIGRATION_SETS
                 for path in sorted((self.repo / "backend/migrations" / plane).iterdir())]
        for schema in (fresh, upgraded):
            statements += [f"SET LOCAL search_path TO {schema};",
                           "CREATE TABLE __diesel_schema_migrations (version text PRIMARY KEY);"]
            if schema == upgraded:
                # Original baseline first; then storage lands; then workspace rebases.
                order = sorted(paths, key=lambda p: checker.diesel_version(p.name))
            else:
                order = paths  # Fresh startup applies each complete plane in turn.
            seen = set()
            for path in order:
                version = checker.diesel_version(path.name)
                if version not in seen:
                    statements += [(path / "up.sql").read_text(),
                                   f"INSERT INTO __diesel_schema_migrations VALUES ('{version}');"]
                    seen.add(version)
        statements.append(f"""
DO $$ BEGIN
    IF EXISTS (
        (SELECT table_name, column_name, data_type FROM information_schema.columns WHERE table_schema = '{fresh}'
         EXCEPT SELECT table_name, column_name, data_type FROM information_schema.columns WHERE table_schema = '{upgraded}')
        UNION ALL
        (SELECT table_name, column_name, data_type FROM information_schema.columns WHERE table_schema = '{upgraded}'
         EXCEPT SELECT table_name, column_name, data_type FROM information_schema.columns WHERE table_schema = '{fresh}')
    ) THEN RAISE EXCEPTION 'Fresh and sequential branch schemas differ'; END IF;
    IF (SELECT count(*) FROM {upgraded}.__diesel_schema_migrations) <> 5 THEN
        RAISE EXCEPTION 'A migration was skipped';
    END IF;
END $$;
ROLLBACK;
""")
        result = subprocess.run(["psql", "-X", "-v", "ON_ERROR_STOP=1", "-q",
                                 "-d", os.environ["MIGRATION_TEST_DATABASE_URL"]],
                                input="\n".join(statements), text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_global_high_water_includes_other_plane(self):
        self.migration("data", "2026-10-21-000150_late_data")
        ok, output = self.check()
        self.assertFalse(ok)
        self.assertIn("does not sort after target", output)

    def test_edit_merged_migration_rejected(self):
        (self.data / "up.sql").write_text("SELECT 2;")
        self.assertFalse(self.check()[0])

    def test_delete_merged_migration_rejected(self):
        self.remove(self.app)
        self.assertFalse(self.check()[0])

    def test_rename_same_version_rejected(self):
        self.app.rename(self.app.with_name("20261021000200_renamed_baseline"))
        self.assertFalse(self.check()[0])

    def test_new_forward_migration_allowed(self):
        self.migration("data", "2026-10-22-000100_new_data")
        self.assertTrue(self.check()[0])

    def test_missing_target_fails_closed(self):
        with self.assertRaises(subprocess.CalledProcessError):
            self.check("missing-target")

    def test_unshipped_app_suffix_squash(self):
        first = self.migration("app", "2026-10-22-000100_first")
        last = self.migration("app", "2026-10-23-000100_last")
        self.commit()
        self.remove(first)
        self.remove(last)
        self.migration("app", "2026-10-23-000100_release_squash", "SELECT 3;")
        self.assertTrue(self.check()[0])

    def test_published_app_migration_cannot_be_squashed(self):
        first = self.migration("app", "2026-10-22-000100_first")
        last = self.migration("app", "2026-10-23-000100_last")
        self.commit()
        self.git("tag", "v4.1.0")
        self.remove(first)
        (last / "up.sql").write_text("SELECT 3;")
        self.assertFalse(self.check()[0])

    def test_rc_and_train_tags_do_not_publish_product_migrations(self):
        first = self.migration("app", "2026-10-22-000100_first")
        last = self.migration("app", "2026-10-23-000100_last")
        self.commit()
        self.git("tag", "v4.1.0-rc.1")
        self.git("tag", "train-3")
        self.remove(first)
        (last / "up.sql").write_text("SELECT 3;")
        self.assertTrue(self.check()[0])

    def test_squash_cannot_remove_permanent_baseline(self):
        last = self.migration("app", "2026-10-23-000100_last")
        self.commit()
        self.remove(self.app)
        (last / "up.sql").write_text("SELECT 3;")
        self.assertFalse(self.check()[0])

    def test_data_squash_rejected(self):
        first = self.migration("data", "2026-10-22-000100_first")
        last = self.migration("data", "2026-10-23-000100_last")
        self.commit()
        self.remove(first)
        (last / "up.sql").write_text("SELECT 3;")
        self.assertFalse(self.check()[0])

    def test_seed_replacement_preserves_source_and_advances_version(self):
        seed = self.migration("data", "2026-10-22-000100_seed_census_reference_codes")
        self.commit()
        self.git("tag", "v4.1.0")
        self.remove(seed)
        self.migration("data", "2026-10-23-000100_seed_census_reference_codes")
        self.assertTrue(self.check()[0])

    def test_seed_cannot_be_edited_in_place(self):
        seed = self.migration("data", "2026-10-22-000100_seed_census_reference_codes")
        self.commit()
        (seed / "up.sql").write_text("SELECT 2;")
        self.assertFalse(self.check()[0])

    def test_other_source_is_not_a_seed_replacement(self):
        seed = self.migration("data", "2026-10-22-000100_seed_census_reference_codes")
        self.commit()
        self.remove(seed)
        self.migration("data", "2026-10-23-000100_seed_bea_reference_codes")
        self.assertFalse(self.check()[0])

    def test_event_target_selection(self):
        event_path = self.repo / "event.json"
        event_path.write_text(json.dumps({"pull_request": {"base": {"sha": "pr-tip"}},
                                          "merge_group": {"base_sha": "queue-tip"},
                                          "before": "push-parent", "ref": "refs/heads/main"}))
        self.assertEqual(checker.event_base_ref(str(event_path), "pull_request"), "pr-tip")
        self.assertEqual(checker.event_base_ref(str(event_path), "merge_group"), "queue-tip")
        self.assertEqual(checker.event_base_ref(str(event_path), "push"), "push-parent")
        self.assertIsNone(checker.event_base_ref(str(event_path), "workflow_dispatch"))

    def test_push_history_only_protects_serialized_integration_branches(self):
        self.git("update-ref", "refs/remotes/origin/main", self.baseline)
        self.git("checkout", "-b", "optimize/ci-core-primary")
        migration = self.migration("app", "2026-10-22-000100_unmerged")
        self.commit()
        previous_feature_tip = self.git("rev-parse", "HEAD").strip()
        # This migration has only appeared on the feature branch. Renumbering it
        # remains allowed even though CI also runs on pushes to that branch.
        migration.rename(migration.with_name("2026-10-23-000100_unmerged"))
        event_path = self.repo / "event.json"
        for ref in ("refs/heads/optimize/ci-core-primary", "refs/heads/feat/train3"):
            with self.subTest(ref=ref):
                event_path.write_text(json.dumps({"ref": ref, "before": previous_feature_tip}))
                selected = checker.event_base_ref(str(event_path), "push")
                self.assertIsNone(selected)
                self.assertTrue(self.check(selected or "origin/main")[0])
        # The same edit would rewrite merged history on an integration branch.
        for ref in ("refs/heads/main", "refs/heads/develop", "refs/heads/release/v4.1"):
            with self.subTest(ref=ref):
                event_path.write_text(json.dumps({"ref": ref, "before": previous_feature_tip}))
                selected = checker.event_base_ref(str(event_path), "push")
                self.assertEqual(selected, previous_feature_tip)
                ok, output = self.check(selected)
                self.assertFalse(ok)
                self.assertIn("Protected merged migration was edited or removed", output)

    def test_initial_integration_push_has_no_previous_history(self):
        event_path = self.repo / "event.json"
        event_path.write_text(json.dumps({"ref": "refs/heads/main", "before": "0" * 40}))
        self.assertIsNone(checker.event_base_ref(str(event_path), "push"))

    def test_exact_diesel_versions_with_sequence_suffix(self):
        first = self.migration("data", "2026-10-22-000100-0001_first")
        second = self.migration("data", "2026-10-22-000100-0002_second")
        migrations = [(checker.parse_migration_filename(p.name)[0], p.name, str(p))
                      for p in (first, second)]
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertTrue(checker.check_for_duplicate_timestamps(migrations))
            self.assertTrue(checker.check_migration_order(migrations))


if __name__ == "__main__":
    unittest.main()
