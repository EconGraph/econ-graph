#!/usr/bin/env python3
# qa-scenario: backup-restore
# qa-mutates: yes
# qa-profiles: lab
# qa-timeout: 3600
# qa-description: Back up the lab's database, restore it into a new scratch database, and compare table owners and row counts.
"""Backup and restore into a scratch database. Labs only, never production's database.

Inputs:
  QA_BACKUP_SOURCE_DATABASE_URL  the lab's database to back up (postgres://...).
  QA_SCRATCH_DATABASE_URL        where to restore: a database that must not exist yet and whose
                                 name contains "scratch". The scenario creates it (the URL's
                                 role needs CREATEDB) and drops it afterwards unless
                                 QA_BACKUP_KEEP_SCRATCH=1.
  QA_BACKUP_COMMAND              optional: the lab's own backup, run with bash; the last line it
                                 prints is the dump's path (a pg_dump custom-format file).
                                 Default: pg_dump --format=custom of the source.
  QA_RESTORE_COMMAND             optional: run with bash with QA_BACKUP_PATH and
                                 QA_SCRATCH_DATABASE_URL set. Default: pg_restore --exit-on-error.
  QA_EXPECTED_OWNER              optional: the role every restored table must belong to.
  QA_BACKUP_STEP_TIMEOUT_SECONDS how long one command may run (default 1200); an overrunning
                                 command is stopped and the scratch database still dropped.

Output: one check per comparison. The source's row counts are read right after the dump, so
writers to the lab database (a crawler) should be paused for an exact comparison. Needs psql,
pg_dump and pg_restore. Skipped without the two database URLs. Their passwords reach the
client tools as PGPASSWORD, never as an argument.
"""

import os
import re
import shutil
import subprocess
import urllib.parse
from pathlib import Path

from qa import WORK_DIR, check, fail, finish, skip
from qa import run as run_bounded

SOURCE = os.environ.get('QA_BACKUP_SOURCE_DATABASE_URL', '')
SCRATCH = os.environ.get('QA_SCRATCH_DATABASE_URL', '')
EXPECTED_OWNER = os.environ.get('QA_EXPECTED_OWNER', '')
KEEP = os.environ.get('QA_BACKUP_KEEP_SCRATCH') == '1'
# Each command's limit, so a hung dump or restore is stopped with time left to drop the scratch
# database inside the scenario's own hour.
STEP_TIMEOUT = float(os.environ.get('QA_BACKUP_STEP_TIMEOUT_SECONDS', '1200'))
SEP = '\x1f'

if not SOURCE or not SCRATCH:
    skip('set QA_BACKUP_SOURCE_DATABASE_URL and QA_SCRATCH_DATABASE_URL (a new database whose '
         'name contains "scratch")')


def parts(database_url):
    u = urllib.parse.urlsplit(database_url)
    return u.hostname, u.port or 5432, u.path.lstrip('/')


def with_database(database_url, name):
    u = urllib.parse.urlsplit(database_url)
    return urllib.parse.urlunsplit((u.scheme, u.netloc, '/' + name, u.query, u.fragment))


source_host, source_port, source_db = parts(SOURCE)
scratch_host, scratch_port, scratch_db = parts(SCRATCH)
if not re.fullmatch(r'[A-Za-z0-9_]+', scratch_db):
    fail('scratch database', f'refusing {scratch_db!r}: use letters, digits and underscores')
if 'scratch' not in scratch_db.lower():
    fail('scratch database', f'refusing {scratch_db!r}: its name must contain "scratch"')
if (source_host, source_port, source_db) == (scratch_host, scratch_port, scratch_db):
    fail('scratch database', 'the scratch database is the source database')
for tool in ('psql', 'pg_dump', 'pg_restore'):
    if shutil.which(tool) is None:
        skip(f'{tool} is not installed')


def without_password(database_url):
    """The URL with its password removed, and the password: the password goes to the client
    tools as PGPASSWORD, never on a command line other users can read."""
    u = urllib.parse.urlsplit(database_url)
    password = urllib.parse.unquote(u.password) if u.password is not None else ''
    netloc = u.hostname or ''
    if ':' in netloc:
        netloc = f'[{netloc}]'
    if u.port:
        netloc += f':{u.port}'
    if u.username:
        netloc = f'{u.username}@{netloc}'
    return urllib.parse.urlunsplit((u.scheme, netloc, u.path, u.query, u.fragment)), password


def client(command, database_url, *rest):
    """Run a PostgreSQL client tool against a database, its password in PGPASSWORD."""
    url, password = without_password(database_url)
    return bounded([*command, '-d', url, *rest], {'PGPASSWORD': password} if password else {})


def bounded(command, extra_env=None):
    """Run a command within STEP_TIMEOUT; RuntimeError when it overruns."""
    try:
        return run_bounded(command, STEP_TIMEOUT, env=dict(os.environ, **(extra_env or {})))
    except subprocess.TimeoutExpired as error:
        raise RuntimeError(f'{command[0]} overran {error.timeout:.0f}s and was stopped') from error


