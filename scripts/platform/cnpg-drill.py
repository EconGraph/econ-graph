#!/usr/bin/env python3
"""Opt-in copy/restore/drain acceptance on explicitly owned scratch CNPG resources."""
import argparse
import copy
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time
import uuid
from safety import Kubernetes, LABEL, DISPOSABLE, identifier, sql_identifier


def poll(read, accepted, seconds=1200):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        value = read()
        if accepted(value):
            return value
        time.sleep(5)
    raise RuntimeError('bounded wait expired; retain owned resources for inspection')


def primary(kube, namespace, cluster):
    value = kube.get('cluster', cluster, namespace)
    pod = value.get('status', {}).get('currentPrimary')
    if not pod:
        raise RuntimeError('CNPG primary unavailable')
    return pod


def sql(kube, args, cluster, statement, database='postgres', user='postgres'):
    sql_identifier(database)
    if user != 'postgres':
        statement = 'SET ROLE \"' + identifier(user) + '\";\n' + statement
    return kube.run('exec', '-i', '-n', args.namespace, primary(kube, args.namespace, cluster), '-c', 'postgres',
                    '--', 'psql', '-X', '-v', 'ON_ERROR_STOP=1', '-U', 'postgres', '-d', database, '-qAt', input=statement)


def recovery_cluster(source, name, owner, object_store, target):
    spec = copy.deepcopy(source['spec'])
    spec.pop('managed', None)
    spec.pop('replica', None)
    spec['bootstrap'] = {'recovery': {'source': 'probe-source', 'recoveryTarget': {'targetTime': target}}}
    spec['externalClusters'] = [{'name': 'probe-source', 'plugin': {
        'name': 'barman-cloud.cloudnative-pg.io',
        'parameters': {'barmanObjectName': object_store, 'serverName': source['metadata']['name']}}}]
    return {'apiVersion': 'postgresql.cnpg.io/v1', 'kind': 'Cluster',
            'metadata': {'name': name, 'namespace': source['metadata']['namespace'], 'labels': {LABEL: owner}, 'annotations': {DISPOSABLE: 'true'}}, 'spec': spec}


