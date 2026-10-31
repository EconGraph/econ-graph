#!/usr/bin/env python3
"""Lossless observation parity and reversible recovery tools; mutations require an exact target."""
from __future__ import annotations

import argparse
from contextlib import contextmanager
from datetime import date, datetime, timezone
from decimal import Decimal, InvalidOperation
import fcntl
import hashlib
import json
import os
import re
from pathlib import Path, PurePosixPath
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import uuid

FORMAT = 1
LOCK = 1162303316
NAMESPACE = uuid.UUID('e447c889-a5e3-44ae-a567-d35fd1b55e9d')


class Refused(ValueError):
    pass


def read_json(path):
    return json.loads(Path(path).read_text(), parse_float=Decimal)


def exact_json(value):
    """JSON without float conversion, including arbitrary-precision provider attributes."""
    if isinstance(value, Decimal):
        if not value.is_finite():
            raise Refused('non-finite JSON number')
        return str(value)
    if isinstance(value, dict):
        return '{' + ','.join(json.dumps(k, ensure_ascii=False) + ':' + exact_json(v)
                              for k, v in sorted(value.items())) + '}'
    if isinstance(value, list):
        return '[' + ','.join(exact_json(v) for v in value) + ']'
    if isinstance(value, float):
        raise Refused('binary floating-point value is not a lossless recovery input')
    return json.dumps(value, ensure_ascii=False, separators=(',', ':'), allow_nan=False)


def decimal_key(value):
    try:
        number = Decimal(value)
    except (InvalidOperation, ValueError):
        raise Refused('invalid exact decimal') from None
    if not number.is_finite():
        raise Refused('non-finite observation value')
    sign, digits, exponent = number.as_tuple()
    digits = list(digits)
    while digits and digits[-1] == 0:
        digits.pop()
        exponent += 1
    return [('-' if sign else '') + ''.join(map(str, digits)), exponent] if digits else ['0', 0]


def timestamp_key(value):
    if not isinstance(value, str):
        raise Refused('observation provenance timestamps must be strings')
    match = re.fullmatch(r'(.*T\d{2}:\d{2}:\d{2})(?:\.(\d{1,9}))?(Z|[+-]\d{2}:\d{2})', value)
    if not match:
        raise Refused('invalid observation provenance timestamp')
    base, fraction, zone = match.groups()
    instant = datetime.fromisoformat(base + ('+00:00' if zone == 'Z' else zone))
    return [instant.astimezone(timezone.utc).isoformat(timespec='seconds'), (fraction or '').rstrip('0')]


def canonical(value):
    # Type tags prevent an attribute string/list from colliding with an encoded number.
    if value is None:
        return ['null']
    if isinstance(value, bool):
        return ['bool', value]
    if isinstance(value, (int, Decimal)):
        return ['number', *decimal_key(str(value))]
    if isinstance(value, str):
        return ['string', value]
    if isinstance(value, list):
        return ['array', [canonical(v) for v in value]]
    if isinstance(value, dict):
        return ['object', [[key, canonical(item)] for key, item in sorted(value.items())]]
    raise Refused(f'unsupported JSON type {type(value).__name__}')


def digest(value):
    return hashlib.sha256(exact_json(canonical(value)).encode()).hexdigest()


def file_hash(path):
    result = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for part in iter(lambda: stream.read(1024 * 1024), b''):
            result.update(part)
    return result.hexdigest()


def atomic_json(path, value, *, replace=True):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    handle, temporary = tempfile.mkstemp(prefix='.' + path.name, dir=path.parent)
    try:
        with os.fdopen(handle, 'w') as stream:
            stream.write(exact_json(value) + '\n')
            stream.flush()
            os.fsync(stream.fileno())
        if replace:
            os.replace(temporary, path)
        else:
            os.link(temporary, path)  # Exclusive publication: never replace a backup or batch.
            os.unlink(temporary)
        descriptor = os.open(path.parent, os.O_DIRECTORY)
        try:
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def identity(value):
    try:
        return str(uuid.UUID(value))
    except (ValueError, TypeError, AttributeError):
        raise Refused('invalid stable UUID') from None


