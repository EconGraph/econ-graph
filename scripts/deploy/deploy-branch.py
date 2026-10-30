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
import tempfile
import time

import yaml

spec = importlib.util.spec_from_file_location('branch_renderer', Path(__file__).with_name('render-branch.py'))
renderer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(renderer)

COMMAND_TIMEOUT = 120
MAX_SEED_BYTES = 128 * 1024 * 1024
MIN_FREE_BYTES = 4 * 1024 * 1024 * 1024


def verify_ownership(existing, slug, branch, local_data=None):
    if not existing:
        raise ValueError('namespace disappeared while acquiring the lock')
    meta = existing['metadata']
    labels = meta.get('labels', {})
    if (labels.get('econgraph.io/env') != 'branch' or labels.get('econgraph.io/branch') != slug
            or meta.get('annotations', {}).get('econgraph.io/branch-name') != branch):
        raise ValueError('namespace ownership does not match; refusing changes')
    if meta.get('deletionTimestamp'):
        raise ValueError('namespace is terminating; wait before redeploying')
    if local_data is not None and labels.get('econgraph.io/data-mode') != ('local' if local_data else 'primary'):
        raise ValueError('data mode cannot change in place; remove the disposable branch first')


def wait_job(k, namespace, name, seconds):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        result = k('-n', namespace, 'get', 'job', name, '-o', 'json', capture=True)
        job = json.loads(result.stdout)
        conditions = job.get('status', {}).get('conditions', [])
        if any(c.get('type') == 'Failed' and c.get('status') == 'True' for c in conditions):
            raise ValueError(f'Job {namespace}/{name} failed; inspect its logs')
        if any(c.get('type') == 'Complete' and c.get('status') == 'True' for c in conditions):
            return
        time.sleep(2)
    raise ValueError(f'Job {namespace}/{name} did not complete within {seconds}s')


def inspect_seed_size(dump):
    """Inspect expanded bytes in a temporary file with a hard output limit."""
    # A regular output file is subject to RLIMIT_FSIZE. A compressed archive is
    # never treated as evidence of its expanded storage requirement.
    import resource
    limit = 256 * 1024 * 1024
    def bound_output():
        resource.setrlimit(resource.RLIMIT_FSIZE, (limit, limit))
    with tempfile.TemporaryFile() as output:
        subprocess.run(['pg_restore', '--file=-', str(dump)], stdout=output,
                       check=True, timeout=COMMAND_TIMEOUT, preexec_fn=bound_output)
        return output.tell()


def freshen_mutable_images(docs):
    """A registry tag rebuild must restart pods as well as use Always pulling."""
    for doc in docs:
        if doc['kind'] != 'Deployment':
            continue
        template = doc['spec']['template']
        if any(c['name'] in ('backend', 'frontend', 'proxy', 'crawler-worker')
               and not renderer.immutable_image(c['image']) for c in template['spec']['containers']):
            template['metadata'].setdefault('annotations', {})['econgraph.io/redeploy-token'] = secrets.token_hex(8)


def local_candidate_images(args):
    candidates = [args.backend_image, args.frontend_image]
    if args.local_data:
        candidates.append(args.data_image)
    return list(dict.fromkeys(image for image in candidates if image and renderer.local_image(image)))


def selected_docker_env(environ=os.environ):
    selected = dict(environ)
    # DOCKER_CONTEXT overrides DOCKER_HOST even when the latter is explicit.
    # Preserve credentials/TLS settings while selecting the intended daemon.
    selected.pop('DOCKER_CONTEXT', None)
    selected['DOCKER_HOST'] = environ.get('ECONGRAPH_DOCKER_HOST') or 'unix:///var/run/docker.sock'
    return selected


def load_local_images(args):
    images = local_candidate_images(args)
    if not images:
        return
    # Context is explicit and validated before cluster access. Never infer a
    # kind cluster from current-context or silently load a different daemon.
    if not args.context.startswith('kind-') or len(args.context) <= len('kind-'):
        raise ValueError('local candidate images require an explicit kind-<cluster-name> context')
    subprocess.run(['kind', 'load', 'docker-image', '--name', args.context[len('kind-'):], *images],
                   check=True, timeout=900, env=selected_docker_env())


