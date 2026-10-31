#!/usr/bin/env python3
"""Release lab, data-copy mode: a disposable kind cluster running the app stack on its own
copy of the data, restored from dumps given by path.

Usage:
  scripts/deploy/release-lab.sh create --lab NAME \\
      --data-dump FILE --app-dump FILE --keycloak-dump FILE \\
      --backend-image REF --frontend-image REF --crawler-image REF \\
      [--release vX.Y.Z] [--dry-run]
  scripts/deploy/release-lab.sh status --lab NAME
  scripts/deploy/release-lab.sh port-forward --lab NAME
  scripts/deploy/release-lab.sh measure [--lab NAME]
  scripts/deploy/release-lab.sh list
  scripts/deploy/release-lab.sh teardown --lab NAME [--yes] [--keep-storage] [--dry-run]

NAME is 1 to 32 lowercase letters, digits and inner hyphens. The lab is the kind cluster
econ-graph-lab-NAME (context kind-econ-graph-lab-NAME) on the rootful Docker daemon, on
its own Docker network econ-graph-lab-NAME. Its files live in
${ECONGRAPH_LAB_ROOT:-~/econgraph-labs}/NAME/ (never under /tmp or on a tmpfs):
  lab.json      the lab's record, written at creation; every later command takes the
                cluster, context, network and kubeconfig from it, never from an argument
  kind-config.yaml, kube/ (the cluster's kubeconfig and kubectl), node-storage/ (the
                databases' volumes, bind-mounted into the node)

create:
  1. Checks the dumps (regular pg_dump custom-format files outside the lab directory),
     records their SHA-256, size and modification time, and checks that the lab root has
     10 GiB plus 8 times the dumps' size free. Checks the images (below).
  2. Creates the lab directory atomically (an existing lab is reported, never reused for
     other dumps), then the kind cluster: one node, no port mappings, its API server on
     127.0.0.1. Writes the ownership marker ConfigMap kube-system/econgraph-lab-owner
     (the record's random lab id and the cluster's kube-system UID); if that fails, deletes
     the cluster it has just created.
  3. Loads the images under lab-only tags (econ-graph-lab-NAME/<image>:<image id>).
  4. Applies the app stack from scripts/deploy/render-branch.py's local-data rendering:
     ClusterIP Services only, no gateway, no cloudflared, no tunnel token, no API keys,
     lab-generated passwords, crawler-worker at 0 replicas.
  5. Restores each dump into its own empty database (one transaction each), removes
     the Keycloak copy's identity providers, signing keys, client secrets, sessions,
     events and SMTP settings (docs/deployment/RELEASE_LAB.md), runs the data
     migrations, starts Keycloak (which migrates an older copy), the backend and the
     frontend, adds a lab-only Keycloak admin (kc.sh bootstrap-admin) and restarts
     Keycloak, then applies the realm without identity providers.
  6. Checks that the dumps' SHA-256, size and modification time are unchanged.

Images: each REF is an image on the rootful daemon: a digest reference (@sha256:...) or a
full-commit tag (:sha-<40 hex>), which is pulled when missing, or with --release vX.Y.Z
the production tag econ-graph-<image>:vX.Y.Z of exactly that release. A production-style
tag is refused without --release or for another release. An image whose
org.opencontainers.image.revision label disagrees with its sha- tag, or with the commit of
the --release tag in this checkout, is refused. The lab never creates or moves a
production-style tag.

--dry-run makes the read-only checks for real (the dumps, the daemon, the clusters, the
images) and prints the plan; it creates, pulls and loads nothing.

teardown: deletes the lab's own kind cluster, and nothing else, after checking that the
record names a lab cluster (never a cluster without the econ-graph-lab- prefix), that the
cluster carries the ownership marker with the record's lab id and the cluster's own
kube-system UID, and that it carries no production marker. Then removes the lab's Docker
network if nothing else uses it, untags the lab-only image names, and empties
node-storage/, the lab's copy of the data (owned by the containers' users, so it is
removed through Docker). --keep-storage keeps that copy, with a warning; a later teardown
removes it. The dumps are never touched, and lab.json is kept. Asks for the lab name at a
terminal, or --yes. --dry-run makes the same checks and shows what it would delete.
Once torn down, with its storage removed, the lab name can be created again; the old
directory is kept as .NAME.torn-down-<time>/ in the lab root.

port-forward: forwards the frontend, backend and Keycloak to 127.0.0.1 only, on the ports
in the record. measure: memory, CPU, and disk of each lab (docs/deployment/RELEASE_LAB.md).
"""
import argparse
import contextlib
import copy
import datetime
import fcntl
import importlib.util
import json
import os
from pathlib import Path
import re
import secrets
import socket
import subprocess
import sys
import time

import yaml

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE / 'lib'))
import kind_tools  # noqa: E402

_spec = importlib.util.spec_from_file_location('branch_renderer', HERE / 'render-branch.py')
renderer = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(renderer)

LAB_PREFIX = 'econ-graph-lab-'
NAME_PATTERN = re.compile(r'[a-z0-9](?:[a-z0-9-]{0,30}[a-z0-9])?')
NAMESPACE = 'econ-graph'
OWNER_CONFIGMAP = 'econgraph-lab-owner'
CLUSTER_ROLE_CONFIGMAP = 'econgraph-cluster-role'
RECORD_FORMAT = 1
NODE_STORAGE_PATH = '/var/local-path-provisioner'
FREE_RESERVE_BYTES = 10 * 1024 ** 3
FREE_DUMP_FACTOR = 8
IMAGE_NAMES = ('backend', 'frontend', 'crawler-worker')
# Kinds the lab never applies: no gateway, no public routes.
DROPPED_KINDS = {'HTTPRoute', 'BackendTrafficPolicy'}
DROPPED_POLICIES = {'gateway-to-app'}

# The three databases: dump argument, StatefulSet pod and container, superuser, database,
# and the row counts reported after the restore (each only when its table exists).
DATABASES = {
    'data': {'pod': 'postgresql-0', 'container': 'postgresql', 'user': 'postgres', 'database': 'econ_graph',
             'counts': ['economic_series', 'data_points']},
    'app': {'pod': 'app-postgres-0', 'container': 'postgres', 'user': 'postgres', 'database': 'econ_graph_app',
            'counts': ['users', 'chart_annotations']},
    'keycloak': {'pod': 'keycloak-postgres-0', 'container': 'postgres', 'user': 'keycloak',
                 'database': 'keycloak', 'counts': ['realm', 'user_entity']},
}

