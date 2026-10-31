#!/usr/bin/env bash
# Offline tests for scripts/deploy/switch-main-host.sh: kubectl, curl and the gateway and
# public-site helpers are stubs, and HOME and the deploy config are sandboxed, so nothing
# reaches a cluster or the network.
#   - dry run: no cluster, network or config access, and no --confirm-production needed;
#   - refusals before any change: unrelated hosts, agent sessions, a cluster not marked
#     production, a stale realm ConfigMap, a missing or wrong --confirm-production;
#   - order: Keycloak's KC_HOSTNAME rollout and the realm import (frontendUrl) come before
#     the backend's OIDC_ISSUER, so the backend never expects an issuer Keycloak does not
#     issue (the kubectl stub fails the run otherwise), forward and on rollback;
#   - the app rolls through lib/app-config.sh's hash, once per config change;
#   - forward, rerun, rollback, legacy frontendUrl, interrupted-run recovery;
#   - --finalize makes the redirects permanent only after a completed switch, idempotently,
#     and a switch to another main host drops it again.
#
# Usage: scripts/deploy/test-switch-main-host.sh
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
python3 - "$ROOT" <<'PY'
import hashlib
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
                   'scripts/deploy/lib/app-config.sh',
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
elif args[:3] == ['-n', 'econ-graph', 'get'] and args[3].startswith('deployment/') and 'app-config-hash' in args[-1]:
    print(state.get('app_hash', {}).get(args[3][11:], ''), end='')
elif args[:5] == ['-n', 'econ-graph', 'get', 'configmap', 'econ-graph-config']:
    if args[-1] == 'jsonpath={.data}': print(json.dumps({'CORS_ORIGIN': state['cors']}), end='')
    else: print(state['cors'])
elif args[:4] == ['-n', 'econ-graph', 'set', 'env']:
    if args[4] == 'deployment/keycloak':
        state['hostname'] = next(arg.split('=', 1)[1] for arg in args if arg.startswith('KC_HOSTNAME='))
    else:
        state['issuer'] = next(arg.split('=', 1)[1] for arg in args if arg.startswith('OIDC_ISSUER='))
        # The realm's frontendUrl overrides KC_HOSTNAME: Keycloak issues tokens for it.
        issued = state['frontend_url'] + '/realms/econ-graph'
        if issued != state['issuer']:
            print('backend expects %s while Keycloak issues %s' % (state['issuer'], issued), file=sys.stderr)
            sys.exit(97)
elif args[:5] == ['-n', 'econ-graph', 'patch', 'configmap', 'econ-graph-config']:
    state['cors'] = json.loads(args[-1])['data']['CORS_ORIGIN']
elif args[:3] == ['-n', 'econ-graph', 'patch'] and args[3].startswith('deployment/'):
    value = json.loads(args[-1])['spec']['template']['metadata']['annotations']['econgraph.io/app-config-hash']
    hashes = state.setdefault('app_hash', {})
    if hashes.get(args[3][11:]) != value:
        state['restarts'] = state.get('restarts', 0) + 1
    hashes[args[3][11:]] = value
