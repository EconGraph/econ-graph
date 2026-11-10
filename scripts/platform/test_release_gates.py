#!/usr/bin/env python3
"""Offline tests for release-gates.py. No cluster, host or external network."""
import contextlib
import datetime as dt
import io
import http.server
import importlib.util
import json
from pathlib import Path
import tempfile
import threading
import unittest
from types import SimpleNamespace

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('release_gates', HERE / 'release-gates.py')
gates = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gates)

UTC = dt.timezone.utc
NOW = dt.datetime(2026, 11, 10, 12, 0, tzinfo=UTC)


def deployment(name, want=1, have=1, available=True):
    return {'metadata': {'name': name}, 'spec': {'replicas': want},
            'status': {'availableReplicas': have,
                       'conditions': [{'type': 'Available', 'status': 'True' if available else 'False'}]}}


def cluster(name, ready=2, phase='Cluster in healthy state', archiving=True, plugin='barman-cloud.cloudnative-pg.io'):
    return {'metadata': {'name': name},
            'spec': {'instances': 2, 'plugins': [{'name': plugin, 'isWALArchiver': True}]},
            'status': {'readyInstances': ready, 'phase': phase,
                       'conditions': [{'type': 'ContinuousArchiving', 'status': 'True' if archiving else 'False'}]}}


def backup(name, cluster_name, stopped, phase='completed', created=None):
    created = created or stopped
    return {'metadata': {'name': name, 'creationTimestamp': created.isoformat()},
            'spec': {'cluster': {'name': cluster_name}, 'method': 'plugin',
                     'pluginConfiguration': {'name': 'barman-cloud.cloudnative-pg.io'}},
            'status': {'phase': phase, 'stoppedAt': stopped.isoformat() if phase == 'completed' else None}}


def hourly(cluster_name, start, end, every=dt.timedelta(hours=1), skip=()):
    result, at, n = [], start, 0
    while at <= end:
        if at not in skip:
            result.append(backup(f'{cluster_name}-{n}', cluster_name, at))
        at, n = at + every, n + 1
    return result


class CnpgStartupTests(unittest.TestCase):
    def healthy(self):
        return ([deployment('cnpg-cloudnative-pg'), deployment('barman-plugin-barman-cloud')],
                [cluster(n) for n in gates.CLUSTERS])

    def test_healthy_platform_passes(self):
        self.assertEqual(gates.check_cnpg_startup(*self.healthy())['failures'], [])

    def test_missing_plugin_deployment_fails(self):
        deployments, clusters = self.healthy()
        failures = gates.check_cnpg_startup(deployments[:1], clusters)['failures']
        self.assertIn('no barman Deployment in cnpg-system', failures)

    def test_unavailable_operator_fails(self):
        _, clusters = self.healthy()
        deployments = [deployment('cnpg-cloudnative-pg', have=0, available=False), deployment('barman-x')]
        self.assertTrue(any('0/1' in f for f in gates.check_cnpg_startup(deployments, clusters)['failures']))

    def test_each_cluster_condition_is_checked(self):
        deployments, _ = self.healthy()
        clusters = [cluster('econgraph-app', ready=1), cluster('econgraph-data', archiving=False),
                    cluster('econgraph-keycloak', plugin='other')]
        failures = gates.check_cnpg_startup(deployments, clusters)['failures']
        self.assertTrue(any(f.startswith('econgraph-app: 1/2') for f in failures))
        self.assertIn('econgraph-data: WAL archiving is not confirmed healthy', failures)
        self.assertIn('econgraph-keycloak: WAL archiver is not the barman plugin', failures)

    def test_missing_cluster_and_wrong_phase_fail(self):
        deployments, _ = self.healthy()
        failures = gates.check_cnpg_startup(deployments, [cluster('econgraph-app', phase='Setting up primary')])['failures']
        self.assertIn('econgraph-data: Cluster missing', failures)
        self.assertTrue(any('Setting up primary' in f for f in failures))