# Run on the restored Keycloak database before Keycloak starts, in one transaction.
# Removed, so no production secret or sign-in path survives in the lab:
#   - identity providers, their mappers, configuration and account links (no sign-in
#     through Google or Facebook, and none of their client secrets);
#   - the realms' key providers (RSA, HMAC and AES private keys): Keycloak makes new
#     ones, so nothing production signed or encrypted is valid in the lab, and nothing
#     the lab signs is valid in production;
#   - every confidential client's secret, replaced by a random one (the realm import
#     then sets the clients it manages from the lab's Secret), and rotated secrets;
#   - offline sessions, revoked tokens, login and admin events, SMTP settings;
#   - secret-looking values (secret, password, credential, token, key) in the remaining
#     components' and authenticators' configuration, such as a federation bind password.
# Kept on purpose, for realistic sign-in tests: users, their password hashes and other
# credentials, attributes, groups, roles and consents. Tables a Keycloak version lacks
# are skipped.
KEYCLOAK_SANITIZE_SQL = """
DO $$
DECLARE t text;
BEGIN
  IF to_regclass('public.component') IS NOT NULL THEN
    IF to_regclass('public.component_config') IS NOT NULL THEN
      DELETE FROM public.component_config WHERE component_id IN
        (SELECT id FROM public.component WHERE provider_type = 'org.keycloak.keys.KeyProvider');
      UPDATE public.component_config SET value = ''
        WHERE name ~* '(secret|password|credential|token|key)';
    END IF;
    DELETE FROM public.component WHERE provider_type = 'org.keycloak.keys.KeyProvider';
  END IF;
  IF to_regclass('public.client') IS NOT NULL THEN
    UPDATE public.client SET secret = replace(gen_random_uuid()::text, '-', '')
                                      || replace(gen_random_uuid()::text, '-', '')
      WHERE NOT public_client AND secret IS NOT NULL;
  END IF;
  IF to_regclass('public.client_attributes') IS NOT NULL THEN
    DELETE FROM public.client_attributes WHERE name LIKE 'client.secret.rotated%';
  END IF;
  IF to_regclass('public.authenticator_config_entry') IS NOT NULL THEN
    UPDATE public.authenticator_config_entry SET value = ''
      WHERE name ~* '(secret|password|credential|token|key)';
  END IF;
  FOREACH t IN ARRAY ARRAY['idp_mapper_config', 'identity_provider_mapper', 'identity_provider_config',
                           'federated_identity', 'identity_provider', 'offline_client_session',
                           'offline_user_session', 'realm_smtp_config', 'revoked_token',
                           'event_entity', 'admin_event_entity'] LOOP
    IF to_regclass('public.' || t) IS NOT NULL THEN
      EXECUTE format('DELETE FROM public.%I', t);
    END IF;
  END LOOP;
END $$;
"""


class LabError(Exception):
    pass


def log(message):
    print(f'==> {message}', flush=True)


# --- names, paths and the record ------------------------------------------------------

def check_name(name):
    if not name or not NAME_PATTERN.fullmatch(name):
        raise LabError(f'lab name {name!r}: use 1 to 32 lowercase letters, digits and inner hyphens')
    return name


def cluster_for(name):
    return LAB_PREFIX + check_name(name)


def check_lab_cluster(cluster):
    """Refuse any cluster that is not a lab's: production, QA, or a name without the prefix."""
    if cluster in kind_tools.PROTECTED_CLUSTERS:
        raise LabError(f'{cluster} is the production or QA cluster; a lab never touches it')
    if not cluster.startswith(LAB_PREFIX) or not NAME_PATTERN.fullmatch(cluster[len(LAB_PREFIX):]):
        raise LabError(f'{cluster!r} is not a lab cluster name ({LAB_PREFIX}<name>)')
    return cluster


def lab_root(environ=None):
    environ = os.environ if environ is None else environ
    root = Path(environ.get('ECONGRAPH_LAB_ROOT') or Path(environ.get('HOME', '~')).expanduser() / 'econgraph-labs')
    root = Path(os.path.realpath(root))
    if root == Path('/tmp') or Path('/tmp') in root.parents:
        raise LabError(f'lab root {root} is under /tmp (RAM on this host); use a directory under /home')
    probe = root
    while not probe.exists() and probe != probe.parent:
        probe = probe.parent
    if kind_tools.filesystem_type(probe) in ('tmpfs', 'ramfs'):
        raise LabError(f'lab root {root} is on a RAM filesystem; use a directory under /home')
    return root


def lab_dir(name, environ=None):
    return lab_root(environ) / check_name(name)


def read_record(name, environ=None):
    path = lab_dir(name, environ) / 'lab.json'
    try:
        record = json.loads(path.read_text())
    except FileNotFoundError:
        raise LabError(f'no lab {name}: {path} does not exist') from None
    except (OSError, ValueError) as error:
        raise LabError(f'cannot read {path}: {error}') from None
    if record.get('format') != RECORD_FORMAT or record.get('name') != name:
        raise LabError(f'{path} is not the record of lab {name}')
    expected = cluster_for(name)
    if record.get('cluster') != expected or check_lab_cluster(record['cluster']) != expected:
        raise LabError(f'{path} names cluster {record.get("cluster")!r}, not {expected}; refusing')
    if record.get('context') != 'kind-' + expected or record.get('network') != expected:
        raise LabError(f'{path} names context {record.get("context")!r} or network {record.get("network")!r}, '
                       f'not kind-{expected} and {expected}; refusing')
    if not re.fullmatch(r'[0-9a-f]{32}', record.get('lab_id', '')):
        raise LabError(f'{path} has no lab id; refusing')
    return record


def write_record(directory, record):
    temporary = directory / 'lab.json.new'
    temporary.write_text(json.dumps(record, indent=2, sort_keys=True) + '\n')
    os.replace(temporary, directory / 'lab.json')


@contextlib.contextmanager
def lab_lock(directory):
    """One command at a time per lab (create, teardown); the lock goes with the process."""
    with open(directory / 'lock', 'a') as handle:
        try:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise LabError(f'another release-lab command holds {directory / "lock"}') from None
        yield


def now():
    return datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')


# --- inputs ----------------------------------------------------------------------------

