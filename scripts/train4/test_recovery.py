#!/usr/bin/env python3
"""Offline lossless/parity/recovery failures; optional real PG18 controls use a private DB."""
from copy import deepcopy
from decimal import Decimal
import importlib.util
import json
import os
import shutil
import sys
from pathlib import Path
import subprocess
import tempfile
import unittest
import uuid

spec = importlib.util.spec_from_file_location('recovery', Path(__file__).with_name('recovery.py'))
r = importlib.util.module_from_spec(spec)
spec.loader.exec_module(r)
D, S, T = [str(uuid.UUID(int=n)) for n in (1, 2, 3)]
TOKEN = str(uuid.UUID(int=4))


def scratch_database_url(admin, name):
    from urllib.parse import urlsplit, urlunsplit, parse_qsl, urlencode, quote
    parts = urlsplit(admin)
    # libpq URI values percent-decode; a literal '+' is not form-urlencoded space.
    parameters = parse_qsl(parts.query.replace('+', '%2B'), keep_blank_values=True)
    query = urlencode([(key, value) for key, value in parameters if key != 'dbname'], quote_via=quote)
    return urlunsplit((parts.scheme, parts.netloc, '/' + name, query, ''))


def fixture(path, values=None, target=T, checkpoint='source-checkpoint', attrs=None):
    values = values if values is not None else ['123456789012345678901234567890.1234567890123456789', None, '0.00000000000000000000000000000000000000000000000001']
    header = {'format': 1, 'kind': 'snapshot', 'target_id': target, 'checkpoint': checkpoint,
              'datasets': [{'id': D, 'units': 'USD', 'measures': ['value']}],
              'series': [{'id': S, 'dataset_id': D, 'external_id': 'legacy-id', 'units': 'USD'}]}
    rows = [{'id': str(uuid.UUID(int=i+10)), 'series_id': S, 'dataset_id': D,
             'date': '2024-01-01', 'revision_date': f'2024-02-{i+1:02}',
             'is_original_release': i == 0, 'value': value,
             'created_at': '2024-01-01T00:00:00.123456Z', 'updated_at': '2024-01-01T00:00:00.123456Z',
             'attributes': attrs} for i, value in enumerate(values)]
    write_snapshot(path, header, rows)
    return header, rows


def write_snapshot(path, header, rows):
    Path(path).write_text('\n'.join(r.exact_json(v) for v in [header, *rows]) + '\n')


