#!/usr/bin/env python3
"""Operator-only, locked conversion of a combined database; never deploys anything."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
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


def backup_root():
    """The host backup root shared with backup-kind-databases.sh (scripts/deploy/lib/kind-env.sh)."""
    return Path(os.environ.get('ECONGRAPH_BACKUP_ROOT') or Path.home() / 'econgraph-backups')


def new_backup_dir():
    """A new UTC-timestamped directory under the backup root, named like kind-env.sh's new_backup_dir."""
    return backup_root() / datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ')


def write_private(path, text):
    """Create a new owner-only file and persist it."""
    with path.open('x') as output:
        path.chmod(0o600)
        output.write(text)
        output.flush()
        os.fsync(output.fileno())


def run_logged(command, log, **kwargs):
    """Run a PostgreSQL client tool, keeping its stderr in an owner-only log in the backup directory."""
    descriptor = os.open(log, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o600)
    with os.fdopen(descriptor, 'a') as output:
        output.write(f'$ {" ".join(command)}\n')
        output.flush()
        result = subprocess.run(command, stderr=output, **kwargs)
    if result.returncode != 0:
        lines = log.read_text(errors='replace').splitlines()[-5:]
        raise RuntimeError(f'{command[0]} exited with status {result.returncode}; last lines of {log}: '
                           + ' | '.join(lines))
    return result


def verified_backup(source, service, directory, evidence):
    """Write a verified dump of the locked source to a new owner-only directory.

    The directory must not exist yet. It is created mode 0700 and every file in it is mode
    0600: combined.dump (the whole combined database, app records included), SHA256SUMS,
    the client tools' stderr in pg_tools.log, and the pre-copy evidence. Nothing here is
    ever deleted, not even a partial dump after a failure. No VERIFIED marker is written, so
    the off-site copy of the nightly dumps never selects this directory, and there is no
    econ_graph.dump, so restore-kind-databases.sh refuses it instead of comparing only data
    row counts that the conversion never changes.
    """
    require(not directory.exists() and not directory.is_symlink(),
            f'{directory} already exists; refusing to overwrite a backup')
    directory.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    directory.mkdir(mode=0o700)
    directory.chmod(0o700)
    log = directory / 'pg_tools.log'
    snapshot = query(source, 'SELECT pg_export_snapshot()')[0][0]
    partial = directory / 'combined.dump.partial'
    # Open the private archive ourselves, including when pg_dump is wrapped in a container.
    with partial.open('xb') as archive:
        partial.chmod(0o600)
        run_logged(['pg_dump', '--no-password', '--dbname', f'service={service}',
                    '--format=custom', '--snapshot', snapshot, '--lock-wait-timeout=10s'], log, stdout=archive)
        archive.flush()
        os.fsync(archive.fileno())
    # Decode every data block, not just the archive's table of contents.
    with partial.open('rb') as archive:
        run_logged(['pg_restore', '--file', os.devnull], log, stdin=archive, stdout=subprocess.DEVNULL)
    with partial.open('rb') as archive:
        digest = hashlib.file_digest(archive, 'sha256').hexdigest()
    partial.rename(directory / 'combined.dump')
    write_private(directory / 'SHA256SUMS', f'{digest}  combined.dump\n')
    write_private(directory / 'source-evidence.json', json.dumps(evidence, indent=2) + '\n')
    sync_directory(directory)
    sync_directory(directory.parent)
    return digest


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
    """Read ordered column names, types and nullability so COPY cannot reorder or coerce app fields."""
    return query(conn, "SELECT attname, format_type(atttypid, atttypmod), attnotnull FROM pg_attribute "
                 "WHERE attrelid = to_regclass(%s) AND attnum > 0 AND NOT attisdropped ORDER BY attnum",
                 (f'public.{table}',))


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
        if source_app:
            for name in app_tables:
                require(columns(source, name) == columns(target, name), f'Column inventory differs for {name}')
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
            previous = json.loads(args.previous_evidence.read_text()) if args.previous_evidence else None
            if previous is not None:
                # A copy this tool made and verified earlier, which went stale when writers resumed
                # on the combined database, may be replaced; anything else in the target may not.
                require(previous.get('copy_verified') is True
                        and previous.get('source_app') == target_before
                        and previous.get('source_identity') == list(source_identity)
                        and previous.get('target_identity') == list(target_identity),
                        'Target app data does not match the previous verified copy; refusing replacement')
                report['replaced_previous_copy'] = True
            else:
                only_seed_templates(target, app_tables, baseline)
        report['source_app'] = source_app_before
        print(json.dumps(report, indent=2))
        if args.dry_run:
            print('Dry run complete. No backup, copy, or drop performed; transactions roll back.')
            source.rollback()
            target.rollback()
            return
        require(args.writers_stopped, 'Stop all app/crawler writers and pass --writers-stopped before copying')
        if args.backup_dir is None:
            args.backup_dir = new_backup_dir()
        report['backup_dir'] = str(args.backup_dir)
        report['backup_sha256'] = verified_backup(source, args.source_service, args.backup_dir, report)
        if not matches:
            copy_app(source, target, app_tables)
        require(fingerprints(target, app_tables) == source_app_before, 'App copy content verification failed')
        require(fingerprints(source, data_tables) == data_before, 'Data table content changed')
        report['copy_verified'] = True
        save_evidence(args.backup_dir, report)
        # Commit the verified app copy first. Source locks remain held until the final commit.
        target.commit()
        if args.drop_source_app:
            report['removed_versions'] = sorted(app_versions - data_versions)
            phrase = f'DROP APP TABLES FROM {query(source, "SELECT current_database()")[0][0]}'
            require(sys.stdin.isatty(), 'Source drop requires typed confirmation on a terminal; copy is retained')
            require(input(f'Type {phrase!r}: ') == phrase, 'Confirmation did not match; source is unchanged')
            source.execute(sql.SQL('DROP TABLE {}').format(identifiers(app_tables)))
            for enum in app_enums:
                source.execute(sql.SQL('DROP TYPE {}').format(sql.Identifier('public', enum)))
            source.execute('DELETE FROM public.__diesel_schema_migrations WHERE version = ANY(%s)',
                           (report['removed_versions'],))
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


