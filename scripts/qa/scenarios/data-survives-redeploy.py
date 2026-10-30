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

Run it in a quiet window, with the crawler in static mode (CRAWLER_SCHEDULER=false; see
docs/monitoring/crawler-coverage-runbook.md). Before and after each reading the data plane
must be quiet:
  - every running crawler-worker pod has its scheduler off (--scheduler in its args, else
    CRAWLER_SCHEDULER in its env, any value the worker reads as off; a value from a ConfigMap
    or Secret can't be read here and counts as on) and is ready, not still starting;
  - crawl_queue has no job pending, processing, retrying or locked by a worker.
The pods are read, not the deployment, so one left over from before a `set env` counts.
When the data plane isn't quiet, the scenario ends inconclusive (a skip whose reason starts
"inconclusive:"), neither a pass nor a failure, saying what to wait for.

Each reading lists the base tables of the data database's public schema (except
__diesel_schema_migrations) and, per table, its row count and a checksum: the sum of a hash
of each whole row, so a changed value counts as well as an added or removed row.
Order-independent, so it streams through tables with millions of rows. After the command,
the kubectl is picked again (a kind cluster recreate changes it) and the data database's
StatefulSet must finish rolling out. Then one check per table, plus one that the set of
tables is the same, except:
  - crawl_queue is not compared: it is the worker's own queue, whose finished jobs older
    than 14 days the worker purges at startup and hourly, also in static mode;
  - when the redeploy restarted crawler-worker and the only tables that changed are
    datasets and reference_file_cache, which a starting worker writes even in static mode
    (its build's dataset definitions, and reference data whose source changed), the
    scenario ends inconclusive. A change to any other table fails.
The data database is only read.
"""

import os
import subprocess

from qa import Kube, KubeError, check, fail, finish, skip, sql_literal
from qa import run as run_bounded

COMMAND = os.environ.get('QA_DATA_REDEPLOY_COMMAND', '')
NAMESPACE = os.environ.get('QA_DATA_NAMESPACE', 'econ-graph')
TIMEOUT = float(os.environ.get('QA_DATA_REDEPLOY_TIMEOUT_SECONDS', '2400'))
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


ACTIVE_JOBS = """
SELECT count(*) FROM crawl_queue
WHERE status IN ('pending', 'processing', 'retrying') OR locked_by IS NOT NULL"""
# The values the worker's --scheduler / CRAWLER_SCHEDULER parser (clap's BoolishValueParser)
# reads as off.
OFF = {'false', 'f', 'no', 'n', 'off', '0'}
# The worker's own queue, not crawled data: it purges finished jobs older than 14 days at
# startup and hourly, also in static mode.
NOT_COMPARED = {'crawl_queue'}
# What a starting worker writes even in static mode: the dataset definitions of its build,
# and reference data whose source changed (conditional GETs).
STARTUP_WRITES = {'datasets', 'reference_file_cache'}


def scheduler_off(container):
    """Whether the container runs the worker with its scheduler off: --scheduler in its
    args wins over CRAWLER_SCHEDULER in its env; a value from a ConfigMap or Secret can't be
    read here and counts as on."""
    args = [str(a) for a in container.get('args') or []]
    for i, arg in enumerate(args):
        if arg.startswith('--scheduler='):
            return arg.partition('=')[2].strip().lower() in OFF
        if arg == '--scheduler' and i + 1 < len(args):
            return args[i + 1].strip().lower() in OFF
    env = {e.get('name'): e.get('value') for e in container.get('env') or []}
    return (env.get('CRAWLER_SCHEDULER') or '').strip().lower() in OFF


def quiet(when):
    """Record that the data plane is quiet, and return the uids of the worker pods; end the
    scenario inconclusive when it isn't. The running pods are read, not the deployment, so
    a pod left from before a `set env` still counts."""
    pods = kube.items('pods', '-n', NAMESPACE, '-l', 'app=crawler-worker')
    why = []
    scheduling = [p['metadata']['name'] for p in pods
                  if not all(scheduler_off(c) for c in p['spec']['containers'])]
    if scheduling:
        why.append(f'{", ".join(scheduling)} runs with its scheduler on; set CRAWLER_SCHEDULER=false '
                   '(docs/monitoring/crawler-coverage-runbook.md)')
    starting = [p['metadata']['name'] for p in pods if not all(
        s.get('ready') for s in (p.get('status') or {}).get('containerStatuses') or [{}])]
    if starting:
        why.append(f'{", ".join(starting)} is still starting; wait for its rollout')
    jobs = int(kube.psql(NAMESPACE, POD, CONTAINER, DATABASE, ACTIVE_JOBS)[0][0])
    if jobs:
        why.append(f'crawl_queue has {jobs} pending, processing, retrying or locked jobs; wait for it to drain')
    if why:
        skip(f'inconclusive: the crawler is active {when}: {"; ".join(why)}. Run it again in a quiet window')
    check(f'quiet {when}', True, f'{len(pods)} crawler-worker pods, scheduler off; crawl_queue drained')
    return {p['metadata']['uid'] for p in pods}


try:
    workers_before = quiet('before the first reading')
    before = reading()
    quiet('after the first reading')
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
    quiet('before the second reading')
    after = reading()
    workers_after = quiet('after the second reading')
except KubeError as error:
    fail('after', str(error))

compared = sorted((set(before) & set(after)) - NOT_COMPARED)
changed = {name for name in compared if before[name] != after[name]}
restarted = bool(workers_after - workers_before)
if changed and restarted and changed <= STARTUP_WRITES:
    skip(f'inconclusive: only {", ".join(sorted(changed))} changed, which a starting crawler-worker writes '
         'even in static mode, and the redeploy restarted it. Run it again for a build with the same '
         'dataset definitions, when upstream reference data has not changed')

check('same tables', set(before) == set(after),
      'the same tables' if set(before) == set(after) else
      f'missing {sorted(set(before) - set(after))}, new {sorted(set(after) - set(before))}')
for name in compared:
    (rows_before, sum_before), (rows_after, sum_after) = before[name], after[name]
    check(f'{name} unchanged', rows_before == rows_after and sum_before == sum_after,
          f'rows {rows_before} -> {rows_after}, checksum '
          f'{"unchanged" if sum_before == sum_after else "changed"}', rows=rows_after)

finish()
