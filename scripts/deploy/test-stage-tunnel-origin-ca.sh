#!/usr/bin/env bash
# Offline integration checks against stub kubectl, never a cluster: explicit context,
# no-contact dry run, public-only certificate transfer, mount-only patch, no restart
# when the bundle and mount are already in place, staging before the connector
# exists, replacement of a renewed root (same subject and key) and refusal of any
# other root; and apply-gateway-routes.sh staging the CA before it selects issuer
# econ-graph-origin-ca, printing it under --dry-run and not changing the certificate
# when staging fails; its issuer choice (the environment; else the host's deploy
# config only when it matches the current certificate's issuer or there is none,
# refused when it differs, never moving off econ-graph-origin-ca implicitly; else
# the current issuer; else self-signed), malformed, exported or unreadable config
# refused, and the config unread without a public host, with HOME and
# ECONGRAPH_CONFIG_DIR in a scratch directory, never the real ~/.config/econgraph.
set -euo pipefail
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
python3 - "$project_root" <<'PY'
import base64
import json
import os
import shutil
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
def public_cert(variable):
    print(base64.b64encode(Path(os.environ[variable]).read_bytes()).decode(), end='')
if 'secret' in args:
    # Only ever the public certificate field.
    assert args[-2:] == ['-o','jsonpath={.data.tls\\\\.crt}'], args
    if args[2:7] == ['-n','cert-manager','get','secret','econ-graph-origin-ca']:
        public_cert('CA')
    elif args[2:7] == ['-n','cert-manager','get','secret',os.environ.get('RET_SECRET_PRESENT', os.environ.get('RET_SECRET'))]:
        public_cert('RET_ROOT')
    elif args[2:7] == ['-n','econ-graph-gateway','get','secret','econ-graph-tls']:
        public_cert('RET_LEAF')
    else:
        sys.exit('no such secret: ' + ' '.join(args))
elif 'certificate' in args and 'econ-graph-tls' in args:
    print(os.environ.get('RET_KIND', 'ClusterIssuer') if 'kind' in args[-1] else os.environ.get('RET_ISSUER', ''), end='')
elif 'clusterissuer' in args and 'get' in args:
    print(os.environ.get('RET_SECRET', ''), end='')
elif 'get' in args and 'configmap' in args:
    print(os.environ.get('EXISTING_CA', ''),end='')
elif 'get' in args and 'deployment' in args:
    if 'name' in args:
        print('' if os.environ.get('NO_CONNECTOR') else 'deployment.apps/cloudflared')
    elif os.environ.get('MOUNTED'):
        print('cloudflared-origin-ca' if 'volumes' in args[-1] else '/etc/cloudflared/origin-ca', end='')
