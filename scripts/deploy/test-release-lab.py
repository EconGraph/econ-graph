#!/usr/bin/env python3
"""Safety tests for scripts/deploy/release-lab.py against stub kind, kubectl and docker.

No cluster, Docker daemon or database is touched. The stubs record every call (with its
DOCKER_HOST and its stdin) and keep a little state (clusters, the ownership marker, what
was applied). The dumps are synthetic files. Scratch lives under $RELEASE_LAB_TEST_DIR,
else ~/.cache/econgraph-release-lab-tests (the lab refuses a root under /tmp), and is
removed afterwards.

Run: python3 scripts/deploy/test-release-lab.py [-v]
"""
import contextlib
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import textwrap
import unittest
from unittest.mock import patch

import yaml

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
_spec = importlib.util.spec_from_file_location('release_lab', HERE / 'release-lab.py')
lab = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(lab)
kind_tools = lab.kind_tools

TOKEN = 'tunnel-token-that-must-never-reach-a-lab-0123456789'
ROOTFUL = 'unix:///var/run/docker.sock'
ROOTLESS = 'unix:///run/user/1000/docker.sock'
COMMIT = 'a' * 40
IMAGE_ID = 'sha256:' + 'b' * 64

# One stub for docker, kind and kubectl (dispatching on its own name). State is a JSON
# file; every call is appended to a JSON-lines log.
STUB = textwrap.dedent(r'''
    #!/usr/bin/env python3
    import json, os, sys, hashlib
    from pathlib import Path
    STATE = Path(os.environ['LAB_STUB_STATE'])
    def load(): return json.loads(STATE.read_text())
    def save(s): STATE.write_text(json.dumps(s))
    name = Path(sys.argv[0]).name
    tool = 'kind' if name.startswith('kind') else name
    args = sys.argv[1:]
    stdin = ''
    s = load()
    if tool == 'kubectl' and ('-f' in args or ('exec' in args and '-i' in args)):
        data = sys.stdin.buffer.read()
        stdin = data.decode('utf-8', 'replace') if 'exec' not in args else 'sha256:' + hashlib.sha256(data).hexdigest()
    with open(os.environ['LAB_STUB_LOG'], 'a') as log:
        log.write(json.dumps({'tool': tool, 'args': args, 'docker_host': os.environ.get('DOCKER_HOST'),
                              'network': os.environ.get('KIND_EXPERIMENTAL_DOCKER_NETWORK'),
                              'kubeconfig_env': os.environ.get('KUBECONFIG'),
                              'credentials': sorted(k for k in os.environ if 'TOKEN' in k or 'SECRET' in k),
                              'stdin': stdin}) + '\n')
    joined = ' '.join(args)
    def out(text): sys.stdout.write(text)
    if tool == 'kind':
        if args[:1] == ['version']: out('kind v0.33.0 go1.25 linux/amd64\n')
        elif args[:2] == ['get', 'clusters']: out(''.join(c + '\n' for c in s['clusters']))
        elif args[:2] == ['get', 'kubeconfig']: out('apiVersion: v1\nkind: Config\n')
        elif args[:2] == ['get', 'nodes']: out(args[args.index('--name') + 1] + '-control-plane\n')
        elif args[:2] == ['create', 'cluster']:
            # A new cluster: nothing applied, no marker yet.
            s['clusters'].append(args[args.index('--name') + 1])
            s.update(applied=[], secrets=[], owner=None, owner_reads=0); save(s)
        elif args[:2] == ['delete', 'cluster']:
            s['clusters'] = [c for c in s['clusters'] if c != args[args.index('--name') + 1]]
            s['owner'] = None; save(s)
        sys.exit(0)
    if tool == 'docker':
        if args[:1] == ['info'] and 'DockerRootDir' in joined: out(s.get('docker_root', '/') + '\n')
        elif args[:1] == ['info']:
            out('[name=rootless]\n' if s.get('rootless') else '[name=seccomp,profile=builtin]\n')
        elif args[:3] == ['image', 'inspect', '--format']:
            out(str(s.get('image_size', 1000)) + '\n')
        elif args[:2] == ['image', 'inspect']:
            image = s['images'].get(args[2])
            if image is None: sys.exit(1)
            out(json.dumps([image]))
        elif args[:1] == ['pull']:
            if args[1] not in s.get('pullable', {}): sys.exit(1)
            s['images'][args[1]] = s['pullable'][args[1]]; save(s)
        elif args[:1] == ['cp']:
            target = Path(args[-1]); target.write_bytes(Path(sys.argv[0]).read_bytes())
            target.chmod(0o755)
        elif args[:2] == ['network', 'inspect']:
            if not s.get('network_exists', True): sys.exit(1)
            out(s.get('network_containers', '0') + '\n')
        sys.exit(0)
    # kubectl
    if 'create' in args and '-f' in args:
        for doc in __import__('yaml').safe_load_all(stdin):
            if doc['kind'] == 'ConfigMap' and doc['metadata']['name'] == 'econgraph-lab-owner':
                if s.get('owner') or s.get('fail_owner'):
                    sys.stderr.write('AlreadyExists\n'); sys.exit(1)
                s['owner'] = doc
            if doc['kind'] == 'Secret':
                s.setdefault('secrets', []).append(doc['metadata']['name'])
        save(s); sys.exit(0)
    if 'apply' in args:
        s.setdefault('applied', []).extend(d for d in __import__('yaml').safe_load_all(stdin) if d)
        save(s); sys.exit(0)
    if 'get namespace kube-system' in joined: out(s.get('uid', 'uid-lab')); sys.exit(0)
    if 'get configmap econgraph-lab-owner' in joined:
        s['owner_reads'] = s.get('owner_reads', 0) + 1
        if s.get('forge_owner_after') is not None and s['owner_reads'] > s['forge_owner_after'] and s.get('owner'):
            s['owner']['data']['lab-id'] = 'f' * 32
        save(s)
        if s.get('owner'): out(json.dumps(s['owner']))
        sys.exit(0)
    if 'get configmap econgraph-cluster-role' in joined:
        if s.get('cluster_role'): out('configmap/econgraph-cluster-role')
        sys.exit(0)
    if 'logs' in args: out('PostgreSQL init process complete; ready for start up.\n'); sys.exit(0)
    if 'exec' in args and 'psql' in args:
        sql = args[-1]
        if 'pg_tables' in sql: out(s.get('tables', '0'))
        elif 'to_regclass' in sql: out('f')
        sys.exit(0)
    if 'get job' in joined: out('Complete'); sys.exit(0)
    if 'get deployment crawler-worker' in joined:
        for d in s.get('applied', []):
            if d['kind'] == 'Deployment' and d['metadata']['name'] == 'crawler-worker':
                out(str(d['spec']['replicas']))
        sys.exit(0)
    sys.exit(0)
''').lstrip()


