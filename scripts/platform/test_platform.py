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

    def test_health_requires_all_planes_wal_and_recent_backup(self):
        now = dt.datetime.now(dt.timezone.utc)
        clusters = [{'metadata': {'name': name}, 'status': {'lastSuccessfulBackup': now.isoformat(), 'conditions': [{'type': 'ContinuousArchiving', 'status': 'True'}]}} for name in health.LIMITS]
        self.assertEqual(health.assess(clusters, [], now), [])
        for name in health.LIMITS:
            broken = copy.deepcopy(clusters)
            next(x for x in broken if x['metadata']['name'] == name)['status']['conditions'] = []
            self.assertTrue(any(name in x for x in health.assess(broken, [], now)))
        self.assertTrue(health.assess([], [], now))
        clusters[0]['status']['lastSuccessfulBackup'] = (now - dt.timedelta(days=1)).isoformat()
        self.assertTrue(health.assess(clusters, [], now))

    def test_latest_failed_backup_alerts_without_permanent_old_failure(self):
        now = dt.datetime.now(dt.timezone.utc)
        clusters = [{'metadata': {'name': name}, 'status': {'lastSuccessfulBackup': now.isoformat(), 'conditions': [{'type': 'ContinuousArchiving', 'status': 'True'}]}} for name in health.LIMITS]
        def backup(date, phase):
            return {'metadata': {'creationTimestamp': date}, 'spec': {'cluster': {'name': 'econgraph-app'}}, 'status': {'phase': phase}}
        self.assertTrue(health.assess(clusters, [backup('2026-10-30', 'failed')], now))
        self.assertEqual(health.assess(clusters, [backup('2026-10-29', 'failed'), backup('2026-10-30', 'completed')], now), [])

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

    def test_cilium_talos_hostdns_contract(self):
        release = yaml.safe_load((ROOT / 'platform/addons/cilium/release.yaml').read_text())
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
