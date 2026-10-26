#!/usr/bin/env python3
"""Offline safety and routing regression tests for disposable branch environments."""
import importlib.util
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import yaml

ROOT = Path(__file__).resolve().parents[2]


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


r = module('renderer', 'render-branch.py')
deploy = module('deploy', 'deploy-branch.py')


def by_name(docs, kind, name):
    return next(d for d in docs if d['kind'] == kind and d['metadata']['name'] == name)


class RenderTests(unittest.TestCase):
    def setUp(self):
        self.docs = r.render('feature/one', 'backend:branch', 'frontend:branch')
        self.slug, self.namespace = r.identity('feature/one')

    def test_production_base_exactly_preserves_source_application_objects(self):
        actual = list(yaml.safe_load_all((ROOT/'k8s/base/application.yaml').read_text()))
        self.assertEqual(actual, r.source_objects(), 'refresh base after changing production source manifests')
        for d in actual:
            self.assertNotIn('namespace', d['metadata'])

    @unittest.skipUnless(os.environ.get('KUSTOMIZE_KUBECTL'), 'set KUSTOMIZE_KUBECTL for offline kubectl parity checks')
    def test_kustomize_production_and_example_match_application_and_runtime(self):
        def build(path):
            result = subprocess.run([os.environ['KUSTOMIZE_KUBECTL'], 'kustomize', str(ROOT/path)],
                                    capture_output=True, text=True, check=True)
            return sorted(yaml.safe_load_all(result.stdout), key=lambda d: (d['kind'], d['metadata']['name']))
        production = r.source_objects()
        for obj in production:
            obj['metadata']['namespace'] = 'econ-graph'
        self.assertEqual(build('k8s/overlays/production'), sorted(production, key=lambda d: (d['kind'], d['metadata']['name'])))
        example = r.render('example', 'econ-graph-backend:branch-example', 'econ-graph-frontend:branch-example')
        self.assertEqual(build('k8s/overlays/branch'), sorted(example, key=lambda d: (d['kind'], d['metadata']['name'])))

    def test_identity_does_not_alias_branch_names_or_exceed_dns_limits(self):
        self.assertNotEqual(r.identity('Feature/a'), r.identity('feature-a'))
        self.assertLessEqual(len(r.identity('x'*256)[1]), 63)
        for bad in ['', '\n', 'x'*257]:
            with self.assertRaises(ValueError):
                r.identity(bad)

    def test_primary_has_no_database_worker_secret_or_cluster_objects(self):
        for d in self.docs:
            self.assertNotIn(d['kind'], ['PersistentVolume', 'StorageClass', 'ClusterRole', 'Secret'])
            if d['kind'] != 'Namespace':
                self.assertEqual(d['metadata']['namespace'], self.namespace)
        self.assertFalse(any(d['metadata']['name'] in ['postgresql', 'crawler-worker', 'crawler-migrate'] for d in self.docs))
        for d in self.docs:
            if d['kind'] in ['StatefulSet', 'Deployment', 'Job']:
                pod = d['spec']['template']['spec']
                self.assertFalse(pod['automountServiceAccountToken'])
                self.assertNotIn('nodeSelector', pod)
                for claim in d['spec'].get('volumeClaimTemplates', []):
                    self.assertNotIn('volumeName', claim['spec'])
                    self.assertEqual(claim['spec']['storageClassName'], 'standard')
        backend = by_name(self.docs, 'Deployment', 'econ-graph-backend')['spec']['template']['spec']['containers'][0]
        env = {e['name']: e.get('value') for e in backend['env']}
        self.assertNotIn('DATA_DATABASE_URL', env)
        self.assertEqual(env['DATA_GRAPHQL_HOST'], self.slug + '.localhost')
        self.assertEqual(env['OIDC_ISSUER'], 'http://' + self.slug + '.localhost/idp/realms/econ-graph')

    def test_only_data_route_crosses_namespace_and_removes_credentials(self):
        for route in (d for d in self.docs if d['kind'] == 'HTTPRoute'):
            self.assertEqual(route['spec']['parentRefs'][0]['namespace'], 'econ-graph-gateway')
            self.assertEqual(route['spec']['hostnames'], [self.slug+'.localhost'])
            for rule in route['spec']['rules']:
                for backend in rule['backendRefs']:
                    if route['metadata']['name'] == 'econ-graph-data-graphql':
                        self.assertEqual(backend['namespace'], 'econ-graph')
                        filters = {f['type']: f for f in rule['filters']}
                        self.assertEqual(set(filters['RequestHeaderModifier']['requestHeaderModifier']['remove']), {'Authorization', 'Cookie'})
                        self.assertEqual(filters['URLRewrite']['urlRewrite']['path']['replacePrefixMatch'], '/graphql')
                    else:
                        self.assertNotIn('namespace', backend)
        grant = r.grant('feature/one')
        self.assertEqual(grant['metadata']['namespace'], 'econ-graph')
        self.assertEqual(grant['spec']['from'][0]['namespace'], self.namespace)
        self.assertEqual(grant['spec']['to'], [{'group': '', 'kind': 'Service', 'name': 'econ-graph-backend-service'}])

    def test_egress_to_proxy_is_backend_only(self):
        policies = [d for d in self.docs if d['kind'] == 'NetworkPolicy']
        proxy_rules = []
        for policy in policies:
            for rule in policy['spec'].get('egress', []):
                for peer in rule.get('to', []):
                    if peer.get('namespaceSelector', {}).get('matchLabels', {}).get('kubernetes.io/metadata.name') == 'envoy-gateway-system':
                        proxy_rules.append((policy, rule, peer))
                    self.assertNotIn('ipBlock', peer)
        self.assertEqual(len(proxy_rules), 1)
        policy, rule, peer = proxy_rules[0]
        self.assertEqual(policy['spec']['podSelector'], {'matchLabels': {'app': 'econ-graph-backend'}})
        self.assertEqual(rule['ports'], [{'port': 10080, 'protocol': 'TCP'}])
        self.assertIn('podSelector', peer)

    def test_branch_services_never_claim_production_nodeports(self):
        for d in self.docs:
            if d['kind'] == 'Service':
                self.assertEqual(d['spec']['type'], 'ClusterIP')
                self.assertTrue(all('nodePort' not in port for port in d['spec']['ports']))

    def test_local_data_uses_own_backend_and_tagged_data_image(self):
        docs = r.render('feature/one', 'backend:branch', 'frontend:branch', True, 'worker:v4.1.0')
        route = by_name(docs, 'HTTPRoute', 'econ-graph-data-graphql')
        self.assertNotIn('namespace', route['spec']['rules'][0]['backendRefs'][0])
        crawler = by_name(docs, 'Deployment', 'crawler-worker')['spec']['template']['spec']['containers'][0]
        self.assertEqual(crawler['image'], 'worker:v4.1.0')
        self.assertIn({'name': 'CRAWLER_SCHEDULER', 'value': 'false'}, crawler['env'])
        for d in docs:
            if d['kind'] == 'StatefulSet':
                self.assertNotIn('volumeName', d['spec']['volumeClaimTemplates'][0]['spec'])