def scratch_parent():
    parent = Path(os.environ.get('RELEASE_LAB_TEST_DIR') or Path.home() / '.cache/econgraph-release-lab-tests')
    parent.mkdir(parents=True, exist_ok=True)
    return parent


def pg_dump_bytes(text):
    return b'PGDMP' + text.encode() * 100


class Stubbed(unittest.TestCase):
    """A scratch HOME with the stubs, synthetic read-only dumps and immutable images."""

    def setUp(self):
        self.scratch = Path(tempfile.mkdtemp(prefix='release-lab-', dir=scratch_parent()))
        self.addCleanup(self.cleanup)
        self.home = self.scratch / 'home'
        self.bin = self.scratch / 'bin'
        self.bin.mkdir(parents=True)
        self.home.mkdir()
        stub = self.bin / 'docker'
        stub.write_text(STUB)
        stub.chmod(0o755)
        kind_dir = self.home / '.local/share/econgraph/bin'
        kind_dir.mkdir(parents=True)
        shutil.copy(stub, kind_dir / 'kind-v0.33.0')
        # Plain kubectl and kind on PATH must never run.
        # git answers only for the release tag the tests use.
        tool = self.bin / 'git'
        tool.write_text(f'#!/bin/sh\ncase "$*" in *refs/tags/v4.0.1^{{commit}}*|*"rev-parse HEAD") echo {COMMIT} ;;\n'
                        '*"status --porcelain") ;; *) exit 1 ;; esac\n')
        tool.chmod(0o755)
        for name in ('kubectl', 'kind'):
            trap = self.bin / name
            trap.write_text('#!/bin/sh\necho "$0 from PATH ran" >>"$LAB_STUB_LOG.path"; exit 1\n')
            trap.chmod(0o755)
        self.dumps_dir = self.scratch / 'dumps'
        self.dumps_dir.mkdir()
        self.dumps = {}
        for role in ('data', 'app', 'keycloak'):
            path = self.dumps_dir / f'{role}.dump'
            path.write_bytes(pg_dump_bytes(role))
            path.chmod(0o444)
            self.dumps[role] = path
        self.dumps_dir.chmod(0o555)
        self.state_file = self.scratch / 'state.json'
        self.log_file = self.scratch / 'calls.jsonl'
        self.images = {
            f'ghcr.io/econgraph/econ-graph-{name}:sha-{COMMIT}': {
                'Id': IMAGE_ID, 'RepoDigests': [],
                'Config': {'Labels': {'org.opencontainers.image.revision': COMMIT}}}
            for name in lab.IMAGE_NAMES}
        self.state({'clusters': ['econ-graph', 'econgraph-train1-qa'], 'images': self.images})
        self.env = {
            'PATH': f'{self.bin}:{os.environ["PATH"]}', 'HOME': str(self.home),
            'ECONGRAPH_LAB_ROOT': str(self.scratch / 'labs'),
            'LAB_STUB_STATE': str(self.state_file), 'LAB_STUB_LOG': str(self.log_file),
            # What the operator's shell has, none of which may steer the lab.
            'DOCKER_HOST': ROOTLESS, 'DOCKER_CONTEXT': 'rootless', 'KUBECONFIG': str(self.home / 'prod-kubeconfig'),
            'KIND_CLUSTER_NAME': 'econ-graph', 'CLUSTER_ROLE_CONTEXT': 'kind-econ-graph',
            'CLOUDFLARE_TUNNEL_TOKEN': TOKEN, 'PUBLIC_HOST': 'econgraph.com',
        }
        self.before = {role: self.facts(path) for role, path in self.dumps.items()}

    def cleanup(self):
        subprocess.run(['chmod', '-R', 'u+rwx', str(self.scratch)], check=False)
        shutil.rmtree(self.scratch, ignore_errors=True)

    @staticmethod
    def facts(path):
        stat = path.stat()
        return (hashlib.sha256(path.read_bytes()).hexdigest(), stat.st_size, stat.st_mtime_ns, stat.st_mode)

    def state(self, value=None, **changes):
        if value is not None:
            self.state_file.write_text(json.dumps(value))
        current = json.loads(self.state_file.read_text())
        if changes:
            current.update(changes)
            self.state_file.write_text(json.dumps(current))
        return current

    def calls(self, tool=None):
        if not self.log_file.exists():
            return []
        calls = [json.loads(line) for line in self.log_file.read_text().splitlines()]
        return [c for c in calls if tool is None or c['tool'] == tool]

    def lab_cli(self, *args, stdin=None):
        return subprocess.run([sys.executable, str(HERE / 'release-lab.py'), *args], env=self.env,
                              capture_output=True, text=True, timeout=120,
                              **({'input': stdin} if stdin is not None else {'stdin': subprocess.DEVNULL}))

    def create_args(self, name='one', **override):
        values = {'--data-dump': self.dumps['data'], '--app-dump': self.dumps['app'],
                  '--keycloak-dump': self.dumps['keycloak']}
        for image in lab.IMAGE_NAMES:
            flag = '--crawler-image' if image == 'crawler-worker' else f'--{image}-image'
            values[flag] = f'ghcr.io/econgraph/econ-graph-{image}:sha-{COMMIT}'
        values.update(override)
        args = ['create', '--lab', name]
        for flag, value in values.items():
            if value is not None:
                args += [flag, str(value)]
        return args

    def create(self, name='one', expect_ok=True, **override):
        result = self.lab_cli(*self.create_args(name, **override))
        if expect_ok:
            self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        return result

    def record_path(self, name='one'):
        return self.scratch / 'labs' / name / 'lab.json'

    def assert_inputs_unchanged(self):
        for role, path in self.dumps.items():
            self.assertEqual(self.facts(path), self.before[role], f'{role} dump changed')

    def assert_no_production_cluster_reached(self):
        for call in self.calls():
            text = ' '.join(call['args'])
            self.assertNotIn('kind-econ-graph ', text + ' ')
            self.assertNotIn('kind-econgraph-train1-qa', text)
            if call['tool'] in ('kind', 'docker'):
                self.assertNotIn('--name econ-graph ', text + ' ')
                self.assertNotIn('econgraph-train1-qa', text)
            if call['tool'] == 'kubectl':
                self.assertIn('--context', call['args'])
                self.assertEqual(call['args'][call['args'].index('--context') + 1], 'kind-econ-graph-lab-one')
                kubeconfig = call['args'][call['args'].index('--kubeconfig') + 1]
                self.assertEqual(kubeconfig, str(self.scratch / 'labs/one/kube/kubeconfig'))
        self.assertFalse(Path(str(self.log_file) + '.path').exists(), 'kubectl or kind from PATH ran')


