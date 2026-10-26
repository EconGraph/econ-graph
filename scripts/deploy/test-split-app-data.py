#!/usr/bin/env python3
"""Real PostgreSQL 18 rehearsal, with synthetic records in a disposable container."""
import contextlib
import importlib.util
import hashlib
import io
import json
import os
from pathlib import Path
import socket
import sys
import tarfile
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
DOCKER = ['docker', '--host=unix:///var/run/docker.sock']


class PreflightDiagnostics(unittest.TestCase):
    """Keep maintenance diagnostics useful without echoing arbitrary operator config output."""

    def test_preflight_failure_reports_only_a_fixed_diagnostic(self):
        """Surface missing age while excluding unrelated sensitive stderr and ignoring stdout."""
        failure = subprocess.CompletedProcess([], 1, '/not/a/staging/directory',
                                              'operator-private-value\nage is required for encrypted backups\n')
        with patch.object(conversion.subprocess, 'run', return_value=failure):
            with self.assertRaises(RuntimeError) as raised:
                conversion.encrypted_backup(None, 'unused', Path('/unused'), {})
        self.assertEqual(str(raised.exception),
                         'Backup encryption preflight failed: age is required for encrypted backups')


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
        # Host networking makes the same non-secret libpq services usable by the host
        # Python process and the version-matched pg_dump in this one container.
        subprocess.run(DOCKER + ['run', '-d', '--name', cls.container, '--network=host',
                       '--mount', f'type=bind,src={cls.root},dst={cls.root}',
                       '-e', 'POSTGRES_HOST_AUTH_METHOD=trust',
                       'postgres:18', '-c', 'listen_addresses=127.0.0.1', '-p', str(cls.port)],
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
        cls.identity = cls.root / 'synthetic.agekey'
        subprocess.run(['age-keygen', '-o', str(cls.identity)], check=True, capture_output=True)
        cls.old_recipient = os.environ.get('BACKUP_AGE_RECIPIENT')
        os.environ['BACKUP_AGE_RECIPIENT'] = subprocess.run(['age-keygen', '-y', str(cls.identity)],
            check=True, capture_output=True, text=True).stdout.strip()
        cls.old_path = os.environ['PATH']
        cls.old_service = os.environ.get('PGSERVICEFILE')
        cls.services = cls.root / 'services.conf'
        os.environ['PGSERVICEFILE'] = str(cls.services)
        for tool in ('pg_dump', 'pg_restore'):
            wrapper = cls.root / tool
            wrapper.write_text('#!/bin/sh\nexec docker --host=unix:///var/run/docker.sock exec -i '
                               f'-e PGSERVICEFILE={cls.services} {cls.container} {tool} "$@"\n')
            wrapper.chmod(0o700)
        os.environ['PATH'] = str(cls.root) + ':' + cls.old_path

    @classmethod
    def tearDownClass(cls):
        """Remove only the test container and its synthetic artifacts."""
        subprocess.run(DOCKER + ['rm', '-f', '-v', cls.container], check=False, stdout=subprocess.DEVNULL)
        if hasattr(cls, 'old_path'):
            os.environ['PATH'] = cls.old_path
            if cls.old_recipient is None:
                os.environ.pop('BACKUP_AGE_RECIPIENT', None)
            else:
                os.environ['BACKUP_AGE_RECIPIENT'] = cls.old_recipient
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
                                    backup_dir=self.root / ('backup-' + self.number), dry_run=False,
                                    writers_stopped=True, drop_source_app=False, previous_evidence=None)

    def run_conversion(self):
        """Run the maintenance command while keeping synthetic evidence out of test logs."""
        with contextlib.redirect_stdout(io.StringIO()):
            conversion.convert(self.args)

    def test_copy_cleanup_and_idempotent_rerun(self):
        """Verify locks, exact copy, encrypted recovery, cleanup and evidence-backed rerun."""
        self.args.dry_run = True
        self.run_conversion()
        self.assertFalse(self.args.backup_dir.exists())
        with psycopg.connect(service='conversion_app') as db:
            self.assertEqual(db.execute('SELECT count(*) FROM users').fetchone()[0], 0)
        self.args.dry_run = False
        backup = conversion.encrypted_backup
        def check_locks(*args):
            for service in ('conversion_source', 'conversion_app'):
                with psycopg.connect(service=service) as writer:
                    writer.execute("SET lock_timeout='100ms'")
                    with self.assertRaises(psycopg.errors.LockNotAvailable):
                        writer.execute("UPDATE users SET name='concurrent writer'")
            return backup(*args)
        with patch.object(conversion, 'encrypted_backup', side_effect=check_locks):
            self.run_conversion()
        with psycopg.connect(service='conversion_app') as db:
            self.assertEqual(db.execute('SELECT user_id,series_id FROM chart_annotations').fetchone(), (self.user_id, str(self.series_id)))
        # Reuse neither backup directories nor overwrite a verified dump on rerun.
        self.args.backup_dir = self.root / ('cleanup-' + self.number)
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
        self.assertEqual((self.args.backup_dir / 'backup.tar.age').stat().st_mode & 0o777, 0o600)
        decrypted = subprocess.run(['age', '-d', '-i', str(self.identity),
                                    str(self.args.backup_dir / 'backup.tar.age')],
                                   check=True, capture_output=True).stdout
        with tarfile.open(fileobj=io.BytesIO(decrypted)) as bundle:
            archive = bundle.extractfile('./combined.dump').read()
            self.assertEqual(hashlib.sha256(archive).hexdigest(), evidence['backup_sha256'])
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
        self.assertTrue((self.args.backup_dir / 'backup.tar.age').is_file())
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
        with patch('sys.stdin.isatty', return_value=False):
            with self.assertRaisesRegex(RuntimeError, 'typed confirmation'):
                self.run_conversion()
        with psycopg.connect(service='conversion_source') as db:
            self.assertIn('users', conversion.tables(db))
        with psycopg.connect(service='conversion_app') as db:
            self.assertEqual(db.execute('SELECT count(*) FROM users').fetchone()[0], 1)


if __name__ == '__main__':
    unittest.main()