class GuardTests(unittest.TestCase):
    def invoke(self, argv, response=None):
        calls = []
        def fake(command, **kwargs):
            calls.append((command, kwargs))
            out = response(command) if response else ''
            return subprocess.CompletedProcess(command, 0, stdout=out)
        with patch.object(sys, 'argv', ['deploy-branch.py']+argv), patch.object(subprocess, 'run', side_effect=fake):
            deploy.main()
        return calls

    def test_dryrun_up_down_list_and_build_contact_nothing(self):
        common = ['--context', 'scratch', '--dry-run']
        for argv in [['up', 'one', '--backend-image', 'b:x', '--frontend-image', 'f:x'], ['down', 'one'], ['list'], ['build-frontend', 'one']]:
            self.assertEqual(self.invoke(argv+common), [])

    def test_up_rejects_unowned_existing_namespace_before_mutations(self):
        with self.assertRaisesRegex(ValueError, 'ownership'):
            self.invoke(['up', 'one', '--context', 'scratch', '--backend-image', 'b:x', '--frontend-image', 'f:x'],
                        lambda cmd: json.dumps({'metadata': {'labels': {'econgraph.io/env': 'production'}}}))

    def test_down_absent_namespace_does_not_delete_anything(self):
        calls = self.invoke(['down', 'one', '--context', 'scratch'])
        self.assertEqual(len(calls), 1)
        self.assertIn('get', calls[0][0])

    def test_down_requires_terminal_and_matching_typed_name(self):
        slug, ns = r.identity('one')
        existing = {'metadata': {'labels': {'econgraph.io/env': 'branch', 'econgraph.io/branch': slug},
                                 'annotations': {'econgraph.io/branch-name': 'one'}}}
        with patch.object(sys.stdin, 'isatty', return_value=False), self.assertRaisesRegex(ValueError, 'typed'):
            self.invoke(['down', 'one', '--context', 'scratch'], lambda cmd: json.dumps(existing))

    def test_ci_missing_kubectl_fails_before_python_can_skip_parity(self):
        workflow = yaml.safe_load((ROOT/'.github/workflows/ci-core.yml').read_text())
        step = next(s for s in workflow['jobs']['deploy-manifests']['steps']
                    if s.get('name') == 'Check isolated branch rendering and deployment guards')
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp)
            marker = path/'python-was-run'
            python = path/'python3'
            python.write_text('#!/bin/sh\n: > "'+str(marker)+'"\nexit 0\n')
            python.chmod(0o755)
            result = subprocess.run(['/bin/bash', '-e', '-c', step['run']], env={'PATH': temp},
                                    capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('kubectl is required', result.stderr)
            self.assertFalse(marker.exists())

    def test_local_data_rejects_untagged_crawler(self):
        with self.assertRaises(SystemExit):
            self.invoke(['up', 'one', '--context', 'scratch', '--dry-run', '--local-data', '--data-image', 'worker:latest',
                         '--seed-dir', '/absent', '--backend-image', 'b:x', '--frontend-image', 'f:x'])

    def test_seed_rejects_missing_verification_and_checksum_mismatch_before_cluster_access(self):
        with tempfile.TemporaryDirectory() as temp:
            args = type('Args', (), {'seed_dir': Path(temp), 'context': 'scratch'})()
            with self.assertRaisesRegex(ValueError, 'VERIFIED'):
                deploy.seed_local(args, 'branch', None, None, None)
            (Path(temp)/'VERIFIED').touch()
            (Path(temp)/'econ_graph.dump').write_bytes(b'synthetic backup')
            (Path(temp)/'SHA256SUMS').write_text('0'*64+'  econ_graph.dump\n')
            with self.assertRaisesRegex(ValueError, 'SHA-256'):
                deploy.seed_local(args, 'branch', None, None, None)

    def test_seed_rejects_app_tables_without_users_and_unknown_schemas(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp)
            (path/'VERIFIED').touch()
            (path/'econ_graph.dump').write_bytes(b'synthetic data-only fixture')
            checksum = hashlib.sha256(b'synthetic data-only fixture').hexdigest()
            (path/'SHA256SUMS').write_text(checksum+'  econ_graph.dump\n')
            args = type('Args', (), {'seed_dir': path, 'context': 'scratch'})()
            for toc in ['1; 1259 1 TABLE public chart_annotations postgres\n',
                        '2; 0 1 TABLE DATA public chart_annotations postgres\n',
                        '1; 1259 1 TABLE private economic_series postgres\n',
                        '1; 1259 1 TABLE public unknown_data postgres\n']:
                with patch.object(subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, stdout=toc)):
                    with self.assertRaisesRegex(ValueError, 'app/unknown'):
                        deploy.seed_local(args, 'branch', None, None, None)
            with patch.object(subprocess, 'run', return_value=subprocess.CompletedProcess([], 0,
                    stdout='1; 1259 1 TABLE public economic_series postgres\n2; 0 1 TABLE DATA public economic_series postgres\n')):
                deploy.seed_local(args, 'branch', None, lambda *args: {'data': {'sha256': checksum}}, None)