elif 'create' in args and 'configmap' in args:
    certarg=next(arg for arg in args if arg.startswith('--from-file='))
    cert=Path(certarg.split('=',2)[2]).read_text()
    assert 'PRIVATE KEY' not in cert
    if os.environ.get('PUBLISHED'): Path(os.environ['PUBLISHED']).write_text(cert)
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
    def logged():
        return [json.loads(line) for line in calls.read_text().splitlines()]
    def published(commands):
        return any(command[-3:] == ['apply', '-f', '-'] for command in commands)
    def restarted(commands):
        return any('restart' in command for command in commands)
    def patched(commands):
        return any('patch' in command for command in commands)
    # Already staged: nothing is published, patched or restarted.
    calls.write_text('')
    result = run('--context','scratch-only','--apply',EXISTING_CA=ca.read_text(),MOUNTED='1')
    assert result.returncode == 0, result.stderr
    assert 'not restarted' in result.stdout
    assert not published(logged()) and not patched(logged()) and not restarted(logged())
    # Same bundle but the mount is missing: patch and restart, no republish.
    calls.write_text('')
    assert run('--context','scratch-only','--apply',EXISTING_CA=ca.read_text()).returncode == 0
    assert not published(logged()) and patched(logged()) and restarted(logged())
    # No connector yet (it is applied later in the deploy): publish only.
    calls.write_text('')
    result = run('--context','scratch-only','--apply',NO_CONNECTOR='1')
    assert result.returncode == 0, result.stderr
    assert published(logged()) and not patched(logged()) and not restarted(logged())

    def fingerprint(path):
        """openssl's SHA-256 fingerprint line for a certificate file."""
        return subprocess.run(['openssl', 'x509', '-in', str(path), '-noout', '-fingerprint', '-sha256'],
                              check=True, capture_output=True, text=True).stdout.strip()
    def make_root(path, key, subject, days='365', new_key=False):
        """Self-sign a synthetic CA root; reuse key unless new_key."""
        key_args = (['-newkey', 'ec', '-pkeyopt', 'ec_paramgen_curve:P-256', '-nodes', '-keyout', str(key)]
                    if new_key else ['-key', str(key)])
        subprocess.run(['openssl', 'req', '-x509', *key_args, '-out', str(path), '-days', days,
                        '-subj', subject, '-addext', 'basicConstraints=critical,CA:TRUE'],
                       check=True, capture_output=True)
    # A renewal of the published root (same subject and key, new certificate) replaces it.
    published_root = work / 'published.crt'
    published_root.write_text(ca.read_text())
    make_root(ca, work / 'test.key', '/CN=offline-test-ca', days='400')
    assert ca.read_text() != published_root.read_text()
    calls.write_text('')
    result = run('--context','scratch-only','--apply',EXISTING_CA=published_root.read_text(),MOUNTED='1')
    assert result.returncode == 0, result.stderr
    assert 'renewed' in result.stdout and published(logged()) and restarted(logged())
    # The replaced root is not listed as another root: it is replaced, not kept.
    assert 'other than the current one' not in result.stdout, result.stdout
    # Anything else is refused before publishing or patching.
    other_key = work / 'other.crt'
    make_root(other_key, work / 'other.key', '/CN=offline-test-ca', new_key=True)
    other_subject = work / 'other-subject.crt'
    make_root(other_subject, work / 'test.key', '/CN=another-test-ca')
    for existing in ['different-existing-public-root', other_key.read_text(), other_subject.read_text(),
                     published_root.read_text() + other_key.read_text()]:
        calls.write_text('')
        result = run('--context','scratch-only','--apply',EXISTING_CA=existing,MOUNTED='1')
        assert result.returncode != 0 and 'two-root rotation' in result.stderr, result
        assert not published(logged()) and not patched(logged()) and not restarted(logged())
    # A two-root bundle that still holds the current root (a rotation in progress) is
    # left as it is, in either order, so deploys go on during the rotation.
    for existing in [ca.read_text() + other_key.read_text(), other_key.read_text() + ca.read_text()]:
        calls.write_text('')
        result = run('--context','scratch-only','--apply',EXISTING_CA=existing,MOUNTED='1')
        assert result.returncode == 0, result.stderr
        assert 'two-root rotation' in result.stdout, result.stdout
        assert not published(logged()) and not patched(logged()) and not restarted(logged())
        # Every root other than the current one is listed, so a retired one is noticed.
        listed = result.stdout.split('roots other than the current one:', 1)[1]
        assert fingerprint(other_key) in listed and fingerprint(ca) not in listed, result.stdout
        assert 'expires:     notAfter=' in listed and 'subject:     subject=CN=offline-test-ca' in listed
    # Only the current root published: nothing is listed.
    calls.write_text('')
    result = run('--context','scratch-only','--apply',EXISTING_CA=ca.read_text(),MOUNTED='1')
    assert result.returncode == 0 and 'other than the current one' not in result.stdout, result.stdout
    # The issued root must itself be a self-signed CA: a certificate with the published
    # root's subject and key, but cross-signed by another CA (fails the self-verify),
    # or self-signed without CA:TRUE (fails the CA check), is never published, even
    # though its subject and key match a published root.
    csr = work / 'same.csr'
    subprocess.run(['openssl', 'req', '-new', '-key', str(work / 'test.key'), '-subj', '/CN=offline-test-ca',
                    '-out', str(csr)], check=True, capture_output=True)
    extensions = work / 'ca.ext'
    extensions.write_text('basicConstraints=critical,CA:TRUE\n')
    cross_signed = work / 'cross-signed.crt'
    subprocess.run(['openssl', 'x509', '-req', '-in', str(csr), '-CA', str(other_key),
                    '-CAkey', str(work / 'other.key'),
                    '-CAcreateserial', '-days', '365', '-extfile', str(extensions), '-out', str(cross_signed)],
                   check=True, capture_output=True)
    not_ca = work / 'not-ca.crt'
    subprocess.run(['openssl', 'req', '-x509', '-key', str(work / 'test.key'), '-out', str(not_ca),
                    '-days', '365', '-subj', '/CN=offline-test-ca',
                    '-addext', 'basicConstraints=critical,CA:FALSE'], check=True, capture_output=True)
    for issued in [cross_signed, not_ca]:
        calls.write_text('')
        result = run('--context','scratch-only','--apply',EXISTING_CA=ca.read_text(),MOUNTED='1',CA=str(issued))
        assert result.returncode != 0, (issued.name, result.stdout)
        assert not published(logged()) and not patched(logged()) and not restarted(logged()), issued.name
    # Near-expiry roots must not be published as a new trust bundle.
    subprocess.run(['openssl','req','-x509','-newkey','ec','-pkeyopt','ec_paramgen_curve:P-256',
                    '-nodes','-keyout',str(work/'short.key'),'-out',str(ca),'-days','30',
                    '-subj','/CN=short-test-ca','-addext','basicConstraints=critical,CA:TRUE'],
                   check=True,capture_output=True)
    calls.write_text('')
    assert run('--context','scratch-only','--apply').returncode != 0
    assert not any('patch' in json.loads(line) for line in calls.read_text().splitlines())

    # --retire-other-roots: keep only the root the gateway certificate is issued from.
    new_root = work / 'new-root.crt'
    make_root(new_root, work / 'new-root.key', '/CN=offline-new-ca', new_key=True)
    def leaf_from(signer_crt, signer_key, name):
        """A gateway certificate (leaf plus its CA, as cert-manager writes tls.crt)."""
        leaf = work / f'{name}.crt'
        subprocess.run(['openssl', 'req', '-new', '-newkey', 'ec', '-pkeyopt', 'ec_paramgen_curve:P-256',
                        '-nodes', '-keyout', str(work / f'{name}.key'), '-subj', '/CN=econgraph.com',
                        '-out', str(work / f'{name}.csr')], check=True, capture_output=True)
        subprocess.run(['openssl', 'x509', '-req', '-in', str(work / f'{name}.csr'), '-CA', str(signer_crt),
                        '-CAkey', str(signer_key), '-CAcreateserial', '-days', '90', '-out', str(leaf)],
                       check=True, capture_output=True)
        leaf.write_text(leaf.read_text() + signer_crt.read_text())
        return leaf
    good_leaf = leaf_from(new_root, work / 'new-root.key', 'good-leaf')
    stray_leaf = leaf_from(other_key, work / 'other.key', 'stray-leaf')
    empty = work / 'empty.crt'
    empty.write_text('')
    two_roots = work / 'two-roots.crt'
    two_roots.write_text(new_root.read_text() + other_key.read_text())
    published_file = work / 'published-bundle.crt'
    rotation = ca.read_text() + new_root.read_text()
    retire_env = dict(RET_ISSUER='econ-graph-origin-ca-2', RET_SECRET='econ-graph-origin-ca-2',
                      RET_ROOT=str(new_root), RET_LEAF=str(good_leaf), EXISTING_CA=rotation,
                      PUBLISHED=str(published_file), MOUNTED='1')
    def retire(*args, **updates):
        calls.write_text('')
        if published_file.exists():
            published_file.unlink()
        return run('--context', 'scratch-only', '--retire-other-roots', *args, **dict(retire_env, **updates))
    # Dry run: the checks and the roots it would drop; nothing written.
    result = retire()
    assert result.returncode == 0, result.stderr
    dropped = result.stdout.split('Roots to drop from the published trust bundle:', 1)[1]
    assert fingerprint(ca) in dropped and fingerprint(new_root) not in dropped, result.stdout
    assert 'Dry run' in result.stdout
    assert not published(logged()) and not restarted(logged()) and not published_file.exists()
    # Apply: exactly the new root is published, then cloudflared restarts and is awaited.
    result = retire('--apply')
    assert result.returncode == 0, result.stderr
    assert published_file.read_text() == new_root.read_text()
    commands = logged()
    publish_at = next(i for i, command in enumerate(commands) if command[-3:] == ['apply', '-f', '-'])
    restart_at = next(i for i, command in enumerate(commands) if 'restart' in command)
    status_at = next(i for i, command in enumerate(commands) if 'status' in command)
    assert publish_at < restart_at < status_at
    assert not patched(commands) and not any('delete' in command for command in commands)
    # Only the new root published already: nothing to retire, nothing restarted.
    result = retire('--apply', EXISTING_CA=new_root.read_text())
    assert result.returncode == 0 and 'nothing to retire' in result.stdout, result
    assert not published(logged()) and not restarted(logged())
    # Each refusal changes nothing, in a dry run and with --apply.
    for message, updates in [
            ('has no public certificate', dict(RET_ROOT=str(empty))),
            ('exactly one self-signed CA', dict(RET_ROOT=str(two_roots))),
            ('exactly one self-signed CA', dict(RET_ROOT=str(cross_signed))),
            ('exactly one self-signed CA', dict(RET_ROOT=str(not_ca))),
            ('does not verify against the root', dict(RET_LEAF=str(stray_leaf))),
            ('has no certificate', dict(RET_LEAF=str(empty))),
            ('does not contain the root', dict(EXISTING_CA=ca.read_text())),
            ('not issued by a ClusterIssuer', dict(RET_KIND='Issuer')),
            ('not a CA issuer', dict(RET_SECRET='')),
            ('could not read the public certificate',
             dict(RET_SECRET='mistyped-secret-name', RET_SECRET_PRESENT='econ-graph-origin-ca-2'))]:
        for args in [(), ('--apply',)]:
            result = retire(*args, **updates)
            assert result.returncode != 0 and message in result.stderr, (message, args, result)
            assert 'nothing was changed' in result.stderr
            assert not published(logged()) and not restarted(logged()) and not published_file.exists(), message