class RecoveryContracts(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.source = self.root / 'source.jsonl'
        self.header, self.rows = fixture(self.source)

    def tearDown(self):
        self.tmp.cleanup()

    def test_cli_never_forwards_failed_subprocess_output_or_resource_paths(self):
        marker = 'review-secret-fragment'
        tools = self.root / 'tools'; tools.mkdir()
        failing = tools / 'psql'
        failing.write_text(f'#!{sys.executable}\nimport sys\nprint("{marker}")\nprint("invalid percent-encoded token: {marker}", file=sys.stderr)\nsys.exit(7)\n')
        failing.chmod(0o755)
        script = Path(__file__).with_name('recovery.py')
        environment = dict(os.environ, PATH=str(tools), REVIEW_DSN='postgresql://localhost/db?password='+marker)
        control = ['control','initialize','--target-id',T,'--label','fixture','--apply-target',T,'--database-env','REVIEW_DSN']
        self.header['fence'] = {'target_id':T,'backend':'postgres','epoch':1,'writes_paused':True,'pause_token':TOKEN}
        write_snapshot(self.source,self.header,self.rows)
        backfill = ['backfill',str(self.source),'--state',str(self.root/'journal'), '--destination',T,'--apply-target',T]
        replay = ['replay-postgres',str(self.source),'--target-id',T,'--pause-token',TOKEN,'--epoch','1','--apply-target',T,'--database-env','REVIEW_DSN']
        cases = [(control,'Postgres transaction failed (exit 7)'),
                 (replay,'replay transaction failed (exit 7)'),
                 (backfill+['--binary',str(failing)],'recovery subprocess failed (exit 7)'),
                 (backfill+['--binary',str(self.root/marker)],'unavailable resource')]
        for arguments, expected in cases:
            with self.subTest(arguments=arguments[0], expected=expected):
                result = subprocess.run([sys.executable,str(script),*arguments],env=environment,text=True,capture_output=True,timeout=10)
                self.assertNotEqual(result.returncode,0)
                self.assertIn(expected,result.stderr)
                self.assertNotIn(marker,result.stdout+result.stderr)
        # A generic parsing exception must also avoid interpolating untrusted input.
        self.source.write_text(marker)
        result = subprocess.run([sys.executable,str(script),'inventory',str(self.source)],text=True,capture_output=True,timeout=10)
        self.assertNotEqual(result.returncode,0)
        self.assertIn('invalid input',result.stderr)
        self.assertNotIn(marker,result.stdout+result.stderr)

    def test_cli_checkpoint_wait_timeouts_never_print_connection_credentials(self):
        script = Path(__file__).with_name('recovery.py')
        marker = 'review-secret-timeout'
        driver = '''import io, json, os, runpy, subprocess, sys
from pathlib import Path
from unittest.mock import patch
mode, script, output, target, token = sys.argv[1:]
class Keeper:
    def __init__(self, command, **kwargs):
        self.command = command
        self.stdin = io.StringIO()
        self.stdout = io.StringIO('invalid-snapshot\\n' if mode == 'cleanup' else '00000001-00000001-1\\n')
        self.stderr = io.StringIO()
    def poll(self): return None if mode == 'cleanup' else 0
    def terminate(self): pass
    def wait(self, timeout):
        expected = 5 if mode == 'cleanup' else 30
        assert timeout == expected, (timeout, expected)
        raise subprocess.TimeoutExpired(self.command, timeout, output=os.environ['REVIEW_DSN'])
def run(command, **kwargs):
    if '--file' in command:
        Path(command[command.index('--file')+1]).write_bytes(b'catalog fixture')
    return subprocess.CompletedProcess(command, 0, json.dumps({'target_id':target,'fence':{'epoch':1},'objects':[]}))
sys.argv = [script, 'capture-bundle', '--output', output, '--target-id', target,
            '--pause-token', token, '--epoch', '1', '--binary', 'fixture-binary',
            '--database-env', 'REVIEW_DSN']
with patch('subprocess.Popen', Keeper), patch('subprocess.run', run):
    runpy.run_path(script, run_name='__main__')
'''
        for mode in ('commit', 'cleanup'):
            with self.subTest(mode=mode):
                output = self.root / mode
                result = subprocess.run([sys.executable, '-c', driver, mode, str(script), str(output), T, TOKEN],
                                        env=dict(os.environ, REVIEW_DSN=f'postgresql://fixture:{marker}@127.0.0.1/synthetic'),
                                        text=True, capture_output=True, timeout=10)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('recovery subprocess timed out', result.stderr)
                self.assertNotIn(marker, result.stdout + result.stderr)
                self.assertNotIn('Traceback', result.stderr)
                self.assertTrue((output / 'INCOMPLETE.json').exists())
                self.assertFalse((output / 'manifest.json').exists())

    def test_real_libpq_malformed_password_diagnostics_are_not_printed(self):
        client = shutil.which('psql')
        if not client:
            self.skipTest('installed psql required for offline URI parser regression')
        script = Path(__file__).with_name('recovery.py')
        marker = 'review-secret-fragment'
        for dsn in (f'postgresql://fixture:{marker}%zz@127.0.0.1:1/db',
                    f'postgresql://127.0.0.1:1/db?password={marker}%zz'):
            # Invalid percent escapes are rejected by libpq before opening a socket.
            raw = subprocess.run([client,'--no-psqlrc','--dbname',dsn],text=True,capture_output=True,timeout=5)
            self.assertNotEqual(raw.returncode,0)
            self.assertIn(marker,raw.stderr, 'the fixture must exercise a credential-bearing diagnostic')
            result = subprocess.run([sys.executable,str(script),'control','initialize','--target-id',T,
                                     '--label','fixture','--apply-target',T,'--database-env','REVIEW_DSN'],
                                    env=dict(os.environ,REVIEW_DSN=dsn),text=True,capture_output=True,timeout=5)
            self.assertNotEqual(result.returncode,0)
            self.assertIn('Postgres transaction failed',result.stderr)
            self.assertNotIn(marker,result.stdout+result.stderr)

    def test_matching_clients_suppress_failed_docker_stdout_and_stderr(self):
        marker = 'review-secret-fragment'
        tools = self.root/'tools'; tools.mkdir()
        docker = tools/'docker'
        docker.write_text(f'#!{sys.executable}\nimport sys\nprint("{marker}")\nprint("{marker}",file=sys.stderr)\nsys.exit(8)\n')
        docker.chmod(0o755)
        helper = Path(__file__).with_name('pg-client.py')
        dump = self.root/'fixture.dump'; dump.write_bytes(b'fixture')
        for operation in ('psql','dump','restore'):
            output = self.root/'output.dump'
            args = [sys.executable,str(helper),operation,'--dbname','postgres://fixture:secret@127.0.0.1/eg_train4_recovery_probe']
            if operation == 'dump': args += ['--file',str(output)]
            if operation == 'restore': args += [str(dump)]
            result = subprocess.run(args,text=True,capture_output=True,timeout=5,
                                    env=dict(os.environ,PATH=str(tools),TRAIN4_TEST_PG_CONTAINER='fixture'))
            self.assertEqual(result.returncode,8)
            self.assertIn('failed (exit 8)',result.stderr)
            self.assertNotIn(marker,result.stdout+result.stderr)

    def test_scratch_dsn_keeps_spaces_literal_plus_and_explicit_empty_options(self):
        from urllib.parse import urlsplit, parse_qsl
        source='postgresql://fixture:secret@127.0.0.1:5448/admin?options=-c%20statement_timeout%3D12s&application_name=review+probe%20space&connect_timeout=5&dbname=wrong&client_encoding='
        rebuilt=scratch_database_url(source,'eg_train4_recovery_probe')
        parsed=urlsplit(rebuilt)
        self.assertEqual(parsed.path,'/eg_train4_recovery_probe')
        self.assertEqual(parsed.netloc,urlsplit(source).netloc)
        self.assertNotIn('+',parsed.query)
        self.assertEqual(parse_qsl(parsed.query,keep_blank_values=True),[
            ('options','-c statement_timeout=12s'),('application_name','review+probe space'),
            ('connect_timeout','5'),('client_encoding','')])

    def test_failed_fixture_migration_cleans_only_its_owned_database(self):
        from unittest.mock import patch
        case = PostgresRecoveryContracts()
        with patch.dict(os.environ, TRAIN4_RECOVERY_TEST_DATABASE_URL='postgres://fixture/admin',
                        RECOVERY_TEST_CASE_DSN='retained-parent-value'):
            with patch.object(subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, stderr='')) as process:
                with patch.object(r, 'pg_run', side_effect=r.Refused('fixture migration failed')):
                    with self.assertRaisesRegex(r.Refused, 'fixture migration failed'):
                        case.setUp()
                case.doCleanups()
            self.assertEqual(os.environ['RECOVERY_TEST_CASE_DSN'], 'retained-parent-value')
        commands = [call.args[0] for call in process.call_args_list]
        self.assertEqual(len(commands), 2)
        self.assertEqual(commands[0][-1], 'CREATE DATABASE ' + case.name)
        self.assertEqual(commands[1][-1], 'DROP DATABASE ' + case.name + ' WITH (FORCE)')
        self.assertFalse(case.root.exists())

    def test_backfill_normalizes_confirmation_before_any_bootstrap(self):
        from unittest.mock import patch
        destination='e447c889-a5e3-44ae-a567-d35fd1b55e9d'
        self.header['target_id']=destination
        self.header['fence']={'target_id':destination,'backend':'postgres','writes_paused':True,'pause_token':TOKEN,'epoch':1}
        write_snapshot(self.source,self.header,self.rows)
        def importer(command, **options):
            self.assertEqual(command[command.index('--expect-target')+1],destination)
            if command[1]=='bootstrap':
                receipt={'target_id':destination,'epoch':1,'bootstrapped':True}
            else:
                request=json.loads(options['input'])
                self.assertEqual(request['target_id'],destination)
                receipt={key:value for key,value in request.items() if key!='observations'}
                receipt['committed']=True
            return subprocess.CompletedProcess(command,0,stdout=json.dumps(receipt),stderr='')
        for index,confirmation in enumerate((destination.upper(),'{'+destination.upper()+'}')):
            with self.subTest(confirmation=confirmation):
                with patch.object(r.subprocess,'run',side_effect=importer) as process:
                    result=r.backfill(self.source,self.root/f'journal-{index}.json',destination,2,'storage-bridge',confirmation)
                    self.assertEqual(result['committed_batches'],2)
                    self.assertEqual(process.call_count,3)
        with patch.object(r.subprocess,'run') as process:
            with self.assertRaisesRegex(r.Refused,'exact destination'):
                r.backfill(self.source,self.root/'wrong-target.json',destination,2,'storage-bridge',str(uuid.uuid4()))
            process.assert_not_called()

    def test_replay_refuses_foreign_snapshot_before_planning_or_database_access(self):
        from unittest.mock import patch
        # Shared dataset/series UUIDs cannot substitute for the snapshot's durable target.
        fixture(self.source, target=str(uuid.uuid4()))
        for confirmation in (None, T):
            with self.subTest(confirmation=confirmation):
                with patch.object(r.subprocess, 'run') as process, patch.object(r, 'database_url') as database:
                    with self.assertRaisesRegex(r.Refused, 'snapshot target'):
                        r.replay_postgres(self.source, T, TOKEN, 1, 'NOT_CONFIGURED', confirmation)
                    process.assert_not_called()
                    database.assert_not_called()

    def test_replay_uses_frontend_copy_eof_with_sql_separate_from_csv(self):
        import csv
        import io
        from unittest.mock import patch
        attributes={'footnote':"  Café Ω\n\\.\nCOMMIT; 'quoted'  "}
        fixture(self.source,attrs=attributes)
        def inspect_transport(command, **options):
            sql=[command[i+1] for i,value in enumerate(command) if value=='--command']
            self.assertEqual(len(sql),3)
            self.assertTrue(sql[0].startswith('BEGIN ISOLATION LEVEL READ COMMITTED;'))
            self.assertIn('pg_advisory_xact_lock',sql[0])
            self.assertEqual(sql[1],r'\copy recovery_replay(payload) FROM pstdin WITH (FORMAT csv)')
            self.assertTrue(sql[2].rstrip().endswith('COMMIT;'))
            payload=options['stdin'].read()
            self.assertNotIn('\\.\n',payload.splitlines(keepends=True), 'EOF, never an inline data terminator')
            rows=[json.loads(row[0]) for row in csv.reader(io.StringIO(payload))]
            self.assertEqual(len(rows),3)
            self.assertTrue(all(row['attributes']==attributes for row in rows))
            return subprocess.CompletedProcess(command,0,stdout='',stderr='')
        with patch.dict(os.environ,RECOVERY_PROTOCOL_TEST_URL='postgres://not-contacted/scratch'):
            with patch.object(r.subprocess,'run',side_effect=inspect_transport) as process:
                self.assertTrue(r.replay_postgres(self.source,T,TOKEN,1,'RECOVERY_PROTOCOL_TEST_URL',T)['committed'])
                self.assertEqual(process.call_count,1,'one psql connection must own the entire fenced transaction')

    def test_matching_clients_reject_other_databases_and_uri_routing_overrides(self):
        helper=Path(__file__).with_name('pg-client.py')
        for suffix in ('production','eg_train4_recovery_x?dbname=production','eg_train4_recovery_x?host=other-server'):
            output=self.root/'must-not-exist.dump'
            result=subprocess.run(['python3',str(helper),'dump','--dbname','postgres://fixture:secret@127.0.0.1/'+suffix,'--file',str(output)],
                                  text=True,capture_output=True,env=dict(os.environ,TRAIN4_TEST_PG_CONTAINER='not-contacted'))
            self.assertNotEqual(result.returncode,0)
            self.assertFalse(output.exists())
            self.assertNotIn('secret',result.stderr)

    def test_exact_decimal_and_equivalent_timestamp_representation(self):
        destination = self.root / 'destination.jsonl'
        rows = deepcopy(self.rows)
        rows[0]['value'] += '0000'
        rows[0]['created_at'] = '2024-01-01T01:00:00.123456000+01:00'
        write_snapshot(destination, self.header, list(reversed(rows)))
        result = r.parity(self.source, destination)
        self.assertTrue(result['passed'])
        self.assertEqual(result['source']['observation_count'], 3)
        self.assertEqual(result['source']['per_dataset'][D]['count'], 3)
        self.assertEqual(r.decimal_key('1e1000000'), ['1', 1000000])

    def test_null_unknown_attributes_are_not_empty_or_false(self):
        for attrs in ({}, {'preliminary': False}, {'footnotes': []}, {'footnotes': None}):
            destination = self.root / 'destination.jsonl'
            fixture(destination, attrs=attrs)
            self.assertFalse(r.parity(self.source, destination)['passed'])
        a, b = self.root / 'a', self.root / 'b'
        fixture(a, attrs={'unknown': Decimal('12345678901234567890.12345678901234567890')})
        fixture(b, attrs={'unknown': Decimal('12345678901234567890.12345678901234567891')})
        self.assertFalse(r.parity(a, b)['passed'])

    def test_missing_revision_metadata_change_and_identity_duplicates_fail(self):
        destination = self.root / 'destination.jsonl'
        write_snapshot(destination, self.header, self.rows[:-1])
        self.assertFalse(r.parity(self.source, destination)['passed'])
        changed = deepcopy(self.header); changed['datasets'][0]['units'] = 'EUR'
        write_snapshot(destination, changed, self.rows)
        self.assertFalse(r.parity(self.source, destination)['passed'])
        write_snapshot(destination, self.header, self.rows + [self.rows[0]])
        with self.assertRaisesRegex(r.Refused, 'duplicate'):
            r.Snapshot(destination)
        rows = deepcopy(self.rows); rows[0]['value'] = 1
        write_snapshot(destination, self.header, rows)
        with self.assertRaisesRegex(r.Refused, 'decimal strings'):
            r.Snapshot(destination)

    def test_backfill_receipts_resume_after_interrupted_second_batch(self):
        # This simulates the external importer's durable acknowledgment boundary only; actual
        # SQL/Iceberg import/commit behavior is tested with the real storage binary separately.
        binary = self.root / 'importer'
        receipt_dir = self.root / 'receipts'; receipt_dir.mkdir()
        binary.write_text('''#!/usr/bin/env python3
import json,sys
from pathlib import Path
if sys.argv[1]=='bootstrap':
 print(json.dumps({'target_id':sys.argv[sys.argv.index('--expect-target')+1],'epoch':int(sys.argv[sys.argv.index('--epoch')+1]),'bootstrapped':True})); sys.exit(0)
request=json.load(sys.stdin)
root=Path(__file__).parent/'receipts'
receipt={k:v for k,v in request.items() if k!='observations'}
receipt['committed']=True
(root/(request['batch_id']+'.json')).write_text(json.dumps(receipt))
if len(list(root.glob('*.json'))) == 2 and not (root/'interrupted').exists():
 (root/'interrupted').touch(); sys.exit(9)
print(json.dumps(receipt))
''')
        binary.chmod(0o755)
        self.header['fence'] = {'target_id':T,'backend':'postgres','writes_paused':True,'pause_token':TOKEN,'epoch':1}
        write_snapshot(self.source,self.header,self.rows)
        state = self.root / 'journal.json'
        plan = r.backfill(self.source, state, T, 1)
        self.assertEqual(len(plan['pending']), 3)
        self.assertFalse(state.exists(), 'dry run must not claim imported state')
        with self.assertRaises(subprocess.CalledProcessError):
            r.backfill(self.source, state, T, 1, binary, T)
        self.assertEqual(len(r.read_json(state)['receipts']), 1)
        result = r.backfill(self.source, state, T, 1, binary, T)
        self.assertEqual(result['committed_batches'], 3)
        self.assertEqual(len(list(receipt_dir.glob('*.json'))), 3)
        self.assertEqual(r.backfill(self.source, state, T, 1, binary, T)['pending'], [])
        with self.assertRaisesRegex(r.Refused, 'resume source'):
            r.backfill(self.source, state, T, 2, binary, T)

    def make_bundle(self):
        objects = self.root / 'objects'; objects.mkdir()
        (objects / 'table').mkdir()
        (objects / 'table/metadata.json').write_text('{"snapshot":1}')
        (objects / 'table/data.parquet').write_bytes(b'fixture object bytes')
        catalog = self.root / 'catalog.dump'; catalog.write_bytes(b'fixture SQL catalog bytes')
        manifest = self.root / 'reachable.json'
        r.atomic_json(manifest, {'format': 1, 'target_id': T, 'checkpoint': 'catalog-checkpoint',
                      'catalog_sha256': r.file_hash(catalog), 'objects': [
                          {'path': str(p.relative_to(objects)), 'sha256': r.file_hash(p)}
                          for p in sorted(objects.rglob('*')) if p.is_file()]})
        destination = self.root / 'backup'
        r.bundle(catalog, objects, manifest, destination)
        return objects, destination

    def test_consistent_bundle_restores_fresh_objects_and_retains_copy(self):
        objects, backup = self.make_bundle()
        restored = self.root / 'fresh'
        result = r.restore_objects(backup, restored, T)
        self.assertEqual(result['checkpoint'], 'catalog-checkpoint')
        for p in objects.rglob('*'):
            if p.is_file(): self.assertEqual(p.read_bytes(), (restored / p.relative_to(objects)).read_bytes())
        with self.assertRaises(FileExistsError): r.restore_objects(backup, restored, T)
        self.assertTrue((backup / 'catalog.dump').exists())
        with self.assertRaisesRegex(r.Refused, 'exact intended target'):
            r.restore_objects(backup, self.root / 'other', str(uuid.uuid4()))
        (backup / 'objects/table/data.parquet').write_bytes(b'corrupt')
        with self.assertRaisesRegex(r.Refused, 'checksum'):
            r.verify_bundle(backup)

    def test_catalog_changed_between_validation_and_copy_never_publishes_bundle(self):
        from unittest.mock import patch
        import shutil
        objects = self.root / 'objects'; objects.mkdir()
        catalog = self.root / 'catalog.dump'; catalog.write_bytes(b'original checkpoint')
        manifest = self.root / 'reachable.json'
        r.atomic_json(manifest, {'format': 1, 'target_id': T, 'checkpoint': 'c',
                      'catalog_sha256': r.file_hash(catalog), 'objects': []})
        original_copy = shutil.copyfile
        def changed_source(source, destination):
            if Path(source) == catalog:
                catalog.write_bytes(b'different checkpoint during copy')
            return original_copy(source, destination)
        output = self.root / 'incomplete'
        with patch.object(r.shutil, 'copyfile', side_effect=changed_source):
            with self.assertRaisesRegex(r.Refused, 'catalog changed'):
                r.bundle(catalog, objects, manifest, output)
        self.assertTrue((output / 'INCOMPLETE.json').exists())
        self.assertFalse((output / 'manifest.json').exists())
        with self.assertRaisesRegex(r.Refused, 'incomplete'):
            r.verify_bundle(output)

    def test_recovery_paths_do_not_follow_outside_symlinks(self):
        root = self.root / 'objects'; root.mkdir()
        outside = self.root / 'outside'; outside.write_text('must not be copied')
        (root / 'alias').symlink_to(outside)
        with self.assertRaisesRegex(r.Refused, 'symlink'): r.confined(root, 'alias')
        with self.assertRaisesRegex(r.Refused, 'relative'): r.safe_relative('../outside')

    def test_cutover_requires_same_checkpoint_complete_evidence_and_agreed_policy(self):
        request = {'target_id': T, 'checkpoint': 'c', 'action': 'cutover', 'candidate_commit': 'a'*40,
                   'rollback_strategy': 'retained_atomic_postgres_shadow',
                   'fence': {'target_id': T, 'epoch': 1, 'pause_token': TOKEN, 'writes_paused': True},
                   'recovery_policy': {'approved': True, 'approved_by': 'fixture operator',
                    'rollback_window_seconds': 3600,
                    'capacity': {'series': 1, 'observations': 3, 'max_memory_bytes': 1024, 'max_disk_bytes': 4096},
                    **{plane: {'rpo_seconds': 0, 'rto_seconds': 120} for plane in ('observations_raw','app_content','keycloak')}}}
        path = self.root / 'request.json'; r.atomic_json(path, request)
        evidence = []
        for kind in ('parity','restore-drill','rollback-drill','release-e2e','load-capacity'):
            p = self.root / (kind + '.json')
            r.atomic_json(p, {'format': 1, 'kind': kind, 'passed': True, 'target_id': T, 'checkpoint': 'c',
                       'candidate_commit': 'a'*40, 'preserves_post_cutover_writes': kind == 'rollback-drill',
                       'measurements': {'series': 1, 'observations': 3, 'peak_memory_bytes': 512, 'disk_bytes': 2048},
                       'recovery_measurements': {plane: {'rpo_seconds': 0, 'rto_seconds': 60} for plane in ('observations_raw','app_content','keycloak')}})
            evidence.append(p)
        self.assertFalse(r.gate(path, evidence)['apply'])
        broken = r.read_json(evidence[0]); broken['checkpoint'] = 'stale'; r.atomic_json(evidence[0], broken)
        with self.assertRaisesRegex(r.Refused, 'different target/checkpoint/commit'): r.gate(path, evidence)
        request['recovery_policy']['approved'] = False; r.atomic_json(path, request)
        with self.assertRaisesRegex(r.Refused, 'operator agreement'): r.gate(path, evidence)