class NetworkProbeTests(unittest.TestCase):
    def check(self, insecure=False, primary_open=False, proxy_open=False, canary_open=True):
        network = module('network', 'branch-network-check.py')
        calls = []
        def k(*cmd, **kwargs):
            calls.append(cmd)
            code = 0
            if 'nc' in cmd:
                client, target = cmd[3], cmd[-2]
                allowed = ((client == 'branch-network-allow' and target == '10.244.1.3' and canary_open)
                           or (client == 'branch-network-deny' and insecure)
                           or (target in ['10.96.0.11', '10.96.0.12'] and primary_open)
                           or (target == '10.96.0.13' and (client == 'branch-network-backend' or proxy_open)))
                code = 0 if allowed else 1
            return subprocess.CompletedProcess(cmd, code, stdout='')
        def lookup(kind, name, namespace):
            if kind == 'pod':
                return {'status': {'podIP': '10.244.1.3'}}
            ip = {'econ-graph-backend-service': '10.96.0.11', 'data-postgres-service': '10.96.0.12',
                  'econ-graph-gateway-proxy': '10.96.0.13'}[name]
            return {'spec': {'clusterIP': ip}}
        try:
            network.verify('econ-graph-br-test', False, k, lookup, lambda docs: None)
        finally:
            self.assertEqual(calls[-1][0:4], ('-n', 'econ-graph-br-test', 'delete', 'pods'))
            self.assertIn('--wait=false', calls[-1])

    def test_success_requires_positive_and_negative_connectivity(self):
        self.check()

    def test_non_enforcing_cni_fails_closed_and_cleans_up(self):
        with self.assertRaisesRegex(ValueError, 'not enforced'):
            self.check(insecure=True)

    def test_missing_positive_control_fails_closed(self):
        with self.assertRaisesRegex(ValueError, 'positive canary'):
            self.check(canary_open=False)

    def test_primary_or_nonbackend_proxy_access_fails_closed(self):
        with self.assertRaisesRegex(ValueError, 'directly reach'):
            self.check(primary_open=True)
        with self.assertRaisesRegex(ValueError, 'non-backend'):
            self.check(proxy_open=True)


if __name__ == '__main__':
    unittest.main()
