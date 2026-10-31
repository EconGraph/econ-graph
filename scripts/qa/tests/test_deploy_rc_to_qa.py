#!/usr/bin/env python3
"""Tests for scripts/qa/deploy-rc-to-qa.sh, against stubs only.

A scratch git repository (with a local "origin") holds the script and a tagged release
candidate whose deploy scripts are recording stubs; fake docker, kind, btrfs and kubectl
first on PATH simulate the kind cluster. Nothing reaches Docker, a cluster or the network,
and the QA root is a scratch directory under $HOME (the script refuses /tmp).

  python3 -m unittest discover -s scripts/qa/tests -v
"""

from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import tempfile
import textwrap
import unittest

import yaml

ROOT = Path(__file__).resolve().parents[3]
TAG = 'v4.1.0-rc.1'
NAME = 'qa-v410-rc1'

# Records each call (program, argv, the environment that matters) and plays the cluster from
# state.json: whether it exists, its markers, its kube-system UID, the Keycloak users.
# Production's Secret econ-graph-keycloak as a restore brings it over.
PRODUCTION_KEYCLOAK = {'db-password': 'restored-db-password', 'admin-username': 'prod-admin',
                       'admin-password': 'PRODUCTION-ADMIN-PASSWORD',
                       'anonymous-client-secret': 'PRODUCTION-ANONYMOUS-SECRET',
                       'google-idp-client-id': 'prod-google-id', 'google-idp-client-secret': 'PRODUCTION-GOOGLE-SECRET'}

# The local names production's manifests run, as the candidate's do; production's daemon has
# the backend's (pointing at production's image) and not the others.
LOCAL_IMAGES = sorted({c['image'] for f in ('backend-deployment.yaml', 'crawler-worker.yaml', 'frontend-deployment.yaml')
                       for d in yaml.safe_load_all((ROOT / 'k8s/manifests' / f).read_text()) if d and d.get('spec')
                       for c in d['spec'].get('template', {}).get('spec', {}).get('containers', [])
                       if c.get('image', '').startswith('econ-graph-')})

