#!/usr/bin/env bash
# Offline operator-switch tests: all kubectl/curl/helper execution is stubbed.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
python3 - "$ROOT" <<'PY'
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile

root = Path(sys.argv[1])
with tempfile.TemporaryDirectory() as temporary:
    work = Path(temporary)
    repo = work / 'repo'
    for source in ['scripts/deploy/switch-main-host.sh', 'scripts/deploy/lib/cluster-role.sh',
                   'scripts/deploy/lib/deploy-config.sh', 'scripts/deploy/lib/agent-access.sh',
                   'scripts/deploy/render-public-host.py', 'scripts/keycloak/render-realm.sh',
                   'scripts/keycloak/preserve-catalog-grants.py', 'config/keycloak/econ-graph-realm.json',
                   'k8s/manifests/keycloak/realm-import-job.yaml']:
        target = repo / source
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(root / source, target)
    bin_dir = work / 'bin'
    bin_dir.mkdir()
    calls = work / 'calls.jsonl'
    state = work / 'state.json'
    config = work / 'deploy.env'
    kubectl = bin_dir / 'kubectl'
    kubectl.write_text('''#!/usr/bin/env python3
import json, os, pathlib, sys
args = sys.argv[1:]
with open(os.environ['TEST_CALLS'], 'a') as output: output.write(json.dumps(args) + '\\n')
assert args[:2] == ['--context', 'test-operator-context'], args
args = args[2:]
state_file = pathlib.Path(os.environ['TEST_STATE'])
state = json.loads(state_file.read_text())
if args[:2] == ['config', 'view']: print('https://test.invalid')
elif args[:2] == ['get', 'nodes']: print('test-node')
elif args[:3] == ['get', 'namespace', 'kube-system']: print('test-uid')
elif args[:5] == ['-n', 'kube-system', 'get', 'configmap', 'econgraph-cluster-role']:
    if args[-1] == 'name': print('configmap/econgraph-cluster-role')
    elif 'kube-system-uid' in args[-1]: print('test-uid#')
    else: print(('production' if state.get('production', True) else 'none') + '#')
elif args[:5] == ['-n', 'econ-graph', 'get', 'configmap', 'keycloak-realm']:
    import re
    key = re.search(r'"([^"\\\\]+)"', args[-1]).group(1)
    source = {'econ-graph-realm.json': 'config/keycloak/econ-graph-realm.json',
              'render-realm.sh': 'scripts/keycloak/render-realm.sh',
              'preserve-catalog-grants.py': 'scripts/keycloak/preserve-catalog-grants.py'}[key]
    print('stale' if state.get('stale_realm') else (pathlib.Path(os.environ['TEST_REPO']) / source).read_text(), end='')
elif args[:5] == ['-n', 'econ-graph', 'get', 'deployment', 'keycloak']: print(state['hostname'])
elif args[:5] == ['-n', 'econ-graph', 'get', 'deployment', 'econ-graph-backend']: pass
elif args[:5] == ['-n', 'econ-graph', 'get', 'configmap', 'econ-graph-config']: print(state['cors'])
elif args[:4] == ['-n', 'econ-graph', 'set', 'env']:
    if args[4] == 'deployment/keycloak':
        state['hostname'] = next(arg.split('=', 1)[1] for arg in args if arg.startswith('KC_HOSTNAME='))
    else: state['issuer'] = next(arg.split('=', 1)[1] for arg in args if arg.startswith('OIDC_ISSUER='))
elif args[:5] == ['-n', 'econ-graph', 'patch', 'configmap', 'econ-graph-config']:
    state['cors'] = json.loads(args[-1])['data']['CORS_ORIGIN']
elif args[:5] == ['-n', 'econ-graph', 'patch', 'deployment', 'econ-graph-backend']:
    checksum = json.loads(args[-1])['spec']['template']['metadata']['annotations']['econgraph.io/cors-origins-sha256']
    if checksum != state.get('cors_checksum'):
        state['restarts'] = state.get('restarts', 0) + 1
    state['cors_checksum'] = checksum
elif args[:4] == ['-n', 'econ-graph', 'exec', 'deployment/keycloak']:
    if args[-4:] == ['get', 'realms/econ-graph', '--fields', 'attributes']:
        print(json.dumps({'attributes': {'frontendUrl': state.get('frontend_url', state['hostname'])}}))
    else:
        assert args[-4:] == ['create', 'logout-all', '-r', 'econ-graph'], args
        state['logouts'] = state.get('logouts', 0) + 1
elif args[:2] == ['apply', '-f']:
    import yaml
    job = yaml.safe_load(pathlib.Path(args[-1]).read_text())
    env = {item['name']: item.get('value') for item in job['spec']['template']['spec']['initContainers'][0]['env']}
    assert env['KC_WEB_BASE_URL'] == state['hostname'][:-4], env
    assert env['KC_WEB_E2E_BASE_URL'] == env['KC_WEB_BASE_URL'], env
    state['frontend_url'] = state['hostname']
    state['imports'] = state.get('imports', 0) + 1
elif args[:4] == ['-n', 'econ-graph', 'rollout', 'status']:
    if state.get('fail_rollout'): sys.exit(1)
elif args[:4] == ['-n', 'econ-graph', 'rollout', 'restart']: state['restarts'] = state.get('restarts', 0) + 1
elif args[:5] == ['-n', 'econ-graph', 'delete', 'job', 'keycloak-realm-import']: pass
elif args[:3] == ['-n', 'econ-graph', 'wait']: pass
else: raise AssertionError(args)
state_file.write_text(json.dumps(state))
''')
    kubectl.chmod(0o755)
    # Unexpected curl must fail: the public checker below is a deterministic stub.
    curl = bin_dir / 'curl'
    curl.write_text('#!/bin/sh\necho unexpected-live-curl >&2\nexit 91\n')
    curl.chmod(0o755)
    for helper in ['apply-gateway-routes.sh', 'check-public-site.sh']:
        target = repo / 'scripts/deploy' / helper
        target.write_text('''#!/usr/bin/env python3
import json, os, pathlib, sys
with open(os.environ['TEST_CALLS'], 'a') as output:
    output.write(json.dumps([pathlib.Path(__file__).name, os.environ['PUBLIC_HOST'], os.environ['MAIN_HOST']] + sys.argv[1:]) + '\\n')
if pathlib.Path(__file__).name == 'apply-gateway-routes.sh':
    assert sys.argv[1:] == ['--context', 'test-operator-context']
else:
    assert sys.argv[1:] == ['--host', os.environ['PUBLIC_HOST'], '--main-host', os.environ['MAIN_HOST']]
''')
        target.chmod(0o755)
    env = dict(os.environ, PATH=str(bin_dir) + os.pathsep + os.environ['PATH'],
               KUBECTL=str(kubectl), KUBECONFIG=str(work / 'no-kubeconfig'),
               ECONGRAPH_DEPLOY_ENV=str(config), TEST_CALLS=str(calls),
               TEST_STATE=str(state), TEST_REPO=str(repo))
    env.pop('ECONGRAPH_AGENT_SESSION', None)
    env.pop('MAIN_HOST', None)
    original = '# retained comment\nGATEWAY_TLS_ISSUER=econ-graph-origin-ca\nPUBLIC_HOST=econgraph.com\nMAIN_HOST=econgraph.com\n'
    initial = {'hostname': 'https://econgraph.com/idp', 'cors': 'https://econgraph.com,http://localhost,http://localhost:3000'}
    def reset(**extra):
        calls.write_text('')
        config.write_text(original)
        config.chmod(0o640)
        state.write_text(json.dumps({**initial, **extra}))
    def run(*args, environment=None):
        return subprocess.run([str(repo / 'scripts/deploy/switch-main-host.sh'), '--context', 'test-operator-context',
                               '--host', 'econgraph.com', *args], env=environment or env,
                              text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    reset()
    result = run('--dry-run')
    assert result.returncode == 0, result.stdout
    assert calls.read_text() == '' and config.read_text() == original
    assert 'DNS:' in result.stdout and 'Tunnel:' in result.stdout
    print('ok dry-run has no cluster, network or config mutation')
    result = run('--main-host', 'unrelated.invalid')
    assert result.returncode != 0 and calls.read_text() == ''
    reset()
    result = run(environment=dict(env, ECONGRAPH_AGENT_SESSION='1'))
    assert result.returncode != 0 and calls.read_text() == '' and config.read_text() == original
    print('ok unrelated hosts and agent sessions rejected before cluster access')
    for refusal in [{'production': False}, {'stale_realm': True}]:
        reset(**refusal)
        result = run()
        assert result.returncode != 0 and config.read_text() == original, result.stdout
        assert json.loads(state.read_text())['hostname'] == initial['hostname']
    print('ok nonproduction and stale realm refuse before mutation')
    reset()
    result = run()
    assert result.returncode == 0, result.stdout
    current = json.loads(state.read_text())
    assert current['hostname'] == 'https://www.econgraph.com/idp'
    assert current['issuer'] == 'https://www.econgraph.com/idp/realms/econ-graph'
    assert current['cors'] == 'https://econgraph.com,https://www.econgraph.com,http://localhost'
    assert current['logouts'] == 1 and current['restarts'] == 1 and current['imports'] == 1
    assert config.read_text() == original.replace('MAIN_HOST=econgraph.com', 'MAIN_HOST=www.econgraph.com')
    assert stat.S_IMODE(config.stat().st_mode) == 0o640
    result = run()
    assert result.returncode == 0, result.stdout
    current = json.loads(state.read_text())
    assert current['logouts'] == 1 and current['restarts'] == 1 and current['imports'] == 2
    print('ok forward and rerun preserve settings/mode and sign out only once')
    result = run('--main-host', 'econgraph.com')
    assert result.returncode == 0, result.stdout
    current = json.loads(state.read_text())
    assert current['hostname'] == 'https://econgraph.com/idp' and current['logouts'] == 2
    assert current['issuer'] == 'https://econgraph.com/idp/realms/econ-graph'
    assert current['cors'] == 'https://econgraph.com,https://www.econgraph.com,http://localhost'
    result = run('--main-host', 'econgraph.com')
    assert result.returncode == 0 and json.loads(state.read_text())['logouts'] == 2, result.stdout
    assert config.read_text() == original
    print('ok rollback and rollback rerun keep both CORS origins and restore persisted apex')
    reset(hostname='https://www.econgraph.com/idp', frontend_url='https://econgraph.com/idp')
    result = run()
    assert result.returncode == 0 and json.loads(state.read_text())['logouts'] == 1, result.stdout
    result = run()
    assert result.returncode == 0 and json.loads(state.read_text())['logouts'] == 1, result.stdout
    print('ok legacy realm frontendUrl triggers signout even with already-aligned hostname')
    reset(cors='https://econgraph.com,https://www.econgraph.com,http://localhost')
    result = run()
    assert result.returncode == 0 and json.loads(state.read_text())['restarts'] == 1, result.stdout
    print('ok already-updated CORS ConfigMap still reconciles stale backend pods')
    reset(fail_rollout=True)
    result = run()
    assert result.returncode != 0 and 'MAIN_HOST=www.econgraph.com' in config.read_text()
    assert not any('apply-gateway-routes.sh' in line for line in calls.read_text().splitlines())
    current = json.loads(state.read_text())
    current['fail_rollout'] = False
    state.write_text(json.dumps(current))
    result = run()
    assert result.returncode == 0 and json.loads(state.read_text())['logouts'] == 1, result.stdout
    print('ok failed rollout stops before edge switch and rerun recovers without another signout')
print('main-host operator switch: all passed')
PY
