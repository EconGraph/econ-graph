#!/usr/bin/env python3
"""Offline safety and routing regression tests for disposable branch environments."""
import importlib.util
import io
import hashlib
import json
import os
from pathlib import Path
import re
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
        self.assertEqual(policy['spec']['podSelector'], {'matchLabels': {'app': 'branch-data-proxy'}})
        self.assertEqual(rule['ports'], [{'port': 10080, 'protocol': 'TCP'}])
        self.assertIn('podSelector', peer)

    def test_shared_gateway_is_confined_to_fixed_host_and_data_path(self):
        backend = by_name(self.docs, 'Deployment', 'econ-graph-backend')['spec']['template']['spec']['containers'][0]
        self.assertIn({'name': 'DATA_GRAPHQL_URL', 'value': 'http://branch-data-proxy:8080/data/graphql'}, backend['env'])
        config = by_name(self.docs, 'ConfigMap', 'branch-data-proxy')['data']['nginx.conf']
        self.assertIn('proxy_set_header Host ' + self.slug + '.localhost;', config)
        self.assertIn('location = /data/graphql {', config)
        self.assertIn('location / { return 403; }', config)
        self.assertIn('proxy_set_header Authorization "";', config)
        self.assertIn('proxy_set_header Cookie "";', config)
        self.assertNotIn('$http_host', config)
        self.assertNotIn('$request_uri', config)
        local = r.render('local', 'b:branch', 'f:branch', True, 'crawler:sha-' + 'a' * 40)
        self.assertFalse(any(d['metadata']['name'] == 'branch-data-proxy' for d in local))

    def test_local_data_accepts_full_commit_or_digest_and_stops_restored_queue(self):
        for image in ['crawler:sha-' + 'a' * 40, 'registry:5000/crawler@sha256:' + 'b' * 64,
                      'crawler:v4.1.0']:
            docs = r.render('local', 'b:branch', 'f:branch', True, image)
            worker = by_name(docs, 'Deployment', 'crawler-worker')
            self.assertEqual(worker['spec']['replicas'], 0)
            self.assertEqual(worker['spec']['template']['spec']['containers'][0]['image'], image)
        for image in ['crawler:latest', 'crawler:main', 'crawler:sha-abcdef', 'crawler',
                      'crawler@sha256:' + 'b' * 63]:
            with self.assertRaises(ValueError):
                r.render('local', 'b:x', 'f:x', True, image)
        with self.assertRaisesRegex(ValueError, 'only allowed'):
            r.render('shared', 'b:x', 'f:x', False, 'crawler:sha-' + 'a' * 40)

    def test_mutable_tags_are_pulled_again_and_commit_images_can_be_loaded_locally(self):
        self.assertEqual(r.pull_policy('backend:branch-one'), 'Always')
        self.assertEqual(r.pull_policy('backend:sha-' + 'a' * 40), 'IfNotPresent')
        self.assertEqual(r.pull_policy('backend@sha256:' + 'a' * 64), 'IfNotPresent')
        docs = r.render('mutable', 'backend:branch-one', 'frontend:sha-' + 'a' * 40)
        backend = by_name(docs, 'Deployment', 'econ-graph-backend')['spec']['template']
        frontend = by_name(docs, 'Deployment', 'econ-graph-frontend')['spec']['template']
        deploy.freshen_mutable_images(docs)
        first = backend['metadata']['annotations']['econgraph.io/redeploy-token']
        deploy.freshen_mutable_images(docs)
        self.assertNotEqual(first, backend['metadata']['annotations']['econgraph.io/redeploy-token'])
        self.assertNotIn('econgraph.io/redeploy-token', frontend['metadata'].get('annotations', {}))

    def test_branch_services_never_claim_production_nodeports(self):
        for d in self.docs:
            if d['kind'] == 'Service':
                self.assertEqual(d['spec']['type'], 'ClusterIP')
                self.assertTrue(all('nodePort' not in port for port in d['spec']['ports']))

    def test_branch_is_never_production_public_or_tunnelled(self):
        # The https listener admits only namespaces labelled econgraph.io/env=production,
        # cloudflared serves only that listener, and the production marker is a kube-system
        # ConfigMap. A branch, in either data mode, must carry none of these.
        for docs in [self.docs, r.render('feature/one', 'backend:branch', 'frontend:branch', True, 'worker:v4.1.0')]:
            namespace = docs[0]
            self.assertEqual(namespace['kind'], 'Namespace')
            self.assertEqual(namespace['metadata']['labels']['econgraph.io/env'], 'branch')
            self.assertEqual(sum(d['kind'] == 'Namespace' for d in docs), 1)
            for d in docs[1:]:
                self.assertEqual(d['metadata']['namespace'], self.namespace)
                self.assertNotEqual(d['metadata'].get('labels', {}).get('econgraph.io/env'), 'production')
                self.assertNotIn('cloudflared', d['metadata']['name'])
            text = yaml.safe_dump_all(docs).lower()
            for marker in ['econgraph.com', 'econ-graph.com', 'cloudflare-tunnel', 'tunnel_token',
                           'econgraph-cluster-role', 'kube-system/']:
                self.assertNotIn(marker, text)
            for route in (d for d in docs if d['kind'] == 'HTTPRoute'):
                self.assertEqual(route['spec']['hostnames'], [self.slug + '.localhost'])
                self.assertEqual([p.get('sectionName') for p in route['spec']['parentRefs']], ['http'])

    def test_branch_pods_are_not_scrape_targets(self):
        # Prometheus runs in econ-graph, which branch-isolation admits no traffic from, and the
        # backend alert rules (prometheus-rules-auth.yaml) carry no namespace filter.
        for docs in [self.docs, r.render('feature/one', 'backend:branch', 'frontend:branch', True, 'worker:v4.1.0')]:
            for d in docs:
                if d['kind'] in ['StatefulSet', 'Deployment', 'Job']:
                    annotations = d['spec']['template'].get('metadata', {}).get('annotations', {})
                    self.assertFalse([key for key in annotations if key.startswith('prometheus.io/')], d['metadata']['name'])

    def test_backend_starts_with_a_data_plane_and_branch_local_anonymous_grants(self):
        # With sign-in on, a backend with neither DATA_DATABASE_URL nor DATA_GRAPHQL_URL refuses
        # to start. Anonymous catalog grants come from the branch Keycloak, and the data plane
        # is asked anonymously (no caller-token forwarding to the primary).
        local = r.render('feature/one', 'backend:branch', 'frontend:branch', True, 'worker:v4.1.0')
        for docs, data_env in [(self.docs, 'DATA_GRAPHQL_URL'), (local, 'DATA_DATABASE_URL')]:
            backend = by_name(docs, 'Deployment', 'econ-graph-backend')['spec']['template']['spec']['containers'][0]
            env = {e['name']: e for e in backend['env']}
            self.assertIn('OIDC_ISSUER', env)
            self.assertIn(data_env, env)
            self.assertNotIn('DATA_GRAPHQL_FORWARD_AUTHORIZATION', env)
            self.assertEqual(env['OIDC_ANONYMOUS_TOKEN_URL']['value'],
                             'http://keycloak-service:8080/idp/realms/econ-graph/protocol/openid-connect/token')
            self.assertEqual(env['OIDC_ANONYMOUS_CLIENT_SECRET']['valueFrom']['secretKeyRef'],
                             {'name': 'econ-graph-keycloak', 'key': 'anonymous-client-secret'})

    def test_social_login_credentials_are_never_referenced(self):
        def social(docs):
            found = []
            for d in docs:
                if d['kind'] in ['Deployment', 'Job'] and d['metadata']['name'] in ['keycloak', 'keycloak-realm-import']:
                    pod = d['spec']['template']['spec']
                    for c in pod.get('initContainers', []) + pod['containers']:
                        found += [e['name'] for e in c.get('env', []) if e['name'].startswith(('KC_GOOGLE', 'KC_FACEBOOK'))]
            return found
        self.assertTrue(social(r.source_objects()), 'production Keycloak no longer reads Google credentials; update this test')
        self.assertEqual(social(self.docs), [])

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
        with patch.object(sys, 'argv', ['deploy-branch.py']+argv), patch.object(subprocess, 'run', side_effect=fake), \
                patch.dict(os.environ, {'KUBECTL': 'stub-kubectl'}):
            deploy.main()
        return calls

    def test_dryrun_up_down_list_and_build_contact_nothing(self):
        common = ['--context', 'scratch', '--dry-run']
        for argv in [['up', 'one', '--backend-image', 'b:x', '--frontend-image', 'f:x'], ['down', 'one'], ['list'], ['build-frontend', 'one']]:
            self.assertEqual(self.invoke(argv+common), [])
            # No kubectl is chosen either, so a dry run needs none.
            with tempfile.TemporaryDirectory() as home, patch.dict(os.environ, {'HOME': home}, clear=True), \
                    patch.object(sys, 'argv', ['deploy-branch.py']+argv+common), \
                    patch.object(subprocess, 'run', side_effect=AssertionError('contacted')):
                deploy.main()

    def test_commands_run_the_chosen_kubectl_with_the_explicit_context(self):
        calls = self.invoke(['down', 'one', '--context', 'scratch'])
        self.assertEqual(calls[0][0][:3], ['stub-kubectl', '--context', 'scratch'])

    def test_no_kubectl_choice_stops_before_any_command(self):
        with tempfile.TemporaryDirectory() as home:
            Path(home, 'kubectl').write_text('#!/bin/sh\nexit 0\n')
            Path(home, 'kubectl').chmod(0o755)
            for action in [['list'], ['down', 'one'], ['up', 'one', '--backend-image', 'b:x', '--frontend-image', 'f:x']]:
                with patch.dict(os.environ, {'HOME': home, 'PATH': home}, clear=True), \
                        patch.object(sys, 'argv', ['deploy-branch.py']+action+['--context', 'kind-econ-graph']), \
                        patch.object(subprocess, 'run', side_effect=AssertionError('contacted')), \
                        self.assertRaisesRegex(ValueError, 'no KUBECTL is set'):
                    deploy.main()

    def test_up_rejects_unowned_existing_namespace_before_mutations(self):
        with self.assertRaisesRegex(ValueError, 'ownership'):
            self.invoke(['up', 'one', '--context', 'scratch', '--backend-image', 'b:x', '--frontend-image', 'f:x'],
                        lambda cmd: json.dumps({'metadata': {'labels': {'econgraph.io/env': 'production'}}}))

    def test_down_absent_namespace_does_not_delete_anything(self):
        calls = self.invoke(['down', 'one', '--context', 'scratch'])
        self.assertEqual(len(calls), 2)
        self.assertTrue(all('get' in call[0] for call in calls))

    def test_down_requires_terminal_and_matching_typed_name(self):
        slug, ns = r.identity('one')
        existing = {'metadata': {'labels': {'econgraph.io/env': 'branch', 'econgraph.io/branch': slug},
                                 'annotations': {'econgraph.io/branch-name': 'one'}}}
        with patch.object(sys.stdin, 'isatty', return_value=False), self.assertRaisesRegex(ValueError, 'typed'):
            self.invoke(['down', 'one', '--context', 'scratch'], lambda cmd: json.dumps(existing))

    def test_orphan_cleanup_uses_inspected_grant_preconditions(self):
        _, namespace = r.identity('one')
        grant = r.grant('one')
        grant['metadata'].update(uid='grant-uid', resourceVersion='42')
        error, calls, sent = self.cluster(['down', 'one'], grant=grant, typed=namespace)
        self.assertIsNone(error)
        deletes = [c for c in calls if 'delete' in c]
        self.assertEqual(len(deletes), 1)
        self.assertIn('--raw', deletes[0])
        options = next(d for d in sent if d.get('kind') == 'DeleteOptions')
        self.assertEqual(options['preconditions'], {'uid': 'grant-uid', 'resourceVersion': '42'})

    def test_orphan_cleanup_refuses_namespace_created_during_confirmation(self):
        _, namespace = r.identity('one')
        grant = r.grant('one')
        grant['metadata'].update(uid='grant-uid', resourceVersion='42')
        reads = 0
        def respond(cmd):
            nonlocal reads
            if 'namespace' in cmd:
                reads += 1
                return '' if reads == 1 else json.dumps({'metadata': self.owned()})
            return json.dumps(grant)
        with patch.object(sys.stdin, 'isatty', return_value=True), patch('builtins.input', return_value=namespace):
            with self.assertRaisesRegex(ValueError, 'appeared during confirmation'):
                self.invoke(['down', 'one', '--context', 'scratch'], respond)

    def test_ownership_is_checked_again_after_acquiring_lock(self):
        reads = 0
        def respond(cmd):
            nonlocal reads
            if 'namespace' in cmd and 'get' in cmd:
                reads += 1
                return json.dumps({'metadata': self.owned(mode='primary' if reads == 1 else 'local')})
            return ''
        with self.assertRaisesRegex(ValueError, 'data mode cannot change'):
            self.invoke(self.UP + ['--context', 'scratch'], respond)

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

    def test_up_creates_every_secret_key_and_realm_file_the_workloads_use(self):
        slug, namespace = r.identity('one')
        sent = []
        created = {}
        def fake(command, **kwargs):
            if kwargs.get('input'):
                objects = [d for d in yaml.safe_load_all(kwargs['input']) if d]
                sent.extend(objects)
                if 'create' in command:
                    for obj in objects:
                        obj['metadata'].update(uid='test-uid', resourceVersion='1')
                        created[(obj['kind'].lower(), obj['metadata']['name'])] = obj
            code, out = 0, ''
            if 'nc' in command:
                client, target = command[command.index('exec') + 1], command[-2]
                code = 0 if ((client == 'branch-network-allow' and target == '10.244.1.3')
                             or (client == 'branch-network-proxy' and target == '10.96.0.13')) else 1
            elif 'get' in command and 'json' in command:
                kind, name = command[command.index('get') + 1:command.index('get') + 3]
                if (kind, name) in created:
                    out = json.dumps(created[(kind, name)])
                elif kind == 'job':
                    out = json.dumps({'status': {'conditions': [{'type': 'Complete', 'status': 'True'}]}})
                elif kind == 'pod':
                    out = json.dumps({'status': {'podIP': '10.244.1.3'}})
                elif kind == 'service':
                    out = json.dumps({'spec': {'clusterIP': {'econ-graph-backend-service': '10.96.0.11',
                        'data-postgres-service': '10.96.0.12', 'econ-graph-gateway-proxy': '10.96.0.13'}[name]}})
            return subprocess.CompletedProcess(command, code, stdout=out)
        argv = ['deploy-branch.py', 'up', 'one', '--context', 'scratch', '--backend-image', 'b:x', '--frontend-image', 'f:x']
        with patch.object(sys, 'argv', argv), patch.object(subprocess, 'run', side_effect=fake), \
                patch.dict(os.environ, {'KUBECTL': 'stub-kubectl'}):
            deploy.main()
        secrets_ = {d['metadata']['name']: d['stringData'] for d in sent if d['kind'] == 'Secret'}
        self.assertIn('anonymous-client-secret', secrets_['econ-graph-keycloak'])
        realm = next(d for d in sent if d['kind'] == 'ConfigMap' and d['metadata']['name'] == 'keycloak-realm')['data']
        workloads = [d for d in sent if d['kind'] in ['Deployment', 'StatefulSet', 'Job']]
        self.assertTrue(workloads)
        for d in workloads:
            pod = d['spec']['template']['spec']
            for c in pod.get('initContainers', []) + pod['containers']:
                for e in c.get('env', []):
                    ref = e.get('valueFrom', {}).get('secretKeyRef')
                    if ref and not ref.get('optional'):
                        self.assertIn(ref['key'], secrets_.get(ref['name'], {}), (d['metadata']['name'], e['name']))
                text = json.dumps([c.get('command', []), c.get('args', [])])
                if any(v.get('configMap', {}).get('name') == 'keycloak-realm' for v in pod.get('volumes', [])):
                    for script in set(re.findall(r'/config/([\w.-]+)', text)):
                        self.assertIn(script, set(realm), (d['metadata']['name'], c['name']))

    def cluster(self, argv, namespace=None, grant=None, present=(), typed=None):
        """Run deploy-branch.py against a fake cluster; return (error, calls, sent documents)."""
        calls, sent = [], []
        created = {}
        def fake(command, **kwargs):
            calls.append(command)
            if kwargs.get('input'):
                objects = [d for d in yaml.safe_load_all(kwargs['input']) if d]
                sent.extend(objects)
                if 'create' in command:
                    for obj in objects:
                        obj['metadata'].update(uid='test-uid', resourceVersion='1')
                        created[(obj['kind'].lower(), obj['metadata']['name'])] = obj
            code, out = 0, ''
            if 'nc' in command:
                client, target = command[command.index('exec') + 1], command[-2]
                code = 0 if ((client == 'branch-network-allow' and target == '10.244.1.3')
                             or (client == 'branch-network-proxy' and target == '10.96.0.13')) else 1
            elif 'get' in command and 'json' in command:
                kind, name = command[command.index('get') + 1:command.index('get') + 3]
                found = created.get((kind, name)) or {'namespace': namespace, 'referencegrant': grant,
                         'job': {'status': {'conditions': [{'type': 'Complete', 'status': 'True'}]}},
                         'pod': {'status': {'podIP': '10.244.1.3'}},
                         'service': {'spec': {'clusterIP': {'econ-graph-backend-service': '10.96.0.11',
                             'data-postgres-service': '10.96.0.12', 'econ-graph-gateway-proxy': '10.96.0.13'}.get(name)}}}.get(kind)
                out = json.dumps(found) if found else ''
            elif 'get' in command and 'secret' in command:
                name = command[command.index('secret') + 1]
                out = 'secret/' + name if name in present else ''
            return subprocess.CompletedProcess(command, code, stdout=out)
        error = None
        with patch.object(sys, 'argv', ['deploy-branch.py'] + argv + ['--context', 'scratch']), \
                patch.object(subprocess, 'run', side_effect=fake), patch.dict(os.environ, {'KUBECTL': 'stub-kubectl'}):
            if typed is None:
                try:
                    deploy.main()
                except ValueError as e:
                    error = e
            else:
                # A real terminal on stdin, with the operator typing `typed`.
                master, slave = os.openpty()
                try:
                    os.write(master, (typed + '\n').encode())
                    with open(slave, 'r', closefd=False) as tty, patch.object(sys, 'stdin', tty), \
                            patch('sys.stdout', new_callable=io.StringIO):
                        try:
                            deploy.main()
                        except ValueError as e:
                            error = e
                finally:
                    os.close(master)
                    os.close(slave)
        return error, calls, sent

    def owned(self, branch='one', mode='primary', **meta):
        slug, _ = r.identity(branch)
        return dict({'labels': {'econgraph.io/env': 'branch', 'econgraph.io/branch': slug, 'econgraph.io/data-mode': mode},
                     'annotations': {'econgraph.io/branch-name': branch}}, **meta)

    UP = ['up', 'one', '--backend-image', 'b:x', '--frontend-image', 'f:x']

    @staticmethod
    def mutations(calls):
        return [c for c in calls if any(verb in c for verb in ['apply', 'create', 'delete'])]

    def test_up_into_owned_namespace_proceeds(self):
        error, calls, sent = self.cluster(self.UP, namespace={'metadata': self.owned()})
        self.assertIsNone(error)
        self.assertTrue(any(d['kind'] == 'ReferenceGrant' for d in sent))

    def test_up_refuses_a_terminating_namespace_before_mutations(self):
        error, calls, _ = self.cluster(self.UP, namespace={'metadata': self.owned(deletionTimestamp='2026-10-28T00:00:00Z')})
        self.assertRegex(str(error), 'terminating')
        self.assertEqual(self.mutations(calls), [])

    def test_up_refuses_a_data_mode_change_before_mutations(self):
        error, calls, _ = self.cluster(self.UP, namespace={'metadata': self.owned(mode='local')})
        self.assertRegex(str(error), 'data mode cannot change')
        self.assertEqual(self.mutations(calls), [])

    def test_down_on_a_terminal_deletes_only_after_the_exact_typed_namespace(self):
        _, namespace = r.identity('one')
        existing = {'metadata': self.owned()}
        error, calls, _ = self.cluster(['down', 'one'], namespace=existing, typed=namespace + 'x')
        self.assertRegex(str(error), 'typed namespace confirmation')
        self.assertEqual(self.mutations(calls), [])
        error, calls, _ = self.cluster(['down', 'one'], namespace=existing, typed=namespace)
        self.assertIsNone(error)
        self.assertIn(['stub-kubectl', '--context', 'scratch', 'delete', 'namespace', namespace], calls)

    def test_down_refuses_a_reference_grant_it_does_not_own(self):
        _, namespace = r.identity('one')
        foreign = {'metadata': {'labels': {'econgraph.io/branch': 'other'}}}
        error, calls, _ = self.cluster(['down', 'one'], namespace={'metadata': self.owned()}, grant=foreign, typed=namespace)
        self.assertRegex(str(error), 'ReferenceGrant ownership differs')
        self.assertFalse([c for c in calls if 'delete' in c and ('referencegrant' in c or 'namespace' in c)])

    def test_up_refuses_a_reference_grant_it_does_not_own(self):
        foreign = {'metadata': {'labels': {'econgraph.io/branch': 'other'}}}
        error, _, sent = self.cluster(self.UP, grant=foreign)
        self.assertRegex(str(error), 'different ownership')
        self.assertFalse([d for d in sent if d['kind'] == 'ReferenceGrant'])

    def test_branch_secrets_are_all_or_none(self):
        names = ['econ-graph-app-postgres', 'econ-graph-keycloak', 'econ-graph-secrets']
        for present in [names[:1], names[1:]]:
            error, _, sent = self.cluster(self.UP, namespace={'metadata': self.owned()}, present=present)
            self.assertRegex(str(error), 'incomplete branch credentials')
            self.assertFalse([d for d in sent if d['kind'] == 'Secret'])
        error, _, sent = self.cluster(self.UP, namespace={'metadata': self.owned()}, present=names)
        self.assertIsNone(error)
        self.assertFalse([d for d in sent if d['kind'] == 'Secret'])

    def test_branch_realm_has_no_identity_providers(self):
        source = json.loads((ROOT/'config/keycloak/econ-graph-realm.json').read_text())
        self.assertTrue(source['identityProviders'], 'the realm no longer has identity providers; update this test')
        _, _, sent = self.cluster(self.UP)
        realm = next(d for d in sent if d['kind'] == 'ConfigMap' and d['metadata']['name'] == 'keycloak-realm')
        self.assertEqual(json.loads(realm['data']['econ-graph-realm.json'])['identityProviders'], [])

    def test_build_frontend_uses_the_kind_docker_daemon_not_the_callers(self):
        for override, expected in [(None, 'unix:///var/run/docker.sock'), ('', 'unix:///var/run/docker.sock'),
                                   ('unix:///run/other.sock', 'unix:///run/other.sock')]:
            env = {'DOCKER_HOST': 'unix:///run/user/1000/docker.sock'}
            if override is not None:
                env['ECONGRAPH_DOCKER_HOST'] = override
            runs = []
            with patch.dict(os.environ, env), patch.object(sys, 'argv', ['deploy-branch.py', 'build-frontend', 'one', '--context', 'x']), \
                    patch.object(subprocess, 'run', side_effect=lambda cmd, **kw: runs.append((cmd, kw))):
                deploy.main()
            (cmd, kwargs), = runs
            self.assertEqual(cmd[:2], ['docker', 'build'])
            self.assertEqual(kwargs['env']['DOCKER_HOST'], expected)
            with patch.dict(os.environ, env), patch.object(sys, 'argv', ['deploy-branch.py', 'build-frontend', 'one', '--context', 'x', '--dry-run']), \
                    patch('sys.stdout', new_callable=io.StringIO) as out:
                deploy.main()
            self.assertTrue(out.getvalue().startswith('DOCKER_HOST=' + expected + ' docker build '))

    def test_local_data_rejects_untagged_crawler(self):
        with self.assertRaises(SystemExit):
            self.invoke(['up', 'one', '--context', 'scratch', '--dry-run', '--local-data', '--data-image', 'worker:latest',
                         '--seed-dir', '/absent', '--seed-sha256', 'a' * 64, '--backend-image', 'b:x', '--frontend-image', 'f:x'])

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
            args = type('Args', (), {'seed_dir': path, 'context': 'scratch', 'seed_sha256': checksum})()
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