STUB = r'''#!/usr/bin/env python3
import json, os, pathlib, sys
# Settings beside the stub, not in the environment: the script runs the candidate's scripts
# with an empty one.
STUB_DIR = pathlib.Path(__file__).resolve().parent
CONFIG = json.loads((STUB_DIR / 'stub-env.json').read_text())
PRODUCTION_KEYCLOAK = CONFIG['production_keycloak']
prog = pathlib.Path(sys.argv[0]).name
prog = 'kind' if prog.startswith('kind-v') else prog
args = sys.argv[1:]
state_path = pathlib.Path(CONFIG['state'])
state = json.loads(state_path.read_text())
reads_stdin = prog == 'kubectl' and ('-i' in args or args[-2:] == ['-f', '-'])
stdin = sys.stdin.read() if reads_stdin and not sys.stdin.isatty() else ''
keep = ('HOME', 'KIND_CLUSTER', 'ECONGRAPH_SCRATCH_STORAGE', 'XDG_CACHE_HOME', 'ECONGRAPH_BACKUP_ROOT',
        'ECONGRAPH_RELEASES_DIR', 'ECONGRAPH_DEPLOY_ENV', 'PUBLIC_HOST', 'APP_DATABASE', 'DOCKER_HOST',
        'FRED_API_KEY', 'KUBECONFIG', 'KUBECTL', 'TMPDIR', 'CLOUDFLARE_TUNNEL_TOKEN', 'DRY_RUN', 'NAMESPACE')
with open(CONFIG['calls'], 'a') as log:
    log.write(json.dumps({'prog': prog, 'args': args, 'cwd': os.getcwd(),
                          'env': {k: os.environ.get(k) for k in keep}, 'stdin': stdin}) + '\n')
def save():
    state_path.write_text(json.dumps(state))
name = CONFIG['name']
out = ''
code = 0
if prog == 'docker':
    if args[:2] == ['ps', '-a']:
        out = (name + '-control-plane\n') if state.get('cluster') else ''
    elif args[0] == 'exec':
        cmd = args[args.index('kubectl') + 1:]
        cmd = [a for a in cmd if not a.startswith('--kubeconfig') and a != '/etc/kubernetes/admin.conf']
        text = ' '.join(cmd)
        if 'get namespace kube-system' in text:
            out = state.get('uid', '')
        elif 'econgraph-cluster-role' in text:
            out = 'configmap/econgraph-cluster-role' if state.get('production') else ''
        elif 'get configmap econgraph-qa-owner' in text:
            out = state.get('owner', '')
        elif 'create configmap econgraph-qa-owner' in text:
            state['owner'] = next(a.split('=', 2)[2] for a in cmd if a.startswith('--from-literal=id='))
            save()
    elif args[0] == 'port':
        out = '127.0.0.1:%s\n' % state.get('port', '')
    elif args[0] == 'cp':
        os.symlink(STUB_DIR / 'stub.py', args[-1])
    elif args[0] == 'run':
        pass
    elif args[:2] == ['image', 'inspect']:
        image = state.setdefault('daemon_images', {}).get(args[-1])
        if image:
            out = image
        else:
            code = 1
    elif args[0] == 'tag':
        state.setdefault('daemon_images', {})[args[2]] = args[1]
        save()
    elif args[:2] == ['image', 'rm']:
        state.setdefault('daemon_images', {}).pop(args[-1], None)
        save()
elif prog == 'kind':
    if args[:2] == ['get', 'kubeconfig']:
        out = 'apiVersion: v1\nkind: Config\n'
    elif args[:2] == ['delete', 'cluster']:
        state.update(cluster=False, owner='')
        save()
elif prog == 'btrfs':
    path = pathlib.Path(args[-1])
    subvolumes = state.setdefault('subvolumes', {})
    if args[:2] == ['subvolume', 'create']:
        path.mkdir()
        subvolumes[str(path)] = 'uuid-%d' % (len(subvolumes) + 1)
        save()
    elif args[:2] == ['subvolume', 'show']:
        if str(path) in subvolumes:
            out = 'Name: x\n\tUUID: \t\t%s\n' % subvolumes[str(path)]
        else:
            code = 1
    elif args[:2] == ['subvolume', 'delete']:
        path.rmdir()
        subvolumes.pop(str(path), None)
        save()
elif prog == 'recreate-kind-cluster.sh':
    if not state.get('cluster'):
        config = args[args.index('--config') + 1]
        created = state.get('created', 0) + 1
        state.update(cluster=True, uid='uid-qa-%d' % created, created=created, config=config)
        cp = [m for n in __import__('yaml').safe_load(open(config))['nodes'] for m in n.get('extraPortMappings', [])]
        state['port'] = cp[0]['hostPort']
        save()
elif prog == 'kubectl':
    text = ' '.join(args)
    if 'get namespace kube-system' in text:
        out = state.get('kubeconfig_uid', state.get('uid', ''))
    elif 'get secret econ-graph-keycloak -o json' in text:
        import base64
        b64 = lambda v: base64.b64encode(v.encode()).decode()
        out = json.dumps({'apiVersion': 'v1', 'kind': 'Secret',
                          'metadata': {'name': 'econ-graph-keycloak', 'namespace': 'econ-graph',
                                       'resourceVersion': '7', 'uid': 'u'},
                          'data': {k: b64(v) for k, v in PRODUCTION_KEYCLOAK.items()}})
    elif 'get deployment crawler-worker' in text:
        out = str(state.get('deployed_crawler_replicas', 1))
    elif args[-3:] == ['replace', '-f', '-']:
        state['keycloak_secret'] = json.loads(stdin)
        save()
    elif ' exec ' in ' ' + text + ' ':
        kc = args[args.index('--') + 1:]
        kc = kc[kc.index('--') + 1:] if '--' in kc else kc
        users = state.setdefault('users', {})
        if kc[:2] == ['get', 'users']:
            user = next(kc[i + 1].split('=', 1)[1] for i, a in enumerate(kc) if a == '-q' and kc[i + 1].startswith('username='))
            out = users.get(user, {}).get('id', '')
        elif kc[:2] == ['create', 'users']:
            user = next(a.split('=', 1)[1] for a in kc if a.startswith('username='))
            users[user] = {'id': 'id-' + user, 'roles': []}
            state['creates'] = state.get('creates', 0) + 1
        elif kc[0] == 'update' and kc[1].endswith('/reset-password'):
            uid = kc[1].split('/')[1]
            user = next(u for u, v in users.items() if v['id'] == uid)
            users[user]['password'] = json.loads(stdin)['value']
        elif kc[0] == 'add-roles':
            uid = kc[kc.index('--uid') + 1]
            user = next(u for u, v in users.items() if v['id'] == uid)
            users[user]['roles'] = sorted(set(users[user]['roles']) | {kc[kc.index('--rolename') + 1]})
        save()
elif prog in ('load-release-images.sh',):
    # As the real one: the candidate's images take the manifests' local names on the daemon.
    for image in CONFIG.get('local_images', []):
        state.setdefault('daemon_images', {})[image] = 'sha256:candidate'
    save()
    code = 3
elif prog == 'deploy.sh':
    import yaml
    docs = [d for d in yaml.safe_load_all(open('k8s/manifests/crawler-worker.yaml')) if d]
    state['deployed_crawler_replicas'] = next(d['spec']['replicas'] for d in docs
                                              if d.get('kind') == 'Deployment')
    save()
sys.stdout.write(out)
sys.exit(code)
'''