class BackupHistoryTests(unittest.TestCase):
    limits = {'econgraph-app': 7200}
    window = dt.timedelta(days=7)

    def run_check(self, backups):
        return gates.check_backup_history(backups, NOW, self.window, self.limits)

    def test_limits_come_from_backup_health_check(self):
        self.assertEqual(set(gates._backup_limits()), set(gates.CLUSTERS))

    def test_unbroken_hourly_history_passes(self):
        report = self.run_check(hourly('econgraph-app', NOW - dt.timedelta(days=8), NOW))
        self.assertEqual(report['failures'], [])
        self.assertEqual(report['clusters']['econgraph-app']['longest_gap_seconds'], 3600)

    def test_gap_longer_than_limit_fails(self):
        gap = {NOW - dt.timedelta(days=3, hours=h) for h in (0, 1, 2)}
        report = self.run_check(hourly('econgraph-app', NOW - dt.timedelta(days=8), NOW, skip=gap))
        self.assertTrue(any('longest time without' in f for f in report['failures']))

    def test_history_shorter_than_window_fails(self):
        report = self.run_check(hourly('econgraph-app', NOW - dt.timedelta(days=2), NOW))
        self.assertTrue(any('shorter than the window' in f for f in report['failures']))

    def test_stale_tail_counts_as_a_gap(self):
        report = self.run_check(hourly('econgraph-app', NOW - dt.timedelta(days=8), NOW - dt.timedelta(hours=5)))
        self.assertTrue(any('longest time without' in f for f in report['failures']))

    def test_failed_backup_in_window_fails_and_old_failure_is_ignored(self):
        backups = hourly('econgraph-app', NOW - dt.timedelta(days=8), NOW)
        backups.append(backup('old', 'econgraph-app', None, phase='failed', created=NOW - dt.timedelta(days=9)))
        self.assertEqual(self.run_check(backups)['failures'], [])
        backups.append(backup('new', 'econgraph-app', None, phase='failed', created=NOW - dt.timedelta(days=1)))
        self.assertTrue(any('1 failed backups' in f for f in self.run_check(backups)['failures']))

    def test_other_methods_and_clusters_are_ignored(self):
        backups = hourly('econgraph-app', NOW - dt.timedelta(days=8), NOW)
        stray = backup('snap', 'econgraph-app', NOW - dt.timedelta(minutes=1))
        stray['spec']['method'] = 'volumeSnapshot'
        backups += [stray, backup('x', 'other', NOW - dt.timedelta(minutes=1), phase='failed')]
        self.assertEqual(self.run_check(backups)['failures'], [])


class ResourceTests(unittest.TestCase):
    limits = {'requests': 0.85, 'usage': 0.80, 'disk': 0.80, 'min_samples': 2}
    nodes = [{'metadata': {'name': 'w1'}, 'status': {'allocatable': {'cpu': '16', 'memory': '64Gi'}}}]

    @staticmethod
    def pod(cpu_request, memory_request, phase='Running', init=None):
        spec = {'nodeName': 'w1', 'containers': [{'resources': {'requests': {'cpu': cpu_request, 'memory': memory_request}}}]}
        if init:
            spec['initContainers'] = [{'resources': {'requests': init}}]
        return {'spec': spec, 'status': {'phase': phase}}

    def test_quantities(self):
        self.assertEqual(gates.cpu('250m'), 0.25)
        self.assertEqual(gates.memory('1Gi'), 2**30)
        self.assertEqual(gates.memory('1G'), 10**9)
        with self.assertRaises(ValueError):
            gates.memory('lots')

    def test_init_container_request_counts_when_larger(self):
        pod = self.pod('100m', '1Gi', init={'cpu': '2', 'memory': '512Mi'})
        self.assertEqual(gates.pod_requests(pod), {'cpu': 2.0, 'memory': 2**30})

    @staticmethod
    def matrix(values_by_node):
        return {'status': 'success', 'data': {'resultType': 'matrix', 'result': [
            {'metric': {'node': node}, 'values': [[1762776000 + 30 * i, str(v)] for i, v in enumerate(values)]}
            for node, values in values_by_node.items()]}}

    def test_prometheus_ranges_and_df_parse(self):
        usage = gates.parse_usage(self.matrix({'w1': [1, 2]}), self.matrix({'w1': [2**30]}))
        self.assertEqual(usage, {'w1': {'cpu': [1.0, 2.0], 'memory': [float(2**30)]}})
        with self.assertRaises(ValueError):
            gates.parse_usage({'status': 'error'}, self.matrix({}))
        unlabelled = self.matrix({'w1': [1]})
        del unlabelled['data']['result'][0]['metric']['node']
        with self.assertRaises(ValueError):
            gates.parse_usage(unlabelled, self.matrix({}))
        disks = gates.parse_df('Filesystem 1-blocks Used Available Capacity Mounted on\n/dev/sda 100 85 15 85% /srv\n')
        self.assertEqual(disks, {'/srv': 0.85})

    def check(self, pods, cpu_values, memory_values, disks=None):
        usage = gates.parse_usage(self.matrix({'w1': cpu_values}), self.matrix({'w1': memory_values}))
        return gates.check_resources(self.nodes, pods, usage, disks or {}, self.limits)['failures']

    def test_within_limits_passes(self):
        pods = [self.pod('8', '32Gi'), self.pod('15', '60Gi', phase='Succeeded')]
        self.assertEqual(self.check(pods, [4, 8], [16 * 2**30, 32 * 2**30], {'/srv': 0.5}), [])

    def test_each_limit_fails(self):
        failures = self.check([self.pod('15', '1Gi')], [14], [16 * 2**30], {'/srv': 0.9})
        self.assertTrue(any('cpu requests' in f for f in failures))
        self.assertTrue(any('peak cpu use' in f for f in failures))
        self.assertTrue(any('/srv' in f for f in failures))
        self.assertTrue(any('usage samples' in f for f in failures))

    def test_node_without_usage_fails(self):
        usage = gates.parse_usage(self.matrix({'other': [1, 1]}), self.matrix({'other': [1, 1]}))
        failures = gates.check_resources(self.nodes, [], usage, {}, self.limits)['failures']
        self.assertIn('w1: 0 cpu usage samples, at least 2 required', failures)