class SeedRestoreTests(unittest.TestCase):
    def bundle(self, temp):
        path = Path(temp)
        (path/'VERIFIED').touch()
        (path/'econ_graph.dump').write_bytes(b'synthetic data-only fixture')
        checksum = hashlib.sha256(b'synthetic data-only fixture').hexdigest()
        (path/'SHA256SUMS').write_text(checksum+'  econ_graph.dump\n')
        return type('Args', (), {'seed_dir': path, 'context': 'scratch', 'seed_sha256': checksum,
                                 'kubectl': staticmethod(lambda: ['stub-kubectl', '--context', 'scratch'])})(), checksum

    def seed(self, marker=None, tables='0'):
        with tempfile.TemporaryDirectory() as temp:
            args, checksum = self.bundle(temp)
            runs, execs = [], []
            def run(command, **kwargs):
                runs.append(command)
                return subprocess.CompletedProcess(command, 0, stdout='1; 1259 1 TABLE public economic_series postgres\n')
            def k(*cmd, **kwargs):
                execs.append(cmd)
                return subprocess.CompletedProcess(cmd, 0, stdout=('10485760' if 'df -Pk' in ' '.join(cmd) else tables) + '\n')
            applied = []
            with patch.object(subprocess, 'run', side_effect=run), patch.object(deploy, 'inspect_seed_size', return_value=128):
                deploy.seed_local(args, 'branch', k, lambda *a: marker and {'data': {'sha256': marker(checksum)}}, applied.extend)
            return runs, execs, applied

    def test_seed_marker_from_another_dump_refuses_before_touching_the_database(self):
        with self.assertRaisesRegex(ValueError, 'different dump'):
            self.seed(marker=lambda checksum: '0' * 64)
        runs, execs, applied = self.seed(marker=lambda checksum: checksum)
        self.assertEqual((execs, applied), ([], []))
        self.assertFalse([c for c in runs if 'exec' in c])

    def test_nonempty_unmarked_database_is_never_restored_over(self):
        runs = []
        with self.assertRaisesRegex(ValueError, 'nonempty'):
            self.seed(tables='3')
        runs, execs, applied = self.seed(tables='0')
        self.assertTrue([c for c in runs if 'pg_restore' in c and 'exec' in c])
        self.assertEqual(applied[0]['metadata']['name'], 'branch-data-seed')

    def test_restore_streams_through_the_chosen_kubectl(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp)
            (path/'VERIFIED').touch()
            (path/'econ_graph.dump').write_bytes(b'synthetic data-only fixture')
            checksum = hashlib.sha256(b'synthetic data-only fixture').hexdigest()
            (path/'SHA256SUMS').write_text(checksum+'  econ_graph.dump\n')
            args = type('Args', (), {'seed_dir': path, 'context': 'scratch', 'seed_sha256': checksum,
                                     'kubectl': staticmethod(lambda: ['stub-kubectl', '--context', 'scratch'])})()
            runs = []
            def run(command, **kwargs):
                runs.append(command)
                return subprocess.CompletedProcess(command, 0, stdout='1; 1259 1 TABLE public economic_series postgres\n')
            k = lambda *cmd, **kwargs: subprocess.CompletedProcess(cmd, 0, stdout=('10485760' if 'df -Pk' in ' '.join(cmd) else '0') + '\n')
            with patch.object(subprocess, 'run', side_effect=run), patch.object(deploy, 'inspect_seed_size', return_value=128):
                deploy.seed_local(args, 'branch', k, lambda *a: None, lambda docs: None)
            restore = next(c for c in runs if 'pg_restore' in c and 'exec' in c)
            self.assertEqual(restore[:3], ['stub-kubectl', '--context', 'scratch'])

    def test_insufficient_node_space_stops_before_restore(self):
        with tempfile.TemporaryDirectory() as temp:
            args, _ = self.bundle(temp)
            commands = []
            def run(command, **kwargs):
                commands.append(command)
                return subprocess.CompletedProcess(command, 0, stdout='1; 1259 1 TABLE public economic_series postgres\n')
            def k(*cmd, **kwargs):
                return subprocess.CompletedProcess(cmd, 0, stdout='1\n' if 'df -Pk' in ' '.join(cmd) else '0\n')
            with patch.object(subprocess, 'run', side_effect=run), patch.object(deploy, 'inspect_seed_size', return_value=128):
                with self.assertRaisesRegex(ValueError, 'insufficient node disk'):
                    deploy.seed_local(args, 'branch', k, lambda *a: None, lambda docs: None)
            self.assertFalse(any('exec' in c and 'pg_restore' in c for c in commands))

    def test_archive_size_limit_stops_before_archive_or_cluster_commands(self):
        with tempfile.TemporaryDirectory() as temp:
            args, _ = self.bundle(temp)
            with patch.object(deploy, 'MAX_SEED_BYTES', 1), patch.object(subprocess, 'run', side_effect=AssertionError('called')):
                with self.assertRaisesRegex(ValueError, 'bounded-fixture'):
                    deploy.seed_local(args, 'branch', None, None, None)

    def test_bundle_cannot_authorize_its_own_provenance_hash(self):
        with tempfile.TemporaryDirectory() as temp:
            args, _ = self.bundle(temp)
            args.seed_sha256 = '0' * 64
            with patch.object(subprocess, 'run', side_effect=AssertionError('called')):
                with self.assertRaisesRegex(ValueError, 'reviewed external'):
                    deploy.seed_local(args, 'branch', None, None, None)