def psql(database_url, sql):
    """Rows of a query as lists of strings."""
    out = client(['psql', '--no-psqlrc', '--quiet', '--tuples-only', '--no-align',
                  f'--field-separator={SEP}', '--set=ON_ERROR_STOP=1'], database_url, '-c', sql)
    if out.returncode:
        raise RuntimeError(out.stderr.strip().splitlines()[-1] if out.stderr.strip() else 'psql failed')
    return [line.split(SEP) for line in out.stdout.splitlines() if line]


def ident(name):
    return '"' + name.replace('"', '""') + '"'


def label(table):
    return f'{table[0]}.{table[1]}'


TABLES = """
SELECT n.nspname, c.relname, pg_get_userbyid(c.relowner)
FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
WHERE c.relkind IN ('r', 'p') AND n.nspname NOT IN ('pg_catalog', 'information_schema')
  AND n.nspname NOT LIKE 'pg_toast%' ORDER BY 1, 2"""


def tables(database_url):
    """Each table, as (schema, name), with its owner."""
    return {(schema, name): owner for schema, name, owner in psql(database_url, TABLES)}


def counts(database_url, names):
    """Row counts of the (schema, name) tables."""
    names = list(names)
    if not names:
        return {}
    sql = ' UNION ALL '.join(f'SELECT {i}, count(*) FROM {ident(schema)}.{ident(table)}'
                             for i, (schema, table) in enumerate(names))
    return {names[int(i)]: int(n) for i, n in psql(database_url, sql)}


def bash(command, extra_env):
    return bounded(['bash', '-c', command], extra_env)


maintenance = with_database(SCRATCH, 'postgres')
try:
    exists = psql(maintenance, f"SELECT 1 FROM pg_database WHERE datname = '{scratch_db}'")
except RuntimeError as error:
    fail('scratch database', f'cannot reach {scratch_host}: {error}')
if exists:
    fail('scratch database', f'{scratch_db} already exists; drop it or name a new one')

# 1. The backup.
try:
    if os.environ.get('QA_BACKUP_COMMAND'):
        backup = bash(os.environ['QA_BACKUP_COMMAND'], {})
    else:
        dump = WORK_DIR / 'backup.dump'
        backup = client(['pg_dump', '--format=custom', f'--file={dump}'], SOURCE)
    source_tables = tables(SOURCE)
    source_counts = counts(SOURCE, source_tables)
except RuntimeError as error:
    fail('backup', str(error))
if os.environ.get('QA_BACKUP_COMMAND'):
    lines = [line for line in backup.stdout.splitlines() if line.strip()]
    dump = Path(lines[-1].strip()) if lines else None
    if backup.returncode or not dump or not dump.is_file():
        fail('backup', f'QA_BACKUP_COMMAND exited {backup.returncode}; last line: {lines[-1] if lines else ""}')
elif backup.returncode:
    fail('backup', backup.stderr.strip()[-300:])
check('backup', dump.stat().st_size > 0,
      f'{dump.name}: {dump.stat().st_size} bytes, {len(source_tables)} tables in {source_db}')

# 2. The restore, into a database created for it.
created = False
try:
    psql(maintenance, f'CREATE DATABASE "{scratch_db}"')
    created = True
    if os.environ.get('QA_RESTORE_COMMAND'):
        restore = bash(os.environ['QA_RESTORE_COMMAND'],
                       {'QA_BACKUP_PATH': str(dump), 'QA_SCRATCH_DATABASE_URL': SCRATCH})
    else:
        restore = client(['pg_restore', '--exit-on-error'], SCRATCH, str(dump))
    check('restore', restore.returncode == 0,
          f'into {scratch_db}' if restore.returncode == 0 else restore.stderr.strip()[-300:])

    # 3. Ownership and contents.
    restored = tables(SCRATCH)
    missing = sorted(set(source_tables) - set(restored))
    check('every table restored', not missing and bool(restored),
          f'{len(restored)} of {len(source_tables)} tables'
          + (f'; missing {[label(t) for t in missing[:5]]}' if missing else ''))
    wrong_owner = sorted(t for t in restored if t in source_tables and restored[t] != source_tables[t])
    check('ownership kept', not wrong_owner,
          'every table has its original owner' if not wrong_owner
          else f'{len(wrong_owner)} tables changed owner, e.g. {label(wrong_owner[0])}: '
               f'{source_tables[wrong_owner[0]]} -> {restored[wrong_owner[0]]}')
    if EXPECTED_OWNER:
        others = sorted(t for t, owner in restored.items() if owner != EXPECTED_OWNER)
        check('expected owner', not others,
              f'every table owned by {EXPECTED_OWNER}' if not others
              else f'{len(others)} tables not owned by {EXPECTED_OWNER}, e.g. {label(others[0])}')
    restored_counts = counts(SCRATCH, [t for t in restored if t in source_counts])
    differ = sorted(t for t in restored_counts if restored_counts[t] != source_counts[t])
    check('row counts match', not differ,
          f'{sum(restored_counts.values())} rows in {len(restored_counts)} tables' if not differ
          else f'{len(differ)} tables differ, e.g. {label(differ[0])}: {source_counts[differ[0]]} -> '
               f'{restored_counts[differ[0]]}')
except RuntimeError as error:
    check('restore', False, str(error))
finally:
    if created and not KEEP:
        try:
            psql(maintenance, f'DROP DATABASE "{scratch_db}"')
        except RuntimeError as error:
            check('drop the scratch database', False, str(error))

finish()