APP_ENTRY_TYPES = {'TABLE', 'TABLE DATA', 'CONSTRAINT', 'FK CONSTRAINT', 'TRIGGER', 'POLICY', 'ROW SECURITY',
                   'DEFAULT', 'COMMENT', 'ACL', 'INDEX', 'TYPE'}
TOC_ENTRY = re.compile(r'^\d+; \d+ \d+ (?P<kind>' + '|'.join(sorted(APP_ENTRY_TYPES, key=len, reverse=True))
                       + r') (?P<schema>\S+) (?P<tag>.+) (?P<owner>\S+)$')


def app_restore_list(listing, schema_sql, app_tables, app_enums):
    """Select the archive entries that recreate the app tables, their rows, enum types and dependents."""
    indexes = {match[1] for match in re.finditer(
        r'^CREATE (?:UNIQUE )?INDEX (\w+) ON (?:ONLY )?public\.(\w+) ', schema_sql, re.M)
        if match[2] in app_tables}
    selected = []
    for line in listing.splitlines():
        entry = TOC_ENTRY.match(line)
        if entry is None or entry['schema'] != 'public':
            continue
        kind, words = entry['kind'], entry['tag'].split(' ')
        if kind in ('TABLE', 'TABLE DATA'):
            keep = entry['tag'] in app_tables
        elif kind == 'TYPE':
            keep = entry['tag'] in app_enums
        elif kind == 'INDEX':
            keep = entry['tag'] in indexes
        elif kind in ('COMMENT', 'ACL'):
            # Tags such as "TABLE users", "COLUMN users.email" or "TYPE user_role".
            keep = (len(words) == 2 and ((words[0] == 'TABLE' and words[1] in app_tables)
                                        or (words[0] == 'COLUMN' and words[1].split('.')[0] in app_tables)
                                        or (words[0] == 'TYPE' and words[1] in app_enums)))
        else:
            # Constraints, triggers, policies and defaults are tagged "<table> <name>".
            keep = words[0] in app_tables
        if keep:
            selected.append(line)
    return '\n'.join(selected) + '\n'