def inspect_dump(path, directory):
    """Facts about one dump, read only. Refuses anything but a regular pg_dump archive
    outside the lab directory, so that nothing the lab deletes can be a dump."""
    given = Path(path)
    resolved = Path(os.path.realpath(given))
    if not resolved.is_file():
        raise LabError(f'{given} is not a regular file')
    root = Path(os.path.realpath(directory.parent))
    if resolved == root or root in resolved.parents:
        raise LabError(f'{given} is inside the lab root {root}; keep dumps outside it')
    fd = os.open(resolved, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        header = os.read(fd, 5)
        stat = os.fstat(fd)
    finally:
        os.close(fd)
    if header != b'PGDMP':
        raise LabError(f'{given} is not a pg_dump custom-format archive (pg_dump -Fc)')
    return {'path': str(resolved), 'size': stat.st_size, 'mtime_ns': stat.st_mtime_ns,
            'sha256': kind_tools.sha256_file(resolved)}


def check_inputs_unchanged(inputs):
    for role, facts in inputs.items():
        path = Path(facts['path'])
        stat = path.stat()
        if (stat.st_size, stat.st_mtime_ns) != (facts['size'], facts['mtime_ns']) or \
                kind_tools.sha256_file(path) != facts['sha256']:
            raise LabError(f'the {role} dump {path} changed while the lab ran; the lab record keeps the original '
                           'SHA-256')


def check_free_space(root, inputs):
    needed = FREE_RESERVE_BYTES + FREE_DUMP_FACTOR * sum(facts['size'] for facts in inputs.values())
    probe = root
    while not probe.exists() and probe != probe.parent:
        probe = probe.parent
    free = kind_tools.free_bytes(probe)
    if free < needed:
        raise LabError(f'{root} has {free // 1024 ** 3} GiB free; a lab of these dumps needs '
                       f'{needed // 1024 ** 3} GiB (10 GiB plus 8 times the dumps)')
    return free, needed


# --- images ----------------------------------------------------------------------------

def check_image_reference(name, reference, release):
    """Refuse a mutable reference, and a production-style tag unless it is --release's."""
    style = kind_tools.production_style_tag(reference)
    if style is not None:
        if style != name:
            raise LabError(f'{reference} is the production {style} image, given as the {name} image')
        if release is None:
            raise LabError(f'{reference} is a production-style tag; pass --release with exactly that release '
                           'to run it in the lab, or give a digest or sha-<commit> reference')
        if not reference.split('@', 1)[0].endswith(f'econ-graph-{name}:{release}'):
            raise LabError(f'{reference} is not the {release} {name} image (--release {release})')
        return
    if not kind_tools.immutable_reference(reference):
        raise LabError(f'{reference}: use a digest (@sha256:<64 hex>), a full-commit tag (:sha-<40 hex>), '
                       'or with --release the release tag; a mutable tag could change under the lab')


def lab_tag(name, image, image_id):
    return f'{LAB_PREFIX}{name}/{image}:{image_id.removeprefix("sha256:")[:12]}'


# --- the tools -------------------------------------------------------------------------

class Tools:
    """kind, docker and the lab's kubectl, each on the lab's own cluster and daemon."""

    def __init__(self, record, directory, environ=None):
        self.record = record
        self.directory = directory
        self.env = kind_tools.selected_docker_env(environ)
        # The kubeconfig is always named on the command line; KUBECONFIG and the kind
        # cluster name from the environment never reach a tool.
        self.env.pop('KUBECONFIG', None)
        self.env.pop('KIND_CLUSTER_NAME', None)
        # No credential from the operator's shell (the tunnel token, API keys) reaches a tool.
        for key in [k for k in self.env if re.search(r'TOKEN|SECRET|PASSWORD|API_KEY', k)]:
            del self.env[key]
        self.env['KIND_EXPERIMENTAL_DOCKER_NETWORK'] = record['network']
        self._kind = None

    @property
    def kubeconfig(self):
        return str(self.directory / 'kube/kubeconfig')

    @property
    def kubectl_binary(self):
        return str(self.directory / 'kube/kubectl')

    def kind_binary(self):
        if self._kind is None:
            self._kind = kind_tools.pinned_kind(
                self.env, run=lambda cmd, **kwargs: subprocess.run(cmd, env=self.env, **kwargs))
        return self._kind

    def run(self, cmd, input=None, capture=True, check=True, stdin=None, timeout=3600):
        result = subprocess.run(cmd, input=input, stdin=stdin, text=stdin is None, env=self.env,
                                stdout=subprocess.PIPE if capture else None,
                                stderr=subprocess.PIPE if capture else None, timeout=timeout, check=False)
        if check and result.returncode != 0:
            detail = result.stderr if capture and isinstance(result.stderr, str) else ''
            raise LabError(f'{" ".join(cmd[:4])} ... failed ({result.returncode}): {detail.strip()[-2000:]}')
        return result

    def docker(self, *args, **kwargs):
        return self.run(['docker', *args], **kwargs)

    def kind(self, *args, **kwargs):
        return self.run([self.kind_binary(), *args], **kwargs)

    def kubectl(self, *args, **kwargs):
        return self.run([self.kubectl_binary, '--kubeconfig', self.kubeconfig,
                         '--context', self.record['context'], *args], **kwargs)

    def k(self, *args, **kwargs):
        return self.kubectl('-n', NAMESPACE, *args, **kwargs)

    def clusters(self):
        result = self.kind('get', 'clusters', check=True)
        return set(result.stdout.split())

    def check_rootful(self):
        info = self.docker('info', '--format', '{{.SecurityOptions}}').stdout
        if 'rootless' in info:
            raise LabError(f'{self.env["DOCKER_HOST"]} is a rootless Docker daemon; kind can\'t start there on '
                           'this host. Unset ECONGRAPH_DOCKER_HOST to use the rootful one.')


# --- the kind cluster --------------------------------------------------------------------

def kind_config(record, directory):
    """One node, no port mappings, API server on 127.0.0.1, volumes under the lab directory."""
    return {
        'kind': 'Cluster', 'apiVersion': 'kind.x-k8s.io/v1alpha4', 'name': record['cluster'],
        'networking': {'apiServerAddress': '127.0.0.1', 'apiServerPort': 0},
        'nodes': [{
            'role': 'control-plane', 'image': kind_tools.kind_node_image(),
            'labels': {'econgraph.io/lab': record['name']},
            'extraMounts': [{'hostPath': str(directory / 'node-storage'), 'containerPath': NODE_STORAGE_PATH}],
        }],
    }


def check_kind_config(config):
    """No host port, and an API server reachable from this host only."""
    networking = config.get('networking', {})
    if networking.get('apiServerAddress') != '127.0.0.1':
        raise LabError('a lab API server listens on 127.0.0.1 only')
    for node in config.get('nodes', []):
        if node.get('extraPortMappings'):
            raise LabError('a lab node maps no host ports')
    check_lab_cluster(config.get('name', ''))


def install_kubectl(tools):
    """The cluster's own kubectl and a kubeconfig for this cluster only, in the lab directory."""
    kube = tools.directory / 'kube'
    kube.mkdir(mode=0o700, exist_ok=True)
    node = tools.record['cluster'] + '-control-plane'
    tools.docker('cp', '-q', '-L', f'{node}:/usr/bin/kubectl', str(kube / 'kubectl.new'))
    os.chmod(kube / 'kubectl.new', 0o755)
    os.replace(kube / 'kubectl.new', kube / 'kubectl')
    config = tools.kind('get', 'kubeconfig', '--name', tools.record['cluster']).stdout
    target = kube / 'kubeconfig'
    fd = os.open(target, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, 'w') as handle:
        handle.write(config)


def kube_system_uid(tools):
    uid = tools.kubectl('get', 'namespace', 'kube-system', '-o', 'jsonpath={.metadata.uid}').stdout.strip()
    if not uid:
        raise LabError(f'could not read the kube-system UID of {tools.record["context"]}')
    return uid


def owner_marker(record, uid):
    return {'apiVersion': 'v1', 'kind': 'ConfigMap',
            'metadata': {'name': OWNER_CONFIGMAP, 'namespace': 'kube-system',
                         'labels': {'econgraph.io/lab': record['name']}},
            'data': {'lab-id': record['lab_id'], 'lab': record['name'], 'kube-system-uid': uid}}


def check_ownership(tools, record):
    """The live cluster is this lab's: its marker carries the record's lab id and the
    cluster's own kube-system UID, and it carries no production (cluster role) marker."""
    uid = kube_system_uid(tools)
    if record.get('kube_system_uid') and uid != record['kube_system_uid']:
        raise LabError(f'{record["cluster"]} has kube-system UID {uid}, not the {record["kube_system_uid"]} the lab '
                       'recorded: it was recreated by something else; refusing')
    found = tools.kubectl('-n', 'kube-system', 'get', 'configmap', OWNER_CONFIGMAP, '--ignore-not-found',
                          '-o', 'json').stdout.strip()
    if not found:
        raise LabError(f'{record["cluster"]} has no ownership marker kube-system/{OWNER_CONFIGMAP}; refusing')
    data = json.loads(found).get('data') or {}
    if data.get('lab-id') != record['lab_id'] or data.get('lab') != record['name'] or \
            data.get('kube-system-uid') != uid:
        raise LabError(f'the ownership marker on {record["cluster"]} is not this lab\'s (lab id, name or kube-system '
                       'UID differ); refusing')
    role = tools.kubectl('-n', 'kube-system', 'get', 'configmap', CLUSTER_ROLE_CONFIGMAP, '--ignore-not-found',
                         '-o', 'name').stdout.strip()
    if role:
        raise LabError(f'{record["cluster"]} carries kube-system/{CLUSTER_ROLE_CONFIGMAP}; a lab never does. '
                       'Refusing: find out who marked it before doing anything to it')
    return uid


# --- manifests ---------------------------------------------------------------------------

def render_manifests(record):
    """The local-data branch rendering, as a lab: namespace econ-graph, no gateway objects,
    lab images, crawler-worker at 0 replicas, identity providers off."""
    images = record['images']
    branch = 'lab-' + record['name']
    docs = renderer.render(branch, images['backend']['lab_tag'], images['frontend']['lab_tag'],
                           True, images['crawler-worker']['lab_tag'])
    _, branch_namespace = renderer.identity(branch)
    text = json.dumps(docs).replace(branch_namespace, NAMESPACE)
    docs = [d for d in json.loads(text)
            if d['kind'] not in DROPPED_KINDS
            and not (d['kind'] == 'NetworkPolicy' and d['metadata']['name'] in DROPPED_POLICIES)]
    for d in docs:
        labels = d['metadata'].setdefault('labels', {})
        if d['kind'] == 'Namespace':
            d['metadata']['labels'] = {'econgraph.io/env': 'lab', 'econgraph.io/lab': record['name']}
            d['metadata']['annotations'] = {}
            continue
        labels['econgraph.io/lab'] = record['name']
        if d['kind'] == 'Deployment' and d['metadata']['name'] == 'crawler-worker':
            # A restored crawl queue never reaches upstream APIs until someone scales it up.
            d['spec']['replicas'] = 0
        if d['kind'] in ('Deployment', 'StatefulSet', 'Job'):
            for c in d['spec']['template']['spec'].get('initContainers', []) + \
                    d['spec']['template']['spec']['containers']:
                if c['image'] in {i['lab_tag'] for i in images.values()}:
                    # Loaded into the node by the lab; never pulled.
                    c['imagePullPolicy'] = 'Never'
    realm = json.loads((renderer.ROOT / 'config/keycloak/econ-graph-realm.json').read_text())
    realm['identityProviders'] = []
    docs.append({'apiVersion': 'v1', 'kind': 'ConfigMap',
                 'metadata': {'name': 'keycloak-realm', 'namespace': NAMESPACE,
                              'labels': {'econgraph.io/lab': record['name']}},
                 'data': {'econ-graph-realm.json': json.dumps(realm),
                          'render-realm.sh': (renderer.ROOT / 'scripts/keycloak/render-realm.sh').read_text(),
                          'preserve-catalog-grants.py':
                              (renderer.ROOT / 'scripts/keycloak/preserve-catalog-grants.py').read_text()}})
    docs.append(keycloak_admin_job(docs, record))
    check_manifests(docs)
    return docs


def keycloak_admin_job(docs, record):
    """A Job that adds the lab's Keycloak admin to the restored database (kc.sh
    bootstrap-admin). Keycloak only creates its bootstrap admin on a new database, and the
    restored one has production's admin, whose password the lab does not have."""
    deployment = next(d for d in docs if d['kind'] == 'Deployment' and d['metadata']['name'] == 'keycloak')
    pod = copy.deepcopy(deployment['spec']['template']['spec'])
    container = next(c for c in pod['containers'] if c['name'] == 'keycloak')
    container['name'] = 'bootstrap-admin'
    container['args'] = ['exec /opt/keycloak/bin/kc.sh bootstrap-admin user '
                         '--username:env KC_BOOTSTRAP_ADMIN_USERNAME '
                         '--password:env KC_BOOTSTRAP_ADMIN_PASSWORD --no-prompt']
    for probe in ('startupProbe', 'livenessProbe', 'readinessProbe', 'ports'):
        container.pop(probe, None)
    pod['containers'] = [container]
    pod['restartPolicy'] = 'Never'
    return {'apiVersion': 'batch/v1', 'kind': 'Job',
            'metadata': {'name': 'keycloak-lab-admin', 'namespace': NAMESPACE,
                         'labels': {'econgraph.io/lab': record['name'], 'app': 'keycloak-lab-admin'}},
            'spec': {'backoffLimit': 0, 'template': {'metadata': {'labels': {'app': 'keycloak-lab-admin'}},
                                                     'spec': pod}}}


def check_manifests(docs):
    """What a lab may never apply: host ports, node ports, host networking, a load balancer,
    the tunnel connector or its token, the production namespace's PVs, a running crawler."""
    for d in docs:
        kind, name = d['kind'], d['metadata']['name']
        text = json.dumps(d)
        if 'cloudflare' in text.lower() and kind != 'ConfigMap':
            raise LabError(f'{kind}/{name} mentions cloudflared or its tunnel token; a lab never runs it')
        if kind == 'Secret' and name == 'cloudflare-tunnel':
            raise LabError('a lab never holds the tunnel token')
        if kind == 'Deployment' and name == 'cloudflared':
            raise LabError('a lab never runs cloudflared')
        if kind == 'Service' and d['spec'].get('type', 'ClusterIP') != 'ClusterIP':
            raise LabError(f'Service {name} is {d["spec"]["type"]}; a lab exposes nothing but ClusterIP Services')
        if kind == 'Service' and any('nodePort' in p for p in d['spec'].get('ports', [])):
            raise LabError(f'Service {name} has a nodePort')
        if kind in ('Deployment', 'StatefulSet', 'Job', 'DaemonSet', 'Pod'):
            pod = d['spec']['template']['spec'] if kind != 'Pod' else d['spec']
            if pod.get('hostNetwork') or pod.get('hostPID') or pod.get('hostIPC'):
                raise LabError(f'{kind}/{name} uses the host network or namespaces')
            for c in pod.get('initContainers', []) + pod.get('containers', []):
                if any('hostPort' in p for p in c.get('ports', [])):
                    raise LabError(f'{kind}/{name} container {c["name"]} has a hostPort')
            for volume in pod.get('volumes', []):
                if 'hostPath' in volume:
                    raise LabError(f'{kind}/{name} mounts a hostPath')
        if kind == 'StatefulSet':
            for claim in d['spec'].get('volumeClaimTemplates', []):
                if claim['spec'].get('volumeName') or claim['spec'].get('storageClassName') != 'standard':
                    raise LabError(f'StatefulSet {name} would bind a static volume; a lab uses its own local-path')
        if kind == 'PersistentVolume':
            raise LabError('a lab creates no PersistentVolume; local-path makes them in the lab directory')
        if kind == 'Deployment' and name == 'crawler-worker' and d['spec'].get('replicas') != 0:
            raise LabError('crawler-worker starts at 0 replicas in a lab')
        if d['metadata'].get('namespace', NAMESPACE) != NAMESPACE and kind != 'Namespace':
            raise LabError(f'{kind}/{name} is outside namespace {NAMESPACE}')


def lab_secrets(record):
    """Lab-generated credentials; never production's, and never the tunnel token or API keys."""
    app, app_admin, kc_db, kc_admin, kc_anonymous, data, data_admin = [secrets.token_hex(24) for _ in range(7)]
    admin_user = 'lab-admin-' + record['lab_id'][:8]
    return [
        ('econ-graph-app-postgres', {'postgres-password': app_admin, 'app-password': app}),
        ('econ-graph-keycloak', {'admin-username': admin_user, 'admin-password': kc_admin, 'db-password': kc_db,
                                 'anonymous-client-secret': kc_anonymous}),
        ('econ-graph-postgres', {'postgres-password': data_admin, 'app-password': data}),
        ('econ-graph-secrets', {
            'app-database-url': f'postgresql://econgraph:{app}@app-postgres-service:5432/econ_graph_app',
            'data-database-url': f'postgresql://econgraph:{data}@data-postgres-service:5432/econ_graph'}),
    ]


def apply(tools, docs):
    if docs:
        tools.kubectl('apply', '-f', '-', input=yaml.safe_dump_all(docs, sort_keys=False))


# --- the databases -----------------------------------------------------------------------

def psql(tools, spec, sql):
    return tools.k('exec', spec['pod'], '-c', spec['container'], '--', 'psql', '-v', 'ON_ERROR_STOP=1',
                   '-U', spec['user'], '-d', spec['database'], '-Atc', sql).stdout.strip()


def wait_postgres_settled(tools, spec, attempts=300, interval=2):
    """The real server, after the image's init scripts ran against a temporary one."""
    for _ in range(attempts):
        logs = tools.k('logs', spec['pod'], '-c', spec['container'], check=False).stdout or ''
        if ('PostgreSQL init process complete' in logs or 'Skipping initialization' in logs) and \
                tools.k('exec', spec['pod'], '-c', spec['container'], '--', 'pg_isready', '-q',
                        check=False).returncode == 0:
            return
        time.sleep(interval)
    raise LabError(f'{spec["pod"]} did not finish starting')


def restore_dump(tools, spec, dump):
    """Restore DUMP into the empty database of SPEC, in one transaction. The dump is opened
    read only and streamed; it is never copied, moved or written."""
    tables = psql(tools, spec, "select count(*) from pg_tables where schemaname = 'public'")
    if tables != '0':
        raise LabError(f'{spec["database"]} in {spec["pod"]} already has {tables} tables; a lab restores only '
                       'into a new database. Tear the lab down and create it again.')
    log(f'Restoring {dump} into {spec["database"]} ({spec["pod"]})')
    fd = os.open(dump, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(fd, 'rb') as source:
        tools.k('exec', '-i', spec['pod'], '-c', spec['container'], '--', 'pg_restore', '-U', spec['user'],
                '-d', spec['database'], '--single-transaction', '--exit-on-error', stdin=source, capture=False)
    psql(tools, spec, 'ANALYZE')


def row_counts(tools, spec):
    counts = {}
    for table in spec['counts']:
        present = psql(tools, spec, f"select to_regclass('public.{table}') is not null")
        if present == 't':
            counts[table] = int(psql(tools, spec, f'select count(*) from public.{table}'))
    return counts


def wait_job(tools, name, seconds, interval=5):
    for _ in range(max(1, seconds // interval)):
        status = tools.k('get', 'job', name, '-o',
                         'jsonpath={.status.conditions[?(@.status=="True")].type}', check=False).stdout or ''
        if 'Complete' in status:
            return
        if 'Failed' in status:
            logs = tools.k('logs', f'job/{name}', '--all-containers', check=False).stdout or ''
            raise LabError(f'job {name} failed:\n{logs[-4000:]}')
        time.sleep(interval)
    raise LabError(f'job {name} did not finish within {seconds}s')


# --- commands ------------------------------------------------------------------------------

def free_ports(count, root):
    """COUNT free loopback ports, none of them recorded by another lab."""
    taken = set()
    if root.is_dir():
        for path in root.glob('*/lab.json'):
            with contextlib.suppress(OSError, ValueError, AttributeError):
                taken.update(json.loads(path.read_text()).get('ports', {}).values())
    ports = []
    while len(ports) < count:
        with socket.socket() as listener:
            listener.bind(('127.0.0.1', 0))
            port = listener.getsockname()[1]
        if port not in taken and port not in ports:
            ports.append(port)
    return ports


def release_commit(release):
    """The commit the release tag names in this checkout (read only)."""
    result = subprocess.run(['git', '-C', str(renderer.ROOT), 'rev-parse', '--verify', '--quiet',
                             f'refs/tags/{release}^{{commit}}'], capture_output=True, text=True, check=False)
    commit = result.stdout.strip()
    if result.returncode != 0 or not re.fullmatch(r'[0-9a-f]{40}', commit):
        raise LabError(f'tag {release} is not in this checkout; run git fetch --tags origin')
    return commit


def resolve_image(tools, name, reference, release, pull=True):
    """Inspect (pulling an immutable reference when it is missing) and check one image."""
    check_image_reference(name, reference, release)
    found = tools.docker('image', 'inspect', reference, check=False)
    if found.returncode != 0:
        if kind_tools.production_style_tag(reference):
            raise LabError(f'{reference} is not on {tools.env["DOCKER_HOST"]}; the lab does not pull production '
                           'tags. Load the release first, or give its digest.')
        if not pull:
            print(f'+ docker pull {reference} (not on the daemon yet)')
            return {'reference': reference, 'image_id': 'sha256:' + '0' * 64, 'revision': '(not pulled)'}
        tools.docker('pull', reference, capture=False)
        found = tools.docker('image', 'inspect', reference)
    info = json.loads(found.stdout)[0]
    labels = (info.get('Config') or {}).get('Labels') or {}
    revision = labels.get('org.opencontainers.image.revision', '')
    commit = re.search(r':sha-([0-9a-f]{40})$', reference.split('@', 1)[0])
    expected = commit[1] if commit else release_commit(release) if kind_tools.production_style_tag(reference) else None
    if expected and revision != expected:
        raise LabError(f'{reference} carries revision label {revision!r}, not {expected}')
    digest = reference.split('@', 1)[1] if '@' in reference else None
    if digest and not any(d.endswith('@' + digest) for d in info.get('RepoDigests') or []):
        raise LabError(f'{reference} is not that digest on {tools.env["DOCKER_HOST"]}')
    return {'reference': reference, 'image_id': info['Id'], 'revision': revision}


def load_images(tools, record):
    """Tag each image under its lab-only name on the daemon and load it into the lab's
    cluster, the cluster named in the record, never another."""
    check_lab_cluster(record['cluster'])
    for name, image in record['images'].items():
        tag = image['lab_tag']
        if kind_tools.production_style_tag(tag) or not tag.startswith(LAB_PREFIX + record['name'] + '/'):
            raise LabError(f'{tag} is not a lab-only tag; refusing to tag or load it')
        tools.docker('tag', image['image_id'], tag)
        log(f'Loading {name} ({image["reference"]}) into {record["cluster"]} as {tag}')
        tools.kind('load', 'docker-image', tag, '--name', record['cluster'], capture=False)


def retire_lab_dir(name, root, directory):
    """Move a torn-down lab's directory aside (kept, renamed) so the name can be used
    again. Refused while it still holds a kept copy of the data."""
    storage = directory / 'node-storage'
    with lab_lock(directory):
        if storage.is_dir() and any(storage.iterdir()):
            raise LabError(f'{storage} still holds the data copy kept with --keep-storage; run '
                           f'scripts/deploy/release-lab.sh teardown --lab {name} again to remove it first')
        retired = root / f'.{name}.torn-down-{now().replace(":", "")}'
        os.rename(directory, retired)
    log(f'Lab {name} was torn down; its old directory is kept as {retired}')


def create(args):
    environ = os.environ
    name = check_name(args.lab)
    root = lab_root(environ)
    directory = root / name
    cluster = cluster_for(name)
    if (directory / 'lab.json').exists():
        record = read_record(name, environ)
        if record.get('status') == 'ready':
            log(f'Lab {name} exists and is ready ({cluster}); nothing to do. scripts/deploy/release-lab.sh status '
                f'--lab {name} shows it; teardown removes it.')
            return
        if record.get('status') != 'torn-down':
            raise LabError(f'lab {name} exists with status {record.get("status")!r}; tear it down '
                           f'(scripts/deploy/release-lab.sh teardown --lab {name}) and create it again')
        if not args.dry_run:
            retire_lab_dir(name, root, directory)
    if args.release and not re.fullmatch(r'v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?', args.release):
        raise LabError(f'--release {args.release}: use a release tag such as v4.0.1')
    record = {'format': RECORD_FORMAT, 'name': name, 'cluster': cluster, 'context': 'kind-' + cluster,
              'network': cluster, 'lab_id': secrets.token_hex(16), 'created_at': now(), 'status': 'creating',
              'release': args.release, 'mode': 'data-copy',
              'ports': dict(zip(('frontend', 'backend', 'keycloak'), free_ports(3, root)))}
    dumps = {'data': args.data_dump, 'app': args.app_dump, 'keycloak': args.keycloak_dump}
    inputs = {role: inspect_dump(path, directory) for role, path in dumps.items()}
    if len({facts['sha256'] for facts in inputs.values()}) != len(inputs):
        raise LabError('two of the dumps have the same contents; give the data, app and Keycloak dumps')
    free, needed = check_free_space(root, inputs)
    log(f'Dumps checked; {root} has {free // 1024 ** 3} GiB free, the lab needs {needed // 1024 ** 3} GiB')
    record['inputs'] = inputs
    # A dry run makes the same read-only checks (daemon, clusters, images) for real.
    tools = Tools(record, directory, environ)
    tools.check_rootful()
    if cluster in tools.clusters():
        raise LabError(f'a kind cluster {cluster} exists without a lab record here; it is not this lab\'s. '
                       'Choose another name.')
    record['images'] = {}
    for image, reference in (('backend', args.backend_image), ('frontend', args.frontend_image),
                             ('crawler-worker', args.crawler_image)):
        facts = resolve_image(tools, image, reference, args.release, pull=not args.dry_run)
        facts['lab_tag'] = lab_tag(name, image, facts['image_id'])
        record['images'][image] = facts
    config = kind_config(record, directory)
    check_kind_config(config)
    docs = render_manifests(record)
    if args.dry_run:
        print(f'Would create {directory} and kind cluster {cluster} (network {cluster}, no port mappings),')
        print(f'load {", ".join(i["reference"] for i in record["images"].values())} under lab tags, apply '
              f'{len(docs)} objects in {NAMESPACE} (crawler-worker at 0 replicas), and restore:')
        for role, facts in inputs.items():
            print(f'  {role:8} {facts["path"]} ({facts["size"]} bytes, sha256 {facts["sha256"]})')
        return
    # The lab directory is the lock on the name: mkdir either creates it or fails.
    root.mkdir(parents=True, exist_ok=True)
    try:
        directory.mkdir(mode=0o700)
    except FileExistsError:
        raise LabError(f'{directory} exists without a ready lab record; another create may be running, or an '
                       'earlier one failed before writing it. Remove the directory once no create runs.') from None
    (directory / 'node-storage').mkdir(mode=0o755)
    with lab_lock(directory):
        write_record(directory, record)
        try:
            build_lab(tools, record, directory, config, docs)
        except BaseException as error:
            record['status'] = 'failed'
            record['error'] = str(error)[-2000:]
            write_record(directory, record)
            raise
    record['status'] = 'ready'
    record['ready_at'] = now()
    write_record(directory, record)
    log(f'Lab {name} is ready. Reach it on 127.0.0.1 with: scripts/deploy/release-lab.sh port-forward --lab {name}')
    log(f'crawler-worker is at 0 replicas; remove the lab with: scripts/deploy/release-lab.sh teardown --lab {name}')


def build_lab(tools, record, directory, config, docs):
    (directory / 'kind-config.yaml').write_text(yaml.safe_dump(config, sort_keys=False))
    log(f'Creating kind cluster {record["cluster"]} on {tools.env["DOCKER_HOST"]} (network {record["network"]})')
    tools.kind('create', 'cluster', '--name', record['cluster'], '--config', str(directory / 'kind-config.yaml'),
               '--kubeconfig', tools.kubeconfig, '--wait', '300s', capture=False)
    try:
        install_kubectl(tools)
        uid = kube_system_uid(tools)
        # create, not apply: fails when the marker exists, so two labs never share a cluster.
        tools.kubectl('create', '-f', '-', input=yaml.safe_dump(owner_marker(record, uid)))
    except BaseException:
        log(f'Could not mark {record["cluster"]} as the lab\'s; deleting the cluster this run just created')
        tools.kind('delete', 'cluster', '--name', record['cluster'], '--kubeconfig', tools.kubeconfig, check=False)
        raise
    record['kube_system_uid'] = uid
    write_record(directory, record)
    check_ownership(tools, record)
    load_images(tools, record)

    log('Applying the namespace, policies, Secrets and databases')
    namespace = [d for d in docs if d['kind'] == 'Namespace']
    apply(tools, namespace)
    for name, values in lab_secrets(record):
        secret = {'apiVersion': 'v1', 'kind': 'Secret', 'type': 'Opaque', 'stringData': values,
                  'metadata': {'name': name, 'namespace': NAMESPACE, 'labels': {'econgraph.io/lab': record['name']}}}
        # create, not apply: no last-applied annotation holding the values.
        tools.kubectl('create', '-f', '-', input=yaml.safe_dump(secret))
    apply(tools, [d for d in docs if d['kind'] not in ('Namespace', 'Deployment', 'Job')])
    for statefulset in ('postgresql', 'app-postgres', 'keycloak-postgres'):
        tools.k('rollout', 'status', f'statefulset/{statefulset}', '--timeout=600s')
    counts = {}
    for role, spec in DATABASES.items():
        wait_postgres_settled(tools, spec)
        restore_dump(tools, spec, record['inputs'][role]['path'])
        if role == 'keycloak':
            log('Removing identity providers, keys, client secrets, sessions and events from the Keycloak copy')
            psql(tools, spec, KEYCLOAK_SANITIZE_SQL)
        counts[role] = row_counts(tools, spec)
    record['restored_counts'] = counts
    write_record(directory, record)

    jobs = {d['metadata']['name']: d for d in docs if d['kind'] == 'Job'}
    log('Running the data migrations (crawler-migrate)')
    apply(tools, [jobs['crawler-migrate']])
    wait_job(tools, 'crawler-migrate', 1800)
    log('Starting Keycloak, the backend and the frontend (crawler-worker at 0 replicas)')
    apply(tools, [d for d in docs if d['kind'] == 'Deployment'])
    # Keycloak first: its first start migrates a copy from an older Keycloak, and
    # kc.sh bootstrap-admin refuses a database that is not migrated yet.
    tools.k('rollout', 'status', 'deployment/keycloak', '--timeout=900s')
    log('Adding the lab Keycloak admin (keycloak-lab-admin), then restarting Keycloak')
    apply(tools, [jobs['keycloak-lab-admin']])
    wait_job(tools, 'keycloak-lab-admin', 900)
    tools.k('rollout', 'restart', 'deployment/keycloak')
    for deployment in ('keycloak', 'econ-graph-backend', 'econ-graph-frontend'):
        tools.k('rollout', 'status', f'deployment/{deployment}', '--timeout=900s')
    log('Applying the realm without identity providers (keycloak-realm-import)')
    apply(tools, [jobs['keycloak-realm-import']])
    wait_job(tools, 'keycloak-realm-import', 900)
    replicas = tools.k('get', 'deployment', 'crawler-worker', '-o', 'jsonpath={.spec.replicas}').stdout.strip()
    if replicas != '0':
        raise LabError(f'crawler-worker has {replicas!r} replicas; a lab starts it at 0')
    check_inputs_unchanged(record['inputs'])


def open_lab(name):
    record = read_record(name)
    directory = lab_dir(name)
    return record, directory, Tools(record, directory)


def status(args):
    record, directory, tools = open_lab(check_name(args.lab))
    print(json.dumps({k: v for k, v in record.items() if k != 'lab_id'}, indent=2, sort_keys=True))
    if record['cluster'] not in tools.clusters():
        print(f'kind cluster {record["cluster"]} does not exist')
        return
    install_kubectl(tools)
    check_ownership(tools, record)
    print(tools.k('get', 'pods', '-o', 'wide').stdout)


def start_forward(cmd, env):
    return subprocess.Popen(cmd, env=env)


def port_forward(args):
    record, directory, tools = open_lab(check_name(args.lab))
    check_ownership(tools, record)
    targets = [('frontend', 'svc/econ-graph-frontend-service', 3000),
               ('backend', 'svc/econ-graph-backend-service', 9876),
               ('keycloak', 'svc/keycloak-service', 8080)]
    processes = []
    try:
        for name, service, port in targets:
            local = record['ports'][name]
            print(f'  {name:9} http://127.0.0.1:{local}' + ('/idp/admin' if name == 'keycloak' else ''))
            cmd = [tools.kubectl_binary, '--kubeconfig', tools.kubeconfig, '--context', record['context'],
                   '-n', NAMESPACE, 'port-forward', '--address', '127.0.0.1', service, f'{local}:{port}']
            processes.append(start_forward(cmd, tools.env))
        print('Press Ctrl-C to stop.')
        while all(p.poll() is None for p in processes):
            time.sleep(1)
    except KeyboardInterrupt:
        pass
    finally:
        for p in processes:
            p.terminate()


def measure(args):
    names = [check_name(args.lab)] if args.lab else sorted(
        p.name for p in lab_root().iterdir() if (p / 'lab.json').is_file() and not p.name.startswith('.')) \
        if lab_root().is_dir() else []
    for name in names:
        record, directory, tools = open_lab(name)
        node = record['cluster'] + '-control-plane'
        if record['cluster'] not in tools.clusters():
            print(f'{name}: no cluster ({record.get("status")})')
            continue
        stats = tools.docker('stats', '--no-stream', '--format', '{{.CPUPerc}} {{.MemUsage}}', node).stdout.strip()
        storage = tools.docker('exec', node, 'du', '-sxb', NODE_STORAGE_PATH, check=False).stdout.split()
        images = tools.docker('exec', node, 'du', '-sxb', '/var/lib/containerd', check=False).stdout.split()
        layer = tools.docker('ps', '--size', '--filter', f'name=^{node}$', '--format', '{{.Size}}').stdout.strip()
        print(f'{name}: cpu+memory {stats}; databases {int(storage[0]) // 1024 ** 2 if storage else "?"} MiB '
              f'({directory / "node-storage"}); images {int(images[0]) // 1024 ** 2 if images else "?"} MiB; '
              f'node container {layer}')


def list_labs(args):
    root = lab_root()
    if not root.is_dir():
        print(f'no labs ({root} does not exist)')
        return
    for path in sorted(root.iterdir()):
        if (path / 'lab.json').is_file() and not path.name.startswith('.'):
            try:
                record = read_record(path.name)
                print(f'{record["name"]:20} {record.get("status"):10} {record["cluster"]}  '
                      f'created {record["created_at"]}')
            except LabError as error:
                print(f'{path.name:20} unreadable: {error}')


def teardown(args):
    name = check_name(args.lab)
    record = read_record(name)
    directory = lab_dir(name)
    cluster = check_lab_cluster(record['cluster'])
    tools = Tools(record, directory)
    with lab_lock(directory):
        tools.check_rootful()
        exists = cluster in tools.clusters()
        if exists:
            install_kubectl(tools)
            uid = check_ownership(tools, record)
            nodes = tools.kind('get', 'nodes', '--name', cluster).stdout.split()
            if not nodes or any(not n.startswith(cluster + '-') for n in nodes):
                raise LabError(f'kind lists nodes {nodes} for {cluster}; refusing')
            log(f'{cluster}: kube-system UID {uid}, ownership marker of lab {name}, nodes {" ".join(nodes)}')
        else:
            log(f'kind cluster {cluster} does not exist; nothing to delete there')
        if args.dry_run:
            if exists:
                print(f'+ kind delete cluster --name {cluster}')
            print(f'+ docker network rm {record["network"]} (only if no container uses it)')
            if not args.keep_storage:
                print(f'+ empty {directory / "node-storage"} (the lab\'s copy of the data) through Docker')
            return
        if not args.yes:
            if not sys.stdin.isatty():
                raise LabError('teardown needs the lab name typed at a terminal, or --yes')
            if input(f'Type {name} to delete kind cluster {cluster}: ') != name:
                raise LabError('confirmation did not match; nothing was deleted')
        if exists:
            # The ownership is read again right before the delete.
            check_ownership(tools, record)
            tools.kind('delete', 'cluster', '--name', cluster, '--kubeconfig', tools.kubeconfig, capture=False)
        remove_network(tools, record)
        remove_lab_tags(tools, record)
        if args.keep_storage:
            print(f'WARNING: {directory / "node-storage"} still holds the lab\'s full copy of the data '
                  '(--keep-storage). Remove it with: scripts/deploy/release-lab.sh teardown --lab '
                  f'{name}', file=sys.stderr)
            record['storage'] = 'kept'
        else:
            remove_storage(tools, record, directory)
            record['storage'] = 'removed'
        record['status'] = 'torn-down'
        record['torn_down_at'] = now()
        write_record(directory, record)
        log(f'Lab {name} torn down; {directory / "lab.json"} is kept, and the dumps were not touched')


def remove_lab_tags(tools, record):
    """Untag the lab-only names on the daemon. An image keeps its other names (a release
    tag, a digest); one that had only the lab's name is removed."""
    for image in record.get('images', {}).values():
        tag = image.get('lab_tag', '')
        if tag.startswith(LAB_PREFIX + record['name'] + '/') and not kind_tools.production_style_tag(tag):
            tools.docker('image', 'rm', tag, check=False)


def remove_network(tools, record):
    network = record['network']
    check_lab_cluster(network)
    found = tools.docker('network', 'inspect', network, '--format', '{{len .Containers}}', check=False)
    if found.returncode != 0:
        return
    if found.stdout.strip() != '0':
        log(f'Docker network {network} still has containers; kept')
        return
    tools.docker('network', 'rm', network)


def remove_storage(tools, record, directory):
    """Remove node-storage/, which the containers' users own, through a container that
    mounts only that directory. Only once the cluster is gone."""
    storage = directory / 'node-storage'
    if storage.is_symlink() or not storage.is_dir():
        raise LabError(f'{storage} is not the lab\'s storage directory; not removing it')
    if record['cluster'] in tools.clusters():
        raise LabError(f'{record["cluster"]} still exists; not removing its storage')
    if not any(storage.iterdir()):
        return
    image = kind_tools.kind_node_image()
    tools.docker('run', '--rm', '--network', 'none', '--entrypoint', '/usr/bin/find',
                 '-v', f'{storage}:/lab-storage', image, '/lab-storage', '-xdev', '-mindepth', '1', '-delete')
    log(f'Removed the contents of {storage}')


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest='command', required=True)
    c = sub.add_parser('create')
    c.add_argument('--lab', required=True)
    for flag in ('--data-dump', '--app-dump', '--keycloak-dump', '--backend-image', '--frontend-image',
                 '--crawler-image'):
        c.add_argument(flag, required=True)
    c.add_argument('--release', help='only to run the production-style tags of exactly this release')
    c.add_argument('--dry-run', action='store_true')
    for command in ('status', 'port-forward'):
        sub.add_parser(command).add_argument('--lab', required=True)
    sub.add_parser('measure').add_argument('--lab')
    sub.add_parser('list')
    t = sub.add_parser('teardown')
    t.add_argument('--lab', required=True)
    t.add_argument('--yes', action='store_true')
    t.add_argument('--keep-storage', action='store_true',
                   help="keep node-storage/, the lab's copy of the data (removed by default)")
    t.add_argument('--dry-run', action='store_true')
    args = p.parse_args(argv)
    {'create': create, 'status': status, 'port-forward': port_forward, 'measure': measure,
     'list': list_labs, 'teardown': teardown}[args.command](args)


if __name__ == '__main__':
    try:
        main()
    except (LabError, ValueError, OSError, subprocess.TimeoutExpired) as error:
        print(f'release-lab: error: {error}', file=sys.stderr)
        sys.exit(1)