class JobWaitTests(unittest.TestCase):
    def test_failed_job_stops_without_waiting_out_deadline(self):
        def k(*cmd, **kwargs):
            return subprocess.CompletedProcess(cmd, 0, stdout=json.dumps({'status': {'conditions': [
                {'type': 'Failed', 'status': 'True'}]}}))
        with patch.object(deploy.time, 'sleep', side_effect=AssertionError('slept')):
            with self.assertRaisesRegex(ValueError, 'Job branch/migrate failed'):
                deploy.wait_job(k, 'branch', 'migrate', 900)

    def test_completion_and_timeout_are_explicit(self):
        def k(*cmd, **kwargs):
            return subprocess.CompletedProcess(cmd, 0, stdout=json.dumps({'status': {'conditions': [
                {'type': 'Complete', 'status': 'True'}]}}))
        deploy.wait_job(k, 'branch', 'migrate', 900)
        with self.assertRaisesRegex(ValueError, 'did not complete'):
            deploy.wait_job(k, 'branch', 'migrate', 0)



@unittest.skipUnless(os.environ.get('BRANCH_PROXY_IMAGE'), 'set BRANCH_PROXY_IMAGE for isolated nginx runtime check')
class ProxyRuntimeTests(unittest.TestCase):
    def test_http_requests_cannot_select_another_host_path_or_forward_credentials(self):
        config = r.data_proxy('unused', 'only-this-branch.localhost')[0]['data']['nginx.conf']
        # Replace only the upstream port with a loopback canary in the same
        # network-isolated container. Exercise the generated proxy rules.
        config = config.replace('.svc.cluster.local/data/graphql', '.svc.cluster.local:18080/data/graphql')
        end = config.rfind('}')
        config = config[:end] + '''server {
          listen 18080;
          location = /data/graphql { return 200 "$host|$http_authorization|$http_cookie"; }
          location / { return 500; }
        }
''' + config[end:]
        with tempfile.TemporaryDirectory() as temp:
            conf = Path(temp)/'nginx.conf'
            conf.write_text(config)
            command = ['docker', '--host=' + os.environ.get('BRANCH_PROXY_DOCKER_HOST', 'unix:///var/run/docker.sock'),
                       'run', '--rm', '--pull=never', '--network=none', '--read-only',
                       '--tmpfs', '/tmp:rw,size=67108864', '--user', '101', '--cap-drop', 'ALL',
                       '--security-opt', 'no-new-privileges', '--add-host',
                       'econ-graph-gateway-proxy.envoy-gateway-system.svc.cluster.local:127.0.0.1',
                       '--mount', 'type=bind,src=' + str(conf) + ',dst=/branch/nginx.conf,readonly',
                       '--entrypoint', 'sh', os.environ['BRANCH_PROXY_IMAGE'], '-ec', '''
nginx -c /branch/nginx.conf -g 'daemon off;' &
for i in 1 2 3 4 5; do
  wget -qO /dev/null http://127.0.0.1:8080/health && break
  sleep 1
done
body="$(wget -qO- --header 'Host: another-branch.localhost' --header 'Authorization: Bearer secret' --header 'Cookie: secret=yes' http://127.0.0.1:8080/data/graphql)"
[ "$body" = 'only-this-branch.localhost||' ]
for path in /graphql /idp/realms/econ-graph /data/graphql/extra; do
  if wget -qO /dev/null "http://127.0.0.1:8080$path"; then exit 1; fi
done
''']
            env = dict(os.environ)
            for key in ['DOCKER_HOST', 'DOCKER_CONTEXT', 'DOCKER_TLS', 'DOCKER_TLS_VERIFY', 'DOCKER_CERT_PATH']:
                env.pop(key, None)
            subprocess.run(command, check=True, timeout=30, env=env, capture_output=True, text=True)


