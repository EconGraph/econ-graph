#!/usr/bin/env python3
"""Deploy only disposable branch namespaces; never rebuild the shared cluster."""
import argparse
from contextlib import contextmanager
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import secrets
import subprocess
import sys

import yaml

spec = importlib.util.spec_from_file_location('branch_renderer', Path(__file__).with_name('render-branch.py'))
renderer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(renderer)


def default_kubectl(environ=os.environ):
    """The kubectl to run, as cluster_role_default_kubectl in lib/cluster-role.sh chooses it.

    Never plain kubectl from PATH: on the production host that is the MicroK8s wrapper,
    which ignores KUBECONFIG. KUBECTL wins; else, with KUBECONFIG set, the kind cluster's
    kubectl kept in ~/.cache/econgraph runs with it; else that kubectl and its kubeconfig
    together. Anything else fails closed. A chosen KUBECONFIG is exported in environ.
    """
    directory = Path(environ.get('XDG_CACHE_HOME') or Path(environ.get('HOME', '~')).expanduser()/'.cache')/'econgraph'
    binary, config = directory/'bin/kubectl', directory/'kubeconfig-econ-graph'
    fix = ('reach the kind cluster once with a script that keeps its kubectl there (any kind-env.sh script '
           'run for real keeps them, e.g. scripts/deploy/precheck-db-migration.sh, which only reads the cluster '
           'without --stop-crawler; a --dry-run keeps nothing; scripts/deploy/recreate-kind-cluster.sh '
           '--if-missing creates the cluster), or set KUBECTL (and KUBECONFIG) to choose the cluster explicitly')
    executable = os.access(binary, os.X_OK)
    if environ.get('KUBECTL'):
        return environ['KUBECTL']
    if environ.get('KUBECONFIG'):
        if not executable:
            raise ValueError(f"KUBECONFIG is set ({environ['KUBECONFIG']}) but KUBECTL is not, and there is no kind "
                             f'kubectl at {binary}; plain kubectl may ignore KUBECONFIG (the MicroK8s wrapper does). '
                             f'Set KUBECTL too, or {fix}')
        print(f"Using the kind cluster's kubectl {binary} with KUBECONFIG={environ['KUBECONFIG']} "
              '(from the environment; set KUBECTL to choose another kubectl)', file=sys.stderr)
        return str(binary)
    has_config = config.exists() and config.stat().st_size > 0
    if executable and has_config:
        environ['KUBECONFIG'] = str(config)
        print(f"Using the kind cluster's kubectl {binary} and KUBECONFIG={config} "
              '(set KUBECTL and KUBECONFIG to choose another cluster)', file=sys.stderr)
        return str(binary)
    missing = config if executable else binary if has_config else f'{binary} and {config}'
    raise ValueError(f"no KUBECTL is set and the kind cluster's kubectl is not kept here (no {missing}); "
                     f'plain kubectl may be another cluster\'s. {fix[0].upper()}{fix[1:]}')


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('action', choices=['up', 'down', 'list', 'build-frontend'])
    p.add_argument('branch', nargs='?')
    p.add_argument('--context', required=True, help='explicit context; current-context is never used')
    p.add_argument('--dry-run', action='store_true', help='print plan; contact no cluster and read no credentials')
    p.add_argument('--local-data', action='store_true')
    p.add_argument('--backend-image')
    p.add_argument('--frontend-image')
    p.add_argument('--data-image', help='tagged release crawler image, required for local data')
    p.add_argument('--seed-dir', type=Path, help='verified primary dump bundle; required for local data')
    args = p.parse_args()
    if args.action != 'list' and not args.branch:
        p.error('branch is required')
    if args.action == 'up' and (not args.backend_image or not args.frontend_image):
        p.error('up requires --backend-image and --frontend-image (built for this branch hostname)')
    if args.local_data and (not args.data_image or not args.seed_dir):
        p.error('--local-data requires --data-image and --seed-dir')
    if args.local_data and not re.search(r':v\d+\.\d+\.\d+(?:[-.][A-Za-z0-9.-]+)?(?:@sha256:[0-9a-f]{64})?$', args.data_image):
        p.error('--data-image must use a release tag, e.g. econ-graph-crawler-worker:v4.1.0')
    base = []
    def kubectl():
        # Chosen on first use: a dry run and build-frontend contact no cluster.
        if not base:
            base.extend([default_kubectl(), '--context', args.context])
        return base
    args.kubectl = kubectl
    def k(*cmd, data=None, capture=False, check=True):
        return subprocess.run(kubectl() + list(cmd), input=data, text=True, check=check,
                              stdout=subprocess.PIPE if capture else None)
    def apply(docs):
        k('apply', '-f', '-', data=yaml.safe_dump_all(docs, sort_keys=False))
    def lookup(kind, name, namespace=None):
        cmd = (['-n', namespace] if namespace else []) + ['get', kind, name, '--ignore-not-found', '-o', 'json']
        result = k(*cmd, capture=True)
        return json.loads(result.stdout) if result.stdout.strip() else None
    @contextmanager
    def lock(namespace):
        # Atomic create is a cluster-wide lock, including invocations on other
        # hosts or with kubeconfig aliases. Never steal an existing lock.
        obj = {'apiVersion': 'v1', 'kind': 'ConfigMap',
               'metadata': {'name': 'branch-deploy-lock', 'namespace': namespace},
               'data': {'owner': secrets.token_hex(16)}}
        k('create', '-f', '-', data=yaml.safe_dump(obj))
        try:
            yield
        finally:
            k('-n', namespace, 'delete', 'configmap', 'branch-deploy-lock', '--ignore-not-found', check=False)
    if args.action == 'list':
        if args.dry_run:
            print('List namespaces labelled econgraph.io/env=branch')
        else:
            k('get', 'namespaces', '-l', 'econgraph.io/env=branch',
              '-L', 'econgraph.io/branch,econgraph.io/data-mode')
        return
    slug, namespace = renderer.identity(args.branch)
    origin = 'http://' + slug + '.localhost'
    if args.action == 'build-frontend':
        cmd = ['docker', 'build', '-t', args.frontend_image or 'econ-graph-frontend:branch-' + slug,
               '--build-arg', 'VITE_OIDC_ISSUER=' + origin + '/idp/realms/econ-graph',
               '--build-arg', 'VITE_OIDC_CLIENT_ID=econ-graph-web',
               '--build-arg', 'VITE_GRAPHQL_URL=/graphql', '--build-arg', 'VITE_DATA_GRAPHQL_URL=/data/graphql',
               '--build-arg', 'FLAGS_PROFILE=release', str(renderer.ROOT/'frontend')]
        # Build on the daemon the kind cluster runs on, as lib/kind-env.sh does: the
        # operator's DOCKER_HOST is a rootless daemon, and `kind load` from the rootful one
        # would otherwise load a stale image under the same tag.
        docker_host = os.environ.get('ECONGRAPH_DOCKER_HOST') or 'unix:///var/run/docker.sock'
        if args.dry_run:
            print('DOCKER_HOST=' + docker_host + ' ' + ' '.join(cmd))
        else:
            subprocess.run(cmd, check=True, env=dict(os.environ, DOCKER_HOST=docker_host))
        return
    if args.dry_run:
        print(f'{args.action}: namespace={namespace} host={origin} context={args.context}')
        if args.action == 'up':
            docs = renderer.render(args.branch, args.backend_image, args.frontend_image, args.local_data, args.data_image)
            print(f'Probe NetworkPolicy enforcement before credentials/workloads; apply {len(docs)} branch resources only if it passes.')
            print('Local data: verify/seed dump, migrate, then start worker.' if args.local_data else
                  f'Create only ReferenceGrant data-graphql-from-{slug} in econ-graph; no primary database credentials.')
        else:
            print('Require matching branch ownership labels and typed namespace confirmation; delete only namespace and its ReferenceGrant.')
        return
    existing = lookup('namespace', namespace)
    if existing:
        meta = existing['metadata']
        labels = meta.get('labels', {})
        if labels.get('econgraph.io/env') != 'branch' or labels.get('econgraph.io/branch') != slug or meta.get('annotations', {}).get('econgraph.io/branch-name') != args.branch:
            raise ValueError('namespace ownership does not match; refusing changes')
        if meta.get('deletionTimestamp'):
            raise ValueError('namespace is terminating; wait before redeploying')
        if args.action == 'up' and labels.get('econgraph.io/data-mode') != ('local' if args.local_data else 'primary'):
            raise ValueError('data mode cannot change in place; remove the disposable branch first')
    if args.action == 'down':
        if not existing:
            print('Branch namespace absent; no resources deleted.')
            return
        if not sys.stdin.isatty() or input(f'Type {namespace} to delete its disposable users and databases: ') != namespace:
            raise ValueError('typed namespace confirmation required')
        with lock(namespace):
            ref = lookup('referencegrant', 'data-graphql-from-' + slug, 'econ-graph')
            if ref and ref.get('metadata', {}).get('labels', {}).get('econgraph.io/branch') != slug:
                raise ValueError('ReferenceGrant ownership differs; refusing deletion')
            if ref:
                k('-n', 'econ-graph', 'delete', 'referencegrant', 'data-graphql-from-' + slug)
            k('delete', 'namespace', namespace)
        return
    # Confirm gateway dependency exists before creating disposable resources.
    k('-n', 'econ-graph-gateway', 'get', 'gateway', 'econ-graph-gateway', capture=True)
    docs = renderer.render(args.branch, args.backend_image, args.frontend_image, args.local_data, args.data_image)
    apply([docs[0]])
    with lock(namespace):
        apply([d for d in docs if d['kind'] == 'NetworkPolicy'])
        network_spec = importlib.util.spec_from_file_location('branch_network_check', Path(__file__).with_name('branch-network-check.py'))
        network = importlib.util.module_from_spec(network_spec)
        network_spec.loader.exec_module(network)
        network.verify(namespace, args.local_data, k, lookup, apply)
        def secret(name, values):
            # Read only existence, never an existing secret's contents. Server-side
            # creation omits last-applied annotations containing plaintext credentials.
            result = k('-n', namespace, 'get', 'secret', name, '--ignore-not-found', '-o', 'name', capture=True)
            if not result.stdout.strip():
                obj = {'apiVersion': 'v1', 'kind': 'Secret', 'metadata': {'name': name, 'namespace': namespace},
                       'type': 'Opaque', 'stringData': values}
                k('create', '-f', '-', data=yaml.safe_dump(obj))
        # If app secrets exist, they must ALL exist. Re-generating a subset would
        # make existing databases' passwords and connection URLs diverge.
        names = ['econ-graph-app-postgres', 'econ-graph-keycloak', 'econ-graph-secrets']
        if args.local_data:
            names.append('econ-graph-postgres')
        present = [bool(k('-n', namespace, 'get', 'secret', n, '--ignore-not-found', '-o', 'name', capture=True).stdout.strip()) for n in names]
        if any(present) and not all(present):
            raise ValueError('incomplete branch credentials; restore missing Secrets or recreate this disposable namespace')
        if not any(present):
            app, app_admin, kc_db, kc_admin, kc_anonymous, data, data_admin = [secrets.token_hex(24) for _ in range(7)]
            secret('econ-graph-app-postgres', {'postgres-password': app_admin, 'app-password': app})
            # anonymous-client-secret: the branch backend's anonymous catalog reader in the
            # branch Keycloak (read by Keycloak, the realm import and the backend).
            secret('econ-graph-keycloak', {'admin-username': 'admin', 'admin-password': kc_admin, 'db-password': kc_db,
                                           'anonymous-client-secret': kc_anonymous})
            urls = {'app-database-url': f'postgresql://econgraph:{app}@app-postgres-service:5432/econ_graph_app'}
            if args.local_data:
                secret('econ-graph-postgres', {'postgres-password': data_admin, 'app-password': data})
                urls['data-database-url'] = f'postgresql://econgraph:{data}@data-postgres-service:5432/econ_graph'
            secret('econ-graph-secrets', urls)
        realm = json.loads((renderer.ROOT/'config/keycloak/econ-graph-realm.json').read_text())
        realm['identityProviders'] = []
        docs.append({'apiVersion': 'v1', 'kind': 'ConfigMap',
            'metadata': {'name': 'keycloak-realm', 'namespace': namespace}, 'data': {
                'econ-graph-realm.json': json.dumps(realm),
                'render-realm.sh': (renderer.ROOT/'scripts/keycloak/render-realm.sh').read_text(),
                'preserve-catalog-grants.py': (renderer.ROOT/'scripts/keycloak/preserve-catalog-grants.py').read_text()}})
        early = [d for d in docs[1:] if d['kind'] not in ('Deployment', 'Job')]
        apply(early)
        for statefulset in ['app-postgres', 'keycloak-postgres'] + (['postgresql'] if args.local_data else []):
            k('-n', namespace, 'rollout', 'status', 'statefulset/' + statefulset, '--timeout=300s')
        if args.local_data:
            seed_local(args, namespace, k, lookup, apply)
            migrate = next(d for d in docs if d['kind'] == 'Job' and d['metadata']['name'] == 'crawler-migrate')
            k('-n', namespace, 'delete', 'job', 'crawler-migrate', '--ignore-not-found')
            apply([migrate])
            k('-n', namespace, 'wait', '--for=condition=complete', 'job/crawler-migrate', '--timeout=1800s')
        apply([d for d in docs if d['kind'] == 'Deployment'])
        for deployment in ['keycloak', 'econ-graph-backend', 'econ-graph-frontend'] + (['crawler-worker'] if args.local_data else []):
            k('-n', namespace, 'rollout', 'status', 'deployment/' + deployment, '--timeout=900s')
        k('-n', namespace, 'delete', 'job', 'keycloak-realm-import', '--ignore-not-found')
        apply([d for d in docs if d['kind'] == 'Job' and d['metadata']['name'] == 'keycloak-realm-import'])
        k('-n', namespace, 'wait', '--for=condition=complete', 'job/keycloak-realm-import', '--timeout=900s')
        if not args.local_data:
            ref = renderer.grant(args.branch)
            current = lookup('referencegrant', ref['metadata']['name'], 'econ-graph')
            if current and current.get('metadata', {}).get('labels', {}).get('econgraph.io/branch') != slug:
                raise ValueError('ReferenceGrant already exists with different ownership')
            apply([ref])
        print(f'Branch ready at {origin}; create branch users through Keycloak admin port-forward.')


