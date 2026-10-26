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
# Cross-file contract: readiness and public-certificate extraction must refer to
# the root actually issued and consumed by cert-manager, never an unrelated old Secret.
resources = list(yaml.safe_load_all((root / 'k8s/manifests/tunnel-trust/ca.yaml').read_text()))
by_kind = {resource['kind']: resource for resource in resources}
assert len(resources) == len(by_kind) == 3
issuer = by_kind['Issuer']
certificate = by_kind['Certificate']
clusterissuer = by_kind['ClusterIssuer']
assert issuer['metadata'] == {'name': 'econ-graph-origin-bootstrap', 'namespace': 'cert-manager'}
assert issuer['spec'] == {'selfSigned': {}}
assert certificate['metadata'] == {'name': 'econ-graph-origin-ca', 'namespace': 'cert-manager'}
assert certificate['spec']['isCA'] is True
assert certificate['spec']['secretName'] == 'econ-graph-origin-ca'
assert certificate['spec']['issuerRef'] == {'group': 'cert-manager.io', 'kind': 'Issuer', 'name': issuer['metadata']['name']}
assert clusterissuer['metadata'] == {'name': 'econ-graph-origin-ca'}
assert clusterissuer['spec']['ca']['secretName'] == certificate['spec']['secretName']
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
    certificate_wait = ['--context', 'scratch-only', '-n', 'cert-manager', 'wait',
                        '--for=condition=Ready', 'certificate/econ-graph-origin-ca', '--timeout=120s']
    issuer_wait = ['--context', 'scratch-only', 'wait', '--for=condition=Ready',
                   'clusterissuer/econ-graph-origin-ca', '--timeout=120s']
    assert certificate_wait in commands and issuer_wait in commands
    secret = next(i for i, command in enumerate(commands) if 'secret' in command)
    restart=next(i for i, command in enumerate(commands) if 'restart' in command)
    status=next(i for i, command in enumerate(commands) if 'status' in command)
    publish=next(i for i, command in enumerate(commands) if command[-3:] == ['apply','-f','-'])
    assert commands.index(certificate_wait) < commands.index(issuer_wait) < secret < publish < restart < status, 'must await the issuing CA before extraction, then publish, restart and wait'

    patch=yaml.safe_load((root/'k8s/manifests/tunnel-trust/cloudflared-mount.yaml').read_text())
    assert set(patch['spec']) == {'template'}, 'mount patch must preserve replicas'
    pod=patch['spec']['template']['spec']
    assert not any('secret' in volume for volume in pod['volumes'])
    patch_container = pod['containers'][0]
    assert patch_container['name'] == 'cloudflared'
    mount = patch_container['volumeMounts'][0]
    assert mount == {'name': 'origin-ca', 'mountPath': '/etc/cloudflared/origin-ca', 'readOnly': True}
    volume = next(volume for volume in pod['volumes'] if volume['name'] == mount['name'])
    assert volume == {'name': 'origin-ca', 'configMap': {'name': 'cloudflared-origin-ca',
                     'optional': True, 'items': [{'key': 'ca.crt', 'path': 'ca.crt'}]}}
    deployment = yaml.safe_load((root/'k8s/manifests/cloudflared.yaml').read_text())
    deployed_pod = deployment['spec']['template']['spec']
    deployed_container = next(container for container in deployed_pod['containers']
                              if container['name'] == patch_container['name'])
    assert mount in deployed_container['volumeMounts']
    assert volume in deployed_pod['volumes'], 'normal deploys must preserve the staged trust mount'
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
