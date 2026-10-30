#!/usr/bin/env python3
"""Real PostgreSQL 18 rehearsal, with synthetic records in a disposable container.

Starts one uniquely named postgres:18 container (removed afterwards) on the Docker daemon
named by DOCKER_HOST, default the rootful socket, publishing its port on 127.0.0.1 only.
Backups go to a scratch ECONGRAPH_BACKUP_ROOT; it never reads ~/econgraph-backups, a
cluster or production configuration.
"""
import contextlib
import importlib.util
import hashlib
import io
import json
import os
from pathlib import Path
import socket
import sys
import subprocess
import tempfile
import time
from types import SimpleNamespace
import unittest
from unittest.mock import patch
import uuid

import psycopg
from psycopg import sql

sys.dont_write_bytecode = True
SCRIPT = Path(__file__).with_name('split-app-data.py')
spec = importlib.util.spec_from_file_location('split_app_data', SCRIPT)
conversion = importlib.util.module_from_spec(spec)
spec.loader.exec_module(conversion)
DOCKER_HOST = os.environ.get('DOCKER_HOST', 'unix:///var/run/docker.sock')
DOCKER = ['docker', f'--host={DOCKER_HOST}']


class BackupLocation(unittest.TestCase):
    """Backups go to new directories under the shared host backup root."""

    def test_default_directory_is_under_the_backup_root(self):
        """Honor ECONGRAPH_BACKUP_ROOT, else ~/econgraph-backups, with a UTC timestamp name."""
        with patch.dict(os.environ, {'ECONGRAPH_BACKUP_ROOT': '/scratch/backups'}):
            directory = conversion.new_backup_dir()
        self.assertEqual(directory.parent, Path('/scratch/backups'))
        self.assertRegex(directory.name, r'^\d{8}T\d{6}Z$')
        with patch.dict(os.environ, {'ECONGRAPH_BACKUP_ROOT': ''}):
            self.assertEqual(conversion.backup_root(), Path.home() / 'econgraph-backups')

    def test_refuses_an_existing_directory_without_touching_it(self):
        """Never write into, replace or remove an existing backup."""
        with tempfile.TemporaryDirectory() as scratch:
            existing = Path(scratch) / 'backup'
            existing.mkdir()
            (existing / 'econ_graph.dump').write_text('earlier backup')
            with self.assertRaisesRegex(RuntimeError, 'refusing to overwrite a backup'):
                conversion.verified_backup(None, 'unused', existing, {})
            self.assertEqual((existing / 'econ_graph.dump').read_text(), 'earlier backup')


