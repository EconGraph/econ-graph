#!/usr/bin/env bash
# Offline integration checks: explicit context, no-contact dry run, public-only
# certificate transfer, mount-only patch, idempotency and refusal to replace roots.
set -euo pipefail
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
python3 - "$project_root" <<'PY'
import base64
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import yaml

root = Path(sys.argv[1])
script = root / 'scripts/deploy/stage-tunnel-origin-ca.sh'
with tempfile.TemporaryDirectory() as folder:
    work = Path(folder)
    ca = work / 'ca.crt'
    subprocess.run(['openssl', 'req', '-x509', '-newkey', 'ec', '-pkeyopt', 'ec_paramgen_curve:P-256',
                    '-nodes', '-keyout', str(work / 'test.key'), '-out', str(ca), '-days', '365',
                    '-subj', '/CN=offline-test-ca', '-addext', 'basicConstraints=critical,CA:TRUE'],
                   check=True, capture_output=True)
    calls = work / 'calls.jsonl'
    kubectl = work / 'kubectl'
    kubectl.write_text('''#!/usr/bin/env python3
import base64, json, os, sys
from pathlib import Path
args=sys.argv[1:]
assert args[:2] == ['--context', 'scratch-only'], args
with open(os.environ['CALLS'], 'a') as f: f.write(json.dumps(args)+'\\n')
if 'secret' in args:
    assert args == ['--context','scratch-only','-n','cert-manager','get','secret','econ-graph-origin-ca','-o','jsonpath={.data.tls\\\\.crt}'], args
    print(base64.b64encode(Path(os.environ['CA']).read_bytes()).decode(), end='')
elif 'get' in args and 'configmap' in args:
    print(os.environ.get('EXISTING_CA', ''),end='')
elif 'create' in args and 'configmap' in args:
    certarg=next(arg for arg in args if arg.startswith('--from-file='))
    cert=Path(certarg.split('=',2)[2]).read_text()
    assert 'PRIVATE KEY' not in cert
    print(json.dumps({'apiVersion':'v1','kind':'ConfigMap','metadata':{'name':'cloudflared-origin-ca','namespace':'econ-graph'},'data':{'ca.crt':cert}}))
elif args[-1] == '-':
    document=json.load(sys.stdin)
    assert document['kind']=='ConfigMap' and 'PRIVATE KEY' not in json.dumps(document)
elif 'patch' in args:
    assert args[4:7] == ['patch','deployment','cloudflared'], args
''')
    kubectl.chmod(0o755)
    env = dict(os.environ, KUBECTL=str(kubectl), CALLS=str(calls), CA=str(ca))
    def run(*args, **updates):
        """Run staging with a pinned fake cluster and only synthetic public certificates."""
        return subprocess.run([str(script),*args],env=dict(env,**updates),text=True,capture_output=True)
    assert run('--apply').returncode == 2
    plan=run('--context','scratch-only')
    assert plan.returncode == 0 and 'Restart cloudflared' in plan.stdout
    assert not calls.exists(), 'default dry run contacted the cluster'
    assert run('--context','scratch-only','--dry-run').returncode == 0
    assert not calls.exists(), 'explicit dry run contacted the cluster'
    result=run('--context','scratch-only','--apply')
    assert result.returncode == 0, result.stderr
    commands=[json.loads(line) for line in calls.read_text().splitlines()]
    assert all(command[:2]==['--context','scratch-only'] for command in commands)
    assert not any('delete' in command or 'scale' in command for command in commands)
    assert not any('gateway' in command or 'econ-graph-tls' in command for command in commands)
    restart=next(i for i, command in enumerate(commands) if 'restart' in command)
    status=next(i for i, command in enumerate(commands) if 'status' in command)
    publish=next(i for i, command in enumerate(commands) if command[-3:] == ['apply','-f','-'])
    assert publish < restart < status, 'must restart connectors after publishing the CA, then wait'

    patch=yaml.safe_load((root/'k8s/manifests/tunnel-trust/cloudflared-mount.yaml').read_text())
    assert set(patch['spec']) == {'template'}, 'mount patch must preserve replicas'
    pod=patch['spec']['template']['spec']
    assert not any('secret' in volume for volume in pod['volumes'])
    assert pod['containers'][0]['volumeMounts'][0]['readOnly'] is True
    assert run('--context','scratch-only','--apply',EXISTING_CA=ca.read_text()).returncode == 0
    calls.write_text('')
    result=run('--context','scratch-only','--apply',EXISTING_CA='different-existing-public-root')
    assert result.returncode != 0 and 'two-root rotation' in result.stderr
    assert not any('patch' in json.loads(line) for line in calls.read_text().splitlines())
    # Near-expiry roots must not be published as a new trust bundle.
    subprocess.run(['openssl','req','-x509','-newkey','ec','-pkeyopt','ec_paramgen_curve:P-256',
                    '-nodes','-keyout',str(work/'short.key'),'-out',str(ca),'-days','30',
                    '-subj','/CN=short-test-ca','-addext','basicConstraints=critical,CA:TRUE'],
                   check=True,capture_output=True)
    calls.write_text('')
    assert run('--context','scratch-only','--apply').returncode != 0
    assert not any('patch' in json.loads(line) for line in calls.read_text().splitlines())
print('PASS: offline tunnel origin CA staging checks')
PY