class KubectlChoiceTests(unittest.TestCase):
    """deploy-branch.py chooses kubectl as cluster_role_default_kubectl in lib/cluster-role.sh does."""
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.home = Path(self.temp.name)
        self.cache = self.home/'.cache/econgraph'
        self.binary, self.config = self.cache/'bin/kubectl', self.cache/'kubeconfig-econ-graph'

    def keep(self, binary=True, config=True):
        self.binary.parent.mkdir(parents=True, exist_ok=True)
        if binary:
            self.binary.write_text('#!/bin/sh\n')
            self.binary.chmod(0o755)
        if config is not None:
            self.config.write_text('apiVersion: v1\n' if config else '')

    def choose(self, **env):
        environ = dict(HOME=str(self.home), PATH='/usr/bin:/bin', **env)
        with patch('sys.stderr'):
            return deploy.default_kubectl(environ), environ

    def test_kubectl_set_wins_and_keeps_kubeconfig_untouched(self):
        self.keep()
        self.assertEqual(self.choose(KUBECTL='/opt/k'), ('/opt/k', dict(HOME=str(self.home), PATH='/usr/bin:/bin', KUBECTL='/opt/k')))
        self.assertEqual(self.choose(KUBECTL='/opt/k', KUBECONFIG='/x')[1]['KUBECONFIG'], '/x')

    def test_kubeconfig_without_kubectl_uses_the_kind_kubectl_with_it(self):
        self.keep(config=None)
        chosen, environ = self.choose(KUBECONFIG='/elsewhere')
        self.assertEqual((chosen, environ['KUBECONFIG']), (str(self.binary), '/elsewhere'))

    def test_kubeconfig_without_kubectl_or_kind_kubectl_fails_closed(self):
        self.keep(binary=False)
        with self.assertRaisesRegex(ValueError, 'KUBECONFIG is set .* but KUBECTL is not'):
            self.choose(KUBECONFIG='/elsewhere')

    def test_neither_uses_the_kind_kubectl_and_its_kubeconfig(self):
        self.keep()
        chosen, environ = self.choose()
        self.assertEqual((chosen, environ['KUBECONFIG']), (str(self.binary), str(self.config)))

    def test_xdg_cache_home_moves_the_kind_cache(self):
        self.cache = self.home/'xdg/econgraph'
        self.binary, self.config = self.cache/'bin/kubectl', self.cache/'kubeconfig-econ-graph'
        self.keep()
        self.assertEqual(self.choose(XDG_CACHE_HOME=str(self.home/'xdg'))[0], str(self.binary))

    def test_neither_without_both_kept_files_fails_closed_naming_what_is_missing(self):
        for binary, config, missing in [(False, None, 'kubectl and '), (True, False, 'kubeconfig-econ-graph)'),
                                        (True, None, 'kubeconfig-econ-graph)'), (False, True, 'bin/kubectl)')]:
            with self.subTest(binary=binary, config=config):
                for path in [self.binary, self.config]:
                    path.unlink(missing_ok=True)
                self.keep(binary, config)
                with self.assertRaisesRegex(ValueError, 'no KUBECTL is set') as caught:
                    self.choose()
                self.assertIn(missing, str(caught.exception))

    def test_non_executable_kind_kubectl_is_not_used(self):
        self.keep()
        self.binary.chmod(0o644)
        with self.assertRaisesRegex(ValueError, 'no KUBECTL is set'):
            self.choose()