class Snapshot:
    """Disk-backed validation/sorting; observation history is not accumulated in memory."""
    def __init__(self, path):
        self.path = Path(path)
        self.temporary = tempfile.TemporaryDirectory(prefix='train4-parity-')
        self.db_path = Path(self.temporary.name) / 'rows.sqlite'
        self.db = sqlite3.connect(self.db_path)
        self.db.execute('CREATE TABLE observations(id TEXT PRIMARY KEY,dataset TEXT,series TEXT,day TEXT,revision TEXT,original INTEGER,hash TEXT,raw TEXT,UNIQUE(series,day,revision,original))')
        try:
            with self.path.open('rb') as stream:
                parsed_hash = hashlib.sha256()
                first = next(stream, None)
                if first is None:
                    raise Refused('empty snapshot')
                parsed_hash.update(first)
                self.header = json.loads(first, parse_float=Decimal)
                if self.header.get('format') != FORMAT or self.header.get('kind') != 'snapshot':
                    raise Refused('unsupported snapshot header')
                if not self.header.get('target_id') or not isinstance(self.header.get('checkpoint'), str) or not self.header['checkpoint']:
                    raise Refused('snapshot needs target identity and a consistent checkpoint')
                self.header['target_id'] = identity(self.header['target_id'])
                self.datasets = {identity(item['id']): item for item in self.header['datasets']}
                self.series = {identity(item['id']): item for item in self.header['series']}
                if len(self.datasets) != len(self.header['datasets']) or len(self.series) != len(self.header['series']):
                    raise Refused('duplicate dataset/series metadata identity')
                for item in self.series.values():
                    if identity(item['dataset_id']) not in self.datasets:
                        raise Refused('series metadata references missing dataset')
                for line in stream:
                    parsed_hash.update(line)
                    if not line.strip():
                        raise Refused('blank observation record')
                    self.add(json.loads(line, parse_float=Decimal))
            self.db.commit()
            self.sha256 = parsed_hash.hexdigest()
            if self.sha256 != file_hash(path):
                raise Refused('source changed while its snapshot was being validated')
        except BaseException:
            self.close()
            raise

    def add(self, row):
        if not isinstance(row, dict):
            raise Refused('observation record must be an object')
        required = {'id', 'dataset_id', 'series_id', 'date', 'revision_date', 'is_original_release',
                    'value', 'attributes', 'created_at', 'updated_at'}
        if not required <= row.keys():
            raise Refused('observation is missing identity/value/attribute/provenance fields')
        row = dict(row)
        for field in ('id', 'series_id', 'dataset_id'):
            row[field] = identity(row[field])
        for field in ('date', 'revision_date'):
            if date.fromisoformat(row[field]).isoformat() != row[field]:
                raise Refused('observation dates must be canonical ISO dates')
        if type(row['is_original_release']) is not bool:
            raise Refused('original release must be a boolean')
        if row['value'] is not None and not isinstance(row['value'], str):
            raise Refused('exact nullable values must use decimal strings or null')
        if row['attributes'] is not None and not isinstance(row['attributes'], dict):
            raise Refused('attributes must retain an object or explicit unknown/null')
        if row['dataset_id'] not in self.datasets or row['series_id'] not in self.series:
            raise Refused('observation references missing catalog metadata')
        if identity(self.series[row['series_id']]['dataset_id']) != row['dataset_id']:
            raise Refused('series/dataset identity mismatch')
        semantic = dict(row)
        for field in ('created_at', 'updated_at'):
            semantic[field] = timestamp_key(row[field])
        if row['value'] is not None:
            semantic['value'] = {'exact_decimal': decimal_key(row['value'])}
        try:
            self.db.execute('INSERT INTO observations VALUES(?,?,?,?,?,?,?,?)',
                            (row['id'], row['dataset_id'], row['series_id'], row['date'],
                             row['revision_date'], int(row['is_original_release']),
                             digest(semantic), exact_json(row)))
        except sqlite3.IntegrityError:
            raise Refused('duplicate observation UUID or series/date/revision/original identity') from None

    def inventory(self):
        overall = hashlib.sha256()
        metadata = {'datasets': sorted(self.datasets.values(), key=lambda v: v['id']),
                    'series': sorted(self.series.values(), key=lambda v: v['id'])}
        overall.update(digest(metadata).encode())
        groups = {}
        for dataset, series, count, first, last in self.db.execute('SELECT dataset,series,count(*),min(day),max(day) FROM observations GROUP BY dataset,series ORDER BY dataset,series'):
            checksum = hashlib.sha256()
            for row_hash, in self.db.execute('SELECT hash FROM observations WHERE series=? ORDER BY id', (series,)):
                checksum.update(row_hash.encode())
                overall.update(row_hash.encode())
            groups[series] = {'dataset_id': dataset, 'count': count, 'first_date': first,
                              'last_date': last, 'sha256': checksum.hexdigest()}
        datasets = {}
        for dataset in sorted(self.datasets):
            members = {k: v for k, v in groups.items() if v['dataset_id'] == dataset}
            datasets[dataset] = {'count': sum(v['count'] for v in members.values()),
                                 'series_count': len(members), 'sha256': digest(members)}
        return {'format': FORMAT, 'kind': 'inventory', 'target_id': self.header['target_id'],
                'checkpoint': self.header['checkpoint'], 'source_sha256': self.sha256,
                'observation_count': sum(v['count'] for v in groups.values()),
                'datasets': len(self.datasets), 'series': len(self.series),
                'content_sha256': overall.hexdigest(), 'per_series': groups, 'per_dataset': datasets}

    def close(self):
        self.db.close()
        self.temporary.cleanup()

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()


def parity(left, right):
    with Snapshot(left) as source, Snapshot(right) as target:
        a, b = source.inventory(), target.inventory()
        source.db.execute('ATTACH DATABASE ? AS other', (str(target.db_path),))
        differences = source.db.execute('''SELECT a.id,'missing_or_changed' FROM observations a
            LEFT JOIN other.observations b ON a.id=b.id WHERE b.id IS NULL OR a.hash<>b.hash
            UNION ALL SELECT b.id,'unexpected' FROM other.observations b
            LEFT JOIN observations a ON a.id=b.id WHERE a.id IS NULL LIMIT 20''').fetchall()
        passed = a['content_sha256'] == b['content_sha256']
        return {'format': FORMAT, 'kind': 'parity', 'passed': passed,
                'target_id': b['target_id'], 'checkpoint': a['checkpoint'],
                'source': a, 'destination': b, 'differences': differences,
                'catalog_checkpoint': target.header.get('catalog_checkpoint')}