elif args[:4] == ['-n', 'econ-graph', 'exec', 'deployment/keycloak']:
    if args[-4:] == ['get', 'realms/econ-graph', '--fields', 'attributes']:
        print(json.dumps({'attributes': {'frontendUrl': state['frontend_url']}}))
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
    if state.get('fail_rollout') == args[4]: sys.exit(1)
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
    output.write(json.dumps([pathlib.Path(__file__).name, os.environ['PUBLIC_HOST'], os.environ['MAIN_HOST'],
                             'permanent=' + os.environ.get('MAIN_HOST_PERMANENT', '<unset>')] + sys.argv[1:]) + '\\n')
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
    apps = ['econ-graph-backend', 'crawler-worker', 'econ-graph-frontend']
    def app_hash(cors):
        return hashlib.sha256(json.dumps({'CORS_ORIGIN': cors}).encode()).hexdigest()[:16]
    old_cors = 'https://econgraph.com,http://localhost,http://localhost:3000'
    new_cors = 'https://econgraph.com,https://www.econgraph.com,http://localhost'
    # A cluster deploy.sh last rolled for its current ConfigMap (lib/app-config.sh).
    initial = {'hostname': 'https://econgraph.com/idp', 'frontend_url': 'https://econgraph.com/idp',
               'cors': old_cors, 'app_hash': {app: app_hash(old_cors) for app in apps}}
    def reset(config_text=original, **extra):
        calls.write_text('')
        config.write_text(config_text)
        config.chmod(0o640)
        state.write_text(json.dumps({**initial, **extra}))
    def run(*args, environment=None, confirm='test-uid'):
        confirmation = ['--confirm-production', confirm] if confirm else []
        return subprocess.run([str(repo / 'scripts/deploy/switch-main-host.sh'), '--context', 'test-operator-context',
                               '--host', 'econgraph.com', *confirmation, *args], env=environment or env,
                              text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    def logged():
        return [json.loads(line) for line in calls.read_text().splitlines()]
    def position(predicate, what):
        found = [i for i, call in enumerate(logged()) if predicate(call)]
        assert found, (what, logged())
        return found[0]
    def assert_issuer_order():
        """Keycloak's issuer (KC_HOSTNAME, then the realm frontendUrl) moves before the backend's."""
        kc_env = position(lambda c: c[2:6] == ['-n', 'econ-graph', 'set', 'env'] and c[6] == 'deployment/keycloak', 'keycloak env')
        kc_ready = position(lambda c: c[2:7] == ['-n', 'econ-graph', 'rollout', 'status', 'deployment/keycloak'], 'keycloak rollout')
        realm = position(lambda c: c[2:4] == ['apply', '-f'], 'realm import')
        realm_done = position(lambda c: c[2:5] == ['-n', 'econ-graph', 'wait'], 'realm import wait')
        backend = position(lambda c: c[2:6] == ['-n', 'econ-graph', 'set', 'env'] and c[6] == 'deployment/econ-graph-backend', 'backend env')
        roll = position(lambda c: c[2:5] == ['-n', 'econ-graph', 'rollout'] and c[6] == 'deployment/econ-graph-backend', 'backend rollout')
        edge = position(lambda c: c[0] == 'apply-gateway-routes.sh', 'gateway routes')
        assert kc_env < kc_ready < realm < realm_done < backend < roll < edge, logged()
    for dry in ([], ['--finalize'], ['--main-host', 'econgraph.com']):
        reset()
        result = run('--dry-run', *dry, confirm=None)
        assert result.returncode == 0, result.stdout
        assert calls.read_text() == '' and config.read_text() == original
        assert 'DNS:' in result.stdout and 'Tunnel:' in result.stdout
    assert 'realm import, moving the realm frontendUrl' in run('--dry-run', confirm=None).stdout
    print('ok dry-run has no cluster, network or config mutation and needs no confirmation')
    result = run('--main-host', 'unrelated.invalid')
    assert result.returncode != 0 and calls.read_text() == ''
    result = run('--main-host', 'econgraph.com', '--finalize')
    assert result.returncode != 0 and calls.read_text() == '' and 'always permanent' in result.stdout
    reset()
    result = run(environment=dict(env, ECONGRAPH_AGENT_SESSION='1'))
    assert result.returncode != 0 and calls.read_text() == '' and config.read_text() == original
    print('ok unrelated hosts, apex finalize and agent sessions rejected before cluster access')
    for refusal in [{'production': False}, {'stale_realm': True}]:
        reset(**refusal)
        result = run()
        assert result.returncode != 0 and config.read_text() == original, result.stdout
        assert json.loads(state.read_text())['hostname'] == initial['hostname']
    print('ok nonproduction and stale realm refuse before mutation')
    for confirm, message in [(None, 'needs --confirm-production'), ('wrong-ui', 'does not match')]:
        for extra in ([], ['--finalize']):
            reset()
            result = run(*extra, confirm=confirm)
            assert result.returncode != 0 and message in result.stdout, result.stdout
            assert config.read_text() == original and json.loads(state.read_text()) == initial
            assert not any(c[2:4] in (['-n', 'econ-graph'],) and c[4] in ('set', 'patch', 'exec', 'apply', 'delete')
                           for c in logged() if len(c) > 4), logged()
    print('ok production needs the matching --confirm-production, refused before any change')
    reset()
    result = run()
    assert result.returncode == 0, result.stdout
    assert_issuer_order()
    current = json.loads(state.read_text())
    assert current['hostname'] == 'https://www.econgraph.com/idp'
    assert current['frontend_url'] == 'https://www.econgraph.com/idp'
    assert current['issuer'] == 'https://www.econgraph.com/idp/realms/econ-graph'
    assert current['cors'] == new_cors
    assert current['app_hash'] == {app: app_hash(new_cors) for app in apps}
    assert current['logouts'] == 1 and current['restarts'] == 3 and current['imports'] == 1
    assert config.read_text() == original.replace('MAIN_HOST=econgraph.com', 'MAIN_HOST=www.econgraph.com')
    assert stat.S_IMODE(config.stat().st_mode) == 0o640
    assert ['check-public-site.sh', 'econgraph.com', 'www.econgraph.com', 'permanent=',
            '--host', 'econgraph.com', '--main-host', 'www.econgraph.com'] in logged()
    assert '--finalize --confirm-production test-uid' in result.stdout
    result = run()
    assert result.returncode == 0, result.stdout
    current = json.loads(state.read_text())
    assert current['logouts'] == 1 and current['restarts'] == 3 and current['imports'] == 2
    print('ok forward: Keycloak issuer and realm frontendUrl move before the backend; rerun signs out and rolls nothing')
    result = run('--main-host', 'econgraph.com')
    assert result.returncode == 0, result.stdout
    assert_issuer_order()
    current = json.loads(state.read_text())
    assert current['hostname'] == 'https://econgraph.com/idp' and current['logouts'] == 2
    assert current['frontend_url'] == 'https://econgraph.com/idp'
    assert current['issuer'] == 'https://econgraph.com/idp/realms/econ-graph'
    assert current['cors'] == new_cors
    result = run('--main-host', 'econgraph.com')
    assert result.returncode == 0 and json.loads(state.read_text())['logouts'] == 2, result.stdout
    assert config.read_text() == original
    print('ok rollback runs the same order, keeps both CORS origins and restores persisted apex')
    reset(hostname='https://www.econgraph.com/idp', frontend_url='https://econgraph.com/idp')
    result = run()
    assert result.returncode == 0 and json.loads(state.read_text())['logouts'] == 1, result.stdout
    result = run()
    assert result.returncode == 0 and json.loads(state.read_text())['logouts'] == 1, result.stdout
    print('ok legacy realm frontendUrl triggers signout even with already-aligned hostname')
    reset(cors=new_cors)
    result = run()
    assert result.returncode == 0 and json.loads(state.read_text())['restarts'] == 3, result.stdout
    assert json.loads(state.read_text())['app_hash'] == {app: app_hash(new_cors) for app in apps}
    print('ok already-updated CORS ConfigMap still rolls stale app pods (app-config hash)')
    # Interrupted after the realm import: the issuer already moved, so a rerun recovers
    # without signing out again.
    reset(fail_rollout='deployment/econ-graph-backend')
    result = run()
    assert result.returncode != 0 and 'MAIN_HOST=www.econgraph.com' in config.read_text()
    assert not any('apply-gateway-routes.sh' in line for line in calls.read_text().splitlines())
    current = json.loads(state.read_text())
    assert current['frontend_url'] == 'https://www.econgraph.com/idp'
    current['fail_rollout'] = None
    state.write_text(json.dumps(current))
    result = run()
    assert result.returncode == 0 and json.loads(state.read_text())['logouts'] == 1, result.stdout
    # Interrupted before it: Keycloak still issued the old issuer, so sessions made in
    # between are signed out too, and the backend was never changed.
    reset(fail_rollout='deployment/keycloak')
    result = run()
    current = json.loads(state.read_text())
    assert result.returncode != 0 and 'issuer' not in current and current['frontend_url'] == 'https://econgraph.com/idp'
    assert not any(c[2:4] == ['apply', '-f'] for c in logged())
    current['fail_rollout'] = None
    state.write_text(json.dumps(current))
    result = run()
    assert result.returncode == 0 and json.loads(state.read_text())['logouts'] == 2, result.stdout
    print('ok failed rollout stops before the edge switch; rerun recovers, signing out again only if the issuer had not moved')
    # --finalize: only after a completed switch; persists and applies permanent redirects.
    reset()
    result = run('--finalize')
    assert result.returncode != 0 and 'run the switch to www.econgraph.com first' in result.stdout, result.stdout
    assert config.read_text() == original and not any(c[0] == 'apply-gateway-routes.sh' for c in logged())
    reset(original.replace('MAIN_HOST=econgraph.com', 'MAIN_HOST=www.econgraph.com'))
    result = run('--finalize')
    assert result.returncode != 0 and "Keycloak's hostname is https://econgraph.com/idp" in result.stdout, result.stdout
    reset()
    assert run().returncode == 0
    switched = json.loads(state.read_text())
    calls.write_text('')
    result = run('--finalize')
    assert result.returncode == 0, result.stdout
    finalized = original.replace('MAIN_HOST=econgraph.com', 'MAIN_HOST=www.econgraph.com') + 'MAIN_HOST_PERMANENT=www.econgraph.com\n'
    assert config.read_text() == finalized, config.read_text()
    assert json.loads(state.read_text()) == switched, 'finalize changed Keycloak, the backend or the realm'
    assert [c[0] for c in logged() if c[0].endswith('.sh')] == ['apply-gateway-routes.sh', 'check-public-site.sh']
    assert all(c[3] == 'permanent=www.econgraph.com' for c in logged() if c[0].endswith('.sh'))
    assert run('--finalize').returncode == 0 and config.read_text() == finalized
    print('ok --finalize needs a completed switch, persists permanent redirects and is idempotent')
    calls.write_text('')
    assert run().returncode == 0 and config.read_text() == finalized
    assert all(c[3] == 'permanent=www.econgraph.com' for c in logged() if c[0].endswith('.sh'))
    assert run('--main-host', 'ECONGRAPH.com').returncode == 0
    assert 'MAIN_HOST_PERMANENT' not in config.read_text() and 'MAIN_HOST=econgraph.com' in config.read_text()
    calls.write_text('')
    assert run().returncode == 0 and 'MAIN_HOST_PERMANENT' not in config.read_text()
    assert all(c[3] == 'permanent=' for c in logged() if c[0].endswith('.sh'))
    print('ok a finalized forward rerun stays permanent; rollback drops it, so a new switch starts temporary')
    reset()
    result = subprocess.run([str(repo / 'scripts/deploy/switch-main-host.sh'), '--context', 'test-operator-context',
                             '--host', 'EconGraph.com', '--main-host', 'WWW.econgraph.com',
                             '--confirm-production', 'test-uid'], env=env, text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    assert result.returncode == 0, result.stdout
    assert json.loads(state.read_text())['hostname'] == 'https://www.econgraph.com/idp'
    print('ok --host and --main-host compare case-insensitively with the deploy config')
print('main-host operator switch: all passed')
PY
