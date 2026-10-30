#!/usr/bin/env python3
# qa-scenario: data-survives-redeploy
# qa-mutates: yes
# qa-profiles: lab
# qa-timeout: 7200
# qa-description: Record every data-database table's row count and row checksum, run the redeploy, and require them unchanged (v4.1.0 exit criterion 2).
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
  QA_DATA_ALLOW_RUNNING_CRAWLER   1 to go on while crawler-worker is scaled above 0, before
                                  or after the command. Otherwise that fails: a crawl changes
                                  the rows being compared. A redeploy that starts the crawler
                                  (deploy.sh sets it to 1 replica) therefore fails unless the
                                  command scales it back to 0 itself, and a crawl in between
                                  can still show as a change.

Each reading lists the base tables of the data database's public schema (except
__diesel_schema_migrations) and, per table, its row count and a checksum: the sum of a hash
of each whole row, so a changed value counts as well as an added or removed row.
Order-independent, so it streams through tables with millions of rows. After the command,
the kubectl is picked again (a kind cluster recreate changes it) and the data database's
StatefulSet must finish rolling out; then one check per table, plus one that the set of
tables is the same. The data database is only read.
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
SELECT t.table_name
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
    for (name,) in tables:
        ident = '"' + name.replace('"', '""') + '"'
        parts.append(f'SELECT {sql_literal(name)}, count(*), '
                     f'coalesce(sum(hashtextextended({ident}::text, 0)::numeric), 0) FROM {ident}')
    rows = kube.psql(NAMESPACE, POD, CONTAINER, DATABASE, '\nUNION ALL\n'.join(parts) + ';',
                     timeout=READ_TIMEOUT)
    return {name: (int(count), checksum) for name, count, checksum in rows}


def crawler_stopped(name, when):
    """Record whether crawler-worker is scaled to 0 (its spec, so a starting or crash-looping
    worker counts as running); end the scenario when it isn't and that isn't allowed."""
    workers = kube.items('deployments', '-n', NAMESPACE, '-l', 'app=crawler-worker')
    replicas = sum((d.get('spec') or {}).get('replicas', 1) for d in workers)
    if replicas and not ALLOW_CRAWLER:
        fail(name, f'crawler-worker is scaled to {replicas} in {NAMESPACE} {when}; it must be 0 for an '
             'exact comparison (or set QA_DATA_ALLOW_RUNNING_CRAWLER=1)')
    check(name, True, f'crawler-worker scaled to {replicas} {when}'
          + (' (allowed by QA_DATA_ALLOW_RUNNING_CRAWLER)' if replicas else ''))


try:
    crawler_stopped('crawler stopped', 'before the redeploy')
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
    # Unlike waiting on the pod, this also waits while the pod is still being recreated.
    kube.run('rollout', 'status', '-n', NAMESPACE, 'statefulset/postgresql', '--timeout=600s', timeout=660)
    crawler_stopped('crawler still stopped', 'after the redeploy')
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
