"""Offline regressions for the temporary FRED count-only audit; no credentials/network."""
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[1] / 'audit-fred-selection.py'
SPEC = importlib.util.spec_from_file_location('audit_fred', SCRIPT)
AUDIT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(AUDIT)
DATA = SCRIPT.parents[1] / 'backend/crates/econ-graph-crawler/data'


class CountAuditTests(unittest.TestCase):
    def test_redacts_literal_and_query_values_before_artifacts(self):
        secret = 'test-only-secret'
        raw = (f'literal {secret} https://x/?api_key=other-value&x=ok '
               'API_KEY=third%20value\napi%5Fkey%3Dencoded-value&ok=1').encode()
        clean = AUDIT.redact(raw, secret)
        for private in (secret.encode(), b'other-value', b'third%20value', b'encoded-value'):
            self.assertNotIn(private, clean)
        self.assertIn(b'&x=ok', clean)
        self.assertIn(b'&ok=1', clean)

    def test_child_environment_excludes_unrelated_credentials_and_databases(self):
        with patch.dict(os.environ, {'DATABASE_URL': 'secret-db', 'DATA_DATABASE_URL': 'db2',
                                     'APP_DATABASE_URL': 'db3', 'GITHUB_TOKEN': 'token',
                                     'GH_TOKEN': 'token2', 'BLS_API_KEY': 'bls',
                                     'HTTP_PROXY': 'private-proxy', 'HOME': '/private-home'}):
            env = AUDIT.child_environment(Path('/scratch'), 'fake-key')
        self.assertEqual(set(env), {'PATH', 'LANG', 'RUST_LOG', 'CRAWLER_DATA_DIR', 'FRED_API_KEY'})
        self.assertEqual(env['FRED_API_KEY'], 'fake-key')

    def test_copied_guard_only_and_original_unchanged(self):
        before = (DATA / 'catalog_selection/fred.toml').read_bytes()
        with tempfile.TemporaryDirectory() as temporary:
            original, copied = AUDIT.copy_config(DATA, Path(temporary) / 'data')
        self.assertEqual(before, original)
        self.assertEqual((DATA / 'catalog_selection/fred.toml').read_bytes(), before)
        self.assertEqual(copied, before.replace(b'max_catalog_series = 20000',
                                                b'max_catalog_series = 100000'))

    def test_actual_child_statuses_preserved_and_output_sanitized(self):
        for status in (0, 1, 2, 3, 64, 124):
            with self.subTest(status=status), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                binary = root / 'crawler'
                binary.write_text(f'#!{sys.executable}\nimport os,sys\n'
                                  'print(os.environ["FRED_API_KEY"])\n'
                                  'print("https://x/?api_key=another-secret", file=sys.stderr)\n'
                                  f'sys.exit({status})\n')
                binary.chmod(0o700)
                output = root / 'sanitized'
                actual = AUDIT.run(binary, DATA, output, 'fake-private-key', 'test-revision')
                self.assertEqual(actual, status)
                self.assertEqual((output / 'exit-status.txt').read_text(), f'{status}\n')
                for artifact in output.iterdir():
                    self.assertNotIn(b'fake-private-key', artifact.read_bytes())
                    self.assertNotIn(b'another-secret', artifact.read_bytes())
                self.assertEqual(json.loads((output / 'provenance.json').read_text())['exit_status'], status)

    def test_timeout_is_124_and_partial_output_is_retained_sanitized(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / 'crawler'
            binary.write_text("#!/bin/sh\nprintf '%s\\n' \"$FRED_API_KEY\"\nexec /bin/sleep 30\n")
            binary.chmod(0o700)
            output = root / 'sanitized'
            self.assertEqual(AUDIT.run(binary, DATA, output, 'fake-private-key', 'test', 1), 124)
            self.assertEqual((output / 'stdout.json').read_bytes(), b'[REDACTED]\n')
            self.assertEqual((output / 'exit-status.txt').read_text(), '124\n')

    def test_missing_key_is_3_without_starting_child(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(AUDIT, 'capture') as capture:
            output = Path(temporary) / 'sanitized'
            self.assertEqual(AUDIT.run(Path('/nonexistent'), DATA, output, '', 'test'), 3)
            capture.assert_not_called()

    def test_setup_failure_is_nonzero_and_does_not_stringify_exception(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(
                AUDIT, 'copy_config', side_effect=ValueError('fake-private-key')):
            output = Path(temporary) / 'sanitized'
            self.assertEqual(AUDIT.run(Path('/nonexistent'), DATA, output,
                                       'fake-private-key', 'test'), 2)
            self.assertNotIn(b'fake-private-key', (output / 'stderr.txt').read_bytes())


if __name__ == '__main__':
    unittest.main()