class NetworkProbeTests(unittest.TestCase):
    def check(self, insecure=False, primary_open=False, proxy_open=False, canary_open=True, local_data=False, backend_proxy=True, backend_open=False):
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
                           or (target == '10.96.0.13' and ((client == 'branch-network-proxy' and backend_proxy) or proxy_open or (client == 'branch-network-backend' and backend_open))))
                code = 0 if allowed else 1
            return subprocess.CompletedProcess(cmd, code, stdout='')
        def lookup(kind, name, namespace):
            if kind == 'pod':
                return {'status': {'podIP': '10.244.1.3'}}
            ip = {'econ-graph-backend-service': '10.96.0.11', 'data-postgres-service': '10.96.0.12',
                  'econ-graph-gateway-proxy': '10.96.0.13'}[name]
            return {'spec': {'clusterIP': ip}}
        try:
            network.verify('econ-graph-br-test', local_data, k, lookup, lambda docs: None)
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

    def test_backend_gateway_access_must_match_the_data_mode(self):
        self.check(local_data=True, backend_proxy=False)
        with self.assertRaisesRegex(ValueError, 'does not match the selected branch data mode'):
            self.check(local_data=True, backend_proxy=True)
        with self.assertRaisesRegex(ValueError, 'does not match the selected branch data mode'):
            self.check(local_data=False, backend_proxy=False)

    def test_backend_cannot_select_another_host_on_shared_listener(self):
        with self.assertRaisesRegex(ValueError, 'select another branch Host'):
            self.check(backend_open=True)

    def test_primary_or_nonbackend_proxy_access_fails_closed(self):
        with self.assertRaisesRegex(ValueError, 'directly reach'):
            self.check(primary_open=True)
        with self.assertRaisesRegex(ValueError, 'non-backend'):
            self.check(proxy_open=True)


if __name__ == '__main__':
    unittest.main()
