#!/usr/bin/env python3
"""Offline lossless/parity/recovery failures; optional real PG18 controls use a private DB."""
from copy import deepcopy
from decimal import Decimal
import importlib.util
import json
import os
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
        from urllib.parse import urlsplit, urlunsplit, parse_qsl, urlencode
        admin = os.environ['TRAIN4_RECOVERY_TEST_DATABASE_URL']
        self.name = 'eg_train4_recovery_' + uuid.uuid4().hex
        self.admin = admin
        result = subprocess.run(['psql','--no-psqlrc','--dbname',admin,'--set','ON_ERROR_STOP=1','-c','CREATE DATABASE '+self.name], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        parts = urlsplit(admin)
        self.url = urlunsplit((parts.scheme,parts.netloc,'/'+self.name,urlencode([(k,v) for k,v in parse_qsl(parts.query) if k!='dbname']),''))
        os.environ['RECOVERY_TEST_CASE_DSN'] = self.url
        self.variable = 'RECOVERY_TEST_CASE_DSN'
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        repo = Path(__file__).resolve().parents[2]
        sql = ''
        for migration in sorted((repo/'backend/migrations/data').glob('*/up.sql')):
            sql += 'BEGIN;\n'+migration.read_text()+'\nCOMMIT;\n'
        # The attributes lane adds the production column; this fixture also runs while that
        # independent migration is not yet integrated in this isolated worktree.
        sql += 'ALTER TABLE data_points ADD COLUMN IF NOT EXISTS attributes jsonb;\n'
        sql += (repo/'backend/crates/econ-graph-core/tests/fixtures/train3/data.sql').read_text()
        r.pg_run(sql,self.variable)

    def tearDown(self):
        subprocess.run(['psql','--no-psqlrc','--dbname',self.admin,'--set','ON_ERROR_STOP=1','-c','DROP DATABASE '+self.name+' WITH (FORCE)'],check=True,capture_output=True)
        self.tmp.cleanup()

    def control(self, action, **options):
        return json.loads(r.pg_run(r.control_sql(action,T,**options),self.variable))

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
            with self.assertRaisesRegex(r.Refused,'paused'):
                r.pg_run(f"SELECT assert_timeseries_write_allowed('{T}','postgres');",self.variable)
            with self.assertRaisesRegex(r.Refused,'stale'):
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
        previous={key:os.environ.get(key) for key in ('DATA_DATABASE_URL','ICEBERG_WAREHOUSE')}
        os.environ.update({key:env[key] for key in previous})
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
        r.pg_run("""UPDATE data_points SET attributes='{"footnotes":[{"code":"P","text":"  Initial release  "}],"preliminary":false,"unknown":null}'::jsonb WHERE id='30000000-0000-4000-8000-000000000004';""",self.variable)
        original=self.root/'original.jsonl'
        r.export_postgres(original,T,self.variable)
        r.pg_run("DELETE FROM data_points WHERE id='30000000-0000-4000-8000-000000000005';",self.variable)
        for _ in range(2):
            receipt=r.replay_postgres(original,T,TOKEN,1,self.variable,T)
            self.assertTrue(receipt['committed'])
        restored=self.root/'restored.jsonl'; r.export_postgres(restored,T,self.variable)
        self.assertTrue(r.parity(original,restored)['passed'])
        r.pg_run("UPDATE data_points SET value=42 WHERE id='30000000-0000-4000-8000-000000000004';",self.variable)
        before=self.root/'before-conflict.jsonl'; r.export_postgres(before,T,self.variable)
        with self.assertRaisesRegex(r.Refused,'conflicts'):
            r.replay_postgres(original,T,TOKEN,1,self.variable,T)
        after=self.root/'after-conflict.jsonl'; r.export_postgres(after,T,self.variable)
        self.assertTrue(r.parity(before,after)['passed'], 'rejected replay must not partially change history')


if __name__ == '__main__': unittest.main()