def wait_namespace_deleted(lookup, namespace, uid, seconds=120):
    """Wait only for this incarnation; never wait for or delete a replacement."""
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        current = lookup('namespace', namespace)
        if not current or current['metadata']['uid'] != uid:
            return
        time.sleep(2)
    raise ValueError(f'namespace {namespace} deletion did not complete within {seconds}s')


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
    p.add_argument('--data-image', help='release, full sha-<commit> or digest crawler image; local data only')
    p.add_argument('--seed-dir', type=Path, help='verified primary dump bundle; required for local data')
    p.add_argument('--seed-sha256', help='expected archive hash from reviewed fixture evidence, outside the bundle')
    args = p.parse_args()
    if args.action != 'list' and not args.branch:
        p.error('branch is required')
    if args.action == 'up' and (not args.backend_image or not args.frontend_image):
        p.error('up requires --backend-image and --frontend-image (built for this branch hostname)')
    if args.local_data and (not args.data_image or not args.seed_dir or not args.seed_sha256
                            or not re.fullmatch('[0-9a-f]{64}', args.seed_sha256)):
        p.error('--local-data requires --data-image, --seed-dir and a reviewed --seed-sha256')
    if args.local_data:
        try:
            renderer.validate_data_image(args.data_image)
        except ValueError as error:
            p.error(str(error))
    elif args.data_image or args.seed_dir or args.seed_sha256:
        p.error('--data-image, --seed-dir and --seed-sha256 require --local-data')
    if args.action == 'up' and local_candidate_images(args) and (
            not args.context.startswith('kind-') or len(args.context) <= len('kind-')):
        p.error('local candidate images require an explicit kind-<cluster-name> context')
    base = []
    def kubectl():
        # Chosen on first use: a dry run and build-frontend contact no cluster.
        if not base:
            base.extend([default_kubectl(), '--context', args.context])
        return base
    args.kubectl = kubectl
    def k(*cmd, data=None, capture=False, check=True):
        waits = [int(arg.split('=')[1][:-1]) for arg in cmd if re.fullmatch(r'--timeout=\d+s', arg)]
        timeout = max([COMMAND_TIMEOUT] + [seconds + 30 for seconds in waits])
        return subprocess.run(kubectl() + list(cmd), input=data, text=True, check=check,
                              stdout=subprocess.PIPE if capture else None, timeout=timeout)
    def apply(docs):
        k('apply', '-f', '-', data=yaml.safe_dump_all(docs, sort_keys=False))
    def lookup(kind, name, namespace=None):
        cmd = (['-n', namespace] if namespace else []) + ['get', kind, name, '--ignore-not-found', '-o', 'json']
        result = k(*cmd, capture=True)
        return json.loads(result.stdout) if result.stdout.strip() else None
    def delete_grant(ref):
        meta = ref['metadata']
        options = {'apiVersion': 'v1', 'kind': 'DeleteOptions',
                   'preconditions': {'uid': meta['uid'], 'resourceVersion': meta['resourceVersion']}}
        k('delete', '--raw', '/apis/gateway.networking.k8s.io/v1beta1/namespaces/econ-graph/'
          'referencegrants/' + meta['name'], '-f', '-', data=json.dumps(options))
    @contextmanager
    def lock(namespace):
        # Atomic create is a cluster-wide lock, including invocations on other
        # hosts or with kubeconfig aliases. Never steal an existing lock.
        obj = {'apiVersion': 'v1', 'kind': 'ConfigMap',
               'metadata': {'name': 'branch-deploy-lock', 'namespace': namespace},
               'data': {'owner': secrets.token_hex(16)}}
        owner = obj['data']['owner']
        k('create', '-f', '-', data=yaml.safe_dump(obj))
        try:
            yield
        finally:
            current = lookup('configmap', 'branch-deploy-lock', namespace)
            if current and current.get('data', {}).get('owner') == owner:
                # Delete by UID+resourceVersion through a raw API precondition;
                # a replacement lock cannot be deleted between get and delete.
                meta = current['metadata']
                options = {'apiVersion': 'v1', 'kind': 'DeleteOptions',
                           'preconditions': {'uid': meta['uid'], 'resourceVersion': meta['resourceVersion']}}
                k('delete', '--raw', f'/api/v1/namespaces/{namespace}/configmaps/branch-deploy-lock',
                  '-f', '-', data=json.dumps(options), check=False)
    if args.action == 'list':
        if args.dry_run:
            print('List namespaces labelled econgraph.io/env=branch')
        else:
            k('get', 'namespaces', '-l', 'econgraph.io/env=branch',
              '-L', 'econgraph.io/branch,econgraph.io/data-mode')
            k('-n', 'econ-graph', 'get', 'referencegrants', '-l', 'econgraph.io/branch',
              '-L', 'econgraph.io/branch')
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
        docker_env = selected_docker_env()
        if args.dry_run:
            print('DOCKER_HOST=' + docker_env['DOCKER_HOST'] + ' ' + ' '.join(cmd))
        else:
            subprocess.run(cmd, check=True, env=docker_env)
        return
    if args.dry_run:
        print(f'{args.action}: namespace={namespace} host={origin} context={args.context}')
        if args.action == 'up':
            docs = renderer.render(args.branch, args.backend_image, args.frontend_image, args.local_data, args.data_image)
            print(f'Probe NetworkPolicy enforcement before credentials/workloads; apply {len(docs)} branch resources only if it passes.')
            if local_candidate_images(args):
                print('Reload locally built candidate images into ' + args.context + ' before rolling pods: ' +
                      ', '.join(local_candidate_images(args)))
            print('Local data: verify/seed dump and migrate; worker stays stopped for review.' if args.local_data else
                  f'Create only ReferenceGrant data-graphql-from-{slug} in econ-graph; no primary database credentials.')
        else:
            print('Require matching branch ownership labels and typed namespace confirmation; delete only namespace and its ReferenceGrant.')
        return
    existing = lookup('namespace', namespace)
    if existing:
        verify_ownership(existing, slug, args.branch, args.local_data if args.action == 'up' else None)
    if args.action == 'down':
        if not existing:
            ref = lookup('referencegrant', 'data-graphql-from-' + slug, 'econ-graph')
            if ref:
                if (ref.get('metadata', {}).get('labels', {}).get('econgraph.io/branch') != slug
                        or ref.get('spec') != renderer.grant(args.branch)['spec']):
                    raise ValueError('ReferenceGrant ownership differs; refusing orphan deletion')
                if not sys.stdin.isatty() or input(f'Type {namespace} to remove its orphan grant: ') != namespace:
                    raise ValueError('typed namespace confirmation required')
                if lookup('namespace', namespace):
                    raise ValueError('branch namespace appeared during confirmation; refusing orphan cleanup')
                delete_grant(ref)
            print('Branch namespace absent; removed its owned orphan grant if present.')
            return
        confirmed_uid = existing['metadata']['uid']
        if not sys.stdin.isatty() or input(f'Type {namespace} to delete its disposable users and databases: ') != namespace:
            raise ValueError('typed namespace confirmation required')
        with lock(namespace):
            current_namespace = lookup('namespace', namespace)
            verify_ownership(current_namespace, slug, args.branch)
            meta = current_namespace['metadata']
            if meta['uid'] != confirmed_uid:
                raise ValueError('branch namespace was replaced during confirmation; refusing deletion')
            ref = lookup('referencegrant', 'data-graphql-from-' + slug, 'econ-graph')
            if ref and ref.get('metadata', {}).get('labels', {}).get('econgraph.io/branch') != slug:
                raise ValueError('ReferenceGrant ownership differs; refusing deletion')
            if ref:
                bound_uid = ref.get('metadata', {}).get('annotations', {}).get('econgraph.io/namespace-uid')
                if bound_uid and bound_uid != confirmed_uid:
                    raise ValueError('ReferenceGrant belongs to another namespace incarnation; refusing deletion')
            if ref:
                delete_grant(ref)
            options = {'apiVersion': 'v1', 'kind': 'DeleteOptions',
                       'preconditions': {'uid': confirmed_uid, 'resourceVersion': meta['resourceVersion']}}
            k('delete', '--raw', '/api/v1/namespaces/' + namespace,
              '-f', '-', data=json.dumps(options))
            wait_namespace_deleted(lookup, namespace, confirmed_uid)
        return
    # Confirm gateway dependency exists before creating disposable resources.
    k('-n', 'econ-graph-gateway', 'get', 'gateway', 'econ-graph-gateway', capture=True)
    docs = renderer.render(args.branch, args.backend_image, args.frontend_image, args.local_data, args.data_image)
    # Create atomically. Never apply an identity label over a namespace another
    # invocation created between the initial lookup and lock acquisition.
    if not existing:
        k('create', '-f', '-', data=yaml.safe_dump(docs[0]))
    with lock(namespace):
        current_namespace = lookup('namespace', namespace)
        verify_ownership(current_namespace, slug, args.branch, args.local_data)
        namespace_uid = current_namespace['metadata']['uid']
        load_local_images(args)
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
            wait_job(k, namespace, 'crawler-migrate', 1800)
        freshen_mutable_images(docs)
        apply([d for d in docs if d['kind'] == 'Deployment'])
        for deployment in ['keycloak', 'econ-graph-backend', 'econ-graph-frontend'] + ([] if args.local_data else ['branch-data-proxy']):
            k('-n', namespace, 'rollout', 'status', 'deployment/' + deployment, '--timeout=900s')
        k('-n', namespace, 'delete', 'job', 'keycloak-realm-import', '--ignore-not-found')
        apply([d for d in docs if d['kind'] == 'Job' and d['metadata']['name'] == 'keycloak-realm-import'])
        wait_job(k, namespace, 'keycloak-realm-import', 900)
        if not args.local_data:
            ref = renderer.grant(args.branch)
            # An orphan cleanup may already hold the previous grant's deletion
            # preconditions. Stamp this namespace incarnation so reusing a grant
            # changes resourceVersion before the new environment relies on it.
            ref['metadata']['annotations'] = {'econgraph.io/namespace-uid': namespace_uid}
            current = lookup('referencegrant', ref['metadata']['name'], 'econ-graph')
            if current and current.get('metadata', {}).get('labels', {}).get('econgraph.io/branch') != slug:
                raise ValueError('ReferenceGrant already exists with different ownership')
            apply([ref])
        print(f'Branch ready at {origin}; create branch users through Keycloak admin port-forward. Local workers remain stopped.')


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
    if getattr(args, 'seed_sha256', None) != digest:
        raise ValueError('seed does not match the reviewed external --seed-sha256')
    if dump.stat().st_size > MAX_SEED_BYTES:
        raise ValueError('local seed exceeds the 128MiB bounded-fixture limit')
    listing = subprocess.run(['pg_restore', '--list', str(dump)], capture_output=True, text=True,
                             check=True, timeout=COMMAND_TIMEOUT).stdout
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
    expanded = inspect_seed_size(dump)
    free = k('-n', namespace, 'exec', 'postgresql-0', '--', 'sh', '-ec',
             "df -Pk /data | awk 'NR==2 {print $4}'", capture=True)
    if not free.stdout.strip().isdigit() or int(free.stdout.strip()) * 1024 < MIN_FREE_BYTES + expanded * 4:
        raise ValueError('insufficient node disk reserve for bounded local seed (4GiB plus 4x expanded archive)')
    # Stream only into the explicitly owned branch pod. No primary connection
    # string or credentials are ever requested or copied.
    with dump.open('rb') as src:
        subprocess.run(args.kubectl() + ['-n', namespace, 'exec', '-i', 'postgresql-0', '--', 'pg_restore',
                       '-U', 'postgres', '-d', 'econ_graph', '--no-owner', '--role=econgraph',
                       '--exit-on-error', '--single-transaction'], stdin=src, check=True, timeout=900)
    apply([{'apiVersion': 'v1', 'kind': 'ConfigMap', 'metadata': {'name': 'branch-data-seed', 'namespace': namespace},
            'data': {'sha256': digest}}])


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
        print(f'deploy-branch: {error}', file=sys.stderr)
        sys.exit(1)