class ConversionRehearsal(unittest.TestCase):
    """Exercise actual migrations and recovery behavior using synthetic records only."""
    @classmethod
    def setUpClass(cls):
        """Start one uniquely named, disposable PostgreSQL container and test-only services."""
        cls.scratch = tempfile.TemporaryDirectory(prefix='econgraph-conversion-')
        cls.root = Path(cls.scratch.name)
        cls.container = 'econgraph-conversion-' + uuid.uuid4().hex[:12]
        with socket.socket() as listener:
            listener.bind(('127.0.0.1', 0))
            cls.port = listener.getsockname()[1]
        # The server listens on the same port inside the container as the one published on the
        # host's loopback, so one set of non-secret libpq services works for the host Python
        # process and for the version-matched pg_dump run inside this container.
        subprocess.run(DOCKER + ['run', '-d', '--name', cls.container,
                       '--publish', f'127.0.0.1:{cls.port}:{cls.port}',
                       '--mount', f'type=bind,src={cls.root},dst={cls.root}',
                       '-e', 'POSTGRES_HOST_AUTH_METHOD=trust',
                       'postgres:18', '-c', 'listen_addresses=*', '-p', str(cls.port)],
                       check=True, stdout=subprocess.DEVNULL)
        cls.admin = f'host=127.0.0.1 port={cls.port} user=postgres dbname=postgres'
        for attempt in range(60):
            try:
                with psycopg.connect(cls.admin):
                    break
            except psycopg.OperationalError:
                time.sleep(0.5)
        else:
            cls.tearDownClass()
            raise RuntimeError('Scratch PostgreSQL did not start')
        cls.old_backup_root = os.environ.get('ECONGRAPH_BACKUP_ROOT')
        cls.backup_root = cls.root / 'econgraph-backups'
        os.environ['ECONGRAPH_BACKUP_ROOT'] = str(cls.backup_root)
        cls.old_path = os.environ['PATH']
        cls.old_service = os.environ.get('PGSERVICEFILE')
        cls.services = cls.root / 'services.conf'
        os.environ['PGSERVICEFILE'] = str(cls.services)
        for tool in ('pg_dump', 'pg_restore'):
            wrapper = cls.root / tool
            wrapper.write_text(f'#!/bin/sh\nexec docker --host={DOCKER_HOST} exec -i '
                               f'-e PGSERVICEFILE={cls.services} {cls.container} {tool} "$@"\n')
            wrapper.chmod(0o700)
        os.environ['PATH'] = str(cls.root) + ':' + cls.old_path

    @classmethod
    def tearDownClass(cls):
        """Remove only the test container and its synthetic artifacts."""
        subprocess.run(DOCKER + ['rm', '-f', '-v', cls.container], check=False, stdout=subprocess.DEVNULL)
        if hasattr(cls, 'old_path'):
            os.environ['PATH'] = cls.old_path
            if cls.old_backup_root is None:
                os.environ.pop('ECONGRAPH_BACKUP_ROOT', None)
            else:
                os.environ['ECONGRAPH_BACKUP_ROOT'] = cls.old_backup_root
            if cls.old_service is None:
                os.environ.pop('PGSERVICEFILE', None)
            else:
                os.environ['PGSERVICEFILE'] = cls.old_service
        cls.scratch.cleanup()

    def setUp(self):
        """Create independent source and app databases from the checked-in migrations."""
        self.number = uuid.uuid4().hex[:12]
        self.source_db = 'combined_' + self.number
        self.app_db = 'app_' + self.number
        with psycopg.connect(self.admin, autocommit=True) as admin:
            for name in (self.source_db, self.app_db):
                admin.execute(sql.SQL('CREATE DATABASE {}').format(sql.Identifier(name)))
        self.services.write_text(''.join(
            f'[{service}]\nhost=127.0.0.1\nport={self.port}\nuser=postgres\ndbname={database}\n'
            for service, database in [('conversion_source', self.source_db), ('conversion_app', self.app_db)]))
        for service, planes in [('conversion_source', ['data', 'app']), ('conversion_app', ['app'])]:
            with psycopg.connect(service=service) as db:
                db.execute('CREATE TABLE __diesel_schema_migrations (version varchar PRIMARY KEY, run_on timestamp DEFAULT now())')
                for plane in planes:
                    for migration in sorted((conversion.ROOT / 'backend/migrations' / plane).iterdir()):
                        if migration.is_dir():
                            db.execute((migration / 'up.sql').read_text())
                            db.execute('INSERT INTO __diesel_schema_migrations(version) VALUES (%s) ON CONFLICT DO NOTHING',
                                       (migration.name.split('_', 1)[0].replace('-', ''),))
        self.user_id = uuid.uuid4()
        self.series_id = uuid.uuid5(conversion.NAMESPACE, 'FRED:GDP')
        with psycopg.connect(service='conversion_source') as db:
            source_id = db.execute("SELECT id FROM data_sources WHERE name='Federal Reserve Economic Data (FRED)'").fetchone()[0]
            dataset_id = db.execute("INSERT INTO datasets(source_id,code,name) VALUES (%s,'synthetic','Synthetic dataset') RETURNING id", (source_id,)).fetchone()[0]
            db.execute("INSERT INTO economic_series(id,source_id,dataset_id,external_id,title,frequency) VALUES (%s,%s,%s,'GDP','Synthetic GDP','Quarterly')", (self.series_id, source_id, dataset_id))
            db.execute("INSERT INTO users(id,email,name) VALUES (%s,'synthetic@example.invalid','Synthetic User')", (self.user_id,))
            db.execute("INSERT INTO chart_annotations(user_id,series_id,annotation_date,title) VALUES (%s,%s,'2026-01-01','Synthetic note')", (self.user_id, str(self.series_id)))
            # Model the original combined baseline's outgoing cross-plane foreign keys.
            db.execute('ALTER TABLE user_data_source_preferences ADD FOREIGN KEY(data_source_id) REFERENCES data_sources(id) ON DELETE CASCADE')
            db.execute('ALTER TABLE financial_annotations ADD FOREIGN KEY(statement_id) REFERENCES financial_statements(id) ON DELETE CASCADE')
            db.execute('ALTER TABLE financial_annotations ADD FOREIGN KEY(line_item_id) REFERENCES financial_line_items(id) ON DELETE CASCADE')
            db.execute("COMMENT ON TABLE users IS 'synthetic comment'")
            db.execute("COMMENT ON COLUMN users.email IS 'synthetic column comment'")
            db.execute('GRANT SELECT ON users TO pg_monitor')
            self.before = conversion.fingerprints(db, conversion.inventory('data')[0])
            self.app_before = conversion.fingerprints(db, conversion.inventory('app')[0])
            self.versions_before = conversion.versions(db)
        self.mapping = self.root / ('source-map-' + self.number + '.json')
        self.mapping.write_text(json.dumps({str(source_id): 'FRED'}))
        self.args = SimpleNamespace(source_service='conversion_source', app_service='conversion_app', source_map=self.mapping,
                                    backup_dir=self.backup_root / ('backup-' + self.number), dry_run=False,
                                    writers_stopped=True, drop_source_app=False, previous_evidence=None,
                                    restore_source_app=None, init_app_schema=None)
        self.phrase = f'DROP APP TABLES FROM {self.source_db}'

    def run_conversion(self):
        """Run the maintenance command while keeping synthetic evidence out of test logs."""
        with contextlib.redirect_stdout(io.StringIO()):
            conversion.convert(self.args)

    def copy_and_drop(self):
        """Copy, then remove the source app tables with confirmation; return the cleanup backup."""
        self.run_conversion()
        self.args.backup_dir = self.backup_root / ('cleanup-' + self.number)
        self.args.drop_source_app = True
        with patch('sys.stdin.isatty', return_value=True), patch('builtins.input', return_value=self.phrase):
            self.run_conversion()
        self.args.drop_source_app = False
        return self.args.backup_dir

    def app_count(self, service, table='users'):
        """Count rows of an app table, or None when the table does not exist."""
        with psycopg.connect(service=service) as db:
            if table not in conversion.tables(db):
                return None
            return db.execute(sql.SQL('SELECT count(*) FROM {}').format(sql.Identifier(table))).fetchone()[0]

    def schema(self):
        """The source's schema as pg_dump prints it: tables, types, indexes, triggers, comments, grants."""
        return subprocess.run(['pg_dump', '--schema-only', '--restrict-key=rehearsal', '--dbname', 'service=conversion_source'],
                              check=True, capture_output=True, text=True).stdout

    def restore(self, backup):
        """Run the rollback mode quietly."""
        args = SimpleNamespace(source_service='conversion_source', writers_stopped=True, restore_source_app=backup,
                               backup_dir=self.backup_root / ('rollback-' + uuid.uuid4().hex[:12]))
        with contextlib.redirect_stdout(io.StringIO()):
            conversion.restore_source_app(args)
        return args.backup_dir

    def test_copy_cleanup_and_idempotent_rerun(self):
        """Verify locks, exact copy, a restorable verified backup, cleanup and evidence-backed rerun."""
        self.args.dry_run = True
        self.run_conversion()
        self.assertFalse(self.args.backup_dir.exists())
        with psycopg.connect(service='conversion_app') as db:
            self.assertEqual(db.execute('SELECT count(*) FROM users').fetchone()[0], 0)
        self.args.dry_run = False
        backup = conversion.verified_backup
        def check_locks(*args):
            for service in ('conversion_source', 'conversion_app'):
                with psycopg.connect(service=service) as writer:
                    writer.execute("SET lock_timeout='100ms'")
                    with self.assertRaises(psycopg.errors.LockNotAvailable):
                        writer.execute("UPDATE users SET name='concurrent writer'")
            return backup(*args)
        with patch.object(conversion, 'verified_backup', side_effect=check_locks):
            self.run_conversion()
        with psycopg.connect(service='conversion_app') as db:
            self.assertEqual(db.execute('SELECT user_id,series_id FROM chart_annotations').fetchone(), (self.user_id, str(self.series_id)))
        # Reuse neither backup directories nor overwrite a verified dump on rerun.
        self.args.backup_dir = self.backup_root / ('cleanup-' + self.number)
        self.args.drop_source_app = True
        with patch('sys.stdin.isatty', return_value=True), patch('builtins.input', return_value=self.phrase):
            self.run_conversion()
        with psycopg.connect(service='conversion_source') as db:
            self.assertNotIn('users', conversion.tables(db))
            self.assertEqual(conversion.fingerprints(db, conversion.inventory('data')[0]), self.before)
            self.assertEqual(conversion.versions(db), conversion.inventory('data')[2])
        with self.assertRaisesRegex(RuntimeError, '--previous-evidence'):
            self.run_conversion()
        self.args.previous_evidence = self.args.backup_dir / 'evidence.json'
        self.run_conversion()
        evidence = json.loads((self.args.backup_dir / 'evidence.json').read_text())
        self.assertTrue(evidence['source_app_removed'])
        backup = self.args.backup_dir
        self.assertEqual(backup.stat().st_mode & 0o777, 0o700)
        self.assertEqual(sorted(p.name for p in backup.iterdir()),
                         ['SHA256SUMS', 'combined.dump', 'evidence.json', 'pg_tools.log', 'source-evidence.json'])
        for path in backup.iterdir():
            self.assertEqual(path.stat().st_mode & 0o777, 0o600, path.name)
        # No VERIFIED marker: the off-site copy of nightly dumps must never select this directory.
        # No econ_graph.dump: restore-kind-databases.sh must refuse it rather than compare data counts.
        self.assertFalse((backup / 'VERIFIED').exists())
        self.assertFalse((backup / 'econ_graph.dump').exists())
        archive = (backup / 'combined.dump').read_bytes()
        self.assertEqual(hashlib.sha256(archive).hexdigest(), evidence['backup_sha256'])
        self.assertEqual((backup / 'SHA256SUMS').read_text(), f"{evidence['backup_sha256']}  combined.dump\n")
        restored_db = 'restored_' + self.number
        with psycopg.connect(self.admin, autocommit=True) as admin:
            admin.execute(sql.SQL('CREATE DATABASE {}').format(sql.Identifier(restored_db)))
        with self.services.open('a') as services:
            services.write(f'[conversion_restored]\nhost=127.0.0.1\nport={self.port}\n'
                           f'user=postgres\ndbname={restored_db}\n')
        subprocess.run(['pg_restore', '--no-owner', '--no-privileges', '--exit-on-error',
                        '--dbname', 'service=conversion_restored'], input=archive,
                       check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        with psycopg.connect(service='conversion_restored') as restored:
            self.assertEqual(conversion.fingerprints(restored, conversion.inventory('data')[0]), self.before)
            self.assertEqual(conversion.fingerprints(restored, conversion.inventory('app')[0]), evidence['source_app'])

    def test_refuses_to_copy_unless_the_backup_verifies(self):
        """A dump that does not read back stops the conversion before any app row changes."""
        run = subprocess.run
        def unreadable(command, *args, **kwargs):
            if command[0] == 'pg_restore':
                kwargs['stderr'].write('pg_restore: error: synthetic corrupt block\n')
                return subprocess.CompletedProcess(command, 1)
            return run(command, *args, **kwargs)
        with patch.object(conversion.subprocess, 'run', side_effect=unreadable):
            with self.assertRaisesRegex(RuntimeError, 'pg_restore exited with status 1; .*synthetic corrupt block'):
                self.run_conversion()
        # The unverified archive and the tool log are kept for inspection, never promoted or deleted.
        self.assertEqual(sorted(p.name for p in self.args.backup_dir.iterdir()), ['combined.dump.partial', 'pg_tools.log'])
        self.assertEqual((self.args.backup_dir / 'pg_tools.log').stat().st_mode & 0o777, 0o600)
        with psycopg.connect(service='conversion_app') as db:
            self.assertEqual(db.execute('SELECT count(*) FROM users').fetchone()[0], 0)
        with psycopg.connect(service='conversion_source') as db:
            self.assertEqual(db.execute('SELECT count(*) FROM users').fetchone()[0], 1)

    def test_refuses_an_existing_backup_directory(self):
        """Never reuse a backup directory; the conversion stops before copying."""
        self.args.backup_dir.mkdir(parents=True)
        with self.assertRaisesRegex(RuntimeError, 'refusing to overwrite a backup'):
            self.run_conversion()
        self.assertEqual(list(self.args.backup_dir.iterdir()), [])
        with psycopg.connect(service='conversion_app') as db:
            self.assertEqual(db.execute('SELECT count(*) FROM users').fetchone()[0], 0)

    def test_refuses_unrelated_target_and_wrong_stable_ids(self):
        """Reject unrelated target rows, ambiguous services and unstable source IDs."""
        with psycopg.connect(service='conversion_app') as db:
            db.execute("INSERT INTO users(email,name) VALUES ('unrelated@example.invalid','Do not replace')")
        with self.assertRaisesRegex(RuntimeError, 'Target contains existing app data'):
            self.run_conversion()
        self.assertFalse(self.args.backup_dir.exists())
        self.args.source_service = 'conversion_source dbname=another_database'
        with self.assertRaisesRegex(RuntimeError, 'simple libpq service name'):
            self.run_conversion()
        self.args.source_service = 'conversion_source'
        self.mapping.write_text(json.dumps({next(iter(json.loads(self.mapping.read_text()))): 'BLS'}))
        with self.assertRaisesRegex(RuntimeError, 'Unstable series IDs'):
            self.run_conversion()

    def test_dry_run_refuses_app_column_mismatch_before_backup(self):
        """Reject schema drift in rehearsal before producing a backup or modifying app rows."""
        with psycopg.connect(service='conversion_app') as db:
            db.execute('ALTER TABLE users ADD COLUMN unsupported text')
        self.args.dry_run = True
        with self.assertRaisesRegex(RuntimeError, 'Column inventory differs for users'):
            self.run_conversion()
        self.assertFalse(self.args.backup_dir.exists())
        with psycopg.connect(service='conversion_app') as db:
            self.assertEqual(db.execute('SELECT count(*) FROM users').fetchone()[0], 0)
        with psycopg.connect(service='conversion_source') as db:
            self.assertEqual(db.execute('SELECT count(*) FROM users').fetchone()[0], 1)

    def test_copy_failure_retains_backup_and_rolls_back_both_databases(self):
        """Keep recovery artifacts and both original schemas when the app copy fails."""
        with psycopg.connect(service='conversion_app') as db:
            db.execute("ALTER TABLE users ADD CONSTRAINT rehearsal_refusal CHECK(name <> 'Synthetic User')")
        with self.assertRaises(psycopg.errors.CheckViolation):
            self.run_conversion()
        self.assertTrue((self.args.backup_dir / 'combined.dump').is_file())
        with psycopg.connect(service='conversion_app') as db:
            self.assertEqual(db.execute('SELECT count(*) FROM users').fetchone()[0], 0)
            self.assertEqual(db.execute('SELECT count(*) FROM annotation_templates').fetchone()[0], 3)
        with psycopg.connect(service='conversion_source') as db:
            self.assertEqual(db.execute('SELECT count(*) FROM users').fetchone()[0], 1)
            self.assertEqual(conversion.fingerprints(db, conversion.inventory('data')[0]), self.before)

    def test_copy_requires_stopped_writers_and_cleanup_requires_terminal(self):
        """Require maintenance confirmation and a terminal for destructive cleanup."""
        self.args.writers_stopped = False
        with self.assertRaisesRegex(RuntimeError, 'Stop all app/crawler writers'):
            self.run_conversion()
        self.args.writers_stopped = True
        self.args.drop_source_app = True
        # Without --backup-dir the backup goes to a new timestamped directory under the root.
        root = self.backup_root / ('default-' + self.number)
        self.args.backup_dir = None
        with patch.dict(os.environ, {'ECONGRAPH_BACKUP_ROOT': str(root)}), \
                patch('sys.stdin.isatty', return_value=False), \
                patch('builtins.input', side_effect=AssertionError('must not prompt without a terminal')):
            with self.assertRaisesRegex(RuntimeError, 'typed confirmation'):
                self.run_conversion()
        with psycopg.connect(service='conversion_source') as db:
            self.assertIn('users', conversion.tables(db))
        with psycopg.connect(service='conversion_app') as db:
            self.assertEqual(db.execute('SELECT count(*) FROM users').fetchone()[0], 1)
        (backup,) = root.iterdir()
        self.assertEqual(backup, self.args.backup_dir)
        self.assertTrue((backup / 'combined.dump').is_file())


    def test_wrong_confirmation_phrase_keeps_source_app_tables(self):
        """Only the exact phrase removes the source app tables; the verified copy stays committed."""
        self.args.drop_source_app = True
        with patch('sys.stdin.isatty', return_value=True), patch('builtins.input', return_value='DROP APP TABLES'):
            with self.assertRaisesRegex(RuntimeError, 'Confirmation did not match'):
                self.run_conversion()
        self.assertEqual(self.app_count('conversion_source'), 1)
        self.assertEqual(self.app_count('conversion_app'), 1)

    def test_copy_mismatch_rolls_back_the_target(self):
        """A copy whose content differs from the source is never committed."""
        copy = conversion.copy_app
        def corrupt(source, target, app_tables):
            copy(source, target, app_tables)
            target.execute("UPDATE users SET name='altered in transit'")
        with patch.object(conversion, 'copy_app', side_effect=corrupt):
            with self.assertRaisesRegex(RuntimeError, 'App copy content verification failed'):
                self.run_conversion()
        self.assertEqual(self.app_count('conversion_app'), 0)
        self.assertEqual(self.app_count('conversion_source'), 1)

    def test_data_change_during_cleanup_keeps_source_app_tables(self):
        """The data tables are re-verified before the removal commits."""
        fingerprints = conversion.fingerprints
        confirmed = []
        def changed_after_confirmation(conn, names):
            result = fingerprints(conn, names)
            if confirmed and 'data_points' in names:
                result = dict(result, data_points={'rows': -1, 'sha256': 'changed'})
            return result
        def confirm(prompt):
            confirmed.append(prompt)
            return self.phrase
        self.args.drop_source_app = True
        with patch.object(conversion, 'fingerprints', side_effect=changed_after_confirmation), \
                patch('sys.stdin.isatty', return_value=True), patch('builtins.input', side_effect=confirm):
            with self.assertRaisesRegex(RuntimeError, 'Data table content changed during cleanup'):
                self.run_conversion()
        self.assertEqual(self.app_count('conversion_source'), 1)
        with psycopg.connect(service='conversion_source') as db:
            self.assertEqual(conversion.versions(db), self.versions_before)

    def test_previous_evidence_must_match_the_databases(self):
        """An already converted source is accepted only with evidence that matches both databases."""
        cleanup = self.copy_and_drop()
        evidence = json.loads((cleanup / 'evidence.json').read_text())
        for key, value in [('data_before', {}), ('series', {}), ('source_app', {}), ('copy_verified', False),
                           ('source_identity', ['0', 'other']), ('target_identity', ['0', 'other'])]:
            tampered = self.root / f'tampered-{key}-{self.number}.json'
            tampered.write_text(json.dumps(dict(evidence, **{key: value})))
            self.args.previous_evidence = tampered
            with self.assertRaisesRegex(RuntimeError, 'Previous conversion evidence does not match', msg=key):
                self.run_conversion()
        # An emptied target is not a completed conversion, even with the genuine evidence.
        self.args.previous_evidence = cleanup / 'evidence.json'
        self.run_conversion()
        with psycopg.connect(service='conversion_app') as db:
            db.execute(sql.SQL('TRUNCATE {}').format(conversion.identifiers(conversion.inventory('app')[0])))
        with self.assertRaisesRegex(RuntimeError, 'Previous conversion evidence does not match'):
            self.run_conversion()

    def test_stale_copy_is_replaced_only_with_its_evidence(self):
        """After writers resumed on the combined database, its own earlier copy may be replaced."""
        self.run_conversion()
        first = self.args.backup_dir / 'evidence.json'
        with psycopg.connect(service='conversion_source') as db:
            db.execute("UPDATE users SET name='Renamed after the copy'")
        self.args.backup_dir = self.backup_root / ('second-' + self.number)
        with self.assertRaisesRegex(RuntimeError, 'Target contains existing app data'):
            self.run_conversion()
        self.args.previous_evidence = first
        self.run_conversion()
        with psycopg.connect(service='conversion_app') as db:
            self.assertEqual(db.execute('SELECT name FROM users').fetchone()[0], 'Renamed after the copy')
        # Not when the target holds anything other than that verified copy.
        with psycopg.connect(service='conversion_app') as db:
            db.execute("INSERT INTO users(email,name) VALUES ('new@example.invalid','Written to the app database')")
        self.args.backup_dir = self.backup_root / ('third-' + self.number)
        with self.assertRaisesRegex(RuntimeError, 'does not match the previous verified copy'):
            self.run_conversion()

    def test_column_type_and_nullability_must_match(self):
        """Same column names with a different type or nullability are refused."""
        with psycopg.connect(service='conversion_app') as db:
            db.execute('ALTER TABLE users ALTER COLUMN name TYPE text')
        self.args.dry_run = True
        with self.assertRaisesRegex(RuntimeError, 'Column inventory differs for users'):
            self.run_conversion()
        with psycopg.connect(service='conversion_app') as db:
            db.execute('ALTER TABLE users ALTER COLUMN name TYPE varchar(255)')
            nullable = db.execute("SELECT attnotnull FROM pg_attribute WHERE attrelid='public.users'::regclass "
                                  "AND attname='name'").fetchone()[0]
            db.execute(f"ALTER TABLE users ALTER COLUMN name {'DROP' if nullable else 'SET'} NOT NULL")
        with self.assertRaisesRegex(RuntimeError, 'Column inventory differs for users'):
            self.run_conversion()

    def test_rollback_restores_the_source_app_tables(self):
        """Convert, remove, roll back: the source app tables, enums and history are as before."""
        schema_before = self.schema()
        cleanup = self.copy_and_drop()
        self.assertIsNone(self.app_count('conversion_source'))
        record = self.restore(cleanup)
        self.assertEqual(self.schema(), schema_before)
        with psycopg.connect(service='conversion_source') as db:
            self.assertEqual(conversion.fingerprints(db, conversion.inventory('app')[0]), self.app_before)
            self.assertEqual(conversion.fingerprints(db, conversion.inventory('data')[0]), self.before)
            self.assertEqual(conversion.versions(db), self.versions_before)
            enums = {r[0] for r in db.execute("SELECT typname FROM pg_type WHERE typnamespace='public'::regnamespace")}
            self.assertLessEqual(set(conversion.inventory('app')[1]), enums)
            # Foreign keys back to the data tables are restored too.
            fkeys = {r[0] for r in db.execute("SELECT conrelid::regclass::text FROM pg_constraint WHERE contype='f' "
                                              "AND confrelid='public.data_sources'::regclass")}
            self.assertIn('user_data_source_preferences', fkeys)
        self.assertEqual(record.stat().st_mode & 0o777, 0o700)
        self.assertEqual(sorted(p.name for p in record.iterdir()), ['pg_tools.log', 'restore.list'])
        listing = (record / 'restore.list').read_text()
        self.assertIn('TABLE DATA public users ', listing)
        self.assertNotIn('TABLE DATA public data_points ', listing)
        # The restored source converts again and the copy verifies against it.
        self.args.backup_dir = self.backup_root / ('again-' + self.number)
        self.args.previous_evidence = None
        self.run_conversion()
        # A second rollback has nothing to restore and refuses a partial state.
        with self.assertRaisesRegex(RuntimeError, 'not the backup of a run that removed'):
            self.restore(self.args.backup_dir)

    def test_rollback_refuses_a_tampered_or_wrong_backup(self):
        """Rollback needs the untouched backup of the removal run, from this source."""
        cleanup = self.copy_and_drop()
        with self.assertRaisesRegex(RuntimeError, 'not the backup of a run that removed'):
            self.restore(self.backup_root / ('backup-' + self.number))
        evidence = json.loads((cleanup / 'evidence.json').read_text())
        original = cleanup / 'evidence.json'
        tampered = self.backup_root / ('tampered-' + self.number)
        tampered.mkdir()
        for name in ('combined.dump', 'SHA256SUMS'):
            (tampered / name).write_bytes((cleanup / name).read_bytes())
        (tampered / 'evidence.json').write_text(json.dumps(dict(evidence, source_identity=['0', 'other'])))
        with self.assertRaisesRegex(RuntimeError, 'not the database this backup was taken from'):
            self.restore(tampered)
        (tampered / 'evidence.json').write_text(original.read_text())
        with (tampered / 'combined.dump').open('ab') as dump:
            dump.write(b'x')
        with self.assertRaisesRegex(RuntimeError, 'does not match its recorded checksum'):
            self.restore(tampered)
        self.assertIsNone(self.app_count('conversion_source'))


    def test_initializes_an_empty_app_database_as_the_app_role(self):
        """An empty app database gets the app schema, owned by the app role, only with the copy."""
        empty = 'empty_' + self.number
        with psycopg.connect(self.admin, autocommit=True) as admin:
            if not admin.execute("SELECT 1 FROM pg_roles WHERE rolname='econgraph'").fetchone():
                admin.execute('CREATE ROLE econgraph LOGIN')
            admin.execute(sql.SQL('CREATE DATABASE {}').format(sql.Identifier(empty)))
        with self.services.open('a') as services:
            services.write(f'[conversion_empty]\nhost=127.0.0.1\nport={self.port}\nuser=postgres\ndbname={empty}\n')
        with psycopg.connect(service='conversion_empty') as db:
            # As app-postgres-init.yaml does.
            db.execute(sql.SQL('GRANT ALL PRIVILEGES ON DATABASE {} TO econgraph').format(sql.Identifier(empty)))
            db.execute('ALTER SCHEMA public OWNER TO econgraph')
        self.args.app_service = 'conversion_empty'
        with self.assertRaisesRegex(RuntimeError, 'Target table inventory is not exactly the app schema'):
            self.run_conversion()
        self.args.init_app_schema = 'econgraph'
        self.args.dry_run = True
        self.run_conversion()
        with psycopg.connect(service='conversion_empty') as db:
            self.assertEqual(conversion.tables(db), set())
        self.args.dry_run = False
        self.run_conversion()
        with psycopg.connect(service='conversion_empty') as db:
            owners = {r[0] for r in db.execute("SELECT DISTINCT tableowner FROM pg_tables WHERE schemaname='public'")}
            self.assertEqual(owners, {'econgraph'})
            self.assertEqual(conversion.versions(db), conversion.inventory('app')[2])
            self.assertEqual(db.execute('SELECT count(*) FROM users').fetchone()[0], 1)
        evidence = json.loads((self.args.backup_dir / 'evidence.json').read_text())
        self.assertTrue(evidence['app_schema_initialized'])
        self.args.app_service = 'conversion_app'
        self.args.init_app_schema = 'no_such_role'
        with psycopg.connect(self.admin, autocommit=True) as admin:
            admin.execute(sql.SQL('DROP DATABASE {}').format(sql.Identifier(empty)))
            admin.execute(sql.SQL('CREATE DATABASE {}').format(sql.Identifier(empty)))
        self.args.app_service = 'conversion_empty'
        self.args.backup_dir = self.backup_root / ('role-' + self.number)
        with self.assertRaisesRegex(RuntimeError, 'Role no_such_role does not exist'):
            self.run_conversion()


    def test_failure_after_the_removal_commits_still_allows_rollback(self):
        """Evidence marked pending before the commit lets a rollback follow a later failure."""
        self.run_conversion()
        save = conversion.save_evidence
        def full_disk(directory, report):
            if report.get('source_app_removed') is True:
                raise OSError('synthetic full disk')
            return save(directory, report)
        self.args.backup_dir = self.backup_root / ('cleanup-' + self.number)
        self.args.drop_source_app = True
        with patch.object(conversion, 'save_evidence', side_effect=full_disk), \
                patch('sys.stdin.isatty', return_value=True), patch('builtins.input', return_value=self.phrase):
            with self.assertRaises(OSError):
                self.run_conversion()
        self.assertIsNone(self.app_count('conversion_source'))
        evidence = json.loads((self.args.backup_dir / 'evidence.json').read_text())
        self.assertEqual(evidence['source_app_removed'], 'pending')
        self.restore(self.args.backup_dir)
        with psycopg.connect(service='conversion_source') as db:
            self.assertEqual(conversion.fingerprints(db, conversion.inventory('app')[0]), self.app_before)
            self.assertEqual(conversion.versions(db), self.versions_before)

    def test_rollback_refuses_a_different_checkout(self):
        """Tables, enums and versions to restore come from the evidence and must match the checkout."""
        cleanup = self.copy_and_drop()
        inventory = conversion.inventory
        def extra_table(plane):
            tables, enums, versions, baseline = inventory(plane)
            return (tables + ['added_later'], enums, versions, baseline) if plane == 'app' else (tables, enums, versions, baseline)
        with patch.object(conversion, 'inventory', side_effect=extra_table):
            with self.assertRaisesRegex(RuntimeError, 'differ from this checkout'):
                self.restore(cleanup)
        def extra_version(plane):
            tables, enums, versions, baseline = inventory(plane)
            return (tables, enums, versions | {'20990101000000'}, baseline) if plane == 'app' else (tables, enums, versions, baseline)
        with patch.object(conversion, 'inventory', side_effect=extra_version):
            with self.assertRaisesRegex(RuntimeError, 'differ from this checkout'):
                self.restore(cleanup)
        self.assertIsNone(self.app_count('conversion_source'))
        with psycopg.connect(service='conversion_source') as db:
            self.assertEqual(conversion.versions(db), conversion.inventory('data')[2])

    def test_rollback_undoes_a_restore_that_does_not_match_the_evidence(self):
        """A dump whose content differs from the recorded fingerprints leaves the source as it was."""
        cleanup = self.copy_and_drop()
        evidence = json.loads((cleanup / 'evidence.json').read_text())
        mismatch = self.backup_root / ('mismatch-' + self.number)
        mismatch.mkdir()
        for name in ('combined.dump', 'SHA256SUMS'):
            (mismatch / name).write_bytes((cleanup / name).read_bytes())
        evidence['source_app']['users'] = {'rows': 2, 'sha256': '0' * 64}
        (mismatch / 'evidence.json').write_text(json.dumps(evidence))
        with self.assertRaisesRegex(RuntimeError, 'the restore was undone'):
            self.restore(mismatch)
        self.assertIsNone(self.app_count('conversion_source'))
        with psycopg.connect(service='conversion_source') as db:
            self.assertEqual(conversion.versions(db), conversion.inventory('data')[2])
            enums = {r[0] for r in db.execute("SELECT typname FROM pg_type WHERE typnamespace='public'::regnamespace")}
            self.assertFalse(set(conversion.inventory('app')[1]) & enums)
        # The genuine evidence still restores.
        self.restore(cleanup)
        self.assertEqual(self.app_count('conversion_source'), 1)

    def test_conversion_requires_the_source_app_history_of_this_checkout(self):
        """A source missing an app migration this checkout has is a different release: refused."""
        app_only = sorted(conversion.inventory('app')[2] - conversion.inventory('data')[2])
        with psycopg.connect(service='conversion_source') as db:
            db.execute('DELETE FROM __diesel_schema_migrations WHERE version=%s', (app_only[-1],))
        self.args.dry_run = True
        with self.assertRaisesRegex(RuntimeError, 'lacks versions in this checkout'):
            self.run_conversion()


    def test_rollback_never_drops_app_tables_it_did_not_restore(self):
        """A removal that failed before its commit leaves pending evidence; the tables stay put."""
        self.run_conversion()
        save = conversion.save_evidence
        def fail_before_commit(directory, report):
            save(directory, report)
            if report.get('source_app_removed') == 'pending':
                raise OSError('synthetic failure before the commit')
        self.args.backup_dir = self.backup_root / ('cleanup-' + self.number)
        self.args.drop_source_app = True
        with patch.object(conversion, 'save_evidence', side_effect=fail_before_commit), \
                patch('sys.stdin.isatty', return_value=True), patch('builtins.input', return_value=self.phrase):
            with self.assertRaises(OSError):
                self.run_conversion()
        evidence = json.loads((self.args.backup_dir / 'evidence.json').read_text())
        self.assertEqual(evidence['source_app_removed'], 'pending')
        # The removal never committed: the source still has its app tables, now changed.
        with psycopg.connect(service='conversion_source') as db:
            db.execute("UPDATE users SET name='Changed after the failed removal'")
        with self.assertRaisesRegex(RuntimeError, 'nothing recorded'):
            self.restore(self.args.backup_dir)
        with psycopg.connect(service='conversion_source') as db:
            self.assertEqual(db.execute('SELECT name FROM users').fetchone()[0], 'Changed after the failed removal')
            self.assertEqual(conversion.versions(db), self.versions_before)


if __name__ == '__main__':
    unittest.main()