def restore_source_app(args):
    """Put the app tables back into a converted source from the backup taken before their removal."""
    import psycopg
    from psycopg import sql
    app_tables, app_enums, app_versions, _ = inventory('app')
    data_tables, _, data_versions, _ = inventory('data')
    require(re.fullmatch(r'[A-Za-z0-9_.-]+', args.source_service) is not None,
            'Use a simple libpq service name containing only letters, digits, underscore, dot or hyphen')
    require(args.writers_stopped, 'Stop all app/crawler writers and pass --writers-stopped before restoring')
    backup = args.restore_source_app
    evidence = json.loads((backup / 'evidence.json').read_text())
    require(evidence.get('copy_verified') is True and evidence.get('source_app_removed') is True
            and evidence.get('removed_versions'),
            f'{backup} is not the backup of a run that removed the source app tables')
    dump = backup / 'combined.dump'
    with dump.open('rb') as archive:
        digest = hashlib.file_digest(archive, 'sha256').hexdigest()
    require(digest == evidence.get('backup_sha256')
            and (backup / 'SHA256SUMS').read_text() == f'{digest}  combined.dump\n',
            f'{dump} does not match its recorded checksum')
    expected_versions = data_versions | set(evidence['removed_versions'])
    metadata = {'__diesel_schema_migrations'}
    identity = 'SELECT system_identifier::text, current_database() FROM pg_control_system()'

    def inspect(conn):
        present = tables(conn) & set(app_tables)
        enums = {r[0] for r in query(conn, "SELECT typname FROM pg_type WHERE typnamespace='public'::regnamespace")}
        return present, enums & set(app_enums)

    with psycopg.connect(service=args.source_service) as source:
        require(list(query(source, identity)[0]) == evidence.get('source_identity'),
                'The source is not the database this backup was taken from')
        require(tables(source) - set(app_tables) == set(data_tables) | metadata, 'Source has unknown or missing tables')
        present, enums = inspect(source)
    if not present and not enums:
        require(versions_of(args.source_service) == data_versions,
                'Source migration history is not the converted data history; operator investigation required')
        record = new_backup_dir() if args.backup_dir is None else args.backup_dir
        require(not record.exists(), f'{record} already exists; refusing to overwrite it')
        record.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        record.mkdir(mode=0o700)
        log = record / 'pg_tools.log'
        schema_sql = run_logged(['pg_restore', '--schema-only', '--file', '-', str(dump)], log,
                                stdout=subprocess.PIPE, text=True).stdout
        listing = run_logged(['pg_restore', '--list', str(dump)], log, stdout=subprocess.PIPE, text=True).stdout
        selection = app_restore_list(listing, schema_sql, set(app_tables), set(app_enums))
        write_private(record / 'restore.list', selection)
        print(f'Restoring {sum(1 for _ in selection.splitlines())} archive entries listed in {record / "restore.list"}')
        # One transaction: an error leaves the source exactly as it was.
        run_logged(['pg_restore', '--single-transaction', '--exit-on-error', '--no-password',
                    '--use-list', str(record / 'restore.list'), '--dbname', f'service={args.source_service}',
                    str(dump)], log, stdout=subprocess.DEVNULL)
    else:
        # A previous rollback restored the tables but stopped before recording the versions.
        require(present == set(app_tables) and enums == set(app_enums),
                'Source has a partial app schema; operator investigation required')
    with psycopg.connect(service=args.source_service) as source:
        source.execute("SET LOCAL lock_timeout='10s'")
        source.execute("SET LOCAL timezone='UTC'")
        source.execute("SET LOCAL search_path=pg_catalog,public")
        source.execute("SET LOCAL extra_float_digits=3")
        source.execute("SET LOCAL datestyle='ISO, YMD'")
        source.execute("SET LOCAL intervalstyle='postgres'")
        source.execute("SET LOCAL bytea_output='hex'")
        source.execute(sql.SQL('LOCK TABLE {} IN SHARE MODE').format(identifiers(sorted(app_tables) + sorted(metadata))))
        require(inspect(source) == (set(app_tables), set(app_enums)), 'Restored app schema is incomplete')
        require(fingerprints(source, app_tables) == evidence['source_app'],
                'Restored app tables do not match the pre-conversion fingerprints; nothing recorded')
        source.execute('INSERT INTO public.__diesel_schema_migrations(version) SELECT unnest(%s::text[]) '
                       'ON CONFLICT DO NOTHING', (evidence['removed_versions'],))
        require(versions(source) == expected_versions, 'Source migration history does not match the backup')
        source.commit()
    print('Source app tables restored from the pre-removal backup and verified against its fingerprints. '
          'The app database copy is unchanged; point the backend at the combined database before resuming writers.')


def versions_of(service):
    """Read migration versions on a fresh connection."""
    import psycopg
    with psycopg.connect(service=service) as conn:
        return versions(conn)


def main():
    """Parse operator options and redact database/tool failures before reporting them."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-service', required=True, help='libpq service for the existing combined/data database')
    parser.add_argument('--app-service', help='libpq service for a migrated app database (required to convert)')
    parser.add_argument('--source-map', type=Path, help='JSON object: data_sources UUID -> canonical crawler code (required to convert)')
    parser.add_argument('--backup-dir', type=Path,
                        help='New directory for the verified source backup and evidence '
                        '(default: a new UTC-timestamped directory under $ECONGRAPH_BACKUP_ROOT, ~/econgraph-backups)')
    parser.add_argument('--dry-run', action='store_true', help='Read and lock schemas; never write persistent data')
    parser.add_argument('--previous-evidence', type=Path,
                        help='evidence.json of an earlier verified run: verifies an already converted source, '
                        'or allows replacing that run\'s app copy after it went stale')
    parser.add_argument('--writers-stopped', action='store_true', help='Operator confirms every app and crawler writer is stopped')
    parser.add_argument('--drop-source-app', action='store_true', help='After copy verification, ask for typed source cleanup confirmation')
    parser.add_argument('--restore-source-app', type=Path, metavar='BACKUP_DIR',
                        help='Roll back a removal: restore the app tables, enum types and app migration entries into '
                        'the source from the backup directory of the --drop-source-app run, and verify them')
    args = parser.parse_args()
    os.umask(0o077)
    try:
        if args.restore_source_app is not None:
            require(not (args.dry_run or args.drop_source_app or args.previous_evidence),
                    '--restore-source-app runs on its own')
            restore_source_app(args)
            return 0
        require(args.app_service is not None and args.source_map is not None,
                '--app-service and --source-map are required to convert')
        require(not (args.dry_run and args.drop_source_app), '--drop-source-app cannot be combined with --dry-run')
        convert(args)
    except Exception as exc:
        # Database errors can include row values. Never echo them or connection strings.
        message = str(exc) if isinstance(exc, RuntimeError) else type(exc).__name__
        print(f'Conversion stopped: {message}. Uncommitted work rolls back; inspect retained evidence '
              'and committed database state before resuming writers. Backups are never deleted.', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
