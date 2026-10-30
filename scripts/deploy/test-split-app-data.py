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
            self.before = conversion.fingerprints(db, conversion.inventory('data')[0])
        self.mapping = self.root / ('source-map-' + self.number + '.json')
        self.mapping.write_text(json.dumps({str(source_id): 'FRED'}))
        self.args = SimpleNamespace(source_service='conversion_source', app_service='conversion_app', source_map=self.mapping,
                                    backup_dir=self.backup_root / ('backup-' + self.number), dry_run=False,
                                    writers_stopped=True, drop_source_app=False, previous_evidence=None)

    def run_conversion(self):
        """Run the maintenance command while keeping synthetic evidence out of test logs."""
        with contextlib.redirect_stdout(io.StringIO()):
            conversion.convert(self.args)

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
        with patch('sys.stdin.isatty', return_value=True), patch('builtins.input', return_value=f'DROP APP TABLES FROM {self.source_db}'):
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
                         ['SHA256SUMS', 'counts.env', 'econ_graph.dump', 'evidence.json', 'source-evidence.json'])
        for path in backup.iterdir():
            self.assertEqual(path.stat().st_mode & 0o777, 0o600, path.name)
        # No VERIFIED marker: the off-site copy of nightly dumps must never select this directory.
        self.assertFalse((backup / 'VERIFIED').exists())
        self.assertEqual((backup / 'counts.env').read_text(),
                         '# Row counts at backup time, compared after a restore.\n'
                         f"SERIES_COUNT={self.before['economic_series']['rows']}\n"
                         f"DATA_POINTS_COUNT={self.before['data_points']['rows']}\n")
        archive = (backup / 'econ_graph.dump').read_bytes()
        self.assertEqual(hashlib.sha256(archive).hexdigest(), evidence['backup_sha256'])
        self.assertEqual((backup / 'SHA256SUMS').read_text(), f"{evidence['backup_sha256']}  econ_graph.dump\n")
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
                raise subprocess.CalledProcessError(1, command)
            return run(command, *args, **kwargs)
        with patch.object(conversion.subprocess, 'run', side_effect=unreadable):
            with self.assertRaises(subprocess.CalledProcessError):
                self.run_conversion()
        # The unverified archive is kept for inspection, never promoted or deleted.
        self.assertEqual(sorted(p.name for p in self.args.backup_dir.iterdir()), ['econ_graph.dump.partial'])
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
        self.assertTrue((self.args.backup_dir / 'econ_graph.dump').is_file())
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
                patch('sys.stdin.isatty', return_value=False):
            with self.assertRaisesRegex(RuntimeError, 'typed confirmation'):
                self.run_conversion()
        with psycopg.connect(service='conversion_source') as db:
            self.assertIn('users', conversion.tables(db))
        with psycopg.connect(service='conversion_app') as db:
            self.assertEqual(db.execute('SELECT count(*) FROM users').fetchone()[0], 1)
        (backup,) = root.iterdir()
        self.assertEqual(backup, self.args.backup_dir)
        self.assertTrue((backup / 'econ_graph.dump').is_file())


if __name__ == '__main__':
    unittest.main()
