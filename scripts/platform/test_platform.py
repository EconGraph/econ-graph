#!/usr/bin/env python3
"""Behavioral guards; no Kubernetes, S3, VM or database access."""
import copy
import datetime as dt
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from types import SimpleNamespace
from unittest.mock import patch, Mock
import yaml

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))
import safety
import validate


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


health = module('health', ROOT / 'platform/observability/backup-health.py')
drill = module('drill', HERE / 'cnpg-drill.py')


class PlatformTests(unittest.TestCase):
    def test_dry_runs_cannot_contact_even_nonexistent_tools(self):
        with tempfile.TemporaryDirectory() as temporary:
            common = ['--context', 'invalid', '--kubeconfig', temporary + '/absent', '--owner', 'test', '--kubectl', temporary + '/absent-kubectl']
            result = subprocess.run([sys.executable, str(HERE / 'bootstrap.py'), 'cni', *common, '--control-node', 'test', '--cache', temporary], check=True, capture_output=True, text=True)
            self.assertEqual(json.loads(result.stdout)['mode'], 'dry-run')
            report = temporary + '/report.json'
            subprocess.run([sys.executable, str(HERE / 'cnpg-drill.py'), 'backup-restore', *common, '--namespace', 'test', '--source-cluster', 'test', '--report', report], check=True, capture_output=True, text=True)
            self.assertEqual(json.loads(Path(report).read_text())['status'], 'dry-run')

    def test_execute_unavailable_never_reports_pass(self):
        with tempfile.TemporaryDirectory() as temporary:
            report = temporary + '/report.json'
            result = subprocess.run([sys.executable, str(HERE / 'cnpg-drill.py'), 'backup-restore', '--context', 'test', '--kubeconfig', temporary + '/absent', '--owner', 'test', '--namespace', 'test', '--source-cluster', 'test', '--report', report, '--execute'], capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(json.loads(Path(report).read_text())['status'], 'unavailable-or-failed')

    def test_scratch_environment_cannot_target_production_or_other_owner(self):
        owner = '12345678-abcd-4321-abcd-123456789abc'
        self.assertEqual(safety.scratch_environment('fit-12345678abcd4321', owner), 'fit-12345678abcd4321')
        for environment in ['production', 'fit-replace-me', 'scratch-another', '', None]:
            with self.assertRaises(ValueError):
                safety.scratch_environment(environment, owner)

    def test_operational_owner_does_not_authorize_disposable_drills(self):
        kube = object.__new__(safety.Kubernetes)
        owned = {'metadata': {'labels': {safety.LABEL: 'owner'}}}
        with patch.object(kube, 'get', return_value=owned):
            for resource in ['namespace', 'cluster', 'objectstore.barmancloud.cnpg.io', 'node']:
                with self.assertRaises(ValueError):
                    kube.disposable_owned(resource, 'test', 'owner')
        owned['metadata']['annotations'] = {safety.DISPOSABLE: 'true'}
        with patch.object(kube, 'get', return_value=owned):
            self.assertEqual(kube.disposable_owned('cluster', 'test', 'owner'), owned)

    def test_database_names_cannot_redirect_libpq(self):
        kube = Mock()
        for value in ['postgresql://other/db', 'host=other dbname=prod', 'app db', 'app;drop', '', None]:
            with self.assertRaises(ValueError):
                drill.sql(kube, SimpleNamespace(namespace='test'), 'test', 'SELECT 1', value)
        kube.run.assert_not_called()
        self.assertEqual(safety.sql_identifier('econ_graph_app'), 'econ_graph_app')

    def test_wrong_owner_refused(self):
        kube = object.__new__(safety.Kubernetes)
        with patch.object(kube, 'get', return_value={'metadata': {'labels': {safety.LABEL: 'other'}}}):
            with self.assertRaises(ValueError):
                kube.owned('namespace', 'test', 'owner')

    def healthy_plugin_fixtures(self, now):
        clusters = [{'metadata': {'name': name}, 'status': {'conditions': [{'type': 'ContinuousArchiving', 'status': 'True'}]}} for name in health.LIMITS]
        backups = [{'metadata': {'creationTimestamp': (now - dt.timedelta(minutes=1)).isoformat()},
                    'spec': {'cluster': {'name': name}, 'method': 'plugin',
                             'pluginConfiguration': {'name': 'barman-cloud.cloudnative-pg.io'}},
                    'status': {'phase': 'completed', 'stoppedAt': now.isoformat()}} for name in health.LIMITS]
        return clusters, backups

    def test_health_requires_all_planes_wal_and_recent_plugin_backup(self):
        now = dt.datetime.now(dt.timezone.utc)
        clusters, backups = self.healthy_plugin_fixtures(now)
        self.assertEqual(health.assess(clusters, backups, now), [])
        for name in health.LIMITS:
            broken = copy.deepcopy(clusters)
            next(x for x in broken if x['metadata']['name'] == name)['status']['conditions'] = []
            self.assertTrue(any(name in x for x in health.assess(broken, backups, now)))
        self.assertTrue(health.assess([], backups, now))
        self.assertTrue(health.assess(clusters, [], now))
        backups[0]['status']['stoppedAt'] = (now - dt.timedelta(days=1)).isoformat()
        self.assertTrue(health.assess(clusters, backups, now))

    def test_latest_failed_backup_alerts_without_permanent_old_failure(self):
        now = dt.datetime.now(dt.timezone.utc)
        clusters, backups = self.healthy_plugin_fixtures(now)
        failed = copy.deepcopy(backups[0])
        failed['metadata']['creationTimestamp'] = (now + dt.timedelta(seconds=1)).isoformat()
        failed['status'] = {'phase': 'failed'}
        self.assertTrue(health.assess(clusters, backups + [failed], now))
        failed['metadata']['creationTimestamp'] = (now - dt.timedelta(days=1)).isoformat()
        self.assertEqual(health.assess(clusters, backups + [failed], now), [])

    def test_plugin_backup_missing_invalid_future_timestamps_fail_closed(self):
        now = dt.datetime.now(dt.timezone.utc)
        for timestamp in [None, 'invalid', now.replace(tzinfo=None).isoformat(),
                          (now + dt.timedelta(minutes=6)).isoformat()]:
            clusters, backups = self.healthy_plugin_fixtures(now)
            # A legacy Cluster timestamp must never mask missing plugin evidence.
            clusters[0]['status']['lastSuccessfulBackup'] = now.isoformat()
            backups[0]['status']['stoppedAt'] = timestamp
            self.assertTrue(health.assess(clusters, backups, now))

    def test_only_completed_correct_plugin_backups_satisfy_age_check(self):
        now = dt.datetime.now(dt.timezone.utc)
        for changed in ['pending', 'other-cluster', 'other-plugin']:
            clusters, backups = self.healthy_plugin_fixtures(now)
            if changed == 'pending':
                backups[0]['status']['phase'] = 'running'
            elif changed == 'other-cluster':
                backups[0]['spec']['cluster']['name'] = 'different-cluster'
            else:
                backups[0]['spec']['pluginConfiguration']['name'] = 'different-plugin'
            self.assertTrue(health.assess(clusters, backups, now))

    def test_latest_completed_backup_recovers_from_old_stale_and_failed_records(self):
        now = dt.datetime.now(dt.timezone.utc)
        clusters, backups = self.healthy_plugin_fixtures(now)
        stale = copy.deepcopy(backups[0])
        stale['metadata']['creationTimestamp'] = (now - dt.timedelta(days=1)).isoformat()
        stale['status']['stoppedAt'] = (now - dt.timedelta(days=1)).isoformat()
        running = copy.deepcopy(backups[0])
        running['metadata']['creationTimestamp'] = now.isoformat()
        running['status'] = {'phase': 'running'}
        self.assertEqual(health.assess(clusters, [stale, running, *backups], now), [])

    def test_backup_checker_rbac_stays_namespace_scoped_read_only(self):
        documents = list(yaml.safe_load_all((ROOT / 'platform/observability/backup-health.yaml').read_text()))
        roles = [d for d in documents if d['kind'] in ['Role', 'ClusterRole']]
        self.assertEqual(len(roles), 1)
        self.assertEqual(roles[0]['kind'], 'Role')
        self.assertEqual(roles[0]['metadata']['namespace'], 'econ-graph')
        self.assertEqual(roles[0]['rules'], [{'apiGroups': ['postgresql.cnpg.io'], 'resources': ['clusters', 'backups'], 'verbs': ['get', 'list']}])

    def test_cilium_api_egress_is_entity_and_port_scoped(self):
        policies = list(yaml.safe_load_all((ROOT / 'platform/application/api-egress.yaml').read_text()))
        self.assertEqual(len(policies), 2)
        for policy in policies:
            self.assertEqual(policy['spec']['egress'], [{'toEntities': ['kube-apiserver'], 'toPorts': [{'ports': [{'port': '6443', 'protocol': 'TCP'}]}]}])
        cnpg, checker = [p['spec']['endpointSelector'] for p in policies]
        self.assertEqual(cnpg['matchExpressions'][0]['values'], sorted(health.LIMITS))
        self.assertEqual(checker, {'matchLabels': {'app': 'cnpg-backup-health'}})
        self.assertNotIn('${CONTROL_PLANE_CIDR}', (ROOT / 'platform/application/network-policies.yaml').read_text())

    def test_disposable_drain_allows_scratch_emptydir_and_preserves_eviction(self):
        for scenario in ['success', 'drain-failure', 'stubborn-e2e']:
            failure = scenario != 'success'
            with tempfile.TemporaryDirectory() as temporary:
                command = Path(temporary) / 'argv.json'
                command.write_text(json.dumps(['real-release-e2e']))
                args = SimpleNamespace(node='worker-1', owner='proof', e2e_argv=command,
                                       e2e_evidence=None, report=Path(temporary) / 'report.json',
                                       source_cluster='copy', namespace='proof')
                kube = Mock()
                kube.disposable_owned.return_value = {'metadata': {'uid': 'same-node'}, 'spec': {}}
                kube.get.return_value = {'status': {'currentPrimary': 'copy-1', 'conditions': [{'type': 'Ready', 'status': 'True'}]}}
                def operation(*argv, **kwargs):
                    if argv[0] == 'drain':
                        self.assertIn('--delete-emptydir-data', argv)
                        self.assertIn('--ignore-daemonsets', argv)
                        self.assertIn('--timeout=10m', argv)
                        self.assertNotIn('--force', argv)
                        self.assertNotIn('--disable-eviction', argv)
                        if failure:
                            raise RuntimeError('PDB refuses drain')
                    return ''
                kube.run.side_effect = operation
                process = Mock()
                process.wait.return_value = 0
                process.poll.return_value = None if scenario == 'stubborn-e2e' else 0
                if scenario == 'stubborn-e2e':
                    process.wait.side_effect = subprocess.TimeoutExpired('real-release-e2e', 30)
                with patch.object(drill.subprocess, 'Popen', return_value=process):
                    if failure:
                        expected_error = subprocess.TimeoutExpired if scenario == 'stubborn-e2e' else RuntimeError
                        with self.assertRaises(expected_error):
                            drill.drain(kube, args, {'spec': {'instances': 2}}, {})
                    else:
                        report = {}
                        drill.drain(kube, args, {'spec': {'instances': 2}}, report)
                        self.assertEqual(report['status'], 'drain-completed-coverage-pending')
                if scenario == 'stubborn-e2e':
                    process.terminate.assert_called_once()
                    process.kill.assert_called_once()
                kube.run.assert_any_call('uncordon', 'worker-1')
                self.assertFalse(any(call.args[0] == 'delete' for call in kube.run.call_args_list))

    def test_recovery_uses_source_only_for_reading_own_archive_namespace(self):
        source = next(yaml.safe_load_all((ROOT / 'platform/databases/clusters.yaml').read_text()))
        original = copy.deepcopy(source)
        recovery = drill.recovery_cluster(source, 'restore-test', 'test', 'app-backups', '2026-10-30T00:00:00Z')
        self.assertEqual(source, original)
        self.assertEqual(recovery['spec']['externalClusters'][0]['plugin']['parameters']['serverName'], 'econgraph-app')
        self.assertNotIn('initdb', recovery['spec']['bootstrap'])
        self.assertEqual(recovery['metadata']['labels'][safety.LABEL], 'test')

    def test_app_privilege_check_uses_peer_login_then_set_role(self):
        kube = Mock()
        kube.get.return_value = {'status': {'currentPrimary': 'test-1'}}
        drill.sql(kube, SimpleNamespace(namespace='test'), 'test', 'SELECT count(*) FROM public.example;', 'app', 'econgraph')
        command = kube.run.call_args
        self.assertEqual(command.args[command.args.index('-U') + 1], 'postgres')
        self.assertTrue(command.kwargs['input'].startswith('SET ROLE \"econgraph\";'))

    def test_live_client_skew_is_checked_before_api_calls(self):
        with tempfile.NamedTemporaryFile() as config:
            for version, accepted in [('v1.34.0', False), ('v1.35.9', True), ('v1.36.4', True), ('v1.37.1', True), ('v1.38.0', False)]:
                response = SimpleNamespace(stdout=json.dumps({'clientVersion': {'gitVersion': version}}))
                with patch('safety.subprocess.run', return_value=response) as run:
                    if accepted:
                        safety.Kubernetes('kubectl', config.name, 'owned')
                    else:
                        with self.assertRaises(ValueError):
                            safety.Kubernetes('kubectl', config.name, 'owned')
                    self.assertEqual(run.call_args.args[0], ['kubectl', 'version', '--client', '-o', 'json'])
                    self.assertEqual(run.call_count, 1)

    def test_cilium_talos_hostdns_contract(self):
        release = yaml.safe_load((ROOT / 'platform/addons/cilium/release.yaml').read_text())
        self.assertEqual(release['spec']['releaseName'], 'cilium')
        self.assertEqual(release['spec']['targetNamespace'], 'kube-system')
        self.assertEqual(release['spec']['storageNamespace'], 'kube-system')
        self.assertTrue(release['spec']['values']['bpf']['hostLegacyRouting'])
        self.assertFalse(release['spec']['values']['gatewayAPI']['enabled'])
        self.assertTrue(release['spec']['values']['kubeProxyReplacement'])

    def test_pin_checks_and_no_automatic_backup_delete(self):
        for path in (ROOT / 'platform').rglob('*.yaml'):
            if path.name.endswith('.sops.yaml'):
                continue
            text = path.read_text()
            self.assertNotIn('AGE-SECRET-KEY-', text)
            self.assertNotIn('retentionPolicy:', text)
            self.assertNotIn(':latest', text)
        for entry in validate.LOCK['charts'].values():
            self.assertRegex(entry['sha256'], r'^[0-9a-f]{64}$')


if __name__ == '__main__':
    unittest.main()