def run(cmd, **kwargs):
    return subprocess.run(cmd, check=True, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, **kwargs)


class DeployRcToQaTest(unittest.TestCase):
    def setUp(self):
        base = Path(os.environ.get('TEST_TMPDIR') or Path.home() / '.cache')
        base.mkdir(parents=True, exist_ok=True)
        self.work = Path(tempfile.mkdtemp(prefix='test-deploy-rc-to-qa-', dir=base))
        self.addCleanup(shutil.rmtree, self.work, ignore_errors=True)
        self.bin = self.work / 'bin'
        self.bin.mkdir()
        stub = self.bin / 'stub.py'
        stub.write_text(STUB)
        self.state = self.work / 'state.json'
        self.calls = self.work / 'calls.jsonl'
        (self.bin / 'stub-env.json').write_text(json.dumps({
            'state': str(self.state), 'calls': str(self.calls), 'name': NAME,
            'production_keycloak': PRODUCTION_KEYCLOAK, 'local_images': LOCAL_IMAGES}))
        stub.chmod(0o755)
        for prog in ('docker', 'kind', 'btrfs'):
            (self.bin / prog).symlink_to(stub)
        self.data_home = self.work / 'data'
        kind_dir = self.data_home / 'econgraph/bin'
        kind_dir.mkdir(parents=True)
        (kind_dir / 'kind-v0.33.0').symlink_to(stub)
        # The candidate: a repository with the script, stub deploy scripts and the real
        # kind config and realm-import Job, tagged and pushed to a local origin.
        origin = self.work / 'origin.git'
        run(['git', 'init', '-q', '--bare', str(origin)])
        self.repo = self.work / 'repo'
        run(['git', 'init', '-q', str(self.repo)])
        for source in ('scripts/qa/deploy-rc-to-qa.sh', 'scripts/deploy/lib/keycloak_sanitize.py',
                       'terraform/k8s/kind-config.yaml',
                       'k8s/manifests/keycloak/realm-import-job.yaml', 'k8s/manifests/keycloak/deployment.yaml',
                       'k8s/manifests/crawler-worker.yaml', 'k8s/manifests/backend-deployment.yaml',
                       'k8s/manifests/frontend-deployment.yaml', 'k8s/manifests/crawler-migrate-job.yaml',
                       'scripts/deploy/lib/agent-access.sh',
                       'scripts/deploy/lib/kind-version.sh'):
            target = self.repo / source
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / source, target)
        for script in ('recreate-kind-cluster.sh', 'load-release-images.sh', 'restore-kind-databases.sh',
                       'deploy.sh'):
            target = self.repo / 'scripts/deploy' / script
            target.parent.mkdir(parents=True, exist_ok=True)
            target.symlink_to(stub)
        git = ['git', '-C', str(self.repo), '-c', 'user.name=t', '-c', 'user.email=t@t']
        run(git + ['add', '-A'])
        run(git + ['commit', '-q', '-m', 'candidate'])
        run(git + ['tag', TAG])
        run(git + ['remote', 'add', 'origin', str(origin)])
        run(git + ['push', '-q', 'origin', 'HEAD:refs/heads/main', '--tags'])
        self.script = self.repo / 'scripts/qa/deploy-rc-to-qa.sh'
        # The dump: a backup directory outside the QA root.
        self.dump = self.work / 'backups/2026-10-31'
        (self.dump / 'secrets').mkdir(parents=True)
        for name in ('econ_graph.dump', 'keycloak.dump', 'counts.env', 'secrets/econ-graph-secrets.yaml'):
            (self.dump / name).write_text('x' * 10)
        self.dump_before = self.fingerprint(self.dump)
        self.qa_root = self.work / 'qa'
        self.write_state({'daemon_images': {LOCAL_IMAGES[0]: 'sha256:production'}})

    def fingerprint(self, path):
        return sorted((str(p), p.stat().st_size, p.stat().st_mtime_ns) for p in path.rglob('*'))

    def write_state(self, state):
        self.state.write_text(json.dumps(state))

    def read_state(self):
        return json.loads(self.state.read_text())

    def logged(self):
        if not self.calls.exists():
            return []
        return [json.loads(line) for line in self.calls.read_text().splitlines()]

    def env(self, **extra):
        env = dict(os.environ, PATH=f'{self.bin}{os.pathsep}{os.environ["PATH"]}',
                   ECONGRAPH_QA_ROOT=str(self.qa_root), XDG_DATA_HOME=str(self.data_home),
                   FRED_API_KEY='production-fred-key', CLOUDFLARE_TUNNEL_TOKEN='production-tunnel-token',
                   DRY_RUN='false', NAMESPACE='econ-graph-prod', KUBECONFIG=str(self.work / 'operator-kubeconfig'))
        env.pop('ECONGRAPH_DOCKER_HOST', None)
        env.update(extra)
        return env

    def deploy(self, *args, **extra):
        if not args:
            args = ('--tag', TAG, '--dump-dir', str(self.dump))
        return subprocess.run([str(self.script), *args], env=self.env(**extra), text=True,
                              stdout=subprocess.PIPE, stderr=subprocess.STDOUT)

    # --- Refusals before any change ---------------------------------------------------------

    def assert_refused(self, result, message):
        self.assertEqual(result.returncode, 3, result.stdout)
        self.assertIn(message, result.stdout)
        # Only reads: docker ps and kubectl get through docker exec.
        for call in self.logged():
            self.assertEqual(call['prog'], 'docker', call)
            self.assertIn(call['args'][0], ('ps', 'exec', 'info'), call)
            self.assertIn('get', call['args'], call) if call['args'][0] == 'exec' else None
        self.assertFalse((self.qa_root / NAME / 'src').exists())
        self.assertEqual(self.fingerprint(self.dump), self.dump_before)

    def test_production_names_and_bad_names_are_refused(self):
        for name in ('econ-graph', 'econgraph-train1-qa', 'ECON-GRAPH', 'staging', 'qa-', 'qa-a_b'):
            with self.subTest(name=name):
                result = self.deploy('--tag', TAG, '--dump-dir', str(self.dump), '--name', name)
                self.assertEqual(result.returncode, 3, result.stdout)
                result = self.deploy('--tag', TAG, '--teardown', '--name', name)
                self.assertEqual(result.returncode, 3, result.stdout)
        self.assertEqual(self.logged(), [])

    def test_a_dump_inside_the_qa_root_or_containing_it_is_refused(self):
        inside = self.qa_root / 'dump'
        shutil.copytree(self.dump, inside)
        self.assert_refused(self.deploy('--tag', TAG, '--dump-dir', str(inside)), 'is inside the QA root')
        nested_root = self.dump / 'qa'
        result = self.deploy(ECONGRAPH_QA_ROOT=str(nested_root))
        self.assertEqual(result.returncode, 3, result.stdout)
        self.assertIn('is inside the dump', result.stdout)

    def test_a_qa_root_on_tmp_is_refused(self):
        self.assert_refused(self.deploy(ECONGRAPH_QA_ROOT='/tmp/econgraph-qa'), 'RAM filesystem')

    def test_a_dump_without_counts_or_dumps_is_refused(self):
        (self.dump / 'counts.env').unlink()
        self.dump_before = self.fingerprint(self.dump)
        self.assert_refused(self.deploy(), 'no counts.env')

    def test_a_cluster_with_a_production_marker_is_refused(self):
        self.write_state({'cluster': True, 'uid': 'uid-prod', 'production': True})
        self.assert_refused(self.deploy(), 'has a production marker')
        result = self.deploy('--tag', TAG, '--teardown')
        self.assertEqual(result.returncode, 3, result.stdout)
        self.assertIn('has a production marker', result.stdout)
        self.assertTrue(self.read_state()['cluster'])
        self.assertFalse(any(c['prog'] == 'kind' for c in self.logged()))

    def test_a_cluster_this_script_did_not_create_is_refused(self):
        self.write_state({'cluster': True, 'uid': 'uid-other'})
        self.assert_refused(self.deploy(), 'was not created by this script')
        self.write_state({'cluster': True, 'uid': 'uid-other', 'owner': 'someone-else'})
        self.assert_refused(self.deploy(), 'belongs to another QA environment')
        result = self.deploy('--tag', TAG, '--teardown')
        self.assertEqual(result.returncode, 3, result.stdout)
        self.assertTrue(self.read_state()['cluster'])

    def test_a_tag_without_the_scripts_it_runs_is_refused(self):
        git = ['git', '-C', str(self.repo), '-c', 'user.name=t', '-c', 'user.email=t@t']
        run(git + ['rm', '-q', 'scripts/deploy/restore-kind-databases.sh'])
        run(git + ['commit', '-q', '-m', 'old'])
        run(git + ['tag', 'v4.0.1-rc.2'])
        run(git + ['push', '-q', 'origin', '--tags'])
        result = self.deploy('--tag', 'v4.0.1-rc.2', '--dump-dir', str(self.dump))
        self.assertEqual(result.returncode, 3, result.stdout)
        self.assertIn('has no scripts/deploy/restore-kind-databases.sh', result.stdout)

    def test_dry_run_creates_nothing(self):
        result = self.deploy('--tag', TAG, '--dump-dir', str(self.dump), '--dry-run')
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertFalse((self.qa_root / NAME).exists())
        self.assertFalse(any(c['prog'] not in ('docker',) for c in self.logged()))

    # --- A full run, a rerun and a teardown ---------------------------------------------------

    def test_deploy_rerun_and_teardown(self):
        result = self.deploy()
        self.assertEqual(result.returncode, 0, result.stdout)
        qa = self.qa_root / NAME
        state = self.read_state()
        port = state['port']
        # The candidate's own scripts, in order, from its worktree, each isolated.
        rc = [c for c in self.logged() if c['prog'].endswith('.sh')]
        self.assertEqual([c['prog'] for c in rc], ['recreate-kind-cluster.sh', 'load-release-images.sh',
                                                   'restore-kind-databases.sh', 'deploy.sh'])
        for call in rc:
            self.assertEqual(Path(call['cwd']), qa / 'src')
            env = call['env']
            self.assertEqual(env['KIND_CLUSTER'], NAME)
            self.assertEqual(env['HOME'], str(qa / 'home'))
            for key, sub in [('ECONGRAPH_SCRATCH_STORAGE', 'storage'), ('XDG_CACHE_HOME', 'cache'),
                             ('ECONGRAPH_BACKUP_ROOT', 'backups'), ('ECONGRAPH_RELEASES_DIR', 'releases'),
                             ('ECONGRAPH_DEPLOY_ENV', 'deploy.env'), ('TMPDIR', 'tmp')]:
                self.assertEqual(env[key], str(qa / sub), key)
            self.assertEqual(env['PUBLIC_HOST'], '')
            for leaked in ('FRED_API_KEY', 'CLOUDFLARE_TUNNEL_TOKEN', 'DRY_RUN', 'NAMESPACE'):
                self.assertIsNone(env[leaked], leaked)
            self.assertEqual(env['KUBECONFIG'], str(qa / f'cache/econgraph/kubeconfig-{NAME}'))
            self.assertEqual(env['KUBECTL'], str(qa / 'cache/econgraph/bin/kubectl'))
            self.assertEqual(env['DOCKER_HOST'], 'unix:///var/run/docker.sock')
        # The candidate's deploy never starts a crawler.
        self.assertEqual(state['deployed_crawler_replicas'], 0)
        self.assertEqual(rc[1]['args'], ['--tag', TAG, '--cluster', NAME, '--allow-missing-nodes'])
        self.assertEqual(rc[2]['args'], ['--from', str(self.dump)])
        self.assertEqual(rc[2]['env']['APP_DATABASE'], 'shared')
        # The kind config: one port, on 127.0.0.1, to the gateway; the scratch subvolumes.
        config = yaml.safe_load((qa / 'kind-config.yaml').read_text())
        mappings = [m for n in config['nodes'] for m in n.get('extraPortMappings', [])]
        self.assertEqual(mappings, [{'containerPort': 80, 'hostPort': port, 'listenAddress': '127.0.0.1',
                                     'protocol': 'TCP'}])
        self.assertNotIn(port, (80, 443))
        mounts = {m['containerPath']: m['hostPath'] for n in config['nodes'] for m in n.get('extraMounts', [])}
        self.assertEqual(mounts, {'/srv/econgraph/data-plane': str(qa / 'storage/data-plane'),
                                  '/srv/econgraph/app-prod': str(qa / 'storage/app-prod')})
        self.assertEqual(config['networking']['apiServerAddress'], '127.0.0.1')
        # Every kubectl call is on the QA kubeconfig and context; nothing names production.
        kubectl = [c for c in self.logged() if c['prog'] == 'kubectl']
        for call in kubectl:
            self.assertEqual(call['args'][:4], ['--kubeconfig', str(qa / f'cache/econgraph/kubeconfig-{NAME}'),
                                                '--context', f'kind-{NAME}'])
        texts = [' '.join(c['args']) for c in kubectl]
        self.assertTrue(any('delete secret crawler-api-keys' in t for t in texts))
        self.assertTrue(any('delete secret grafana-admin monitoring-auth' in t for t in texts))
        self.assertTrue(any(f'KC_HOSTNAME=http://localhost:{port}/idp' in t for t in texts))
        self.assertTrue(any(f'KC_HOSTNAME_ADMIN=http://localhost:{port}/idp' in t for t in texts))
        # The daemon's local names are back where production had them: the name production's
        # daemon held points at production's image again, the others are gone.
        self.assertEqual(state['daemon_images'], {LOCAL_IMAGES[0]: 'sha256:production'})
        self.assertTrue(any(f'OIDC_ISSUER=http://localhost:{port}/idp/realms/econ-graph' in t for t in texts))
        self.assertTrue(any(f'"CORS_ORIGIN":"http://localhost:{port}"' in t for t in texts))
        self.assertTrue(any('scale deployment/crawler-worker --replicas=0' in t for t in texts))
        realm = (qa / 'realm-import.yaml').read_text()
        self.assertIn(f'http://localhost:{port}', realm)
        self.assertNotIn('value: http://localhost\n', realm)
        for call in self.logged():
            self.assertNotIn('econ-graph-control-plane', ' '.join(call['args']))
            self.assertNotIn('econgraph-train1-qa', ' '.join(call['args']))
        # QA accounts: created with roles, passwords only on stdin and in mode-600 files.
        users = state['users']
        self.assertEqual(sorted(users), ['qa-a', 'qa-admin', 'qa-b'])
        self.assertEqual(users['qa-admin']['roles'], ['admin', 'user'])
        self.assertEqual(users['qa-a']['roles'], ['user'])
        accounts = qa / 'accounts.env'
        self.assertEqual(stat.S_IMODE(accounts.stat().st_mode), 0o600)
        passwords = {u: (qa / f'accounts/{u}.password').read_text().strip() for u in users}
        for user, password in passwords.items():
            self.assertEqual(users[user]['password'], password)
            self.assertIn(f'{user.upper().replace("-", "_")}_PASSWORD={password}', accounts.read_text())
            self.assertNotIn(password, result.stdout)
            for call in self.logged():
                self.assertNotIn(password, ' '.join(call['args']))
        self.assertIn(f'TARGET_URL=http://localhost:{port}', result.stdout)
        self.assertIn(f'TARGET_HOST=localhost:{port}', result.stdout)
        self.assertIn(f'QA_LAB_TARGET_OK=localhost:{port} ', result.stdout)
        self.assertNotIn('QA_LAB_TARGET_OK=localhost ', result.stdout)
        # Production's Keycloak secrets: the database sanitized before Keycloak starts, the
        # Secret rebuilt with only db-password kept, a QA bootstrap admin added before the
        # candidate's deploy; production's admin never used, no secret in an argument.
        order = []
        for call in self.logged():
            text = ' '.join(call['args'])
            if call['prog'] == 'kubectl' and 'psql' in call['args'] and 'KeyProvider' in text:
                order.append('sanitize')
            elif call['prog'] == 'kubectl' and call['args'][-3:] == ['replace', '-f', '-']:
                order.append('secret')
            elif call['prog'] == 'kubectl' and text.endswith('k8s/manifests/keycloak/deployment.yaml'):
                order.append('keycloak')
            elif call['prog'] == 'kubectl' and text.endswith('keycloak-qa-admin.yaml'):
                order.append('bootstrap-admin')
            elif call['prog'] in ('restore-kind-databases.sh', 'deploy.sh'):
                order.append(call['prog'])
        self.assertEqual(order, ['restore-kind-databases.sh', 'sanitize', 'secret', 'keycloak',
                                 'bootstrap-admin', 'deploy.sh'])
        import base64
        secret = {k: base64.b64decode(v).decode() for k, v in state['keycloak_secret']['data'].items()}
        self.assertEqual(sorted(secret), ['admin-password', 'admin-username', 'anonymous-client-secret',
                                          'db-password'])
        self.assertEqual(secret['db-password'], PRODUCTION_KEYCLOAK['db-password'])
        self.assertEqual(secret['admin-username'], 'qa-keycloak-admin')
        self.assertEqual(secret['admin-password'], (qa / 'accounts/keycloak-admin.password').read_text().strip())
        for key in ('admin-password', 'anonymous-client-secret'):
            self.assertNotEqual(secret[key], PRODUCTION_KEYCLOAK[key])
        self.assertNotIn('resourceVersion', state['keycloak_secret']['metadata'])
        job = yaml.safe_load((qa / 'keycloak-qa-admin.yaml').read_text())
        self.assertIn('kc.sh bootstrap-admin user', job['spec']['template']['spec']['containers'][0]['args'][0])
        self.assertIn('QA_KEYCLOAK_ADMIN_USERNAME=qa-keycloak-admin', accounts.read_text())
        self.assertIn('QA_KEYCLOAK_ADMIN_PASSWORD_FILE=', accounts.read_text())
        everything = result.stdout + ''.join(' '.join(c['args']) for c in self.logged())
        for value in [*PRODUCTION_KEYCLOAK.values(), secret['admin-password'], secret['anonymous-client-secret']]:
            if value != 'restored-db-password' and len(value) > 8:
                self.assertNotIn(value, everything)
        self.assertIn(str(accounts), result.stdout)
        self.assertEqual(self.fingerprint(self.dump), self.dump_before)
        # A rerun: same cluster, port and passwords; no user created twice.
        result = self.deploy()
        self.assertEqual(result.returncode, 0, result.stdout)
        state = self.read_state()
        self.assertEqual(state['created'], 1)
        self.assertEqual(state['port'], port)
        self.assertEqual(state['creates'], 3)
        rerun = self.logged()
        self.assertEqual(sum(c['prog'] == 'restore-kind-databases.sh' for c in rerun), 1)
        self.assertEqual(sum('psql' in c['args'] for c in rerun), 1)
        self.assertEqual(sum(c['args'][-3:] == ['replace', '-f', '-'] for c in rerun), 1)
        for user, password in passwords.items():
            self.assertEqual(state['users'][user]['password'], password)
        # Another dump or tag for the same name is refused.
        other = self.work / 'other-dump'
        shutil.copytree(self.dump, other)
        result = self.deploy('--tag', TAG, '--dump-dir', str(other))
        self.assertEqual(result.returncode, 3, result.stdout)
        # Teardown: the cluster, the subvolumes, the worktree and the directory; never the dump.
        self.calls.write_text('')
        result = self.deploy('--tag', TAG, '--teardown')
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertFalse(self.read_state()['cluster'])
        self.assertFalse(qa.exists())
        self.assertEqual(self.fingerprint(self.dump), self.dump_before)
        deletes = [c for c in self.logged() if c['prog'] == 'kind']
        self.assertEqual(deletes[0]['args'][:4], ['delete', 'cluster', '--name', NAME])
        runs = [c for c in self.logged() if c['prog'] == 'docker' and c['args'][:1] == ['run']]
        for call in runs:
            self.assertTrue(any(a.startswith(str(qa / 'storage') + '/') for a in call['args']), call)
        worktrees = run(['git', '-C', str(self.repo), 'worktree', 'list']).stdout
        self.assertNotIn(str(qa), worktrees)
        result = self.deploy('--tag', TAG, '--teardown')
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn('nothing to tear down', result.stdout)

    def test_a_kubeconfig_reaching_another_cluster_is_refused(self):
        self.write_state({'kubeconfig_uid': 'uid-production'})
        result = self.deploy()
        self.assertEqual(result.returncode, 3, result.stdout)
        self.assertIn('reaches another cluster', result.stdout)
        kubectl = [c for c in self.logged() if c['prog'] == 'kubectl']
        self.assertEqual([' '.join(c['args'][4:]) for c in kubectl],
                         ['get namespace kube-system -o jsonpath={.metadata.uid}'])
        # Refused right after the cluster was created: no image, restore or deploy.
        self.assertEqual([c['prog'] for c in self.logged() if c['prog'].endswith('.sh')],
                         ['recreate-kind-cluster.sh'])


    def test_a_cluster_created_anew_runs_the_one_time_steps_again(self):
        self.assertEqual(self.deploy().returncode, 0)
        # Deleted behind the script's back: the rerun recreates it, with a new kube-system UID.
        state = self.read_state()
        state.update(cluster=False, owner='', users={})
        self.write_state(state)
        self.calls.write_text('')
        result = self.deploy()
        self.assertEqual(result.returncode, 0, result.stdout)
        progs = [c['prog'] for c in self.logged()]
        self.assertEqual(progs.count('restore-kind-databases.sh'), 1)
        self.assertEqual(sum('psql' in c['args'] for c in self.logged()), 1)
        self.assertTrue(any(c['args'][-3:] == ['replace', '-f', '-'] for c in self.logged()))
        self.assertTrue(any(' '.join(c['args']).endswith('keycloak-qa-admin.yaml') for c in self.logged()))

    # Nothing the script mounts or deletes as root can be outside its own subvolumes.

    def test_a_symlinked_storage_is_refused_for_deploy_and_teardown(self):
        elsewhere = self.work / 'srv-econgraph'
        (elsewhere / 'data-plane').mkdir(parents=True)
        (elsewhere / 'data-plane' / 'PG_VERSION').write_text('18')
        qa = self.qa_root / NAME
        qa.mkdir(parents=True)
        (qa / 'storage').symlink_to(elsewhere)
        for args in (('--tag', TAG, '--dump-dir', str(self.dump)), ('--tag', TAG, '--teardown')):
            result = self.deploy(*args)
            self.assertEqual(result.returncode, 3, result.stdout)
            self.assertIn('is a symlink', result.stdout)
        self.assertTrue((elsewhere / 'data-plane' / 'PG_VERSION').exists())
        self.assertFalse(any(c['prog'] == 'docker' and c['args'][:1] == ['run'] for c in self.logged()))

    def test_teardown_refuses_a_subvolume_it_did_not_create(self):
        self.assertEqual(self.deploy().returncode, 0)
        qa = self.qa_root / NAME
        state = self.read_state()
        # Replaced behind the script's back: another subvolume at the same path.
        state['subvolumes'][str(qa / 'storage/data-plane')] = 'uuid-other'
        self.write_state(state)
        self.calls.write_text('')
        result = self.deploy('--tag', TAG, '--teardown')
        self.assertEqual(result.returncode, 3, result.stdout)
        self.assertIn('is not the subvolume this script created', result.stdout)
        self.assertFalse(any(c['prog'] == 'docker' and c['args'][:1] == ['run'] for c in self.logged()))

    def test_teardown_of_a_cluster_stopped_before_its_owner_marker(self):
        self.assertEqual(self.deploy().returncode, 0)
        state = self.read_state()
        state['owner'] = ''
        self.write_state(state)
        state_env = self.qa_root / NAME / 'state.env'
        state_env.write_text(state_env.read_text() + 'CLUSTER=creating\n')
        result = self.deploy('--tag', TAG, '--teardown')
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertFalse(self.read_state()['cluster'])


if __name__ == '__main__':
    unittest.main()
