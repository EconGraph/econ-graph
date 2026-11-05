"""Offline regressions for the temporary FRED count-only audit; no credentials/network."""
import importlib.util
import hashlib
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
HISTORY = SCRIPT.parents[1] / 'scripts/audit-fixtures/fred-historical-1533.csv'
REVISION = 'a' * 40


class CountAuditTests(unittest.TestCase):
    def test_run_artifacts_mask_recoverable_escaped_credentials_preserving_json(self):
        key = '0123456789abcdef0123456789abcdef'
        full = ''.join(f'\\u{ord(char):04x}' for char in key)
        mixed = ''.join(f'\\u{ord(char):04x}' if i % 2 else char
                        for i, char in enumerate(key))
        percent = ''.join(f'%{ord(char):02X}' if i % 2 else char
                          for i, char in enumerate(key))
        full_percent = ''.join(f'%{ord(char):02x}' for char in key)
        # The first two payloads use JSON escapes, while the nested forms contain
        # literal backslash-u spellings after the outer JSON is parsed.
        forms = ['"' + full + '"', '"' + mixed + '"', json.dumps(full),
                 json.dumps(full.replace('\\', '\\\\')), json.dumps(percent),
                 json.dumps(full_percent)]
        for encoded in forms:
            with self.subTest(encoded=encoded), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                payload = ('{"error":' + encoded + ',"count":12345,'
                           '"ratio":0.12345678901234567890123456789,'
                           '"public":"snowman \\u2603; literal \\\\u0030"}').encode()
                # Exercise run's entire artifact write path, for both streams.
                with patch.object(AUDIT, 'capture', return_value=(1, payload, payload, .01)):
                    self.assertEqual(AUDIT.run(Path('/fake-crawler'), DATA, root / 'out',
                                               key, 'test-revision'), 1)
                for name in ('stdout.json', 'stderr.txt'):
                    artifact = (root / 'out' / name).read_bytes()
                    parsed = json.loads(artifact)
                    self.assertEqual(parsed['error'], '[REDACTED]')
                    self.assertEqual(parsed['count'], 12345)
                    self.assertEqual(parsed['public'], 'snowman ☃; literal \\u0030')
                    self.assertIn(b'0.12345678901234567890123456789', artifact)
                    self.assertIn(b'"public":"snowman \\u2603; literal \\\\u0030"', artifact)
                    self.assertNotIn(key.encode(), artifact)

    def test_json_escaped_query_name_is_masked_without_invalidating_json(self):
        raw = b'{"error":"https://x/?api\\u005fkey=other-value&x=ok","count":83}'
        parsed = json.loads(AUDIT.redact(raw, 'fake-secret'))
        self.assertEqual(parsed, {'error': 'https://x/?api_key=[REDACTED]&x=ok', 'count': 83})

    def test_plain_text_nested_and_percent_key_spellings_are_masked(self):
        key = '0123456789abcdef0123456789abcdef'
        encoded = ''.join('\\\\u%04X' % ord(char) if i % 2 else '%%%02x' % ord(char)
                          for i, char in enumerate(key))
        self.assertEqual(AUDIT.redact(('error: ' + encoded + ' public:42').encode(), key),
                         b'error: [REDACTED] public:42')

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