class CreateTests(Stubbed):
    def test_create_builds_an_owned_lab_on_the_rootful_daemon_and_its_own_network(self):
        self.create()
        record = json.loads(self.record_path().read_text())
        self.assertEqual(record['status'], 'ready')
        self.assertEqual(record['cluster'], 'econ-graph-lab-one')
        owner = self.state()['owner']
        self.assertEqual(owner['data'], {'lab-id': record['lab_id'], 'lab': 'one', 'kube-system-uid': 'uid-lab'})
        for call in self.calls('kind') + self.calls('docker'):
            self.assertEqual(call['docker_host'], ROOTFUL, call)
        create = next(c for c in self.calls('kind') if c['args'][:2] == ['create', 'cluster'])
        self.assertEqual(create['network'], 'econ-graph-lab-one')
        self.assertIn('--kubeconfig', create['args'])
        self.assert_no_production_cluster_reached()

    def test_never_marked_production(self):
        self.create()
        for call in self.calls('kubectl'):
            if 'create' in call['args'] or 'apply' in call['args']:
                self.assertNotIn('econgraph-cluster-role', call['stdin'])
                self.assertNotIn('role: production', call['stdin'])
        self.assertNotIn('mark-production', self.log_file.read_text())

    def test_refuses_to_build_on_a_cluster_that_carries_a_role_marker(self):
        state = self.state()
        state['cluster_role'] = True
        self.state(state)
        result = self.create(expect_ok=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('econgraph-cluster-role', result.stderr)
        self.assertFalse(any('apply' in c['args'] for c in self.calls('kubectl')), 'applied to a marked cluster')
        self.assertEqual(json.loads(self.record_path().read_text())['status'], 'failed')

    def test_no_tunnel_token_and_no_cloudflared(self):
        self.create()
        self.assertNotIn(TOKEN, self.log_file.read_text())
        self.assertNotIn(TOKEN, self.record_path().read_text())
        applied = self.state()['applied']
        for doc in applied:
            if doc['kind'] != 'ConfigMap':
                self.assertNotIn('cloudflare', json.dumps(doc).lower(), doc['metadata']['name'])
        self.assertNotIn('cloudflare-tunnel', self.state()['secrets'])
        self.assertNotIn('crawler-api-keys', self.state()['secrets'])
        record = {'name': 'one', 'lab_id': '0' * 32, 'ports': dict.fromkeys(lab.PORT_NAMES, 40000), 'images': {
            n: {'lab_tag': lab.lab_tag('one', n, IMAGE_ID),
                'reference': f'ghcr.io/econgraph/econ-graph-{n}:sha-{COMMIT}'} for n in lab.IMAGE_NAMES}}
        docs = lab.render_manifests(record)
        cloudflared = {'apiVersion': 'apps/v1', 'kind': 'Deployment',
                       'metadata': {'name': 'cloudflared', 'namespace': 'econ-graph'},
                       'spec': {'replicas': 0, 'template': {'spec': {'containers': [
                           {'name': 'cloudflared', 'image': 'cloudflare/cloudflared:2026.1.0'}]}}}}
        token = {'apiVersion': 'v1', 'kind': 'Secret', 'metadata': {'name': 'cloudflare-tunnel'}}
        for bad in (cloudflared, token):
            with self.assertRaises(lab.LabError):
                lab.check_manifests(docs + [bad])
        for call in self.calls():
            self.assertEqual(call['credentials'], [], call)
            self.assertIsNone(call['kubeconfig_env'], call)

    def test_no_host_ports(self):
        self.create()
        config = yaml.safe_load((self.scratch / 'labs/one/kind-config.yaml').read_text())
        self.assertEqual(config['networking']['apiServerAddress'], '127.0.0.1')
        self.assertEqual(len(config['nodes']), 1)
        for node in config['nodes']:
            self.assertNotIn('extraPortMappings', node)
            self.assertEqual(node['extraMounts'], [{'hostPath': str(self.scratch / 'labs/one/node-storage'),
                                                    'containerPath': '/var/local-path-provisioner'}])
        text = json.dumps(self.state()['applied'])
        for word in ('hostPort', 'nodePort', 'NodePort', 'LoadBalancer', 'hostNetwork', 'hostPath'):
            self.assertNotIn(word, text)
        with self.assertRaises(lab.LabError):
            lab.check_kind_config(dict(config, nodes=[dict(config['nodes'][0], extraPortMappings=[
                {'containerPort': 80, 'hostPort': 80}])]))
        with self.assertRaises(lab.LabError):
            lab.check_kind_config(dict(config, networking={'apiServerAddress': '0.0.0.0'}))
        service = {'apiVersion': 'v1', 'kind': 'Service', 'metadata': {'name': 's', 'namespace': 'econ-graph'},
                   'spec': {'type': 'NodePort', 'ports': [{'port': 1, 'nodePort': 30080}]}}
        with self.assertRaises(lab.LabError):
            lab.check_manifests([service])
        pod = {'apiVersion': 'apps/v1', 'kind': 'Deployment', 'metadata': {'name': 'd', 'namespace': 'econ-graph'},
               'spec': {'template': {'spec': {'containers': [{'name': 'c', 'ports': [{'hostPort': 80}]}]}}}}
        with self.assertRaises(lab.LabError):
            lab.check_manifests([pod])
        balancer = {'apiVersion': 'v1', 'kind': 'Service', 'metadata': {'name': 's', 'namespace': 'econ-graph'},
                    'spec': {'type': 'LoadBalancer', 'ports': [{'port': 80}]}}
        with self.assertRaisesRegex(lab.LabError, 'LoadBalancer'):
            lab.check_manifests([balancer])
        connector = {'apiVersion': 'apps/v1', 'kind': 'Deployment',
                     'metadata': {'name': 'edge', 'namespace': 'econ-graph'},
                     'spec': {'replicas': 1, 'template': {'spec': {'containers': [
                         {'name': 'edge', 'image': 'example.invalid/edge:1', 'args': ['--via', 'Cloudflare']}]}}}}
        with self.assertRaisesRegex(lab.LabError, 'cloudflared'):
            lab.check_manifests([connector])

    def test_port_forward_listens_on_loopback_only(self):
        self.create()
        started = []

        class Process:
            def __init__(self, cmd, env):
                started.append(cmd)
                self.env = env

            def poll(self):
                raise KeyboardInterrupt

            def terminate(self):
                pass

        with patch.dict(os.environ, self.env, clear=True), patch.object(lab, 'start_forward', Process), \
                contextlib.redirect_stdout(io.StringIO()):
            lab.port_forward(type('Args', (), {'lab': 'one'})())
        self.assertEqual(len(started), 4)
        for cmd in started:
            self.assertEqual(cmd[cmd.index('--address') + 1], '127.0.0.1')
            self.assertEqual(cmd[cmd.index('--context') + 1], 'kind-econ-graph-lab-one')

    def test_crawler_starts_at_zero_replicas(self):
        self.create()
        crawler = [d for d in self.state()['applied'] if d['kind'] == 'Deployment'
                   and d['metadata']['name'] == 'crawler-worker']
        self.assertEqual([d['spec']['replicas'] for d in crawler], [0])
        container = crawler[0]['spec']['template']['spec']['containers'][0]
        self.assertEqual(container['image'], lab.lab_tag('one', 'crawler-worker', IMAGE_ID))
        self.assertEqual(container['imagePullPolicy'], 'Never')
        crawler[0]['spec']['replicas'] = 1
        with self.assertRaisesRegex(lab.LabError, '0 replicas'):
            lab.check_manifests([crawler[0]])

    def test_source_repository_namespace_text_cannot_escape_the_loaded_lab_tag(self):
        _, namespace = lab.renderer.identity('lab-one')
        reference = f'ghcr.io/{namespace}/econ-graph-crawler-worker:sha-{COMMIT}'
        images = self.state()['images']
        images[reference] = images[f'ghcr.io/econgraph/econ-graph-crawler-worker:sha-{COMMIT}']
        self.state(images=images)
        self.create(**{'--crawler-image': reference})
        containers = [c for d in self.state()['applied']
                      if d['kind'] in ('Deployment', 'StatefulSet', 'Job')
                      for c in d['spec']['template']['spec'].get('initContainers', []) +
                      d['spec']['template']['spec']['containers']
                      if c['name'] in ('crawler-worker', 'crawler-migrate')]
        self.assertEqual({c['name'] for c in containers}, {'crawler-worker', 'crawler-migrate'})
        for container in containers:
            self.assertEqual(container['image'], lab.lab_tag('one', 'crawler-worker', IMAGE_ID))
            self.assertEqual(container['imagePullPolicy'], 'Never')

    def test_inputs_never_modified(self):
        self.create()
        restored = [c for c in self.calls('kubectl') if 'pg_restore' in c['args']]
        self.assertEqual(sorted(c['stdin'] for c in restored),
                         sorted('sha256:' + self.before[r][0] for r in self.dumps))
        self.assert_inputs_unchanged()
        # Something in the lab's storage, as the databases leave there.
        (self.scratch / 'labs/one/node-storage/pvc-data').mkdir()
        result = self.lab_cli('teardown', '--lab', 'one', '--yes')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_inputs_unchanged()
        removal = [c for c in self.calls('docker') if c['args'][:1] == ['run']]
        self.assertEqual(len(removal), 1)
        args = removal[0]['args']
        self.assertIn(f'{self.scratch / "labs/one/node-storage"}:/lab-storage', args)
        self.assertEqual(args[args.index('--network') + 1], 'none')
        # Only below the mount, never across another filesystem mounted inside it.
        self.assertEqual(args[args.index('--entrypoint') + 1], '/usr/bin/find')
        self.assertEqual(args[-5:], ['/lab-storage', '-xdev', '-mindepth', '1', '-delete'])

    def test_refuses_dumps_inside_the_lab_root_and_non_archives(self):
        (self.scratch / 'labs').mkdir()
        inside = self.scratch / 'labs/data.dump'
        inside.write_bytes(pg_dump_bytes('x'))
        result = self.create(expect_ok=False, **{'--data-dump': inside})
        self.assertIn('inside the lab root', result.stderr)
        plain = self.scratch / 'plain.sql'
        plain.write_text('select 1;')
        result = self.create(expect_ok=False, **{'--data-dump': plain})
        self.assertIn('pg_dump custom-format', result.stderr)
        self.assertFalse(any(c['args'][:2] == ['create', 'cluster'] for c in self.calls('kind')))

    def created_secrets(self):
        values = {}
        for call in self.calls('kubectl'):
            if 'create' in call['args'] and 'kind: Secret' in call['stdin']:
                doc = yaml.safe_load(call['stdin'])
                values[doc['metadata']['name']] = doc['stringData']
        return values

    def test_without_an_app_dump_the_app_tables_are_in_the_data_database(self):
        self.create(**{'--app-dump': None})
        record = json.loads(self.record_path().read_text())
        self.assertEqual(record['app_database'], 'shared')
        self.assertEqual(sorted(record['inputs']), ['data', 'keycloak'])
        urls = self.created_secrets()['econ-graph-secrets']
        self.assertEqual(urls['app-database-url'], urls['data-database-url'])
        self.assertIn('@data-postgres-service:5432/econ_graph', urls['app-database-url'])
        restores = [c['args'] for c in self.calls('kubectl') if 'pg_restore' in c['args']]
        self.assertEqual(sorted(a[a.index('exec') + 2] for a in restores), ['keycloak-postgres-0', 'postgresql-0'])
        for args in restores:
            for flag in ('--no-owner', '--no-privileges', '--single-transaction', '--exit-on-error'):
                self.assertIn(flag, args)

    def test_with_an_app_dump_the_app_tables_are_in_the_app_database(self):
        self.create()
        record = json.loads(self.record_path().read_text())
        self.assertEqual(record['app_database'], 'separate')
        urls = self.created_secrets()['econ-graph-secrets']
        self.assertIn('@app-postgres-service:5432/econ_graph_app', urls['app-database-url'])
        restores = [c['args'] for c in self.calls('kubectl') if 'pg_restore' in c['args']]
        self.assertIn('--role=econgraph', next(a for a in restores if 'app-postgres-0' in a))
        self.assertIn('--role=keycloak', next(a for a in restores if 'keycloak-postgres-0' in a))

    def test_shared_app_database_takes_no_app_dump(self):
        result = self.create(expect_ok=False, **{'--app-database': 'shared'})
        self.assertIn('takes no --app-dump', result.stderr)
        self.assertFalse(any(c['args'][:2] == ['create', 'cluster'] for c in self.calls('kind')))

    def test_browser_addresses_are_the_port_forward_ports(self):
        self.create()
        record = json.loads(self.record_path().read_text())
        keycloak = f'http://localhost:{record["ports"]["keycloak"]}/idp'
        applied = json.dumps(self.state()['applied'])
        self.assertNotIn('.localhost', applied)
        deployment = next(d for d in self.state()['applied']
                          if d['kind'] == 'Deployment' and d['metadata']['name'] == 'keycloak')
        env = {e['name']: e.get('value') for e in deployment['spec']['template']['spec']['containers'][0]['env']}
        self.assertEqual(env['KC_HOSTNAME'], keycloak)
        self.assertEqual(env['KC_HOSTNAME_ADMIN'], keycloak)
        self.assertEqual(env['KC_WEB_BASE_URL'], f'http://localhost:{record["ports"]["frontend"]}')
        backend = next(d for d in self.state()['applied']
                       if d['kind'] == 'Deployment' and d['metadata']['name'] == 'econ-graph-backend')
        env = {e['name']: e.get('value') for e in backend['spec']['template']['spec']['containers'][0]['env']}
        self.assertEqual(env['OIDC_ISSUER'], keycloak + '/realms/econ-graph')
        self.assertEqual(len(set(record['ports'].values())), len(lab.PORT_NAMES))

    def test_warns_when_the_images_are_not_from_this_checkout(self):
        images = self.state()['images']
        other = 'c' * 40
        ref = f'ghcr.io/econgraph/econ-graph-backend:sha-{other}'
        images[ref] = {'Id': IMAGE_ID, 'RepoDigests': [],
                       'Config': {'Labels': {'org.opencontainers.image.revision': other}}}
        self.state(images=images)
        result = self.lab_cli(*self.create_args(**{'--backend-image': ref}), '--dry-run')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('Run the lab from a worktree at the candidate', result.stderr)

    def test_refuses_too_little_space_in_the_docker_data_root(self):
        class Tools:
            def docker(self, *args, check=True):
                text = '/var/lib/docker' if args[0] == 'info' else str(4 * 1024 ** 3)
                return subprocess.CompletedProcess(args, 0, text, '')
        images = {n: {'image_id': IMAGE_ID} for n in lab.IMAGE_NAMES}
        needed = 10 * 1024 ** 3 + 2 * 3 * 4 * 1024 ** 3
        with self.assertRaisesRegex(lab.LabError, 'Docker data root'):
            lab.check_docker_space(Tools(), images, free=lambda path: needed - 1)
        self.assertEqual(lab.check_docker_space(Tools(), images, free=lambda path: needed),
                         (needed, needed, '/var/lib/docker'))

    def test_one_filesystem_holds_both_the_volumes_and_the_images(self):
        gib = 1024 ** 3
        same = lambda path: 1  # noqa: E731
        apart = iter([1, 2])
        self.assertFalse(lab.check_shared_space(self.scratch, 15 * gib, 12 * gib, '/var/lib/docker', 12 * gib,
                                                device=lambda path: next(apart)))
        # 12 + 12 GiB with one 10 GiB reserve: 14 GiB together.
        self.assertTrue(lab.check_shared_space(self.scratch, 14 * gib, 12 * gib, '/var/lib/docker', 12 * gib,
                                               device=same))
        with self.assertRaisesRegex(lab.LabError, 'share a filesystem'):
            lab.check_shared_space(self.scratch, 14 * gib - 1, 12 * gib, '/var/lib/docker', 12 * gib, device=same)

    def test_env_points_the_deploy_scripts_at_the_lab(self):
        self.create()
        result = self.lab_cli('env', '--lab', 'one')
        self.assertEqual(result.returncode, 0, result.stderr)
        lines = result.stdout.splitlines()
        directory = self.scratch / 'labs/one'
        self.assertIn('export KIND_CLUSTER=econ-graph-lab-one', lines)
        self.assertIn(f'export XDG_CACHE_HOME={directory / "cache"}', lines)
        self.assertIn(f'export ECONGRAPH_BACKUP_ROOT={directory / "backups"}', lines)
        self.assertTrue(any(line.startswith('export ECONGRAPH_SCRATCH_STORAGE=' + str(directory)) for line in lines))
        self.assertIn('unset KUBECTL KUBECONFIG ECONGRAPH_KUBE_DIR CLUSTER_ROLE_CONTEXT PUBLIC_HOST', lines)
        self.assertIn(f'export KEYCLOAK_BACKUP_ROOT={directory / "backups/keycloak"}', lines)

    def test_env_is_shell_quoted(self):
        # A lab root with a space and a quote: evaluated in bash, every value survives.
        self.env['ECONGRAPH_LAB_ROOT'] = str(self.scratch / "lab root's")
        self.create()
        printed = self.lab_cli('env', '--lab', 'one').stdout
        script = (printed + '\nprintf "%s\\n" "$KIND_CLUSTER" "$XDG_CACHE_HOME" "$ECONGRAPH_BACKUP_ROOT" '
                  '"$KEYCLOAK_BACKUP_ROOT" "$ECONGRAPH_SCRATCH_STORAGE"')
        result = subprocess.run(['bash', '-c', script], capture_output=True, text=True, timeout=30,
                                env={'PATH': os.environ['PATH'], 'KUBECONFIG': '/production/kubeconfig'})
        directory = self.scratch / "lab root's/one"
        self.assertEqual(result.stdout.splitlines(), [
            'econ-graph-lab-one', str(directory / 'cache'), str(directory / 'backups'),
            str(directory / 'backups/keycloak'), str(directory / 'no-host-storage')])

    def test_dry_run_creates_and_changes_nothing(self):
        result = self.lab_cli(*self.create_args(), '--dry-run')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('Would create', result.stdout)
        self.assertFalse((self.scratch / 'labs').exists())
        for call in self.calls():
            self.assertIn(call['args'][:2], (['version'], ['info', '--format'], ['get', 'clusters'],
                                             ['image', 'inspect'], ['rev-parse', 'HEAD']), call)
        self.assert_inputs_unchanged()

    def test_existing_ready_lab_is_left_alone_and_others_refused(self):
        self.create()
        calls = len(self.calls())
        again = self.create()
        self.assertIn('nothing to do', again.stdout)
        self.assertEqual(len(self.calls()), calls)
        record = json.loads(self.record_path().read_text())
        record['status'] = 'failed'
        self.record_path().write_text(json.dumps(record))
        result = self.create(expect_ok=False)
        self.assertIn('tear it down', result.stderr)

    def test_refuses_an_existing_cluster_without_a_record(self):
        self.state(clusters=['econ-graph-lab-one'])
        result = self.create(expect_ok=False)
        self.assertIn('without a lab record', result.stderr)
        self.assertFalse(any(c['args'][:2] == ['create', 'cluster'] for c in self.calls('kind')))

    def test_deletes_the_cluster_it_just_created_when_the_marker_fails(self):
        self.state(fail_owner=True)
        result = self.create(expect_ok=False)
        self.assertNotEqual(result.returncode, 0)
        deletes = [c['args'] for c in self.calls('kind') if c['args'][:2] == ['delete', 'cluster']]
        self.assertEqual(len(deletes), 1)
        self.assertEqual(deletes[0][deletes[0].index('--name') + 1], 'econ-graph-lab-one')
        self.assertNotIn('econ-graph-lab-one', self.state()['clusters'])

    def test_refuses_a_rootless_daemon(self):
        self.state(rootless=True)
        result = self.create(expect_ok=False)
        self.assertIn('rootless', result.stderr)
        self.assertFalse(any(c['args'][:2] == ['create', 'cluster'] for c in self.calls('kind')))

    def test_lab_root_is_never_under_tmp(self):
        # Refused by path, whatever filesystem /tmp is on, and on a RAM filesystem anywhere.
        with patch.object(kind_tools, 'filesystem_type', lambda path: 'ext4'):
            for root in ('/tmp/labs', '/tmp'):
                with self.assertRaisesRegex(lab.LabError, '/tmp'):
                    lab.lab_root({'ECONGRAPH_LAB_ROOT': root})
        with patch.object(kind_tools, 'filesystem_type', lambda path: 'tmpfs'):
            with self.assertRaisesRegex(lab.LabError, 'RAM filesystem'):
                lab.lab_root({'ECONGRAPH_LAB_ROOT': str(self.scratch / 'labs')})

    def test_names_never_reach_protected_clusters(self):
        for name in ('', 'Econ', 'a_b', '-a', 'a' * 33, '../x'):
            with self.assertRaises(lab.LabError):
                lab.cluster_for(name)
        for cluster in ('econ-graph', 'econgraph-train1-qa', 'econ-graph-lab-', 'kind-econ-graph-lab-x'):
            with self.assertRaises(lab.LabError):
                lab.check_lab_cluster(cluster)


class TeardownTests(Stubbed):
    def setUp(self):
        super().setUp()
        self.create()
        self.log_file.unlink()

    def teardown(self, *extra):
        return self.lab_cli('teardown', '--lab', 'one', *extra)

    def deletes(self):
        return [c['args'] for c in self.calls('kind') if c['args'][:2] == ['delete', 'cluster']]

    def forge(self, **changes):
        record = json.loads(self.record_path().read_text())
        record.update(changes)
        self.record_path().write_text(json.dumps(record))

    def test_deletes_only_the_recorded_lab_cluster(self):
        result = self.teardown('--yes')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(len(self.deletes()), 1)
        args = self.deletes()[0]
        self.assertEqual(args[args.index('--name') + 1], 'econ-graph-lab-one')
        self.assertEqual(args[args.index('--kubeconfig') + 1], str(self.scratch / 'labs/one/kube/kubeconfig'))
        self.assertEqual(self.state()['clusters'], ['econ-graph', 'econgraph-train1-qa'])
        for call in self.calls('kubectl'):
            self.assertNotIn('delete', call['args'])
        self.assertEqual([c['args'][-1] for c in self.calls('docker') if c['args'][:2] == ['network', 'rm']],
                         ['econ-graph-lab-one'])
        self.assert_no_production_cluster_reached()
        self.assertTrue(self.record_path().exists())
        self.assert_inputs_unchanged()

    def test_refuses_production_and_qa_clusters_in_a_forged_record(self):
        for cluster in ('econ-graph', 'econgraph-train1-qa'):
            self.forge(cluster=cluster, context='kind-' + cluster, network=cluster)
            result = self.teardown('--yes')
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('refusing', result.stderr)
            self.assertEqual(self.deletes(), [])
            self.assertEqual(self.calls('kubectl'), [])
        with self.assertRaisesRegex(lab.LabError, 'production or QA'):
            lab.load_images(None, {'cluster': 'econ-graph', 'name': 'one', 'images': {}})

    def test_refuses_a_missing_ownership_marker(self):
        self.state(owner=None)
        result = self.teardown('--yes')
        self.assertIn('no ownership marker', result.stderr)
        self.assertIn('kind delete cluster --name econ-graph-lab-one', result.stderr)
        self.assertEqual(self.deletes(), [])

    def test_refuses_a_forged_ownership_marker(self):
        owner = self.state()['owner']
        for field, value in (('lab-id', 'f' * 32), ('kube-system-uid', 'uid-other'), ('lab', 'two')):
            forged = json.loads(json.dumps(owner))
            forged['data'][field] = value
            self.state(owner=forged)
            result = self.teardown('--yes')
            self.assertIn('not this lab', result.stderr, field)
            self.assertEqual(self.deletes(), [], field)

    def test_refuses_a_recreated_cluster(self):
        self.state(uid='uid-recreated')
        owner = self.state()['owner']
        owner['data']['kube-system-uid'] = 'uid-recreated'
        self.state(owner=owner)
        result = self.teardown('--yes')
        self.assertIn('recreated', result.stderr)
        self.assertEqual(self.deletes(), [])

    def test_refuses_a_cluster_with_a_role_marker(self):
        self.state(cluster_role=True)
        result = self.teardown('--yes')
        self.assertIn('econgraph-cluster-role', result.stderr)
        self.assertEqual(self.deletes(), [])

    def test_needs_a_typed_name_or_yes(self):
        result = self.teardown()
        self.assertIn('typed at a terminal', result.stderr)
        self.assertEqual(self.deletes(), [])

    def test_dry_run_deletes_nothing(self):
        result = self.teardown('--dry-run')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('kind delete cluster --name econ-graph-lab-one', result.stdout)
        self.assertIn('copy of the data', result.stdout)
        self.assertEqual(self.deletes(), [])
        self.assertFalse(any(c['args'][:1] == ['run'] for c in self.calls('docker')))

    def test_rechecks_ownership_right_before_the_delete(self):
        # The marker changes after the first check (between it and the delete): refused.
        self.state(forge_owner_after=self.state().get('owner_reads', 0) + 1)
        result = self.teardown('--yes')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('not this lab', result.stderr)
        self.assertEqual(self.deletes(), [])
        self.assertIn('econ-graph-lab-one', self.state()['clusters'])

    def test_removes_the_data_copy_by_default_and_keeps_it_only_when_asked(self):
        storage = self.scratch / 'labs/one/node-storage'
        (storage / 'pvc-data').mkdir()
        kept = self.teardown('--yes', '--keep-storage')
        self.assertEqual(kept.returncode, 0, kept.stderr)
        self.assertIn('WARNING', kept.stderr)
        self.assertIn('full copy of the data', kept.stderr)
        self.assertFalse(any(c['args'][:1] == ['run'] for c in self.calls('docker')))
        self.assertEqual(json.loads(self.record_path().read_text())['storage'], 'kept')
        # The name can't be reused while the copy is kept.
        blocked = self.create(expect_ok=False)
        self.assertIn('--keep-storage', blocked.stderr)
        again = self.teardown('--yes')
        self.assertEqual(again.returncode, 0, again.stderr)
        removal = [c['args'] for c in self.calls('docker') if c['args'][:1] == ['run']]
        self.assertEqual(len(removal), 1)
        self.assertIn(f'{storage}:/lab-storage', removal[0])
        self.assertEqual(json.loads(self.record_path().read_text())['storage'], 'removed')

    def test_name_can_be_created_again_after_teardown(self):
        self.assertEqual(self.teardown('--yes').returncode, 0)
        first = json.loads(self.record_path().read_text())
        self.create()
        second = json.loads(self.record_path().read_text())
        self.assertEqual(second['status'], 'ready')
        self.assertNotEqual(second['lab_id'], first['lab_id'])
        retired = [p for p in (self.scratch / 'labs').iterdir() if p.name.startswith('.one.torn-down-')]
        self.assertEqual(len(retired), 1)
        self.assertEqual(json.loads((retired[0] / 'lab.json').read_text())['lab_id'], first['lab_id'])
        listed = self.lab_cli('list')
        self.assertEqual(listed.returncode, 0, listed.stderr)
        self.assertNotIn('unreadable', listed.stdout)

    def test_never_removes_through_a_symlinked_storage_directory(self):
        storage = self.scratch / 'labs/one/node-storage'
        elsewhere = self.scratch / 'elsewhere'
        elsewhere.mkdir()
        (elsewhere / 'keep').write_text('not the lab\'s')
        storage.rmdir()
        storage.symlink_to(elsewhere)
        result = self.teardown('--yes')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('is not the lab\'s storage directory', result.stderr)
        self.assertFalse(any(c['args'][:1] == ['run'] for c in self.calls('docker')))
        self.assertEqual((elsewhere / 'keep').read_text(), 'not the lab\'s')

    def test_keeps_a_network_another_container_uses(self):
        self.state(network_containers='2')
        result = self.teardown('--yes')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual([c for c in self.calls('docker') if c['args'][:2] == ['network', 'rm']], [])
        self.assertIn('still has containers', result.stdout)

    def test_is_safe_to_run_again(self):
        self.assertEqual(self.teardown('--yes').returncode, 0)
        again = self.teardown('--yes')
        self.assertEqual(again.returncode, 0, again.stderr)
        self.assertIn('does not exist', again.stdout)
        self.assertEqual(len(self.deletes()), 1)


class ImageTests(Stubbed):
    def test_production_style_tags_need_exactly_their_release(self):
        prod = 'econ-graph-backend:v4.0.1'
        for release in (None, 'v4.0.0'):
            with self.assertRaises(lab.LabError):
                lab.check_image_reference('backend', prod, release)
        lab.check_image_reference('backend', prod, 'v4.0.1')
        with self.assertRaisesRegex(lab.LabError, 'production frontend'):
            lab.check_image_reference('backend', 'econ-graph-frontend:v4.0.1', 'v4.0.1')
        for mutable in ('econ-graph-backend:latest', 'ghcr.io/econgraph/econ-graph-backend:main',
                        'ghcr.io/econgraph/econ-graph-backend:sha-abc'):
            with self.assertRaisesRegex(lab.LabError, 'mutable'):
                lab.check_image_reference('backend', mutable, None)

    def test_cli_refuses_a_production_tag_without_release_before_any_cluster(self):
        result = self.create(expect_ok=False, **{'--backend-image': 'econ-graph-backend:v4.0.1'})
        self.assertIn('production-style tag', result.stderr)
        self.assertFalse(any(c['args'][:2] == ['create', 'cluster'] for c in self.calls('kind')))

    def test_release_tags_load_under_lab_tags_into_the_lab_cluster_only(self):
        images = self.state()['images']
        overrides = {}
        for name in lab.IMAGE_NAMES:
            ref = f'econ-graph-{name}:v4.0.1'
            images[ref] = {'Id': IMAGE_ID, 'RepoDigests': [],
                           'Config': {'Labels': {'org.opencontainers.image.revision': COMMIT}}}
            overrides['--crawler-image' if name == 'crawler-worker' else f'--{name}-image'] = ref
        self.state(images=images)
        self.create(**overrides, **{'--release': 'v4.0.1'})
        tags = [c['args'] for c in self.calls('docker') if c['args'][:1] == ['tag']]
        loads = [c['args'] for c in self.calls('kind') if c['args'][:1] == ['load']]
        self.assertEqual(len(tags), 3)
        self.assertEqual(len(loads), 3)
        for args in tags:
            self.assertIsNone(kind_tools.production_style_tag(args[-1]), args)
            self.assertTrue(args[-1].startswith('econ-graph-lab-one/'), args)
        for args in loads:
            self.assertEqual(args[args.index('--name') + 1], 'econ-graph-lab-one')
            self.assertTrue(args[2].startswith('econ-graph-lab-one/'))
        self.assertFalse(any(c['args'][:1] == ['pull'] for c in self.calls('docker')))
        for doc in self.state()['applied']:
            if doc['kind'] in ('Deployment', 'Job'):
                for c in doc['spec']['template']['spec']['containers']:
                    self.assertNotRegex(c['image'], r'^econ-graph-(backend|frontend|crawler-worker):')
        # Teardown untags only the lab's names, never by force.
        self.log_file.unlink()
        self.assertEqual(self.lab_cli('teardown', '--lab', 'one', '--yes').returncode, 0)
        untagged = [c['args'] for c in self.calls('docker') if c['args'][:2] == ['image', 'rm']]
        self.assertEqual(sorted(a[-1] for a in untagged), sorted(a[-1] for a in tags))
        self.assertFalse(any('-f' in a or '--force' in a for a in untagged))

    def test_release_image_must_carry_the_release_commit(self):
        images = self.state()['images']
        images['econ-graph-backend:v4.0.1'] = {'Id': IMAGE_ID, 'RepoDigests': [],
                                               'Config': {'Labels': {'org.opencontainers.image.revision': 'c' * 40}}}
        images['econ-graph-backend:v4.0.2'] = {'Id': IMAGE_ID, 'RepoDigests': [],
                                               'Config': {'Labels': {'org.opencontainers.image.revision': COMMIT}}}
        self.state(images=images)
        result = self.create(expect_ok=False, **{'--backend-image': 'econ-graph-backend:v4.0.1',
                                                 '--release': 'v4.0.1'})
        self.assertIn('revision label', result.stderr)
        result = self.create(expect_ok=False, **{'--backend-image': 'econ-graph-backend:v4.0.2',
                                                 '--release': 'v4.0.2'})
        self.assertIn('is not in this checkout', result.stderr)
        self.assertFalse(any(c['args'][:2] == ['create', 'cluster'] for c in self.calls('kind')))

    def test_missing_production_tag_is_never_pulled(self):
        result = self.create(expect_ok=False, **{'--backend-image': 'econ-graph-backend:v4.0.1',
                                                 '--release': 'v4.0.1'})
        self.assertIn('does not pull production tags', result.stderr)
        self.assertFalse(any(c['args'][:1] == ['pull'] for c in self.calls('docker')))

    def test_revision_label_must_match_the_commit_tag(self):
        images = self.state()['images']
        images[f'ghcr.io/econgraph/econ-graph-backend:sha-{COMMIT}']['Config']['Labels'][
            'org.opencontainers.image.revision'] = 'c' * 40
        self.state(images=images)
        result = self.create(expect_ok=False)
        self.assertIn('revision label', result.stderr)
        self.assertFalse(any(c['args'][:2] == ['create', 'cluster'] for c in self.calls('kind')))

    def test_digest_reference_must_be_that_digest(self):
        digest = 'sha256:' + 'e' * 64
        ref = f'ghcr.io/econgraph/econ-graph-backend@{digest}'
        images = self.state()['images']
        images[ref] = {'Id': IMAGE_ID, 'RepoDigests': ['ghcr.io/econgraph/econ-graph-backend@sha256:' + '9' * 64],
                       'Config': {'Labels': {}}}
        self.state(images=images)
        result = self.create(expect_ok=False, **{'--backend-image': ref})
        self.assertIn('is not that digest', result.stderr)
        self.assertFalse(any(c['args'][:2] == ['create', 'cluster'] for c in self.calls('kind')))

    def test_digest_reference_is_pulled_and_checked(self):
        digest = 'sha256:' + 'd' * 64
        ref = f'ghcr.io/econgraph/econ-graph-backend@{digest}'
        self.state(pullable={ref: {'Id': IMAGE_ID, 'RepoDigests': [ref], 'Config': {'Labels': {}}}})
        self.create(**{'--backend-image': ref})
        self.assertEqual([c['args'] for c in self.calls('docker') if c['args'][:1] == ['pull']], [['pull', ref]])

    def test_lab_tags_are_never_production_style(self):
        for name in lab.IMAGE_NAMES:
            tag = lab.lab_tag('one', name, IMAGE_ID)
            self.assertIsNone(kind_tools.production_style_tag(tag))
        record = {'cluster': 'econ-graph-lab-one', 'name': 'one',
                  'images': {'backend': {'lab_tag': 'econ-graph-backend:v4.0.1', 'image_id': IMAGE_ID,
                                         'reference': 'x'}}}
        with self.assertRaisesRegex(lab.LabError, 'not a lab-only tag'):
            lab.load_images(None, record)


class ClusterRoleTests(unittest.TestCase):
    """lib/cluster-role.sh and mark-production-cluster.sh never treat a lab as production."""

    STUB = textwrap.dedent('''\
        #!/usr/bin/env bash
        echo "kubectl $*" >>"$STUB_LOG"
        case "$*" in
          *"config current-context"*) echo kind-econ-graph-lab-one ;;
          *"config view"*) echo https://127.0.0.1:6443 ;;
          *"get namespace kube-system"*) printf uid-this ;;
          *"get nodes"*) printf node-a ;;
          *"get configmap econgraph-cluster-role"*)
            case "$*" in
              *"-o name"*) echo configmap/econgraph-cluster-role ;;
              *"{.data.role}"*) printf 'production#' ;;
              *"{.data.kube-system-uid}"*) printf 'uid-this#' ;;
            esac ;;
          *"get configmap econgraph-lab-owner"*)
            [ "${STUB_LAB:-0}" != fails ] || exit 1
            [ "${STUB_LAB:-0}" != 1 ] || echo configmap/econgraph-lab-owner ;;
        esac
        exit 0
        ''')

    def setUp(self):
        self.dir = Path(tempfile.mkdtemp(prefix='release-lab-role-', dir=scratch_parent()))
        self.addCleanup(shutil.rmtree, self.dir, True)
        self.kubectl = self.dir / 'kubectl'
        self.kubectl.write_text(self.STUB)
        self.kubectl.chmod(0o755)
        self.log = self.dir / 'log'

    def run_bash(self, script, lab_state, *args):
        env = {'PATH': os.environ['PATH'], 'HOME': str(self.dir), 'KUBECTL': str(self.kubectl),
               'STUB_LOG': str(self.log), 'STUB_LAB': lab_state}
        return subprocess.run(['bash', '-c', script, 'test', *args], env=env, capture_output=True, text=True,
                              stdin=subprocess.DEVNULL, timeout=60)

    def check(self, lab_state):
        return self.run_bash(f'source {HERE}/lib/cluster-role.sh; cluster_is_marked_production; rc=$?; '
                             'echo "rc=$rc $CLUSTER_ROLE_REASON"', lab_state)

    def test_conflicting_markers_are_unknown_not_production_and_not_safe(self):
        # A valid production marker and the lab marker together: status 2 (unknown), so
        # callers that relax their production safeguards on 1 refuse instead.
        self.assertIn('rc=0', self.check('0').stdout)
        conflict = self.check('1').stdout
        self.assertIn('rc=2', conflict)
        self.assertIn('conflicting markers', conflict)
        failed = self.check('fails').stdout
        self.assertIn('rc=2', failed)
        self.assertIn('unknown', failed)

    def test_identity_says_unknown_for_conflicting_markers(self):
        def role(lab_state):
            out = self.run_bash(f'source {HERE}/lib/cluster-role.sh; CLUSTER_ROLE_CONTEXT=kind-x '
                                'cluster_role_show_identity', lab_state).stdout
            return next(line for line in out.splitlines() if 'cluster role' in line)
        self.assertIn('PRODUCTION', role('0'))
        self.assertIn('unknown: ', role('1'))
        self.assertNotIn('not production', role('1'))
        self.assertIn('unknown: ', role('fails'))

    def test_mark_production_refuses_a_lab(self):
        result = self.run_bash(f'{HERE}/mark-production-cluster.sh', '1')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('release lab', result.stderr)
        self.assertNotRegex(self.log.read_text(), r'kubectl .*(apply|create|delete|scale)')


if __name__ == '__main__':
    unittest.main()
