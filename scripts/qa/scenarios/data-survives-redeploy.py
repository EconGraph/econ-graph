#!/usr/bin/env python3
# qa-scenario: data-survives-redeploy
# qa-mutates: yes
# qa-profiles: lab
# qa-timeout: 5400
# qa-description: Record every data-database table's row count and id checksum, run the redeploy, and require them unchanged (v4.1.0 exit criterion 2).
"""The data database's rows survive a redeploy or an app-database recreate. Labs only.

Inputs:
  QA_KUBE_CONTEXT                 the kind context (e.g. kind-econ-graph); skipped without it.
  QA_DATA_REDEPLOY_COMMAND        run with bash between the two readings: the redeploy or
                                  app-database recreate under test (e.g. a branch's
                                  `scripts/deploy/deploy-branch.sh up ...`, or UAT's deploy).
                                  Skipped without it.
  QA_DATA_NAMESPACE               the data plane's namespace (default econ-graph); its data
                                  database is econ_graph in pod postgresql-0.
  QA_DATA_REDEPLOY_TIMEOUT_SECONDS
                                  how long the command may run (default 2400).
  QA_DATA_ALLOW_RUNNING_CRAWLER   1 to go on while crawler-worker has ready replicas. Otherwise
                                  that fails first: a crawl changes the rows being compared.

Each reading lists the base tables of the data database's public schema (except
__diesel_schema_migrations) and, per table, its row count and a checksum: the sum of a hash
of each row's id, or of the whole row for a table without an id column. Order-independent,
so it streams through tables with millions of rows. After the command, the data database's
pod must be Ready again (the kubectl is picked again first, as a kind cluster recreate
changes it); then one check per table, plus one that the set of tables is the same. The data database is only read.
"""

import os
import subprocess

from qa import Kube, KubeError, check, fail, finish, skip, sql_literal
from qa import run as run_bounded

COMMAND = os.environ.get('QA_DATA_REDEPLOY_COMMAND', '')
NAMESPACE = os.environ.get('QA_DATA_NAMESPACE', 'econ-graph')
TIMEOUT = float(os.environ.get('QA_DATA_REDEPLOY_TIMEOUT_SECONDS', '2400'))
ALLOW_CRAWLER = os.environ.get('QA_DATA_ALLOW_RUNNING_CRAWLER') == '1'
POD, CONTAINER, DATABASE = 'postgresql-0', 'postgresql', 'econ_graph'
# A whole table's checksum can take minutes on data_points.
READ_TIMEOUT = 1200

if not COMMAND:
    skip('no redeploy: set QA_DATA_REDEPLOY_COMMAND to the redeploy or app-database recreate to test')
kube = Kube()

TABLES = """
SELECT t.table_name, EXISTS (SELECT 1 FROM information_schema.columns c
                             WHERE c.table_schema = 'public' AND c.table_name = t.table_name
                               AND c.column_name = 'id')
FROM information_schema.tables t
WHERE t.table_schema = 'public' AND t.table_type = 'BASE TABLE'
  AND t.table_name <> '__diesel_schema_migrations'
ORDER BY 1"""


def reading():
    """{table: (rows, checksum)} of the data database."""
    tables = kube.psql(NAMESPACE, POD, CONTAINER, DATABASE, TABLES)
    if not tables:
        raise KubeError(f'{DATABASE} in {NAMESPACE}/{POD} has no tables')
    parts = []
    for name, has_id in tables:
        ident = '"' + name.replace('"', '""') + '"'
        hashed = f'{ident}.id::text' if has_id == 't' else f'{ident}::text'
        parts.append(f'SELECT {sql_literal(name)}, count(*), '
                     f'coalesce(sum(hashtextextended({hashed}, 0)::numeric), 0) FROM {ident}')
    rows = kube.psql(NAMESPACE, POD, CONTAINER, DATABASE, '\nUNION ALL\n'.join(parts) + ';',
                     timeout=READ_TIMEOUT)
    return {name: (int(count), checksum) for name, count, checksum in rows}


try:
    workers = kube.items('deployments', '-n', NAMESPACE, '-l', 'app=crawler-worker')
    ready = sum((d.get('status') or {}).get('readyReplicas') or 0 for d in workers)
    if ready and not ALLOW_CRAWLER:
        fail('crawler paused', f'crawler-worker has {ready} ready replicas in {NAMESPACE}; scale it to 0 '
             'for an exact comparison, or set QA_DATA_ALLOW_RUNNING_CRAWLER=1')
    check('crawler paused', True, f'{ready} ready crawler-worker replicas'
          + (' (allowed by QA_DATA_ALLOW_RUNNING_CRAWLER)' if ready else ''))
    before = reading()
    check('before', True, f'{len(before)} tables, {sum(r for r, _ in before.values())} rows',
          tables={name: rows for name, (rows, _) in before.items()})
except KubeError as error:
    fail('before', str(error))

try:
    done = run_bounded(['bash', '-c', COMMAND], TIMEOUT)
except subprocess.TimeoutExpired:
    fail('redeploy', f'QA_DATA_REDEPLOY_COMMAND overran {TIMEOUT:.0f}s and was stopped')
if done.returncode:
    fail('redeploy', f'QA_DATA_REDEPLOY_COMMAND exited {done.returncode}: {done.stderr.strip()[-300:]}')
check('redeploy', True, 'QA_DATA_REDEPLOY_COMMAND exited 0')

# A recreated kind cluster has a new kubeconfig: pick the kubectl again.
kube = Kube()
try:
    kube.run('wait', '-n', NAMESPACE, '--for=condition=Ready', f'pod/{POD}', '--timeout=600s', timeout=660)
    after = reading()
except KubeError as error:
    fail('after', str(error))

check('same tables', set(before) == set(after),
      'the same tables' if set(before) == set(after) else
      f'missing {sorted(set(before) - set(after))}, new {sorted(set(after) - set(before))}')
for name in sorted(set(before) & set(after)):
    (rows_before, sum_before), (rows_after, sum_after) = before[name], after[name]
    check(f'{name} unchanged', rows_before == rows_after and sum_before == sum_after,
          f'rows {rows_before} -> {rows_after}, checksum '
          f'{"unchanged" if sum_before == sum_after else "changed"}', rows=rows_after)

finish()