@contextmanager
def journal_lock(path):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.with_suffix(path.suffix + '.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        yield


def backfill(source_path, state_path, destination, batch_size, binary=None, apply_target=None):
    destination = identity(destination)
    if apply_target is not None:
        apply_target = identity(apply_target)
        if apply_target != destination or not binary:
            raise Refused('backfill requires the exact destination and real recovery binary')
    if batch_size <= 0:
        raise Refused('batch size must be positive')
    with journal_lock(state_path), Snapshot(source_path) as source:
        fence = source.header.get('fence')
        if apply_target is not None and (not isinstance(fence, dict) or fence.get('target_id') != destination or fence.get('writes_paused') is not True or not fence.get('pause_token') or type(fence.get('epoch')) is not int):
            raise Refused('backfill source must come from the exact paused write checkpoint')
        wanted = {'format': FORMAT, 'source_sha256': source.sha256,
                  'source_checkpoint': source.header['checkpoint'], 'destination': destination,
                  'batch_size': batch_size, 'fence': fence}
        state = read_json(state_path) if Path(state_path).exists() else {**wanted, 'receipts': {}}
        if any(state.get(k) != v for k, v in wanted.items()):
            raise Refused('resume source/checkpoint/target/batch size differs from the saved journal')
        if apply_target is not None:
            bootstrap = subprocess.run([str(binary), 'bootstrap', '--expect-target', destination,
                                       '--pause-token', fence['pause_token'], '--epoch', str(fence['epoch'])],
                                      text=True, capture_output=True, check=True)
            receipt = json.loads(bootstrap.stdout)
            if receipt.get('target_id') != destination or receipt.get('epoch') != fence['epoch'] or receipt.get('bootstrapped') is not True:
                raise Refused('storage did not bootstrap the exact paused target')
        pending = []
        for dataset in sorted(source.datasets):
            cursor = source.db.execute('SELECT raw FROM observations WHERE dataset=? ORDER BY series,id', (dataset,))
            while chunk := cursor.fetchmany(batch_size):
                rows = [json.loads(raw, parse_float=Decimal) for raw, in chunk]
                content_hash = digest(rows)
                batch_id = str(uuid.uuid5(NAMESPACE, exact_json([destination, wanted['source_checkpoint'], dataset, content_hash])))
                request = {'format': FORMAT, 'batch_id': batch_id, 'target_id': destination,
                           'source_checkpoint': wanted['source_checkpoint'], 'dataset_id': dataset,
                           'row_count': len(rows), 'content_sha256': content_hash, 'observations': rows, 'fence': fence}
                if batch_id in state['receipts']:
                    continue
                pending.append({k: v for k, v in request.items() if k != 'observations'})
                if apply_target is None:
                    continue
                # An explicit executable/argv, never a shell interpolation or a mock-success path.
                result = subprocess.run([str(binary), 'import-batch', '--expect-target', destination],
                                        input=exact_json(request), text=True, capture_output=True, check=True)
                receipt = json.loads(result.stdout, parse_float=Decimal)
                if receipt.get('committed') is not True or any(receipt.get(k) != request[k] for k in ('batch_id', 'target_id', 'source_checkpoint', 'dataset_id', 'row_count', 'content_sha256')):
                    raise Refused('storage did not acknowledge the exact durable batch/checkpoint')
                state['receipts'][batch_id] = receipt
                atomic_json(state_path, state)
        if apply_target is not None:
            atomic_json(state_path, state)
        return {**wanted, 'apply': apply_target is not None, 'pending': pending,
                'committed_batches': len(state['receipts'])}


def safe_relative(value):
    path = PurePosixPath(value)
    if not value or path.is_absolute() or '..' in path.parts or '.' in path.parts:
        raise Refused('object paths must be relative and remain inside their own namespace')
    return path


def confined(root, relative):
    root = Path(root).resolve(strict=True)
    path = root.joinpath(*safe_relative(relative).parts)
    # Refuse symlink components rather than copying content from outside a namespace.
    cursor = root
    for part in safe_relative(relative).parts:
        cursor /= part
        if cursor.is_symlink():
            raise Refused('symlink in recovery object path')
    if not path.resolve().is_relative_to(root):
        raise Refused('object escapes its declared namespace')
    return path



def sync_tree(root):
    """Publish a completed local recovery copy only after bytes and directory entries persist."""
    root=Path(root)
    for directory, _, files in os.walk(root, topdown=False):
        for name in files:
            with (Path(directory)/name).open('rb') as stream:
                os.fsync(stream.fileno())
        descriptor=os.open(directory,os.O_DIRECTORY)
        try: os.fsync(descriptor)
        finally: os.close(descriptor)
    descriptor=os.open(root.parent,os.O_DIRECTORY)
    try: os.fsync(descriptor)
    finally: os.close(descriptor)

def bundle(catalog_dump, object_root, reachable_manifest, output):
    manifest = read_json(reachable_manifest)
    if manifest.get('format') != FORMAT or not manifest.get('checkpoint') or not manifest.get('target_id'):
        raise Refused('reachable-object inventory requires a target and catalog checkpoint')
    if manifest.get('catalog_sha256') != file_hash(catalog_dump):
        raise Refused('catalog dump is not the checkpoint inventory\'s verified catalog')
    output = Path(output)
    output.mkdir(parents=True, exist_ok=False, mode=0o700)
    # The inventory must come from the catalog traversal at its consistent exported SQL
    # snapshot. This helper cannot infer Iceberg reachability from arbitrary directory contents.
    try:
        shutil.copyfile(catalog_dump, output / 'catalog.dump')
        if file_hash(output / 'catalog.dump') != manifest['catalog_sha256'] or file_hash(catalog_dump) != manifest['catalog_sha256']:
            raise Refused('catalog changed while its checkpoint recovery copy was being made')
        entries = []
        seen = set()
        for item in manifest['objects']:
            name = str(safe_relative(item['path']))
            if name in seen:
                raise Refused('duplicate reachable object')
            seen.add(name)
            source = confined(object_root, name)
            if file_hash(source) != item['sha256']:
                raise Refused('object changed after catalog inventory')
            target = output / 'objects' / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
            if file_hash(target) != item['sha256']:
                raise Refused('copied object checksum differs')
            entries.append({'path': name, 'sha256': item['sha256'], 'bytes': target.stat().st_size})
        saved = {**manifest, 'objects': sorted(entries, key=lambda i: i['path']),
                 'catalog_sha256': manifest['catalog_sha256']}
        sync_tree(output)
        atomic_json(output / 'manifest.json', saved, replace=False)
        return saved
    except BaseException:
        # Incomplete copies are retained for inspection; never delete a recovery copy.
        atomic_json(output / 'INCOMPLETE.json', {'format': FORMAT, 'complete': False})
        raise


def verify_bundle(path):
    path = Path(path)
    if (path / 'INCOMPLETE.json').exists():
        raise Refused('incomplete recovery copy is not a restore source')
    manifest = read_json(path / 'manifest.json')
    if manifest.get('format') != FORMAT or file_hash(path / 'catalog.dump') != manifest['catalog_sha256']:
        raise Refused('catalog recovery checksum mismatch')
    for item in manifest['objects']:
        if file_hash(confined(path / 'objects', item['path'])) != item['sha256']:
            raise Refused('object recovery checksum mismatch')
    return manifest


def restore_objects(bundle_path, destination, expected_target):
    manifest = verify_bundle(bundle_path)
    if expected_target != manifest['target_id']:
        raise Refused('restore source does not match the exact intended target')
    destination = Path(destination)
    # A restored environment uses the same logical target ID, but an explicitly fresh root.
    # No existing object namespace or production path is overwritten.
    destination.mkdir(parents=True, exist_ok=False)
    for item in manifest['objects']:
        source = confined(Path(bundle_path) / 'objects', item['path'])
        target = destination / item['path']
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
        if file_hash(target) != item['sha256']:
            raise Refused('restored object checksum mismatch')
    sync_tree(destination)
    atomic_json(destination / 'RESTORE.json', {'format': FORMAT, 'target_id': expected_target,
                'checkpoint': manifest['checkpoint'], 'catalog_sha256': manifest['catalog_sha256'],
                'catalog_restore_required': True})
    return manifest


def sql_string(value):
    return "'" + value.replace("'", "''") + "'"


def database_url(variable):
    value = os.environ.get(variable)
    if not value:
        raise Refused(f'{variable} is required; no implicit production/development database')
    return value


def pg_run(sql, database_variable='DATA_DATABASE_URL', *, output=None):
    environment = dict(os.environ, PGDATABASE=database_url(database_variable))
    result = subprocess.run(['psql', '--no-psqlrc', '--quiet', '--tuples-only', '--no-align',
                             '--set', 'ON_ERROR_STOP=1', '--dbname', database_url(database_variable)], input=sql, text=True,
                            stdout=output if output else subprocess.PIPE,
                            stderr=subprocess.PIPE, env=environment)
    if result.returncode:
        raise Refused('Postgres transaction failed; inspect target state before retry: ' +
                      result.stderr.strip().splitlines()[0][:300])
    return result.stdout


def control_sql(action, target_id, label=None, token=None, epoch=None):
    target_id = identity(target_id)
    token = identity(token) if token else None
    if action not in ('initialize', 'pause', 'resume'):
        raise Refused('control action requires the storage-aware transition path')
    if action != 'initialize' and not token:
        raise Refused('pause/resume requires an explicit durable pause token')
    if action == 'resume' and (type(epoch) is not int or epoch < 0):
        raise Refused('resume requires the exact paused epoch')
    sql = f'''BEGIN;
SET LOCAL lock_timeout = '30s';
SET LOCAL standard_conforming_strings = on;
DO $$ BEGIN
    PERFORM pg_advisory_xact_lock({LOCK});
'''
    if action == 'initialize':
        if not label or not label.strip():
            raise Refused('initialize requires an explicit environment label')
        sql += '''    IF EXISTS(SELECT 1 FROM timeseries_recovery_state) THEN
        RAISE EXCEPTION 'recovery target is already initialized';
    END IF;
END $$;
'''
        sql += f"INSERT INTO timeseries_recovery_state(target_id,target_label) VALUES ('{target_id}',{sql_string(label)});\n"
    else:
        sql += f'''    IF NOT EXISTS(SELECT 1 FROM timeseries_recovery_state WHERE target_id='{target_id}') THEN
        RAISE EXCEPTION 'recovery target mismatch';
    END IF;
'''
        if action == 'pause':
            sql += f'''    IF EXISTS(SELECT 1 FROM timeseries_recovery_state WHERE writes_paused AND pause_token<>'{token}') THEN
        RAISE EXCEPTION 'another operator owns the pause';
    END IF;
END $$;
UPDATE timeseries_recovery_state SET writes_paused=true,pause_token='{token}',epoch=epoch+1,updated_at=now()
WHERE target_id='{target_id}' AND NOT writes_paused;
'''
        else:
            sql += f'''    IF NOT EXISTS(SELECT 1 FROM timeseries_recovery_state WHERE writes_paused AND pause_token='{token}' AND epoch={epoch}) THEN
        RAISE EXCEPTION 'stale pause token or epoch';
    END IF;
END $$;
UPDATE timeseries_recovery_state SET writes_paused=false,pause_token=NULL,epoch=epoch+1,updated_at=now()
WHERE target_id='{target_id}';
'''
    evidence = exact_json({'format': FORMAT, 'action': action, 'target_id': target_id,
                           'drains_guarded_writers_only': True})
    sql += f'''INSERT INTO timeseries_recovery_transitions(target_id,epoch,action,evidence)
SELECT target_id,epoch,'{action}',{sql_string(evidence)}::jsonb FROM timeseries_recovery_state
ON CONFLICT(target_id,epoch) DO NOTHING;
SELECT to_jsonb(s)::text FROM timeseries_recovery_state s;
COMMIT;
'''
    return sql


def export_postgres(path, target_id, database_variable='DATA_DATABASE_URL'):
    target_id = identity(target_id)
    sql = f'''BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
SET LOCAL TIME ZONE 'UTC';
DO $$ BEGIN
    PERFORM pg_advisory_xact_lock_shared({LOCK});
    IF NOT EXISTS(SELECT 1 FROM timeseries_recovery_state WHERE target_id='{target_id}') THEN
        RAISE EXCEPTION 'export target mismatch or uninitialized state';
    END IF;
END $$;
SELECT jsonb_build_object('format',1,'kind','snapshot','target_id','{target_id}',
 'checkpoint',pg_current_snapshot()::text,
 'fence',(SELECT to_jsonb(s) FROM timeseries_recovery_state s),
 'datasets',(SELECT coalesce(jsonb_agg(to_jsonb(d) ORDER BY id),'[]'::jsonb) FROM datasets d),
 'series',(SELECT coalesce(jsonb_agg(to_jsonb(s) ORDER BY id),'[]'::jsonb) FROM economic_series s))::text;
SELECT (to_jsonb(p) || jsonb_build_object('dataset_id',s.dataset_id,'value',p.value::text,
 'attributes',to_jsonb(p)->'attributes'))::text
 FROM data_points p JOIN economic_series s ON s.id=p.series_id ORDER BY s.dataset_id,p.series_id,p.id;
COMMIT;
'''
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name('.' + path.name + '.' + uuid.uuid4().hex)
    try:
        with temporary.open('x') as stream:
            pg_run(sql, database_variable, output=stream)
            stream.flush()
            os.fsync(stream.fileno())
        with Snapshot(temporary) as snapshot:
            result = snapshot.inventory()
        os.link(temporary, path)
        return result
    finally:
        if temporary.exists():
            temporary.unlink()


def replay_postgres(source_path, target_id, token, epoch, database_variable='DATA_DATABASE_URL', apply_target=None, psql='psql'):
    """Restore missing post-cutover rows without deleting or overwriting conflicting history."""
    target_id, token = identity(target_id), identity(token)
    if type(epoch) is not int or epoch < 0:
        raise Refused('replay needs the exact paused epoch')
    with Snapshot(source_path) as source:
        if apply_target is None:
            return {'format': FORMAT, 'apply': False, 'target_id': target_id,
                    'source': source.inventory(), 'requires_pause_token': True,
                    'preserves_existing_history': True}
        if identity(apply_target) != target_id:
            raise Refused('replay target confirmation mismatch')
        # Keep SQL commands separate from the disk-backed CSV input. SQL COPY's inline
        # \. marker handling differs across clients; frontend \copy FROM pstdin owns
        # the data stream and ends at EOF, without sending any sentinel as JSON data.
        import csv
        with tempfile.TemporaryFile(mode='w+', encoding='utf-8', newline='') as payload:
            preamble = f'''BEGIN ISOLATION LEVEL READ COMMITTED;
SET LOCAL lock_timeout = '30s';
SET LOCAL standard_conforming_strings = on;
SET LOCAL client_encoding = 'UTF8';
DO $$ BEGIN
 PERFORM pg_advisory_xact_lock({LOCK});
 IF NOT EXISTS(SELECT 1 FROM timeseries_recovery_state WHERE target_id='{target_id}' AND writes_paused AND pause_token='{token}' AND epoch={epoch}) THEN
  RAISE EXCEPTION 'replay target, pause token or epoch mismatch';
 END IF;
END $$;
CREATE TEMP TABLE recovery_replay(payload jsonb NOT NULL) ON COMMIT DROP;
'''
            writer = csv.writer(payload, lineterminator='\n')
            for raw, in source.db.execute('SELECT raw FROM observations ORDER BY id'):
                row = json.loads(raw, parse_float=Decimal)
                if any(len(timestamp_key(row[k])[1]) > 6 for k in ('created_at', 'updated_at')):
                    raise Refused('Postgres replay cannot narrow sub-microsecond provenance')
                writer.writerow([raw])
            postamble = '''DO $$ BEGIN
 IF EXISTS(SELECT 1 FROM recovery_replay r LEFT JOIN economic_series s ON s.id=(r.payload->>'series_id')::uuid
  WHERE s.id IS NULL OR s.dataset_id<>(r.payload->>'dataset_id')::uuid) THEN
  RAISE EXCEPTION 'restore series/dataset metadata before replay';
 END IF;
 IF EXISTS(SELECT 1 FROM recovery_replay r JOIN data_points p ON p.id=(r.payload->>'id')::uuid
  WHERE p.series_id IS DISTINCT FROM (r.payload->>'series_id')::uuid
   OR p.date IS DISTINCT FROM (r.payload->>'date')::date
   OR p.revision_date IS DISTINCT FROM (r.payload->>'revision_date')::date
   OR p.is_original_release IS DISTINCT FROM (r.payload->>'is_original_release')::boolean
   OR p.value IS DISTINCT FROM (r.payload->>'value')::numeric
   OR p.created_at IS DISTINCT FROM (r.payload->>'created_at')::timestamptz
   OR p.updated_at IS DISTINCT FROM (r.payload->>'updated_at')::timestamptz
   OR p.attributes IS DISTINCT FROM NULLIF(r.payload->'attributes','null'::jsonb)) THEN
  RAISE EXCEPTION 'replay conflicts with retained observation history';
 END IF;
END $$;
INSERT INTO data_points(id,series_id,date,revision_date,is_original_release,value,attributes,created_at,updated_at)
SELECT (payload->>'id')::uuid,(payload->>'series_id')::uuid,(payload->>'date')::date,
 (payload->>'revision_date')::date,(payload->>'is_original_release')::boolean,
 (payload->>'value')::numeric,NULLIF(payload->'attributes','null'::jsonb),
 (payload->>'created_at')::timestamptz,(payload->>'updated_at')::timestamptz FROM recovery_replay
ON CONFLICT(id) DO NOTHING;
COMMIT;
'''
            payload.seek(0)
            environment = dict(os.environ, PGDATABASE=database_url(database_variable))
            result = subprocess.run([str(psql), '--no-psqlrc', '--quiet', '--set', 'ON_ERROR_STOP=1', '--dbname', database_url(database_variable),
                                     '--command', preamble,
                                     '--command', r'\copy recovery_replay(payload) FROM pstdin WITH (FORMAT csv)',
                                     '--command', postamble],
                                    stdin=payload, capture_output=True, text=True, env=environment)
            if result.returncode:
                raise Refused('replay transaction failed; inspect target before retry: ' + result.stderr.splitlines()[0][:300])
        return {'format': FORMAT, 'kind': 'postgres-replay', 'committed': True,
                'target_id': target_id, 'epoch': epoch, 'source_sha256': source.sha256,
                'source_checkpoint': source.header['checkpoint'],
                'preserves_existing_history': True}


def gate(request_path, evidence_paths):
    request = read_json(request_path)
    required = {'parity', 'restore-drill', 'rollback-drill', 'release-e2e', 'load-capacity'}
    if request.get('action') not in ('cutover', 'rollback'):
        raise Refused('gate action must be cutover or rollback')
    target = identity(request['target_id'])
    checkpoint = request['checkpoint']
    if not isinstance(request.get('candidate_commit'), str) or not re.fullmatch(r'[0-9a-f]{40}', request['candidate_commit']):
        raise Refused('acceptance requires an exact candidate commit')
    policy = request.get('recovery_policy')
    if not isinstance(policy, dict) or policy.get('approved') is not True or not policy.get('approved_by'):
        raise Refused('recovery/capacity objectives require explicit operator agreement')
    for plane in ('observations_raw', 'app_content', 'keycloak'):
        objective = policy.get(plane)
        if not isinstance(objective, dict) or any(type(objective.get(k)) is not int or objective[k] < 0 for k in ('rpo_seconds', 'rto_seconds')):
            raise Refused('distinct explicit recovery objectives are required for each plane')
    if type(policy.get('rollback_window_seconds')) is not int or policy['rollback_window_seconds'] <= 0:
        raise Refused('retention/rollback window requires explicit operator agreement')
    capacity = policy.get('capacity', {})
    for key in ('series', 'observations', 'max_memory_bytes', 'max_disk_bytes'):
        if type(capacity.get(key)) is not int or capacity[key] <= 0:
            raise Refused('explicit measured workload and capacity budgets are required')
    if request.get('rollback_strategy') != 'retained_atomic_postgres_shadow':
        raise Refused('rollback must preserve post-cutover writes, not just change a read flag')
    found, hashes = {}, {}
    for path in evidence_paths:
        evidence = read_json(path)
        kind = evidence.get('kind')
        if kind in found:
            raise Refused('duplicate acceptance evidence kind')
        if evidence.get('format') != FORMAT or evidence.get('passed') is not True or evidence.get('target_id') != target or evidence.get('checkpoint') != checkpoint or evidence.get('candidate_commit') != request.get('candidate_commit'):
            raise Refused('acceptance evidence is failed or for a different target/checkpoint/commit')
        found[kind], hashes[kind] = evidence, file_hash(path)
    if not required <= found.keys():
        raise Refused('cutover requires parity, restore, rollback, E2E and measured load/capacity evidence')
    measurements = found['load-capacity'].get('measurements', {})
    for key in ('series', 'observations', 'peak_memory_bytes', 'disk_bytes'):
        if type(measurements.get(key)) is not int or measurements[key] < 0:
            raise Refused('load/capacity evidence needs actual measurements')
    if measurements['series'] < capacity['series'] or measurements['observations'] < capacity['observations'] or measurements['peak_memory_bytes'] > capacity['max_memory_bytes'] or measurements['disk_bytes'] > capacity['max_disk_bytes']:
        raise Refused('measured workload/capacity does not satisfy the agreed policy')
    recovery = found['restore-drill'].get('recovery_measurements', {})
    for plane in ('observations_raw', 'app_content', 'keycloak'):
        for key in ('rpo_seconds', 'rto_seconds'):
            measured = recovery.get(plane, {}).get(key)
            if type(measured) is not int or measured < 0 or measured > policy[plane][key]:
                raise Refused('restore drill does not meet the agreed recovery objectives')
    if found['rollback-drill'].get('preserves_post_cutover_writes') is not True:
        raise Refused('rollback evidence does not preserve writes after cutover')
    fence = request.get('fence', {})
    if fence.get('writes_paused') is not True or fence.get('target_id') != target or type(fence.get('epoch')) is not int or not fence.get('pause_token'):
        raise Refused('cutover needs an exact drained write fence from the actual runtime')
    return {'format': FORMAT, 'kind': 'transition-plan', 'apply': False, 'target_id': target,
            'checkpoint': checkpoint, 'candidate_commit': request['candidate_commit'],
            'action': request['action'], 'fence': fence, 'evidence_sha256': hashes,
            'request_sha256': file_hash(request_path), 'rollback_strategy': request['rollback_strategy'],
            'catalog_checkpoint': found['parity'].get('catalog_checkpoint'),
            'recovery_policy': policy, 'storage_aware_apply_required': True}



def capture_bundle(output, target_id, token, epoch, binary, database_variable='DATA_DATABASE_URL', pg_dump='pg_dump'):
    target_id, token = identity(target_id), identity(token)
    if type(epoch) is not int or epoch < 0:
        raise Refused('consistent recovery copy needs the exact pause epoch')
    output = Path(output)
    output.mkdir(parents=True, exist_ok=False, mode=0o700)
    dsn = database_url(database_variable)
    command = ['psql','--no-psqlrc','--quiet','--tuples-only','--no-align','--set','ON_ERROR_STOP=1','--dbname',dsn]
    keeper = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    try:
        keeper.stdin.write(f"""SET lock_timeout = '30s';
DO $$ BEGIN PERFORM pg_advisory_lock({LOCK}); END $$;
BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
DO $$ BEGIN
 IF NOT EXISTS(SELECT 1 FROM timeseries_recovery_state WHERE target_id='{target_id}' AND writes_paused AND pause_token='{token}' AND epoch={epoch}) THEN
  RAISE EXCEPTION 'backup target, pause token or epoch mismatch';
 END IF;
END $$;
SELECT pg_export_snapshot();
""")
        keeper.stdin.flush()
        snapshot = keeper.stdout.readline().strip()
        if not re.fullmatch(r'[0-9A-Fa-f]+-[0-9A-Fa-f]+-[0-9]+', snapshot):
            raise Refused('could not establish the exact paused exported SQL snapshot')
        # The exported snapshot remains alive until BOTH catalog dump and reachable object
        # traversal/copy finish. The parent exclusive fence prevents competing recovery imports.
        subprocess.run([str(pg_dump),'--format=custom','--no-owner','--no-acl','--snapshot',snapshot,
                        '--dbname',dsn,'--file',str(output/'catalog.dump')],check=True,capture_output=True)
        catalog_hash = file_hash(output/'catalog.dump')
        env = dict(os.environ, DATA_DATABASE_URL=dsn)
        result = subprocess.run([str(binary),'capture-objects','--expect-target',target_id,
                                 '--sql-snapshot',snapshot,'--output-root',str(output/'objects')],
                                check=True,capture_output=True,text=True,env=env)
        manifest = json.loads(result.stdout,parse_float=Decimal)
        if manifest.get('target_id') != target_id or manifest.get('fence',{}).get('epoch') != epoch:
            raise Refused('object traversal returned a different target/fence')
        if file_hash(output/'catalog.dump') != catalog_hash:
            raise Refused('captured catalog changed during object backup')
        for item in manifest['objects']:
            if file_hash(confined(output/'objects',item['path'])) != item['sha256']:
                raise Refused('captured reachable object checksum mismatch')
        manifest.update({'catalog_sha256':catalog_hash,'sql_snapshot':snapshot})
        keeper.stdin.write(f'COMMIT; DO $$ BEGIN PERFORM pg_advisory_unlock({LOCK}); END $$;\n'); keeper.stdin.close()
        if keeper.wait(timeout=30) != 0:
            raise Refused('checkpoint holder failed before backup completion')
        sync_tree(output)
        atomic_json(output/'manifest.json',manifest,replace=False)
        return manifest
    except BaseException:
        atomic_json(output/'INCOMPLETE.json',{'format':FORMAT,'complete':False})
        raise
    finally:
        if keeper.poll() is None:
            keeper.stdin.close(); keeper.terminate(); keeper.wait(timeout=5)
        keeper.stdout.close(); keeper.stderr.close()


def restore_catalog(bundle_path, target_id, database_variable, pg_restore='pg_restore'):
    target_id=identity(target_id)
    manifest=verify_bundle(bundle_path)
    if manifest['target_id'] != target_id:
        raise Refused('catalog restore target mismatch')
    occupied=pg_run("SELECT EXISTS(SELECT 1 FROM pg_tables WHERE schemaname='public')::text;",database_variable).strip()
    if occupied != 'false':
        raise Refused('catalog restores require an entirely fresh empty database; no --clean or overwrite')
    subprocess.run([str(pg_restore),'--exit-on-error','--single-transaction','--no-owner','--no-acl',
                    '--dbname',database_url(database_variable),str(Path(bundle_path)/'catalog.dump')],
                   check=True,capture_output=True)
    restored=json.loads(pg_run('SELECT to_jsonb(s)::text FROM timeseries_recovery_state s;',database_variable).strip())
    if restored.get('target_id') != target_id or restored.get('writes_paused') is not True:
        raise Refused('restored catalog does not retain the intended paused target')
    return {'format':FORMAT,'kind':'catalog-restored','target_id':target_id,
            'checkpoint':manifest['checkpoint'],'catalog_sha256':manifest['catalog_sha256'],
            'restore_parity_required':True}


def apply_transition(request_path, evidence_paths, binary, apply_target=None):
    plan=gate(request_path,evidence_paths)
    if apply_target is None:
        return plan
    if identity(apply_target) != plan['target_id']:
        raise Refused('transition target confirmation mismatch')
    result=subprocess.run([str(binary),'apply-transition','--expect-target',plan['target_id']],
                          input=exact_json(plan),text=True,capture_output=True,check=True)
    receipt=json.loads(result.stdout,parse_float=Decimal)
    if receipt.get('target_id') != plan['target_id'] or receipt.get('action') != plan['action'] or receipt.get('committed') is not True:
        raise Refused('storage did not acknowledge the exact backend transition')
    return receipt

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    inv = sub.add_parser('inventory'); inv.add_argument('snapshot')
    compare = sub.add_parser('parity'); compare.add_argument('source'); compare.add_argument('destination')
    fill = sub.add_parser('backfill'); fill.add_argument('source'); fill.add_argument('--state', required=True)
    fill.add_argument('--destination', required=True); fill.add_argument('--batch-size', type=int, default=10000)
    fill.add_argument('--binary'); fill.add_argument('--apply-target')
    copy = sub.add_parser('bundle'); copy.add_argument('--catalog-dump', required=True)
    copy.add_argument('--object-root', required=True); copy.add_argument('--reachable-manifest', required=True)
    copy.add_argument('--output', required=True)
    verify = sub.add_parser('verify-bundle'); verify.add_argument('bundle')
    restore = sub.add_parser('restore-objects'); restore.add_argument('bundle')
    restore.add_argument('--destination', required=True); restore.add_argument('--apply-target', required=True)
    capture = sub.add_parser('capture-bundle'); capture.add_argument('--output',required=True); capture.add_argument('--target-id',required=True)
    capture.add_argument('--pause-token',required=True); capture.add_argument('--epoch',required=True,type=int); capture.add_argument('--binary',required=True)
    capture.add_argument('--database-env',default='DATA_DATABASE_URL'); capture.add_argument('--pg-dump',default='pg_dump')
    restore_db = sub.add_parser('restore-catalog'); restore_db.add_argument('bundle'); restore_db.add_argument('--apply-target',required=True)
    restore_db.add_argument('--database-env',required=True); restore_db.add_argument('--pg-restore',default='pg_restore')
    transition = sub.add_parser('transition'); transition.add_argument('request'); transition.add_argument('evidence',nargs='+')
    transition.add_argument('--binary',required=True); transition.add_argument('--apply-target')
    control = sub.add_parser('control'); control.add_argument('action', choices=['initialize', 'pause', 'resume'])
    control.add_argument('--target-id', required=True); control.add_argument('--label'); control.add_argument('--pause-token')
    control.add_argument('--epoch', type=int); control.add_argument('--apply-target'); control.add_argument('--database-env', default='DATA_DATABASE_URL')
    export = sub.add_parser('export-postgres'); export.add_argument('--target-id', required=True)
    export.add_argument('--output', required=True); export.add_argument('--database-env', default='DATA_DATABASE_URL')
    replay = sub.add_parser('replay-postgres'); replay.add_argument('source'); replay.add_argument('--target-id', required=True)
    replay.add_argument('--pause-token', required=True); replay.add_argument('--epoch', required=True, type=int)
    replay.add_argument('--apply-target'); replay.add_argument('--database-env', default='DATA_DATABASE_URL')
    accept = sub.add_parser('gate'); accept.add_argument('request'); accept.add_argument('evidence', nargs='+')
    args = parser.parse_args()
    if args.command == 'inventory':
        with Snapshot(args.snapshot) as snapshot: result = snapshot.inventory()
    elif args.command == 'parity': result = parity(args.source, args.destination)
    elif args.command == 'backfill': result = backfill(args.source, args.state, args.destination, args.batch_size, args.binary, args.apply_target)
    elif args.command == 'bundle': result = bundle(args.catalog_dump, args.object_root, args.reachable_manifest, args.output)
    elif args.command == 'verify-bundle': result = verify_bundle(args.bundle)
    elif args.command == 'restore-objects': result = restore_objects(args.bundle, args.destination, args.apply_target)
    elif args.command == 'export-postgres': result = export_postgres(args.output, args.target_id, args.database_env)
    elif args.command == 'replay-postgres': result = replay_postgres(args.source, args.target_id, args.pause_token, args.epoch, args.database_env, args.apply_target)
    elif args.command == 'capture-bundle': result = capture_bundle(args.output,args.target_id,args.pause_token,args.epoch,args.binary,args.database_env,args.pg_dump)
    elif args.command == 'restore-catalog': result = restore_catalog(args.bundle,args.apply_target,args.database_env,args.pg_restore)
    elif args.command == 'transition': result = apply_transition(args.request,args.evidence,args.binary,args.apply_target)
    elif args.command == 'gate': result = gate(args.request, args.evidence)
    else:
        sql = control_sql(args.action, args.target_id, args.label, args.pause_token, args.epoch)
        if args.apply_target is None:
            result = {'format': FORMAT, 'apply': False, 'target_id': identity(args.target_id), 'action': args.action}
        elif identity(args.apply_target) != identity(args.target_id):
            raise Refused('control target confirmation mismatch')
        else:
            result = json.loads(pg_run(sql, args.database_env).strip(), parse_float=Decimal)
    print(exact_json(result))
    return 1 if args.command == 'parity' and not result['passed'] else 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except subprocess.CalledProcessError as error:
        detail = error.stderr.decode(errors='replace') if isinstance(error.stderr, bytes) else (error.stderr or '')
        detail = re.sub(r'postgres(?:ql)?://[^\s]+', '<configured-database>', detail)
        print(f'recovery subprocess failed ({error.returncode}): {detail[:500].strip()}', file=sys.stderr)
        sys.exit(1)
    except (ValueError, OSError, KeyError) as error:
        print(f'recovery refused: {error}', file=sys.stderr)
        sys.exit(1)
