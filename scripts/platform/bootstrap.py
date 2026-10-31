#!/usr/bin/env python3
"""Plan a scoped Talos CNI/Flux bootstrap; --execute requires explicit owned scratch cluster."""
import argparse
import hashlib
import json
import re
from pathlib import Path
import subprocess
import sys
import tempfile
import yaml
from safety import Kubernetes, identifier

ROOT = Path(__file__).resolve().parents[2]


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('phase', choices=['cni', 'flux', 'reconcile'])
    p.add_argument('--context', required=True)
    p.add_argument('--kubeconfig', type=Path, required=True)
    p.add_argument('--owner', required=True)
    p.add_argument('--control-node', required=True, help='node already labelled econgraph.io/drill-owner=OWNER')
    p.add_argument('--cache', type=Path, required=True, help='verified artifacts populated by validate.py')
    p.add_argument('--settings', type=Path)
    p.add_argument('--git-ref', help='reviewed remote feature ref; never pushes or changes Git')
    p.add_argument('--helm', default='helm')
    p.add_argument('--kubectl', default='kubectl')
    p.add_argument('--execute', action='store_true')
    args = p.parse_args()
    identifier(args.owner)
    lock = json.loads((ROOT / 'platform/versions.lock.json').read_text())
    result = {'mode': 'execute' if args.execute else 'dry-run', 'phase': args.phase,
              'context': args.context, 'owner': args.owner,
              'will_write_git': False, 'will_delete_resources': False}
    if not args.execute:
        print(json.dumps(result, indent=2))
        return
    kube = Kubernetes(args.kubectl, args.kubeconfig, args.context)
    kube.owned('node', args.control_node, args.owner)
    nodes = json.loads(kube.run('get', 'nodes', '-o', 'json'))['items']
    if not nodes or any(n.get('metadata', {}).get('labels', {}).get('econgraph.io/drill-owner') != args.owner for n in nodes):
        raise ValueError('every node must have exact scratch owner label before cluster-wide changes')
    if args.phase == 'cni':
        entry = lock['charts']['cilium']
        chart = args.cache / 'charts' / entry['package']
        if hashlib.sha256(chart.read_bytes()).hexdigest() != entry['sha256']:
            raise ValueError('Cilium package checksum mismatch; run validate.py')
        release = yaml.safe_load((ROOT / 'platform/addons/cilium/release.yaml').read_text())
        if kube.run('get', 'daemonset', 'cilium', '-n', 'kube-system', '--ignore-not-found', '-o', 'name').strip():
            raise ValueError('existing Cilium found; bootstrap never upgrades or adopts an existing installation')
        with tempfile.NamedTemporaryFile(mode='w') as values:
            values.write(yaml.safe_dump(release['spec']['values']))
            values.flush()
            subprocess.run([args.helm, 'install', release['spec']['releaseName'], str(chart),
                            '--namespace', 'kube-system', '--kubeconfig', str(args.kubeconfig),
                            '--kube-context', args.context, '--values', values.name, '--wait', '--timeout', '10m'], check=True)
    elif args.phase == 'flux':
        entry = lock['assets']['flux-install']
        manifest = args.cache / entry['file']
        if hashlib.sha256(manifest.read_bytes()).hexdigest() != entry['sha256']:
            raise ValueError('Flux install checksum mismatch; run validate.py')
        if kube.run('get', 'deployment', 'source-controller', '-n', 'flux-system', '--ignore-not-found', '-o', 'name').strip():
            raise ValueError('existing Flux found; bootstrap never replaces an existing installation')
        kube.run('apply', '-f', str(manifest))
    else:
        if not args.settings or not args.git_ref or args.git_ref in ('main', 'master'):
            raise ValueError('reconcile requires reviewed feature --git-ref and real --settings')
        settings = yaml.safe_load(args.settings.read_text())
        if settings.get('kind') != 'ConfigMap' or settings.get('metadata', {}).get('name') != 'platform-settings':
            raise ValueError('settings must be the platform-settings ConfigMap')
        data = settings['data']
        serialized = json.dumps(data)
        if any(x in serialized for x in ['example.invalid', '192.0.2.', 'localhost', '${']):
            raise ValueError('replace documentation/example settings before reconcile')
        for key in ['BACKEND_IMAGE', 'FRONTEND_IMAGE', 'CRAWLER_IMAGE']:
            if not re.fullmatch(r'[^\s]+@sha256:[0-9a-f]{64}', data.get(key, '')):
                raise ValueError('reviewed feature image digest required: ' + key)
        # Keys are created separately from protected files. Never read/export their values.
        for secret in ['sops-age', 'platform-git-auth']:
            kube.run('get', 'secret', secret, '-n', 'flux-system', '-o', 'name')
        rendered_secrets = subprocess.run([args.kubectl, 'kustomize', str(ROOT / 'platform/secrets')], check=True, capture_output=True, text=True).stdout
        secret_docs = [d for d in yaml.safe_load_all(rendered_secrets) if d]
        required = {('econ-graph', n) for n in ['econ-graph-app-credentials', 'econ-graph-data-credentials', 'econ-graph-keycloak-credentials', 'econ-graph-secrets', 'econ-graph-keycloak', 'app-backup-s3', 'data-backup-s3', 'keycloak-backup-s3']} | {('monitoring', n) for n in ['grafana-admin', 'loki-objectstore']}
        actual = set()
        for document in secret_docs:
            if document.get('kind') != 'Secret' or not document.get('sops'):
                raise ValueError('secret package must contain only SOPS-encrypted Secrets')
            values = {**document.get('data', {}), **document.get('stringData', {})}
            if not values or not all(isinstance(v, str) and v.startswith('ENC[') for v in values.values()):
                raise ValueError('secret values must all be encrypted')
            actual.add((document['metadata'].get('namespace'), document['metadata']['name']))
        if not required <= actual:
            raise ValueError('rendered secret package lacks required encrypted deployment inputs')
        # Flux reads Git, never this filesystem. Pin the reviewed remote-tracking commit
        # and ensure the local encrypted package exactly matches its committed bytes.
        reference = 'refs/remotes/origin/' + args.git_ref
        commit = subprocess.run(['git', 'rev-parse', '--verify', reference + '^{commit}'], cwd=ROOT, check=True, capture_output=True, text=True).stdout.strip()
        subprocess.run(['git', 'diff', '--quiet', commit, '--', 'platform/secrets'], cwd=ROOT, check=True)
        untracked = subprocess.run(['git', 'ls-files', '--others', '--exclude-standard', 'platform/secrets'], cwd=ROOT, check=True, capture_output=True, text=True).stdout
        if untracked.strip():
            raise ValueError('encrypted package must be committed and present on reviewed remote feature ref')
        settings['metadata']['namespace'] = 'flux-system'
        kube.apply(settings)
        rendered = subprocess.run([args.kubectl, 'kustomize', str(ROOT / 'platform/clusters/threadripper')], check=True, capture_output=True, text=True).stdout
        documents = list(yaml.safe_load_all(rendered))
        for document in documents:
            if document['kind'] == 'GitRepository':
                document['spec']['ref'] = {'commit': commit}
            kube.apply(document)
    result['status'] = 'requested'
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    try:
        main()
    except (ValueError, RuntimeError, OSError, subprocess.SubprocessError) as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