def backup_restore(kube, args, source, report):
    kube.disposable_owned('objectstore.barmancloud.cnpg.io', args.object_store, args.owner, args.namespace)
    configured = [(p.get('name'), p.get('parameters', {}).get('barmanObjectName'))
                  for p in source['spec'].get('plugins', []) if p.get('isWALArchiver')]
    if configured != [('barman-cloud.cloudnative-pg.io', args.object_store)]:
        raise ValueError('source WAL archiver must use the exact owned ObjectStore')
    suffix = uuid.uuid4().hex[:10]
    probe = 'train4_probe_' + suffix
    backup_name, restored = 'probe-backup-' + suffix, 'probe-restore-' + suffix
    report['retained_resources'] = {'table': probe, 'backup': backup_name, 'cluster': restored}
    # All writes stay in this previously owner-marked scratch cluster.
    sql(kube, args, args.source_cluster, f'CREATE TABLE public.{probe}(id integer PRIMARY KEY); INSERT INTO public.{probe} VALUES (1); CHECKPOINT;')
    kube.apply({'apiVersion': 'postgresql.cnpg.io/v1', 'kind': 'Backup', 'metadata': {
        'name': backup_name, 'namespace': args.namespace, 'labels': {LABEL: args.owner}},
        'spec': {'cluster': {'name': args.source_cluster}, 'method': 'plugin',
                 'pluginConfiguration': {'name': 'barman-cloud.cloudnative-pg.io'}}})
    def backup_done(value):
        phase = value.get('status', {}).get('phase', '')
        if phase == 'failed':
            raise RuntimeError('actual plugin backup failed')
        return phase == 'completed'
    backup = poll(lambda: kube.get('backup', backup_name, args.namespace), backup_done)
    target = sql(kube, args, args.source_cluster, "SELECT to_char(clock_timestamp() AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"');").strip()
    wal = sql(kube, args, args.source_cluster,
              f'INSERT INTO public.{probe} VALUES (2); SELECT pg_walfile_name(pg_switch_wal());').strip()
    if not re.fullmatch(r'[0-9A-F]{24}', wal):
        raise RuntimeError('PostgreSQL did not identify the switched WAL segment')
    # CNPG 1.30's Cluster status has no lastArchivedWAL. PostgreSQL exposes the
    # exact segment's archive-success marker, including when archive_command
    # delegates to CNPG's configured Barman plugin. A newer last_archived_wal
    # alone would not prove this segment succeeded with parallel archiving.
    poll(lambda: sql(kube, args, args.source_cluster,
                    f"SELECT EXISTS (SELECT 1 FROM pg_ls_archive_statusdir() WHERE name = '{wal}.done');").strip(),
         lambda done: done == 't')
    kube.apply(recovery_cluster(source, restored, args.owner, args.object_store, target))
    poll(lambda: kube.get('cluster', restored, args.namespace),
         lambda c: c.get('status', {}).get('readyInstances', 0) == c['spec']['instances'])
    ids = sql(kube, args, restored, f'SELECT string_agg(id::text,\',\' ORDER BY id) FROM public.{probe};').strip()
    if ids != '1':
        raise RuntimeError('PITR failed: expected pre-target row and exclusion of post-target row')
    report.update({'base_backup_phase': backup['status']['phase'], 'wal_archive_advanced': True,
                   'archived_wal_segment': wal, 'wal_evidence': 'PostgreSQL exact segment archive-success marker',
                   'pitr_restored_ids': ids, 'recovery_target': target, 'status': 'passed'})


def migrate_copy(kube, args, source, report):
    if not args.archive or not args.manifest:
        raise ValueError('migrate-copy requires offline custom-format --archive and --manifest')
    manifest = json.loads(args.manifest.read_text())
    digest = hashlib.file_digest(args.archive.open('rb'), 'sha256').hexdigest()
    if digest != manifest['archive_sha256']:
        raise ValueError('archive checksum mismatch')
    database = sql_identifier(source['spec']['bootstrap']['initdb']['database'])
    role = identifier(source['spec']['bootstrap']['initdb']['owner'])
    if not manifest.get('table_counts') or any(not re.fullmatch(r'[a-z_][a-z0-9_]*\.[a-z_][a-z0-9_]*', t) or not isinstance(n, int) or n < 0 for t, n in manifest['table_counts'].items()):
        raise ValueError('nonempty valid table-count manifest required before restoring')
    count = sql(kube, args, args.source_cluster, "SELECT count(*) FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname NOT IN ('pg_catalog','information_schema') AND n.nspname NOT LIKE 'pg_toast%' AND c.relkind IN ('r','p');", database).strip()
    if count != '0':
        raise ValueError('copy target must be empty; existing tables are never dropped')
    command = [*kube.prefix, 'exec', '-i', '-n', args.namespace, primary(kube, args.namespace, args.source_cluster), '-c', 'postgres', '--',
               'pg_restore', '--exit-on-error', '--single-transaction', '--no-owner', '--no-acl', '--no-comments', '--role', role, '-U', 'postgres', '-d', database]
    with args.archive.open('rb') as archive:
        outcome = subprocess.run(command, stdin=archive, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, timeout=1800)
    if outcome.returncode:
        raise RuntimeError('copy restore failed; inspect owned target privately')
    counts = {}
    for table, expected in manifest['table_counts'].items():
        if not re.fullmatch(r'[a-z_][a-z0-9_]*\.[a-z_][a-z0-9_]*', table):
            raise ValueError('manifest table names must be simple schema.table identifiers')
        count = int(sql(kube, args, args.source_cluster, 'SELECT count(*) FROM ' + table + ';', database, role).strip())
        if count != expected:
            raise RuntimeError('restored table count mismatch: ' + table)
        counts[table] = count
    if not counts:
        raise ValueError('nonempty source table-count evidence is required')
    migrated = False
    if args.migration_argv:
        command = json.loads(args.migration_argv.read_text())
        if not isinstance(command, list) or not command or not all(isinstance(c, str) for c in command):
            raise ValueError('migration command must be a nonempty JSON argv array')
        env = dict(os.environ, TRAIN4_NAMESPACE=args.namespace, TRAIN4_CONTEXT=args.context, TRAIN4_CLUSTER=args.source_cluster)
        with args.report.with_suffix('.migration.log').open('w') as log:
            subprocess.run(command, env=env, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=1800)
        for table, expected in counts.items():
            if int(sql(kube, args, args.source_cluster, 'SELECT count(*) FROM ' + table + ';', database, role).strip()) != expected:
                raise RuntimeError('migration changed preserved table count: ' + table)
        migrated = True
    report.update({'status': 'passed' if migrated else 'copy-verified-migration-pending', 'migration_executed': migrated, 'archive_sha256': digest, 'table_counts': counts,
                   'method': 'offline archive restored into empty owned copy; no source DB contacted',
                   'limitations': ['Counts are verified; caller must run application/schema-specific integrity and release E2E checks.']})


def drain(kube, args, source, report):
    if not args.node or not args.e2e_argv:
        raise ValueError('drain requires owned --node and --e2e-argv JSON file')
    node = kube.disposable_owned('node', args.node, args.owner)
    if node.get('spec', {}).get('unschedulable'):
        raise ValueError('refusing already cordoned node')
    command = json.loads(args.e2e_argv.read_text())
    if not isinstance(command, list) or not command or not all(isinstance(c, str) for c in command):
        raise ValueError('E2E command must be a nonempty JSON argv array')
    if source['spec']['instances'] < 2:
        raise ValueError('drain requires redundant CNPG instances')
    # Caller supplies the existing real release-E2E command. No shell interpolation.
    started = dt.datetime.now(dt.timezone.utc).isoformat()
    with args.report.with_suffix('.e2e.log').open('w') as log:
        process = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT)
        try:
            drain_start = dt.datetime.now(dt.timezone.utc)
            kube.run('drain', args.node, '--ignore-daemonsets', '--delete-emptydir-data', '--timeout=10m', timeout=660)
            drain_end = dt.datetime.now(dt.timezone.utc)
            code = process.wait(timeout=1200)
            if code:
                raise RuntimeError('release E2E failed during drain')
            poll(lambda: kube.get('cluster', args.source_cluster, args.namespace),
                 lambda c: bool(c.get('status', {}).get('currentPrimary')) and any(x.get('type') == 'Ready' and x.get('status') == 'True' for x in c.get('status', {}).get('conditions', [])), seconds=300)
        finally:
            try:
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=30)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=30)
            finally:
                # E2E shutdown failure must not strand the same owned node cordoned.
                current = kube.disposable_owned('node', args.node, args.owner)
                if current['metadata']['uid'] != node['metadata']['uid']:
                    raise ValueError('node replaced: refusing uncordon of replacement')
                kube.run('uncordon', args.node)
    coverage = False
    if args.e2e_evidence:
        evidence = json.loads(args.e2e_evidence.read_text())
        requests = evidence.get('requests', [])
        times = [dt.datetime.fromisoformat(x['completed_at'].replace('Z', '+00:00')) for x in requests if x.get('success') is True]
        coverage = bool(times) and min(times) < drain_start and max(times) > drain_end and all(x.get('success') is True for x in requests)
        if not coverage:
            raise RuntimeError('E2E evidence does not show successful requests before and after drain without failures')
    report.update({'status': 'passed' if coverage else 'drain-completed-coverage-pending', 'e2e_coverage_verified': coverage, 'e2e_exit': code, 'started_at': started, 'node_uncordoned': True,
                   'limitations': ['Verify E2E report includes requests spanning the drain; this harness cannot infer workload coverage.']})


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('mode', choices=['backup-restore', 'migrate-copy', 'drain'])
    p.add_argument('--kubeconfig', type=Path, required=True)
    p.add_argument('--context', required=True)
    p.add_argument('--namespace', required=True)
    p.add_argument('--owner', required=True)
    p.add_argument('--source-cluster', required=True)
    p.add_argument('--object-store')
    p.add_argument('--archive', type=Path)
    p.add_argument('--manifest', type=Path)
    p.add_argument('--migration-argv', type=Path)
    p.add_argument('--node')
    p.add_argument('--e2e-argv', type=Path)
    p.add_argument('--e2e-evidence', type=Path)
    p.add_argument('--report', type=Path, required=True)
    p.add_argument('--kubectl', default='kubectl')
    p.add_argument('--execute', action='store_true')
    args = p.parse_args()
    for value in [args.owner, args.namespace, args.source_cluster]:
        identifier(value)
    report = {'status': 'dry-run', 'mode': args.mode, 'owner': args.owner, 'namespace': args.namespace,
              'cluster': args.source_cluster, 'context': args.context, 'backup_deletion': False}
    try:
        if args.execute:
            kube = Kubernetes(args.kubectl, args.kubeconfig, args.context)
            kube.disposable_owned('namespace', args.namespace, args.owner)
            source = kube.disposable_owned('cluster', args.source_cluster, args.owner, args.namespace)
            # Every configured WAL destination can receive writes during any drill.
            for plugin in source['spec'].get('plugins', []):
                if plugin.get('isWALArchiver'):
                    store = plugin.get('parameters', {}).get('barmanObjectName')
                    if not store:
                        raise ValueError('WAL archiver must name an explicitly disposable ObjectStore')
                    kube.disposable_owned('objectstore.barmancloud.cnpg.io', store, args.owner, args.namespace)
            {'backup-restore': backup_restore, 'migrate-copy': migrate_copy, 'drain': drain}[args.mode](kube, args, source, report)
    except Exception as error:
        report.update({'status': 'unavailable-or-failed', 'reason': str(error)})
        raise
    finally:
        args.report.write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps(report, indent=2))


if __name__ == '__main__':
    try:
        main()
    except (ValueError, RuntimeError, OSError, subprocess.SubprocessError) as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