# apply-gateway-routes.sh, in a copy of the files it uses where staging is a stub.
with tempfile.TemporaryDirectory() as folder:
    tree = Path(folder) / 'root'
    log = Path(folder) / 'log'
    deploy = tree / 'scripts/deploy'
    (deploy / 'lib').mkdir(parents=True)
    for name in ['apply-gateway-routes.sh', 'render-public-host.py', 'lib/cluster-role.sh',
                 'lib/deploy-config.sh', 'lib/public-host.sh']:
        shutil.copy2(root / 'scripts/deploy' / name, deploy / name)
    shutil.copytree(root / 'k8s/manifests/gateway', tree / 'k8s/manifests/gateway')
    stage = deploy / 'stage-tunnel-origin-ca.sh'
    stage.write_text('#!/usr/bin/env bash\necho "stage $*" >>"$LOG"\nexit "${STAGE_RC:-0}"\n')
    stage.chmod(0o755)
    home = Path(folder) / 'home'
    config_dir = Path(folder) / 'config'
    config_dir.mkdir()
    deploy_env = config_dir / 'deploy.env'
    kubectl = Path(folder) / 'kubectl'
    kubectl.write_text('''#!/usr/bin/env bash
echo "kubectl $*" >>"$LOG"
case "$*" in
    *"config current-context"*) echo kind-current ;;
    *"get crd certificates.cert-manager.io"*) echo customresourcedefinition/certificates.cert-manager.io ;;
    *"get certificate econ-graph-tls"*) printf '%s' "${CURRENT_ISSUER:-}" ;;
    *"get httproute econ-graph-frontend-public"*) printf '%s' "${CURRENT_MAIN_HOST:-}" ;;
    *"get httproute -l"*) printf '%s' "${CURRENT_MAIN_HOST:-}" ;;
    *--field-manager*)
        all="$*"; file="${all##* -f }"
        echo "issuer $(sed -n '/issuerRef:/,/name:/s/^ *name: //p' "$file")" >>"$LOG" ;;
esac
exit 0
''')
    kubectl.chmod(0o755)
    def routes(*args, **updates):
        """Run the copied apply-gateway-routes.sh against the stub kubectl."""
        log.write_text('')
        environment = dict(os.environ, KUBECTL=str(kubectl), LOG=str(log), PUBLIC_HOST='econgraph.com',
                           HOME=str(home), ECONGRAPH_CONFIG_DIR=str(config_dir))
        for name in ['GATEWAY_TLS_ISSUER', 'ECONGRAPH_DEPLOY_ENV', 'XDG_CONFIG_HOME', 'MAIN_HOST']:
            environment.pop(name, None)
        environment.update({key: value for key, value in updates.items() if value is not None})
        for key, value in updates.items():
            if value is None:
                environment.pop(key, None)
        result = subprocess.run([str(deploy / 'apply-gateway-routes.sh'), *args], env=environment,
                                text=True, capture_output=True)
        return result, log.read_text().splitlines()
    def https_apply(lines):
        return next(i for i, line in enumerate(lines) if '--field-manager=econ-graph-gateway-https' in line)
    origin = {'GATEWAY_TLS_ISSUER': 'econ-graph-origin-ca'}
    result, lines = routes('--context', 'kind-test', **origin)
    assert result.returncode == 0, result.stderr
    assert all(line.startswith('kubectl --context kind-test ') for line in lines
               if line.startswith('kubectl ')), lines
    assert 'Public host www.econgraph.com (base econgraph.com)' in result.stdout
    # Explicit rollback and bare maintenance apply do not change canonical host.
    rollback, rollback_lines = routes('--context', 'kind-test', MAIN_HOST='econgraph.com')
    assert rollback.returncode == 0 and 'Public host econgraph.com (base econgraph.com)' in rollback.stdout
    assert all(line.startswith('kubectl --context kind-test ') for line in rollback_lines
               if line.startswith('kubectl ')), rollback_lines
    for canonical in ['econgraph.com', 'www.econgraph.com']:
        kept, kept_lines = routes('--context', 'kind-test', PUBLIC_HOST=None, CURRENT_MAIN_HOST=canonical)
        assert kept.returncode == 0 and f'Public host {canonical} (base econgraph.com)' in kept.stdout, kept
        assert all(line.startswith('kubectl --context kind-test ') for line in kept_lines
                   if line.startswith('kubectl ')), kept_lines
    stage_line = lines.index('stage --context kind-test --apply')
    assert stage_line < https_apply(lines), 'staging must precede the certificate'
    # The retained issuer of a later deploy stages too (idempotent).
    result, lines = routes('--context', 'kind-test', CURRENT_ISSUER='econ-graph-origin-ca')
    assert result.returncode == 0 and 'stage --context kind-test --apply' in lines
    # Without --context, staging gets the current context explicitly.
    result, lines = routes(**origin)
    assert result.returncode == 0 and 'stage --context kind-current --apply' in lines, lines
    # Dry run prints the staging command and runs nothing.
    result, lines = routes('--dry-run', '--context', 'kind-test', **origin)
    assert result.returncode == 0, result.stderr
    assert '+ scripts/deploy/stage-tunnel-origin-ca.sh --context kind-test --apply' in result.stdout
    assert not any(line.startswith('stage') for line in lines)
    assert not any(' apply ' in line + ' ' for line in lines if line.startswith('kubectl'))
    # Failed staging: the certificate is not touched, and the command is named.
    result, lines = routes('--context', 'kind-test', STAGE_RC='1', **origin)
    assert result.returncode != 0 and 'stage-tunnel-origin-ca.sh --context kind-test --apply' in result.stderr
    assert not any('--field-manager' in line for line in lines)
    # Other issuers never stage.
    for updates in [{}, {'GATEWAY_TLS_ISSUER': 'letsencrypt-test'}]:
        result, lines = routes('--context', 'kind-test', **updates)
        assert result.returncode == 0 and not any(line.startswith('stage') for line in lines)

    def chosen(**updates):
        """The issuer the certificate was applied with, and whether staging ran."""
        result, lines = routes('--context', 'kind-test', **updates)
        assert result.returncode == 0, result.stderr
        issuers = [line.split(' ', 1)[1] for line in lines if line.startswith('issuer ')]
        assert len(issuers) == 1, lines
        return issuers[0], 'stage --context kind-test --apply' in lines
    def refused(text, **updates):
        """The run failed, saying `text`, before applying or staging anything."""
        result, lines = routes('--context', 'kind-test', **updates)
        assert result.returncode != 0 and text in result.stderr, result
        assert not any(line.startswith(('stage', 'issuer')) or ' apply ' in line for line in lines), lines
    # No config: the current issuer, else self-signed.
    assert chosen() == ('econ-graph-selfsigned', False)
    assert chosen(CURRENT_ISSUER='letsencrypt-test') == ('letsencrypt-test', False)
    assert chosen(CURRENT_ISSUER='econ-graph-origin-ca') == ('econ-graph-origin-ca', True)
    # After a recreate (no certificate), the deploy config keeps the CA issuer and stages it.
    deploy_env.write_text('PUBLIC_HOST=econgraph.com\nGATEWAY_TLS_ISSUER=econ-graph-origin-ca\n')
    assert chosen() == ('econ-graph-origin-ca', True)
    assert chosen(CURRENT_ISSUER='econ-graph-origin-ca') == ('econ-graph-origin-ca', True)
    # A config that differs from the live certificate's issuer never changes it implicitly.
    refused('never changes the issuer implicitly', CURRENT_ISSUER='econ-graph-selfsigned')
    deploy_env.write_text('GATEWAY_TLS_ISSUER=econ-graph-selfsigned\n')
    refused('never changes the issuer implicitly', CURRENT_ISSUER='econ-graph-origin-ca')
    # The environment is an explicit choice, which may change it.
    assert chosen(CURRENT_ISSUER='econ-graph-origin-ca',
                  GATEWAY_TLS_ISSUER='econ-graph-selfsigned') == ('econ-graph-selfsigned', False)
    deploy_env.write_text('GATEWAY_TLS_ISSUER=econ-graph-origin-ca\n')
    assert chosen(CURRENT_ISSUER='econ-graph-selfsigned',
                  GATEWAY_TLS_ISSUER='econ-graph-origin-ca') == ('econ-graph-origin-ca', True)
    # An empty environment value falls through to the config.
    assert chosen(GATEWAY_TLS_ISSUER='') == ('econ-graph-origin-ca', True)
    # The last line wins; quotes are removed.
    deploy_env.write_text('GATEWAY_TLS_ISSUER=econ-graph-origin-ca\nGATEWAY_TLS_ISSUER="letsencrypt-test"\n')
    assert chosen() == ('letsencrypt-test', False)
    # A malformed, exported or unreadable config stops before anything is applied ...
    for text, message in [('GATEWAY_TLS_ISSUER=econ-graph-origin-ca # verified\n', 'with blanks in it'),
                          ('GATEWAY_TLS_ISSUER=econ-graph-origin-ca\r\n', 'carriage return'),
                          ('export GATEWAY_TLS_ISSUER=econ-graph-origin-ca\n', 'without export'),
                          ('GATEWAY_TLS_ISSUER=Bad_Issuer\n', 'not a resource name')]:
        deploy_env.write_text(text)
        refused(message)
        # ... but only with a public host: without one the line is not read.
        result, _ = routes('--context', 'kind-test', PUBLIC_HOST='none')
        assert result.returncode == 0, (text, result.stderr)
    deploy_env.write_text('GATEWAY_TLS_ISSUER=econ-graph-origin-ca\n')
    deploy_env.chmod(0)
    if not os.access(deploy_env, os.R_OK):
        refused('is not a readable file')
    deploy_env.chmod(0o600)
    deploy_env.unlink()
    assert chosen() == ('econ-graph-selfsigned', False)
    assert not home.exists(), 'the scratch HOME must not be written'
print('PASS: offline tunnel origin CA staging checks')
PY