@unittest.skipUnless(os.environ.get('TRAIN4_RECOVERY_TEST_DATABASE_URL'), 'explicit disposable PostgreSQL admin URL required')
class PostgresRecoveryContracts(unittest.TestCase):
    def setUp(self):
        admin = os.environ['TRAIN4_RECOVERY_TEST_DATABASE_URL']
        self.name = 'eg_train4_recovery_' + uuid.uuid4().hex
        self.admin = admin
        result = subprocess.run(['psql','--no-psqlrc','--dbname',admin,'--set','ON_ERROR_STOP=1','-c','CREATE DATABASE '+self.name], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        # Register ownership cleanup immediately: unittest does not call tearDown
        # when a later migration or fixture setup fails.
        self.addCleanup(subprocess.run, ['psql','--no-psqlrc','--dbname',self.admin,
                        '--set','ON_ERROR_STOP=1','-c','DROP DATABASE '+self.name+' WITH (FORCE)'],
                        check=True, capture_output=True)
        self.url = scratch_database_url(admin, self.name)
        previous = os.environ.get('RECOVERY_TEST_CASE_DSN')
        def restore_case_dsn():
            if previous is None: os.environ.pop('RECOVERY_TEST_CASE_DSN', None)
            else: os.environ['RECOVERY_TEST_CASE_DSN'] = previous
        self.addCleanup(restore_case_dsn)
        os.environ['RECOVERY_TEST_CASE_DSN'] = self.url
        self.variable = 'RECOVERY_TEST_CASE_DSN'
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        repo = Path(__file__).resolve().parents[2]
        sql = ''
        for migration in sorted((repo/'backend/migrations/data').glob('*/up.sql')):
            sql += 'BEGIN;\n'+migration.read_text()+'\nCOMMIT;\n'
        sql += (repo/'backend/crates/econ-graph-core/tests/fixtures/train3/data.sql').read_text()
        r.pg_run(sql,self.variable)

    def control(self, action, **options):
        return json.loads(r.pg_run(r.control_sql(action,T,**options),self.variable))

    def test_rebuilt_dsn_applies_spaced_libpq_options_without_losing_literal_plus(self):
        from urllib.parse import urlsplit, urlunsplit
        parts=urlsplit(self.url)
        configured=urlunsplit((parts.scheme,parts.netloc,parts.path,
                               'options=-c%20statement_timeout%3D12s&application_name=review+probe%20space',''))
        rebuilt=scratch_database_url(configured,self.name)
        result=subprocess.run(['psql','--no-psqlrc','--dbname',rebuilt,'--set','ON_ERROR_STOP=1','-Atc',
                               "SELECT current_setting('statement_timeout') || '|' || current_setting('application_name');"],
                              capture_output=True,text=True,check=True)
        self.assertEqual(result.stdout.strip(),'12s|review+probe space')

    def test_unconfigured_writer_drains_before_target_initialization(self):
        import time
        from concurrent.futures import ThreadPoolExecutor
        command=['psql','--no-psqlrc','--quiet','--tuples-only','--no-align','--set','ON_ERROR_STOP=1','--dbname',self.url]
        writer=subprocess.Popen(command,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        try:
            writer.stdin.write("BEGIN; SELECT assert_timeseries_write_allowed(NULL,'postgres'); SELECT 'held';\n"); writer.stdin.flush()
            self.assertEqual(writer.stdout.readline().strip(),'0')
            self.assertEqual(writer.stdout.readline().strip(),'held')
            with ThreadPoolExecutor() as executor:
                initialized=executor.submit(self.control,'initialize',label='disposable recovery fixture')
                time.sleep(.1)
                self.assertFalse(initialized.done(), 'target initialization must wait for legacy in-flight writers')
                writer.stdin.write('COMMIT;\n'); writer.stdin.close()
                self.assertEqual(writer.wait(timeout=5),0)
                self.assertEqual(initialized.result(timeout=5)['target_id'],T)
            paused=self.control('pause',token=TOKEN)
            self.assertEqual(paused['epoch'],1)
            with self.assertRaisesRegex(r.Refused,'Postgres transaction failed'):
                r.pg_run(f"SELECT assert_timeseries_write_allowed('{T}','postgres');",self.variable)
            with self.assertRaisesRegex(r.Refused,'Postgres transaction failed'):
                self.control('resume',token=TOKEN,epoch=0)
            self.assertFalse(self.control('resume',token=TOKEN,epoch=1)['writes_paused'])
        finally:
            if writer.poll() is None: writer.kill(); writer.wait()
            writer.stdout.close(); writer.stderr.close()
            if not writer.stdin.closed: writer.stdin.close()

    @unittest.skipUnless(os.environ.get('TRAIN4_RECOVERY_BINARY'), 'real Iceberg recovery binary required')
    def test_real_backfill_and_catalog_object_restore_without_original_objects(self):
        from urllib.parse import urlsplit,urlunsplit
        binary=Path(os.environ['TRAIN4_RECOVERY_BINARY'])
        warehouse=self.root/'warehouse'; warehouse.mkdir()
        env=dict(os.environ,DATA_DATABASE_URL=self.url,ICEBERG_WAREHOUSE=str(warehouse))
        previous={key:os.environ.get(key) for key in ('DATA_DATABASE_URL','ICEBERG_WAREHOUSE','RECOVERY_RESTORED_TEST_DSN')}
        os.environ.update({key:env[key] for key in ('DATA_DATABASE_URL','ICEBERG_WAREHOUSE')})
        restore_name='eg_train4_recovery_restore_'+uuid.uuid4().hex
        restore_created=False
        try:
            self.control('initialize',label='disposable real Iceberg recovery')
            self.control('pause',token=TOKEN)
            r.pg_run("""UPDATE data_points SET attributes='{"preliminary":false,"footnotes":[{"code":"P","text":"  Preserve whitespace  "}],"unknown":12345678901234567890.1234567890123456789}'::jsonb WHERE id='30000000-0000-4000-8000-000000000004';""",self.variable)
            source=self.root/'source.jsonl'; r.export_postgres(source,T,self.variable)
            journal=self.root/'backfill.json'
            result=r.backfill(source,journal,T,2,binary,T)
            self.assertEqual(result['committed_batches'],2)
            before=r.pg_run('SELECT count(*) FROM iceberg_commits;',self.variable)
            self.assertEqual(r.backfill(source,journal,T,2,binary,T)['pending'],[])
            self.assertEqual(r.pg_run('SELECT count(*) FROM iceberg_commits;',self.variable),before)
            destination=self.root/'iceberg.jsonl'
            subprocess.run([str(binary),'export-iceberg','--expect-target',T,'--output',str(destination)],check=True,capture_output=True,text=True,env=env)
            self.assertTrue(r.parity(source,destination)['passed'])
            backup=self.root/'verified-backup'
            manifest=r.capture_bundle(backup,T,TOKEN,1,binary,self.variable,os.environ.get('TRAIN4_PG_DUMP','pg_dump'))
            self.assertTrue(manifest['objects'])
            retained=self.root/'original-objects-retained'
            warehouse.rename(retained)
            self.assertFalse(warehouse.exists(), 'the original logical object namespace must be unavailable')
            r.restore_objects(backup,warehouse,T)
            for item in manifest['objects']:
                self.assertNotEqual((warehouse/item['path']).stat().st_ino,(retained/item['path']).stat().st_ino)
            self.assertTrue(retained.exists(), 'restore drill never deletes the original recovery objects')
            subprocess.run(['psql','--no-psqlrc','--dbname',self.admin,'--set','ON_ERROR_STOP=1','-c','CREATE DATABASE '+restore_name],check=True,capture_output=True)
            restore_created=True
            parts=urlsplit(self.url); restored_url=urlunsplit((parts.scheme,parts.netloc,'/'+restore_name,parts.query,''))
            os.environ['RECOVERY_RESTORED_TEST_DSN']=restored_url
            r.restore_catalog(backup,T,'RECOVERY_RESTORED_TEST_DSN',os.environ.get('TRAIN4_PG_RESTORE','pg_restore'))
            restored_export=self.root/'restored-iceberg.jsonl'
            subprocess.run([str(binary),'export-iceberg','--expect-target',T,'--output',str(restored_export)],check=True,capture_output=True,text=True,env=dict(env,DATA_DATABASE_URL=restored_url))
            self.assertTrue(r.parity(source,restored_export)['passed'], 'fresh catalog and copied objects must independently reproduce the complete observations')
            # Synthetic operator receipts below exercise transaction guards only. They are
            # deliberately not reported as production E2E/capacity/recovery acceptance.
            plan={'format':1,'kind':'transition-plan','apply':False,'target_id':T,
                  'action':'cutover','rollback_strategy':'retained_atomic_postgres_shadow',
                  'recovery_policy':{'approved':True},
                  'evidence_sha256':{kind:'0'*64 for kind in ('parity','restore-drill','rollback-drill','release-e2e','load-capacity')},
                  'fence':{'target_id':T,'backend':'postgres','epoch':1,'writes_paused':True,'pause_token':TOKEN},
                  'catalog_checkpoint':manifest['catalog_checkpoint']}
            restored_env=dict(env,DATA_DATABASE_URL=restored_url)
            r.pg_run("UPDATE data_points SET value=42 WHERE id='30000000-0000-4000-8000-000000000004';",'RECOVERY_RESTORED_TEST_DSN')
            refused=subprocess.run([str(binary),'apply-transition','--expect-target',T],input=r.exact_json(plan),capture_output=True,text=True,env=restored_env)
            self.assertNotEqual(refused.returncode,0)
            self.assertIn('parity failed',refused.stderr)
            self.assertEqual(r.pg_run('SELECT backend FROM timeseries_recovery_state;','RECOVERY_RESTORED_TEST_DSN').strip(),'postgres')
            # Repair only the intentionally corrupted disposable row using the real safe
            # missing-row replay path, preserving all other recovered history.
            r.pg_run("DELETE FROM data_points WHERE id='30000000-0000-4000-8000-000000000004';",'RECOVERY_RESTORED_TEST_DSN')
            r.replay_postgres(source,T,TOKEN,1,'RECOVERY_RESTORED_TEST_DSN',T)
            changed=subprocess.run([str(binary),'apply-transition','--expect-target',T],input=r.exact_json(plan),capture_output=True,text=True,check=True,env=restored_env)
            cutover=json.loads(changed.stdout)
            self.assertEqual(cutover['backend'],'iceberg')
            self.assertTrue(cutover['writes_paused'], 'backend changes must not automatically resume mismatched workers')
            plan['action']='rollback'; plan['fence'].update({'backend':'iceberg','epoch':cutover['epoch']})
            changed=subprocess.run([str(binary),'apply-transition','--expect-target',T],input=r.exact_json(plan),capture_output=True,text=True,check=True,env=restored_env)
            self.assertEqual(json.loads(changed.stdout)['backend'],'postgres')
            # A stale receipt cannot mutate even a genuinely recovered paused environment.
            with self.assertRaisesRegex(r.Refused,'entirely fresh'):
                r.restore_catalog(backup,T,'RECOVERY_RESTORED_TEST_DSN',os.environ.get('TRAIN4_PG_RESTORE','pg_restore'))
        finally:
            if restore_created:
                subprocess.run(['psql','--no-psqlrc','--dbname',self.admin,'--set','ON_ERROR_STOP=1','-c','DROP DATABASE '+restore_name+' WITH (FORCE)'],check=True,capture_output=True)
            for key,value in previous.items():
                if value is None: os.environ.pop(key,None)
                else: os.environ[key]=value

    @unittest.skipUnless(os.environ.get('TRAIN4_RECOVERY_BINARY'), 'real Iceberg recovery binary required')
    def test_repeatable_read_default_import_waiting_behind_resume_refuses_stale_pause(self):
        import select
        import time
        from urllib.parse import parse_qsl, quote, urlencode, urlsplit, urlunsplit
        binary = Path(os.environ['TRAIN4_RECOVERY_BINARY'])
        warehouse = self.root/'warehouse'; warehouse.mkdir()
        environment = dict(os.environ,DATA_DATABASE_URL=self.url,ICEBERG_WAREHOUSE=str(warehouse))
        self.control('initialize',label='disposable stale recovery import fixture')
        self.control('pause',token=TOKEN)
        subprocess.run([str(binary),'bootstrap','--expect-target',T,'--pause-token',TOKEN,'--epoch','1'],
                       env=environment,check=True,capture_output=True,timeout=30)
        source = self.root/'source.jsonl'; r.export_postgres(source,T,self.variable)
        with r.Snapshot(source) as snapshot:
            dataset, = snapshot.db.execute('SELECT dataset FROM observations ORDER BY dataset LIMIT 1').fetchone()
            rows = [json.loads(raw,parse_float=Decimal) for raw, in snapshot.db.execute('SELECT raw FROM observations WHERE dataset=? ORDER BY id',(dataset,))]
            request = {'format':1,'batch_id':str(uuid.uuid4()),'target_id':T,
                       'source_checkpoint':snapshot.header['checkpoint'],'dataset_id':dataset,
                       'row_count':len(rows),'content_sha256':r.digest(rows),'observations':rows,
                       'fence':snapshot.header['fence']}
        before = r.pg_run('SELECT count(*) FROM iceberg_commits;',self.variable)
        application = 'recovery_stale_' + uuid.uuid4().hex
        parts = urlsplit(self.url)
        query = parse_qsl(parts.query.replace('+','%2B'),keep_blank_values=True)
        options = ' '.join(value for key,value in query if key == 'options')
        query = [(key,value) for key,value in query if key not in ('options','application_name')]
        query += [('options',options+r' -c default_transaction_isolation=repeatable\ read'),('application_name',application)]
        environment['DATA_DATABASE_URL'] = urlunsplit((parts.scheme,parts.netloc,parts.path,urlencode(query,quote_via=quote),''))
        isolation = subprocess.run(['psql','--no-psqlrc','--dbname',environment['DATA_DATABASE_URL'],
                                    '--set','ON_ERROR_STOP=1','-Atc','SHOW default_transaction_isolation'],
                                   check=True,capture_output=True,text=True,timeout=5)
        self.assertEqual(isolation.stdout.strip(),'repeatable read')
        command = ['psql','--no-psqlrc','--quiet','--tuples-only','--no-align','--set','ON_ERROR_STOP=1','--dbname',self.url]
        holder = subprocess.Popen(command,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        importer = None
        try:
            # Keep resume uncommitted while the importer establishes its first snapshot
            # and waits for this exclusive lock. An inherited RR transaction would retain
            # epoch 1 after the lock is released and wrongly publish a new catalog commit.
            holder.stdin.write(f"BEGIN ISOLATION LEVEL READ COMMITTED; DO $$ BEGIN PERFORM pg_advisory_xact_lock({r.LOCK}); END $$; UPDATE timeseries_recovery_state SET writes_paused=false,pause_token=NULL,epoch=epoch+1 WHERE target_id='{T}'; SELECT 'resume-held';\n")
            holder.stdin.flush()
            ready, _, _ = select.select([holder.stdout],[],[],5)
            self.assertTrue(ready, 'resume holder did not acquire its owned fence within 5 seconds')
            self.assertEqual(holder.stdout.readline().strip(),'resume-held')
            importer = subprocess.Popen([str(binary),'import-batch','--expect-target',T],
                                        env=environment,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
            importer.stdin.write(r.exact_json(request)); importer.stdin.close(); importer.stdin = None
            deadline = time.monotonic()+15
            while time.monotonic() < deadline:
                waiting = r.pg_run("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE application_name="+r.sql_string(application)+" AND wait_event='advisory')::text;",self.variable).strip()
                if waiting == 'true': break
                self.assertIsNone(importer.poll(), 'import must reach the contended advisory lock')
                time.sleep(.05)
            else:
                self.fail('import did not wait on the owned recovery fence within 15 seconds')
            holder.stdin.write('COMMIT;\n'); holder.stdin.close()
            self.assertEqual(holder.wait(timeout=5),0)
            _, error = importer.communicate(timeout=45)
            self.assertNotEqual(importer.returncode,0, 'the old pause must not authorize publication after resume')
            self.assertIn('stale paused import fence',error)
            self.assertEqual(r.pg_run('SELECT count(*) FROM iceberg_commits;',self.variable),before)
            self.assertEqual(r.pg_run("SELECT epoch::text || ':' || writes_paused::text FROM timeseries_recovery_state;",self.variable).strip(),'2:false')
        finally:
            for process in (importer,holder):
                if process is None: continue
                if process.poll() is None: process.kill(); process.wait(timeout=5)
                for stream in (process.stdin,process.stdout,process.stderr):
                    if stream is not None and not stream.closed: stream.close()

    def test_repeatable_read_snapshot_before_pause_cannot_authorize_a_later_write(self):
        self.control('initialize',label='disposable isolation fixture')
        command=['psql','--no-psqlrc','--quiet','--tuples-only','--no-align','--set','ON_ERROR_STOP=1','--dbname',self.url]
        writer=subprocess.Popen(command,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        try:
            writer.stdin.write("BEGIN ISOLATION LEVEL REPEATABLE READ; SELECT writes_paused FROM timeseries_recovery_state;\n"); writer.stdin.flush()
            self.assertEqual(writer.stdout.readline().strip(),'f')
            self.control('pause',token=TOKEN)
            writer.stdin.write(f"SELECT assert_timeseries_write_allowed('{T}','postgres'); COMMIT;\n"); writer.stdin.close()
            self.assertNotEqual(writer.wait(timeout=5),0)
            self.assertIn('READ COMMITTED',writer.stderr.read())
        finally:
            if writer.poll() is None: writer.kill(); writer.wait()
            writer.stdout.close(); writer.stderr.close()
            if not writer.stdin.closed: writer.stdin.close()

    def test_replay_preserves_exact_null_attributes_and_is_idempotent(self):
        self.control('initialize',label='disposable replay fixture')
        self.control('pause',token=TOKEN)
        attributes={'footnotes':[{'code':'P','text':"  Initial release\n\\.\nCOMMIT; 'quoted' Café Ω  "}], 'preliminary':False,'unknown':None}
        r.pg_run("UPDATE data_points SET attributes="+r.sql_string(r.exact_json(attributes))+"::jsonb WHERE id='30000000-0000-4000-8000-000000000004';",self.variable)
        original=self.root/'original.jsonl'
        r.export_postgres(original,T,self.variable)
        clients=['psql']
        if os.environ.get('TRAIN4_TEST_PG_CONTAINER'):
            clients.append(str(Path(__file__).with_name('psql')))
        for index,client in enumerate(clients):
            with self.subTest(client=client):
                version=subprocess.run([client,'--dbname',self.url,'--version'],capture_output=True,text=True,check=True).stdout.strip()
                print('Replay client: '+version)
                r.pg_run("DELETE FROM data_points WHERE id='30000000-0000-4000-8000-000000000005';",self.variable)
                for _ in range(2):
                    receipt=r.replay_postgres(original,T,TOKEN,1,self.variable,T,psql=client)
                    self.assertTrue(receipt['committed'])
                restored=self.root/f'restored-{index}.jsonl'; r.export_postgres(restored,T,self.variable)
                self.assertTrue(r.parity(original,restored)['passed'])
        r.pg_run("UPDATE data_points SET value=42 WHERE id='30000000-0000-4000-8000-000000000004';",self.variable)
        before=self.root/'before-conflict.jsonl'; r.export_postgres(before,T,self.variable)
        with self.assertRaisesRegex(r.Refused,'replay transaction failed'):
            r.replay_postgres(original,T,TOKEN,1,self.variable,T)
        after=self.root/'after-conflict.jsonl'; r.export_postgres(after,T,self.variable)
        self.assertTrue(r.parity(before,after)['passed'], 'rejected replay must not partially change history')


if __name__ == '__main__': unittest.main()
