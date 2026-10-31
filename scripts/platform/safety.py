"""Explicit context and ownership checks for opt-in scratch-platform mutations."""
import json
from pathlib import Path
import re
import subprocess

LABEL = 'econgraph.io/drill-owner'
DISPOSABLE = 'econgraph.io/disposable-drill'


def identifier(value):
    if not re.fullmatch(r'[a-z0-9](?:[-a-z0-9]{0,61}[a-z0-9])?', value):
        raise ValueError('owner/name must be a nonempty DNS label')
    return value


def sql_identifier(value):
    # libpq interprets database strings containing '=' or URI schemes as DSNs.
    if not isinstance(value, str) or not re.fullmatch(r'[a-z_][a-z0-9_]{0,62}', value):
        raise ValueError('database must be a plain SQL identifier, never a connection string')
    return value


def scratch_environment(value, owner):
    suffix = identifier(owner).replace('-', '')[:16]
    if value not in ('fit-' + suffix, 'scratch-' + suffix):
        raise ValueError('scratch ENVIRONMENT must be fit- or scratch- plus the first 16 non-hyphen owner characters')
    return value


class Kubernetes:
    def __init__(self, binary, kubeconfig, context):
        config = Path(kubeconfig).resolve()
        if not config.is_file() or not context:
            raise ValueError('explicit existing kubeconfig and context required')
        version = json.loads(subprocess.run([binary, 'version', '--client', '-o', 'json'], check=True, text=True, capture_output=True).stdout)['clientVersion']['gitVersion']
        match = re.fullmatch(r'v(\d+)\.(\d+)\.\d+(?:[-+].*)?', version)
        target = json.loads((Path(__file__).resolve().parents[2] / 'platform/versions.lock.json').read_text())['kubernetes']
        major, minor = map(int, target.split('.')[:2])
        if not match or int(match[1]) != major or abs(int(match[2]) - minor) > 1:
            raise ValueError('kubectl must be within one minor of target Kubernetes ' + target + '; use platform/toolchain.lock.json')
        self.prefix = [binary, '--kubeconfig', str(config), '--context', context, '--request-timeout=30s']

    def run(self, *args, input=None, timeout=180):
        # Capture output: Secret values and connection strings never become command logs.
        result = subprocess.run([*self.prefix, *args], input=input, text=True,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout)
        if result.returncode:
            raise RuntimeError('kubectl operation failed: ' + args[0] + ' (inspect owned cluster privately)')
        return result.stdout

    def get(self, resource, name, namespace=None):
        args = ['get', resource, name, '-o', 'json']
        if namespace:
            args += ['-n', namespace]
        return json.loads(self.run(*args))

    def owned(self, resource, name, owner, namespace=None):
        value = self.get(resource, name, namespace)
        if value.get('metadata', {}).get('labels', {}).get(LABEL) != identifier(owner):
            raise ValueError('refusing resource without exact scratch ownership label: ' + resource + '/' + name)
        return value

    def disposable_owned(self, resource, name, owner, namespace=None):
        value = self.owned(resource, name, owner, namespace)
        if value.get('metadata', {}).get('annotations', {}).get(DISPOSABLE) != 'true':
            raise ValueError('refusing resource without explicit disposable-drill annotation: ' + resource + '/' + name)
        return value

    def apply(self, value):
        return self.run('apply', '-f', '-', input=json.dumps(value))
