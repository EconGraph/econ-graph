"""Helpers shared by Python scripts that run kind clusters on this host.

Import it; don't run it. The shell scripts get the same rules from lib/kind-env.sh
and lib/kind-version.sh; this module reads those files instead of repeating their
values, so the two cannot drift apart.

- The Docker daemon: the rootful system daemon, unix:///var/run/docker.sock, unless
  ECONGRAPH_DOCKER_HOST names another. The operator's DOCKER_HOST (a rootless socket
  for everyday use) is deliberately not the override, and DOCKER_CONTEXT is dropped
  because it would win over DOCKER_HOST. kind can't start under rootless Docker here.
- The kind CLI: the pinned copy scripts/deploy/install-kind.sh installs, never a kind
  from PATH, at the version lib/kind-version.sh names, or newer than its minimum.
- The kind node image: the one terraform/k8s/kind-config.yaml pins, so a test cluster
  runs the Kubernetes release production runs.
- The clusters no tool may delete or load into except the production scripts:
  PROTECTED_CLUSTERS.
"""
import hashlib
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[3]
KIND_VERSION_FILE = ROOT / 'scripts/deploy/lib/kind-version.sh'
KIND_CONFIG = ROOT / 'terraform/k8s/kind-config.yaml'
ROOTFUL_DOCKER_HOST = 'unix:///var/run/docker.sock'

# The production cluster (context kind-econ-graph) and the QA cluster, both on the
# rootful daemon of the production host. Nothing but the production scripts may delete
# them, load images into them or write to them.
PRODUCTION_CLUSTER = 'econ-graph'
QA_CLUSTER = 'econgraph-train1-qa'
PROTECTED_CLUSTERS = frozenset({PRODUCTION_CLUSTER, QA_CLUSTER})

# The names the production manifests run (econ-graph-<image>:v<release>), on the nodes
# and on the rootful daemon. load-release-images.sh moves these; nothing else may.
PRODUCTION_STYLE_TAG = re.compile(r'(?:^|/)econ-graph-(backend|frontend|crawler-worker):v[^/]*$')
IMMUTABLE_REFERENCE = re.compile(r'(?:@sha256:[0-9a-f]{64}|:sha-[0-9a-f]{40})$')


def selected_docker_env(environ=None):
    """The environment for docker and kind: the rootful daemon unless ECONGRAPH_DOCKER_HOST."""
    selected = dict(os.environ if environ is None else environ)
    selected.pop('DOCKER_CONTEXT', None)
    selected['DOCKER_HOST'] = selected.get('ECONGRAPH_DOCKER_HOST') or ROOTFUL_DOCKER_HOST
    return selected


def kind_versions():
    """(KIND_VERSION, KIND_MIN_VERSION) as lib/kind-version.sh sets them."""
    text = KIND_VERSION_FILE.read_text()
    version = re.search(r'^KIND_VERSION="(v\d+\.\d+\.\d+)"$', text, re.MULTILINE)
    minimum = re.search(r'^KIND_MIN_VERSION="(v\d+\.\d+\.\d+)"$', text, re.MULTILINE)
    if not version or not minimum:
        raise ValueError(f'cannot read KIND_VERSION and KIND_MIN_VERSION from {KIND_VERSION_FILE}')
    return version[1], minimum[1]


def version_tuple(text):
    match = re.fullmatch(r'v?(\d+)\.(\d+)\.(\d+)(?:[-+].*)?', text.strip())
    if not match:
        raise ValueError(f'not a version: {text!r}')
    return tuple(int(part) for part in match.groups())


def pinned_kind(environ=None, run=subprocess.run):
    """The path of the pinned kind, after checking that it runs and is new enough."""
    environ = os.environ if environ is None else environ
    version, minimum = kind_versions()
    data_home = environ.get('XDG_DATA_HOME') or str(Path(environ.get('HOME', '~')).expanduser() / '.local/share')
    binary = Path(data_home) / 'econgraph/bin' / f'kind-{version}'
    if not os.access(binary, os.X_OK):
        raise ValueError(f'kind {version} is not installed at {binary}; run scripts/deploy/install-kind.sh '
                         '(once, no sudo). A kind from PATH is never used.')
    result = run([str(binary), 'version'], capture_output=True, text=True, check=False, timeout=60)
    fields = result.stdout.split()
    if result.returncode != 0 or len(fields) < 2:
        raise ValueError(f'{binary} does not run; reinstall it with scripts/deploy/install-kind.sh')
    if version_tuple(fields[1]) < version_tuple(minimum):
        raise ValueError(f'{binary} is kind {fields[1]}, older than {minimum}; run scripts/deploy/install-kind.sh')
    return str(binary)


def kind_node_image():
    """The node image (with its digest) that terraform/k8s/kind-config.yaml pins."""
    images = set(re.findall(r'"image":\s*"(kindest/node:[^"]+@sha256:[0-9a-f]{64})"', KIND_CONFIG.read_text()))
    if len(images) != 1:
        raise ValueError(f'{KIND_CONFIG} must pin exactly one kindest/node image by digest')
    return images.pop()


def immutable_reference(reference):
    """True for a digest reference or a full-commit sha-<40 hex> tag."""
    return bool(IMMUTABLE_REFERENCE.search(reference))


def production_style_tag(reference):
    """The image name (backend, frontend, crawler-worker) when REFERENCE is a production-style tag."""
    match = PRODUCTION_STYLE_TAG.search(reference.split('@', 1)[0])
    return match[1] if match else None


def sha256_file(path):
    """The SHA-256 of a file, read only."""
    with open(path, 'rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def free_bytes(path):
    stat = os.statvfs(path)
    return stat.f_bavail * stat.f_frsize


def filesystem_type(path, mounts='/proc/self/mounts'):
    """The type of the filesystem PATH is on, from the longest matching mount point."""
    path = os.path.realpath(path)
    best, kind = '', 'unknown'
    try:
        lines = Path(mounts).read_text().splitlines()
    except OSError:
        return kind
    for line in lines:
        fields = line.split()
        if len(fields) < 3:
            continue
        point = fields[1].replace('\\040', ' ')
        if (path == point or path.startswith(point.rstrip('/') + '/')) and len(point) >= len(best):
            best, kind = point, fields[2]
    return kind