def seed_local(args, namespace, k, lookup, apply):
    root = args.seed_dir.resolve(strict=True)
    dump = root/'econ_graph.dump'
    if not (root/'VERIFIED').is_file() or not (root/'SHA256SUMS').is_file() or not dump.is_file() or dump.is_symlink():
        raise ValueError('seed requires VERIFIED, SHA256SUMS and a regular econ_graph.dump')
    expected = None
    for line in (root/'SHA256SUMS').read_text().splitlines():
        fields = line.split()
        if len(fields) == 2 and fields[1].lstrip('*') == 'econ_graph.dump':
            expected = fields[0]
    with dump.open('rb') as source:
        digest = hashlib.file_digest(source, 'sha256').hexdigest()
    if expected != digest:
        raise ValueError('primary dump SHA-256 mismatch')
    listing = subprocess.run(['pg_restore', '--list', str(dump)], capture_output=True, text=True, check=True).stdout
    allowed = {'__diesel_schema_migrations'}
    for migration in (renderer.ROOT/'backend/migrations/data').glob('*/up.sql'):
        sql = re.sub(r'/\*.*?\*/|--[^\n]*', '', migration.read_text(), flags=re.DOTALL)
        allowed.update(re.findall(r'(?mi)^\s*CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?(?:public\.)?"?([a-z_]\w*)', sql))
    archived = set(re.findall(r'(?m)^\d+;\s+\d+\s+\d+\s+TABLE(?:\s+DATA)?\s+(\S+)\s+(\S+)\s', listing))
    if not archived or any(schema != 'public' or table not in allowed for schema, table in archived):
        raise ValueError('seed contains app/unknown tables; use a converted data-plane dump matching this checkout')
    marker = lookup('configmap', 'branch-data-seed', namespace)
    if marker:
        if marker.get('data', {}).get('sha256') != digest:
            raise ValueError('branch already seeded from a different dump; refusing overwrite')
        return
    result = k('-n', namespace, 'exec', 'postgresql-0', '--', 'psql', '-U', 'postgres', '-d', 'econ_graph',
               '-Atc', "SELECT count(*) FROM pg_tables WHERE schemaname='public'", capture=True)
    if result.stdout.strip() != '0':
        raise ValueError('unmarked local data database is nonempty; refusing overwrite')
    # Stream only into the explicitly owned branch pod. No primary connection
    # string or credentials are ever requested or copied.
    with dump.open('rb') as src:
        subprocess.run(args.kubectl() + ['-n', namespace, 'exec', '-i', 'postgresql-0', '--', 'pg_restore',
                       '-U', 'postgres', '-d', 'econ_graph', '--no-owner', '--role=econgraph',
                       '--exit-on-error', '--single-transaction'], stdin=src, check=True)
    apply([{'apiVersion': 'v1', 'kind': 'ConfigMap', 'metadata': {'name': 'branch-data-seed', 'namespace': namespace},
            'data': {'sha256': digest}}])


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f'deploy-branch: {error}', file=sys.stderr)
        sys.exit(1)
