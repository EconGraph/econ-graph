#!/usr/bin/env python3
"""Operator-only, locked conversion of a combined database; never deploys anything."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import uuid

ROOT = Path(__file__).resolve().parents[2]
NAMESPACE = uuid.UUID('1f871bd6-25e5-5022-b7ba-790efbc3bd30')
CODES = {'FRED', 'BLS', 'BEA', 'CENSUS', 'WORLD_BANK', 'IMF', 'ECB', 'OECD',
         'BOE', 'BOJ', 'BOC', 'RBA', 'SNB', 'UN_STATS', 'ILO', 'WTO', 'FHFA', 'SEC'}


def inventory(plane):
    """Read baseline table/type inventories and all checked-in migration versions."""
    directory = ROOT / 'backend/migrations' / plane
    baseline = next(directory.glob('*baseline/up.sql'))
    content = baseline.read_text()
    return (re.findall(r'^CREATE TABLE (\w+) ', content, re.M),
            re.findall(r'^CREATE TYPE (\w+) AS ENUM', content, re.M),
            {p.name.split('_', 1)[0].replace('-', '') for p in directory.iterdir() if p.is_dir()},
            content)


def require(condition, message):
    """Stop with an operator-facing explanation that contains no row values."""
    if not condition:
        raise RuntimeError(message)


def query(conn, text, params=None):
    """Execute an inspection query and collect its small result set."""
    return conn.execute(text, params).fetchall()


def tables(conn):
    """Inspect public tables, including any unknown tables that must block conversion."""
    return {r[0] for r in query(conn, "SELECT tablename FROM pg_tables WHERE schemaname='public'")}


def versions(conn):
    """Read Diesel version identifiers without copying migration execution history."""
    return {r[0] for r in query(conn, 'SELECT version FROM public.__diesel_schema_migrations')}


def identifiers(names):
    """Quote table names explicitly in the public schema."""
    from psycopg import sql
    return sql.SQL(', ').join(sql.Identifier('public', n) for n in names)


def fingerprint(conn, table):
    """Stream sorted canonical rows: bounded client memory, includes duplicate rows."""
    from psycopg import sql
    digest = hashlib.sha256()
    statement = sql.SQL('COPY (SELECT to_jsonb(t)::text FROM {} t '
                        'ORDER BY to_jsonb(t)::text COLLATE "C") TO STDOUT').format(
                            sql.Identifier('public', table))
    with conn.cursor().copy(statement) as rows:
        for block in rows:
            digest.update(block)
    count = query(conn, sql.SQL('SELECT count(*) FROM {}').format(sql.Identifier('public', table)))[0][0]
    return {'rows': count, 'sha256': digest.hexdigest()}


def fingerprints(conn, names):
    """Collect row counts and content digests for an explicit table inventory."""
    return {name: fingerprint(conn, name) for name in names}


def stable_ids(conn, mapping):
    """Verify every economic series against the crawler UUIDv5 contract."""
    mismatches = 0
    digest = hashlib.sha256()
    # A server cursor keeps a large catalog out of client memory.
    with conn.cursor(name='series_ids') as cur:
        cur.execute('SELECT id, source_id, external_id FROM public.economic_series ORDER BY id')
        for series_id, source_id, external_id in cur:
            code = mapping.get(str(source_id))
            require(code in CODES, 'A series source is unmapped or has an unknown canonical source code')
            mismatches += series_id != uuid.uuid5(NAMESPACE, f'{code}:{external_id}')
            digest.update(series_id.bytes)
    return {'mismatches': mismatches, 'sha256': digest.hexdigest()}


def private_dump(source, service, directory):
    """Dump the locked snapshot, decode the full archive, and return its checksum."""
    snapshot = query(source, 'SELECT pg_export_snapshot()')[0][0]
    path = directory / 'combined.dump'
    # Open the private archive ourselves, including when pg_dump is wrapped in a container.
    with path.open('xb') as archive:
        path.chmod(0o600)
        subprocess.run(['pg_dump', '--no-password', '--dbname', f'service={service}',
                        '--format=custom', '--snapshot', snapshot], check=True,
                       stdout=archive, stderr=subprocess.DEVNULL)
    # Decode every data block, not just the archive's table of contents.
    with path.open('rb') as archive:
        subprocess.run(['pg_restore', '--file', os.devnull], stdin=archive, check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    with path.open('rb') as archive:
        digest = hashlib.file_digest(archive, 'sha256').hexdigest()
    (directory / 'SHA256SUMS').write_text(f'{digest}  combined.dump\n')
    return digest


def encrypted_backup(source, service, directory, evidence):
    """Seal a verified tmpfs source dump and persist ciphertext before mutation."""
    helper = ROOT / 'scripts/backup/encryption.sh'
    command = ('set -euo pipefail\n'
               'die() { printf "%s\\n" "$*" >&2; exit 1; }\n'
               'source "$1"\nbackup_encryption_preflight\n')
    preflight = subprocess.run(['bash', '-c', command + 'backup_plaintext_dir',
                                'conversion-backup', str(helper)], capture_output=True, text=True)
    # Only fixed helper diagnostics are safe to echo; sourced operator config can emit anything.
    diagnostics = (
        'set BACKUP_AGE_RECIPIENT to the maintainer\'s public age recipient; no plaintext fallback',
        'age is required for encrypted backups',
        'invalid age recipient',
        'BACKUP_PLAINTEXT_ROOT must be tmpfs; plaintext staging on disk is refused',
    )
    diagnostic = next((message for message in diagnostics if message in preflight.stderr.splitlines()),
                      'check the public recipient, age installation, and available private tmpfs staging space')
    require(preflight.returncode == 0, f'Backup encryption preflight failed: {diagnostic}')
    staging = preflight.stdout.strip()
    stage = Path(staging)
    require(stage.is_absolute() and stage.name.startswith('econgraph-backup.') and stage.is_dir(),
            'Backup helper returned an invalid staging directory')
    try:
        # The helper requires tmpfs and validates the public recipient before any mutation.
        digest = private_dump(source, service, stage)
        (stage / 'source-evidence.json').write_text(json.dumps(evidence, indent=2) + '\n')
        subprocess.run(['bash', '-c', command + 'backup_seal_dir "$2" "$3"',
                        'conversion-backup', str(helper), str(stage), str(directory)],
                       check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for name in ('backup.tar.age', 'SHA256SUMS', 'VERIFIED'):
            with (directory / name).open('rb') as retained:
                os.fsync(retained.fileno())
        sync_directory(directory)
        sync_directory(directory.parent)
        return digest
    finally:
        # Only newly allocated plaintext staging is disposable; never remove a backup.
        shutil.rmtree(stage)


def sync_directory(directory):
    """Persist directory entries after publishing recovery artifacts."""
    descriptor = os.open(directory, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def save_evidence(directory, report):
    """Replace evidence atomically and persist it before releasing database locks."""
    temporary = directory / 'evidence.next.json'
    with temporary.open('x') as output:
        temporary.chmod(0o600)
        output.write(json.dumps(report, indent=2) + '\n')
        output.flush()
        os.fsync(output.fileno())
    temporary.replace(directory / 'evidence.json')
    sync_directory(directory)


def columns(conn, table):
    """Read ordered column names so COPY cannot silently reorder app fields."""
    return query(conn, "SELECT column_name FROM information_schema.columns "
                 "WHERE table_schema='public' AND table_name=%s ORDER BY ordinal_position", (table,))


def copy_app(source, target, app_tables):
    """Copy all app rows atomically while preserving self-referencing foreign keys."""
    from psycopg import sql
    # Temporarily defer even self-referencing foreign keys, then restore their definitions.
    fkeys = query(target, "SELECT conrelid::regclass::text, conname, condeferrable, condeferred "
                  "FROM pg_constraint WHERE contype='f' AND connamespace='public'::regnamespace")
    for table, name, _, _ in fkeys:
        target.execute(sql.SQL('ALTER TABLE {} ALTER CONSTRAINT {} DEFERRABLE INITIALLY DEFERRED').format(
            sql.Identifier('public', table.removeprefix('public.')), sql.Identifier(name)))
    target.execute(sql.SQL('TRUNCATE {}').format(identifiers(app_tables)))
    for name in app_tables:
        require(columns(source, name) == columns(target, name), f'Column inventory differs for {name}')
        statement = sql.SQL('COPY {} TO STDOUT').format(sql.Identifier('public', name))
        destination = sql.SQL('COPY {} FROM STDIN').format(sql.Identifier('public', name))
        with source.cursor().copy(statement) as src, target.cursor().copy(destination) as dst:
            for block in src:
                dst.write(block)
    target.execute('SET CONSTRAINTS ALL IMMEDIATE')
    for table, name, deferrable, deferred in fkeys:
        target.execute(sql.SQL('ALTER TABLE {} ALTER CONSTRAINT {} {}').format(
            sql.Identifier('public', table.removeprefix('public.')), sql.Identifier(name),
            sql.SQL(('DEFERRABLE INITIALLY DEFERRED' if deferred else 'DEFERRABLE INITIALLY IMMEDIATE')
                    if deferrable else 'NOT DEFERRABLE')))


def only_seed_templates(target, app_tables, baseline):
    """Refuse existing target app records except exact baseline template content."""
    from psycopg import sql
    for name in app_tables:
        if name != 'annotation_templates':
            require(query(target, sql.SQL('SELECT count(*) FROM {}').format(sql.Identifier('public', name)))[0][0] == 0,
                    'Target contains existing app data which differs from the source; refusing replacement')
    # Compare seed content too, ignoring only generated ids and timestamps.
    target.execute('CREATE TEMP TABLE split_seed (LIKE public.annotation_templates INCLUDING DEFAULTS) ON COMMIT DROP')
    seed = re.search(r'INSERT INTO annotation_templates .*?;', baseline, re.S)
    require(seed is not None, 'Cannot identify baseline template seed')
    target.execute(seed[0].replace('INSERT INTO annotation_templates ', 'INSERT INTO pg_temp.split_seed ', 1))
    stripped = "to_jsonb(t) - ARRAY['id','created_at','updated_at']"
    difference = query(target, f'SELECT EXISTS ((SELECT {stripped} FROM public.annotation_templates t '
                       f'EXCEPT ALL SELECT {stripped} FROM pg_temp.split_seed t) UNION ALL '
                       f'(SELECT {stripped} FROM pg_temp.split_seed t EXCEPT ALL '
                       f'SELECT {stripped} FROM public.annotation_templates t))')[0][0]
    empty = query(target, 'SELECT count(*)=0 FROM public.annotation_templates')[0][0]
    require(empty or not difference, 'Target annotation templates differ from baseline seeds')


def convert(args):
    """Run a maintenance conversion with locked, verified copy and explicit cleanup."""
    import psycopg
    from psycopg import sql
    app_tables, app_enums, app_versions, baseline = inventory('app')
    data_tables, _, data_versions, _ = inventory('data')
    for service in (args.source_service, args.app_service):
        require(re.fullmatch(r'[A-Za-z0-9_.-]+', service) is not None,
                'Use a simple libpq service name containing only letters, digits, underscore, dot or hyphen')
    mapping = json.loads(args.source_map.read_text())
    require(isinstance(mapping, dict), 'Source map must be an object mapping data_sources UUIDs to canonical codes')
    with psycopg.connect(service=args.source_service) as source, psycopg.connect(service=args.app_service) as target:
        # READ COMMITTED deliberately takes a fresh snapshot after locks have been granted.
        for conn in (source, target):
            conn.execute('SET TRANSACTION ISOLATION LEVEL READ COMMITTED')
            conn.execute("SET LOCAL lock_timeout='10s'")
            conn.execute("SET LOCAL statement_timeout='0'")
            conn.execute("SET LOCAL timezone='UTC'")
            conn.execute("SET LOCAL search_path=pg_catalog,public")
            conn.execute("SET LOCAL extra_float_digits=3")
            conn.execute("SET LOCAL datestyle='ISO, YMD'")
            conn.execute("SET LOCAL intervalstyle='postgres'")
            conn.execute("SET LOCAL bytea_output='hex'")
        identity = 'SELECT system_identifier::text, current_database() FROM pg_control_system()'
        source_identity, target_identity = query(source, identity)[0], query(target, identity)[0]
        require(source_identity != target_identity, 'Source and target are the same database')
        source_tables, target_tables = tables(source), tables(target)
        require(not query(target, "SELECT 1 FROM pg_class WHERE relkind='S' AND relnamespace='public'::regnamespace"),
                'Target has sequences: this UUID-based conversion requires operator review for sequence state')
        metadata = {'__diesel_schema_migrations'}
        require(target_tables == set(app_tables) | metadata, 'Target table inventory is not exactly the app schema')
        source_app = source_tables & set(app_tables)
        require(source_app in (set(), set(app_tables)), 'Source has a partial app schema; operator investigation required')
        require(source_tables == set(data_tables) | source_app | metadata, 'Source has unknown or missing tables')
        source.execute(sql.SQL('LOCK TABLE {} IN SHARE MODE').format(identifiers(sorted(source_tables))))
        target.execute(sql.SQL('LOCK TABLE {} IN ACCESS EXCLUSIVE MODE').format(identifiers(sorted(target_tables))))
        require(app_versions == versions(target), 'Target app migrations are not current; initialize/migrate it first')
        require(data_versions <= versions(source), 'Source data migration history is incomplete')
        require(versions(source) <= app_versions | data_versions, 'Source has migration versions unknown to this checkout')
        require(tables(source) == source_tables and tables(target) == target_tables, 'Table inventory changed during locking')
        series = stable_ids(source, mapping)
        print(f"Stable series ID mismatches: {series['mismatches']}")
        require(series['mismatches'] == 0, 'Unstable series IDs: stop for operator investigation')
        data_before = fingerprints(source, data_tables)
        target_before = fingerprints(target, app_tables)
        report = {'series': series, 'data_before': data_before, 'app_before': target_before, 'dry_run': args.dry_run,
                  'source_identity': list(source_identity), 'target_identity': list(target_identity)}
        if not source_app:
            require(not (set(app_enums) & {r[0] for r in query(source, "SELECT typname FROM pg_type WHERE typnamespace='public'::regnamespace")}),
                    'Source app enums remain; investigate partial conversion')
            require(versions(source) == data_versions, 'Source app migration entries remain after a partial conversion')
            require(args.previous_evidence is not None,
                    'Source has no app tables: --previous-evidence is required to verify an already completed conversion')
            previous = json.loads(args.previous_evidence.read_text())
            require(previous.get('copy_verified') is True
                    and previous.get('source_app') == target_before
                    and previous.get('data_before') == data_before
                    and previous.get('series') == series
                    and previous.get('source_identity') == list(source_identity)
                    and previous.get('target_identity') == list(target_identity),
                    'Previous conversion evidence does not match these databases and their current contents')
            print('Already converted: source data and target app contents match the previous verified conversion.')
            return
        source_app_before = fingerprints(source, app_tables)
        matches = target_before == source_app_before
        if not matches:
            only_seed_templates(target, app_tables, baseline)
        report['source_app'] = source_app_before
        print(json.dumps(report, indent=2))
        if args.dry_run:
            print('Dry run complete. No backup, copy, or drop performed; transactions roll back.')
            source.rollback()
            target.rollback()
            return
        require(args.writers_stopped, 'Stop all app/crawler writers and pass --writers-stopped before copying')
        report['backup_sha256'] = encrypted_backup(source, args.source_service, args.backup_dir, report)
        if not matches:
            copy_app(source, target, app_tables)
        require(fingerprints(target, app_tables) == source_app_before, 'App copy content verification failed')
        require(fingerprints(source, data_tables) == data_before, 'Data table content changed')
        report['copy_verified'] = True
        save_evidence(args.backup_dir, report)
        # Commit the verified app copy first. Source locks remain held until the final commit.
        target.commit()
        if args.drop_source_app:
            phrase = f'DROP APP TABLES FROM {query(source, "SELECT current_database()")[0][0]}'
            require(sys.stdin.isatty(), 'Source drop requires typed confirmation on a terminal; copy is retained')
            require(input(f'Type {phrase!r}: ') == phrase, 'Confirmation did not match; source is unchanged')
            source.execute(sql.SQL('DROP TABLE {}').format(identifiers(app_tables)))
            for enum in app_enums:
                source.execute(sql.SQL('DROP TYPE {}').format(sql.Identifier('public', enum)))
            source.execute('DELETE FROM public.__diesel_schema_migrations WHERE version = ANY(%s)',
                           (sorted(app_versions - data_versions),))
            require(fingerprints(source, data_tables) == data_before, 'Data table content changed during cleanup')
            require(stable_ids(source, mapping) == series, 'Series IDs changed during cleanup')
            source.commit()
            report['source_app_removed'] = True
        else:
            source.rollback()
            report['source_app_removed'] = False
        save_evidence(args.backup_dir, report)
        print('Verified app copy retained. ' + ('Source app tables removed.' if args.drop_source_app else
              'Source app tables retained; rerun with --drop-source-app only after reviewing the copy.'))


def main():
    """Parse operator options and redact database/tool failures before reporting them."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-service', required=True, help='libpq service for the existing combined/data database')
    parser.add_argument('--app-service', required=True, help='libpq service for a migrated app database')
    parser.add_argument('--source-map', required=True, type=Path, help='JSON object: data_sources UUID -> canonical crawler code')
    parser.add_argument('--backup-dir', type=Path, help='New directory for the encrypted verified source backup and nonsecret evidence')
    parser.add_argument('--dry-run', action='store_true', help='Read and lock schemas; never write persistent data')
    parser.add_argument('--previous-evidence', type=Path, help='Previous evidence.json required to verify an already converted source')
    parser.add_argument('--writers-stopped', action='store_true', help='Operator confirms every app and crawler writer is stopped')
    parser.add_argument('--drop-source-app', action='store_true', help='After copy verification, ask for typed source cleanup confirmation')
    args = parser.parse_args()
    require(args.dry_run or args.backup_dir is not None, '--backup-dir is required for a real conversion')
    require(not (args.dry_run and args.drop_source_app), '--drop-source-app cannot be combined with --dry-run')
    os.umask(0o077)
    try:
        convert(args)
    except Exception as exc:
        # Database errors can include row values. Never echo them or connection strings.
        message = str(exc) if isinstance(exc, RuntimeError) else type(exc).__name__
        print(f'Conversion stopped: {message}. Transactions roll back; retained backups are never deleted.', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