class ExportAuditTests(unittest.TestCase):
    def export(self, root, mutate=None, child_status=0, complete=True):
        """Real on-disk exporter output, with process-only stand-in (no network)."""
        def child(command, env, timeout):
            self.assertEqual(command[1:], ['--exact', AUDIT.EXPORT_TEST, '--ignored', '--nocapture'])
            self.assertEqual(set(env), {'PATH', 'LANG', 'RUST_LOG', 'CRAWLER_DATA_DIR',
                                       'FRED_API_KEY', 'FRED_AUDIT_OUTPUT_DIR',
                                       'FRED_AUDIT_HISTORICAL_CSV', 'FRED_AUDIT_REVISION'})
            self.assertEqual(hashlib.sha256(Path(env['FRED_AUDIT_HISTORICAL_CSV']).read_bytes()).hexdigest(),
                             AUDIT.HISTORY_SHA256)
            directory = Path(env['FRED_AUDIT_OUTPUT_DIR'])
            directory.mkdir()
            raw = b'{ "count": 83, "data": ["public"] }\n'
            normalized = b'{"count":83,"data":["public"]}'
            (directory / 'release-53-0.response-text.json').write_bytes(raw)
            (directory / 'release-53-0.normalized.json').write_bytes(normalized)
            manifest = {'format': 1, 'revision': REVISION, 'complete': complete,
                        'selection_complete': complete, 'retrieval_complete': complete,
                        'witness_bindings_accepted': complete,
                        'rules_sha256': hashlib.sha256(Path(env['CRAWLER_DATA_DIR'],
                                                           'catalog_selection/fred.toml').read_bytes()).hexdigest(),
                        'historical': {'sha256': AUDIT.HISTORY_SHA256},
                        'page': {'raw_path': 'release-53-0.response-text.json',
                                 'raw_sha256': hashlib.sha256(raw).hexdigest(),
                                 'normalized_path': 'release-53-0.normalized.json',
                                 'normalized_sha256': hashlib.sha256(normalized).hexdigest()}}
            if mutate:
                mutate(directory, manifest)
            (directory / 'manifest.json').write_text(json.dumps(manifest))
            return child_status, b'fake-private-key', b'api_key=another-secret', .01
        output = root / 'out'
        with patch.object(AUDIT, 'capture', side_effect=child):
            status = AUDIT.run_exporter(Path('/fake-test'), DATA, output, 'fake-private-key',
                                        REVISION, HISTORY)
        return status, output, json.loads((output / 'runner-report.json').read_bytes())

    def test_safe_immutable_files_hashes_complete_and_sanitized_logs(self):
        with tempfile.TemporaryDirectory() as temporary:
            status, output, report = self.export(Path(temporary))
            self.assertEqual(status, 0)
            self.assertTrue(report['complete'])
            self.assertEqual(report['artifact_validation_issues'], [])
            self.assertEqual((output / 'evidence/release-53-0.response-text.json').read_bytes(),
                             b'{ "count": 83, "data": ["public"] }\n')
            for name, digest in report['files_sha256'].items():
                self.assertEqual(hashlib.sha256((output / 'evidence' / name).read_bytes()).hexdigest(), digest)
            self.assertEqual((output / 'stdout.txt').read_bytes(), b'[REDACTED]')
            self.assertEqual((output / 'stderr.txt').read_bytes(), b'api_key=[REDACTED]')

    def test_encoded_secret_omitted_not_rewritten_and_manifest_withheld(self):
        key = 'fake-private-key'
        forms = [key, ''.join(f'\\u{ord(c):04x}' for c in key),
                 ''.join(f'%{ord(c):02X}' for c in key),
                 ''.join('\\\\u%04x' % ord(c) for c in key)]
        for value in forms:
            def mutate(directory, manifest):
                unsafe = json.dumps({'error': value}).encode()
                (directory / 'release-53-0.response-text.json').write_bytes(unsafe)
                manifest['page']['raw_sha256'] = hashlib.sha256(unsafe).hexdigest()
            with self.subTest(value=value), tempfile.TemporaryDirectory() as temporary:
                status, output, report = self.export(Path(temporary), mutate)
                self.assertEqual(status, 2)
                self.assertFalse(report['complete'])
                self.assertIn('unsafe_content', report['artifact_validation_issues'])
                self.assertFalse((output / 'evidence/manifest.json').exists())
                self.assertFalse((output / 'evidence/release-53-0.response-text.json').exists())
                self.assertTrue((output / 'evidence/release-53-0.normalized.json').exists())

    def test_traversal_symlink_unexpected_missing_and_wrong_hash_rejected(self):
        def traversal(directory, manifest):
            manifest['page']['raw_path'] = '../outside.json'
        def symlink(directory, manifest):
            path = directory / 'release-53-0.response-text.json'
            path.unlink()
            path.symlink_to('/etc/passwd')
        def unexpected(directory, manifest):
            (directory / 'other.json').write_text('{}')
        def missing(directory, manifest):
            (directory / 'release-53-0.response-text.json').unlink()
        def wrong_hash(directory, manifest):
            manifest['page']['raw_sha256'] = '0' * 64
        for mutate in (traversal, symlink, unexpected, missing, wrong_hash):
            with self.subTest(case=mutate.__name__), tempfile.TemporaryDirectory() as temporary:
                status, output, report = self.export(Path(temporary), mutate)
                self.assertEqual(status, 2)
                self.assertFalse(report['complete'])
                self.assertTrue(report['artifact_validation_issues'])
                self.assertFalse((output / 'evidence/manifest.json').exists())
                self.assertFalse((output / 'evidence/other.json').exists())

    def test_real_child_status_and_partial_completeness_never_green_wrapped(self):
        for child_status, complete, expected in ((101, False, 101), (124, False, 124),
                                                  (0, False, 2), (101, True, 101)):
            with self.subTest(status=child_status, complete=complete), tempfile.TemporaryDirectory() as temporary:
                status, output, report = self.export(Path(temporary), child_status=child_status,
                                                      complete=complete)
                self.assertEqual(status, expected)
                self.assertFalse(report['complete'])
                self.assertTrue((output / 'evidence/release-53-0.response-text.json').exists())
                self.assertEqual((output / 'exit-status.txt').read_text(), f'{expected}\n')

    def test_actual_process_group_timeout_retains_safe_checkpoint(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / 'export-test'
            # Execute a real local process. It writes partial evidence before
            # blocking; the wrapper must kill it and still inspect those bytes.
            binary.write_text(f'#!{sys.executable}\nimport json,os,time,pathlib\n'
                              'p=pathlib.Path(os.environ["FRED_AUDIT_OUTPUT_DIR"]);p.mkdir()\n'
                              '(p/"release-53-0.response-text.json").write_text("{broken but public")\n'
                              'print(os.environ["FRED_API_KEY"],flush=True)\ntime.sleep(30)\n')
            binary.chmod(0o700)
            output = root / 'out'
            self.assertEqual(AUDIT.run_exporter(binary, DATA, output, 'fake-private-key',
                                               REVISION, HISTORY, 1), 124)
            report = json.loads((output / 'runner-report.json').read_bytes())
            self.assertFalse(report['complete'])
            self.assertEqual((output / 'evidence/release-53-0.response-text.json').read_bytes(),
                             b'{broken but public')
            self.assertEqual((output / 'stdout.txt').read_bytes(), b'[REDACTED]\n')

    def test_wrong_csv_or_revision_refuses_before_child(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(AUDIT, 'capture') as child:
            root = Path(temporary)
            invalid = root / 'invalid.csv'
            invalid.write_text('wrong')
            for index, (revision, history) in enumerate(((REVISION, invalid), ('bad', HISTORY))):
                self.assertEqual(AUDIT.run_exporter(Path('/fake'), DATA, root / str(index),
                                                   'fake-private-key', revision, history), 2)
            child.assert_not_called()


class ScopeCountsTests(unittest.TestCase):
    def scope(self, root, mutation=None, process_status=0):
        before = (DATA / 'catalog_selection/fred.toml').read_bytes()
        def child(command, env, timeout):
            self.assertEqual(command[1:], ['--exact', AUDIT.SCOPE_TEST, '--ignored', '--nocapture'])
            self.assertNotIn('FRED_AUDIT_HISTORICAL_CSV', env)
            self.assertEqual(Path(env['CRAWLER_DATA_DIR'], 'catalog_selection/fred.toml').read_bytes(), before)
            directory = Path(env['FRED_AUDIT_OUTPUT_DIR'])
            directory.mkdir()
            catalogs = {}
            rules = AUDIT.tomllib.loads(before.decode())['rules']
            keys = dict.fromkeys(f'release-{rule["release_id"]}' for rule in rules)
            for key in keys:
                value = {'count': 100858, 'offset': 0, 'limit': 1,
                         'seriess': [{'id': 'PUBLIC', 'title': 'Public', 'frequency': 'Quarterly'}]}
                raw, normalized = json.dumps(value).encode(), json.dumps(value, sort_keys=True).encode()
                raw_name, normalized_name = key + '-0.response-text.json', key + '-0.normalized.json'
                (directory / raw_name).write_bytes(raw)
                (directory / normalized_name).write_bytes(normalized)
                catalogs[key] = {'status': 'header_validated', 'provider_count': 100858,
                                 'projected_page_requests': 101,
                                 'page': {'advertised_count': 100858, 'response_offset': 0,
                                          'response_limit': 1, 'raw_path': raw_name,
                                          'raw_sha256': hashlib.sha256(raw).hexdigest(),
                                          'normalized_path': normalized_name,
                                          'normalized_sha256': hashlib.sha256(normalized).hexdigest()}}
            manifest = {'format': 1, 'mode': 'scope_counts', 'revision': REVISION,
                        'rules_sha256': AUDIT.SCOPE_RULES_SHA256, 'complete': False,
                        'membership_complete': False, 'counts_complete': True,
                        'expected_catalogs': 71, 'validated_catalogs': 71,
                        'production_scan_guard': 20000, 'production_selected_guard': 500,
                        'catalogs': catalogs, 'projected_catalog_rows': 71 * 100858,
                        'projected_catalog_page_requests': 71 * 101}
            if mutation:
                mutation(directory, manifest)
            (directory / 'manifest.json').write_text(json.dumps(manifest))
            return process_status, b'fake-private-key', b'', .01
        with patch.object(AUDIT, 'capture', side_effect=child):
            status = AUDIT.run_exporter(Path('/fake'), DATA, root / 'out', 'fake-private-key',
                                        REVISION, None, scope_counts=True)
        self.assertEqual((DATA / 'catalog_selection/fred.toml').read_bytes(), before)
        report = json.loads((root / 'out/runner-report.json').read_bytes())
        return status, report, root / 'out'

    def test_all71_scope_counts_above_production_guard_succeed_without_membership_claim(self):
        with tempfile.TemporaryDirectory() as temporary:
            status, report, output = self.scope(Path(temporary))
            self.assertEqual(status, 0)
            self.assertTrue(report['counts_complete'])
            self.assertFalse(report['membership_complete'])
            self.assertEqual(report['audit_catalog_guard'], 20000)
            manifest = json.loads((output / 'evidence/manifest.json').read_bytes())
            self.assertFalse(manifest['complete'])
            self.assertEqual(manifest['projected_catalog_rows'], 7160918)
            for name, digest in report['files_sha256'].items():
                self.assertEqual(hashlib.sha256((output / 'evidence' / name).read_bytes()).hexdigest(), digest)

    def test_missing_catalog_wrong_totals_wrong_page_and_wrong_identity_fail(self):
        def missing(directory, manifest):
            manifest['catalogs'].pop(next(iter(manifest['catalogs'])))
        def total(directory, manifest):
            manifest['projected_catalog_rows'] -= 1
        def limit(directory, manifest):
            next(iter(manifest['catalogs'].values()))['page']['response_limit'] = 1000
        def identity(directory, manifest):
            key = next(iter(manifest['catalogs']))
            manifest['catalogs']['release-99999999'] = manifest['catalogs'].pop(key)
        for mutation in (missing, total, limit, identity):
            with self.subTest(case=mutation.__name__), tempfile.TemporaryDirectory() as temporary:
                status, report, output = self.scope(Path(temporary), mutation)
                self.assertEqual(status, 2)
                self.assertFalse(report['counts_complete'])
                self.assertFalse((output / 'evidence/manifest.json').exists())

    def test_partial_header_error_keeps_honest_checkpoint_and_failure(self):
        def partial(directory, manifest):
            next(iter(manifest['catalogs'].values())).update(status='provider_or_pagination')
            manifest.update(counts_complete=False, validated_catalogs=70,
                            projected_catalog_rows=70 * 100858, projected_catalog_page_requests=70 * 101)
        with tempfile.TemporaryDirectory() as temporary:
            status, report, output = self.scope(Path(temporary), partial, 101)
            self.assertEqual(status, 101)
            self.assertFalse(report['counts_complete'])
            self.assertTrue((output / 'evidence/manifest.json').exists())
            self.assertEqual(report['artifact_validation_issues'], [])



if __name__ == '__main__':
    unittest.main()
