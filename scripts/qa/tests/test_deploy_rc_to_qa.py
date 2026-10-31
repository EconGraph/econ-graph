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
STUB = r'''#!/usr/bin/env python3
import json, os, pathlib, sys
prog = pathlib.Path(sys.argv[0]).name
prog = 'kind' if prog.startswith('kind-v') else prog
args = sys.argv[1:]
state_path = pathlib.Path(os.environ['STUB_STATE'])
state = json.loads(state_path.read_text())
stdin = '' if sys.stdin.isatty() else (sys.stdin.read() if prog in ('kubectl',) and '-i' in args else '')
keep = ('HOME', 'KIND_CLUSTER', 'ECONGRAPH_SCRATCH_STORAGE', 'XDG_CACHE_HOME', 'ECONGRAPH_BACKUP_ROOT',
        'ECONGRAPH_RELEASES_DIR', 'ECONGRAPH_DEPLOY_ENV', 'PUBLIC_HOST', 'APP_DATABASE', 'DOCKER_HOST',
        'FRED_API_KEY', 'KUBECONFIG', 'TMPDIR')
with open(os.environ['STUB_CALLS'], 'a') as log:
    log.write(json.dumps({'prog': prog, 'args': args, 'cwd': os.getcwd(),
                          'env': {k: os.environ.get(k) for k in keep}, 'stdin': stdin}) + '\n')
def save():
    state_path.write_text(json.dumps(state))
name = os.environ.get('STUB_NAME', '')
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
        pathlib.Path(args[-1]).write_text(os.environ['STUB_KUBECTL_SOURCE'])
        os.chmod(args[-1], 0o755)
    elif args[0] == 'run':
        pass
elif prog == 'kind':
    if args[:2] == ['get', 'kubeconfig']:
        out = 'apiVersion: v1\nkind: Config\n'
    elif args[:2] == ['delete', 'cluster']:
        state.update(cluster=False, owner='')
        save()
elif prog == 'btrfs':
    path = pathlib.Path(args[-1])
    if args[:2] == ['subvolume', 'create']:
        path.mkdir()
    elif args[:2] == ['subvolume', 'delete']:
        path.rmdir()
elif prog == 'recreate-kind-cluster.sh':
    if not state.get('cluster'):
        config = args[args.index('--config') + 1]
        state.update(cluster=True, uid='uid-qa', created=state.get('created', 0) + 1, config=config)
        cp = [m for n in __import__('yaml').safe_load(open(config))['nodes'] for m in n.get('extraPortMappings', [])]
        state['port'] = cp[0]['hostPort']
        save()
elif prog == 'kubectl':
    text = ' '.join(args)
    if 'get namespace kube-system' in text:
        out = state.get('kubeconfig_uid', state.get('uid', ''))
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
    code = int(os.environ.get('STUB_LOAD_EXIT', '3'))
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
        for source in ('scripts/qa/deploy-rc-to-qa.sh', 'terraform/k8s/kind-config.yaml',
                       'k8s/manifests/keycloak/realm-import-job.yaml'):
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
        self.state = self.work / 'state.json'
        self.calls = self.work / 'calls.jsonl'
        self.write_state({})

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
                   STUB_STATE=str(self.state), STUB_CALLS=str(self.calls), STUB_NAME=NAME,
                   STUB_KUBECTL_SOURCE=(self.bin / 'stub.py').read_text(),
                   FRED_API_KEY='production-fred-key', KUBECONFIG=str(Path.home() / '.kube/config'))
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
            self.assertIn(call['args'][0], ('ps', 'exec'), call)
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
            self.assertIsNone(env['FRED_API_KEY'])
            self.assertIsNone(env['KUBECONFIG'])
            self.assertEqual(env['DOCKER_HOST'], 'unix:///var/run/docker.sock')
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
        self.assertTrue(any(f'KC_HOSTNAME=http://localhost:{port}/idp' in t for t in texts))
        self.assertTrue(any(f'OIDC_ISSUER=http://localhost:{port}/idp/realms/econ-graph' in t for t in texts))
        self.assertTrue(any(f'"CORS_ORIGIN":"http://localhost:{port},http://localhost"' in t for t in texts))
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
        self.assertIn('TARGET_HOST=localhost', result.stdout)
        self.assertIn(str(accounts), result.stdout)
        self.assertEqual(self.fingerprint(self.dump), self.dump_before)
        # A rerun: same cluster, port and passwords; no user created twice.
        result = self.deploy()
        self.assertEqual(result.returncode, 0, result.stdout)
        state = self.read_state()
        self.assertEqual(state['created'], 1)
        self.assertEqual(state['port'], port)
        self.assertEqual(state['creates'], 3)
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


if __name__ == '__main__':
    unittest.main()