class ProbeTests(unittest.TestCase):
    def test_records_success_and_failure_and_rewrites_file(self):
        responses = iter([200, 503, 200])

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_POST(self):
                self.rfile.read(int(self.headers['Content-Length']))
                code = next(responses)
                body = json.dumps({'data': {}} if code == 200 else {}).encode()
                self.send_response(code)
                self.send_header('Content-Length', str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *_):
                pass

        server = http.server.HTTPServer(('127.0.0.1', 0), Handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        self.addCleanup(server.shutdown)
        with tempfile.TemporaryDirectory() as directory:
            body = Path(directory, 'body.json')
            body.write_text('{"query": "{ __typename }"}')
            out = Path(directory, 'probe.json')
            ticks = iter(range(100))
            args = SimpleNamespace(url=f'http://127.0.0.1:{server.server_port}/graphql', json_body=body,
                                   graphql=True, cafile=None, interval=0, timeout=5, duration=4, out=out)
            with contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(gates.probe(args, clock=lambda: next(ticks), sleep=lambda _: None), 1)
            evidence = json.loads(out.read_text())
        self.assertEqual([r['success'] for r in evidence['requests']], [True, False, True])
        self.assertEqual(evidence['requests'][1]['status'], 503)
        for record in evidence['requests']:
            dt.datetime.fromisoformat(record['completed_at'])

    def test_graphql_errors_fail_the_request(self):
        class Response:
            status = 200

            def __enter__(self):
                return self

            def __exit__(self, *_):
                return False

            def read(self):
                return b'{"errors": [{"message": "no"}]}'

        original = gates.urllib.request.urlopen
        try:
            for body in (b'{"errors": [{"message": "no"}]}', b'[]', b'not json', b'{}'):
                Response.read = lambda self, body=body: body
                gates.urllib.request.urlopen = lambda *a, **k: Response()
                self.assertFalse(gates.request_once('https://x.invalid/graphql', '{}', 1, None, True)['success'], body)

            def truncated(self):  # the connection closed before Content-Length bytes arrived
                raise gates.http.client.IncompleteRead(b'{"da', 20)
            Response.read = truncated
            gates.urllib.request.urlopen = lambda *a, **k: Response()
            self.assertFalse(gates.request_once('https://x.invalid/graphql', '{}', 1, None, True)['success'])
        finally:
            gates.urllib.request.urlopen = original

    def test_rejects_non_http_url(self):
        args = SimpleNamespace(url='file:///etc/passwd', json_body=None)
        with self.assertRaises(ValueError):
            gates.probe(args)


class CommandLineTests(unittest.TestCase):
    def test_backup_history_exit_code_and_report(self):
        with tempfile.TemporaryDirectory() as directory:
            backups = Path(directory, 'backups.json')
            history = [b for name in gates.CLUSTERS for b in hourly(name, NOW - dt.timedelta(days=8), NOW)]
            backups.write_text(json.dumps({'items': history}))
            report = Path(directory, 'report.json')
            with contextlib.redirect_stdout(io.StringIO()):
                code = gates.main(['backup-history', '--backups', str(backups), '--now', NOW.isoformat(), '--report', str(report)])
                self.assertEqual(code, 0)
                self.assertTrue(json.loads(report.read_text())['passed'])
                backups.write_text(json.dumps({'items': history[:-200]}))
                self.assertEqual(gates.main(['backup-history', '--backups', str(backups), '--now', NOW.isoformat()]), 1)
                backups.write_text(json.dumps({'items': history}))
                # A tighter limit than the hourly schedule fails the same history.
                self.assertEqual(gates.main(['backup-history', '--backups', str(backups), '--now', NOW.isoformat(),
                                             '--limit', 'econgraph-app=1800']), 1)
            with self.assertRaises(ValueError):
                gates.main(['backup-history', '--backups', str(backups), '--limit', 'nope=1'])


if __name__ == '__main__':
    unittest.main()
