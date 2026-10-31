#!/usr/bin/env python3
"""Render real pinned charts and validate manifests against their published schemas."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import urllib.request
import warnings
import yaml
import jsonschema

ROOT = Path(__file__).resolve().parents[2]
LOCK = json.loads((ROOT / 'platform/versions.lock.json').read_text())
ROOTS = ['sources', *['addons/' + x for x in LOCK['charts']], 'network', 'databases',
         'application', 'data-schema', 'observability', 'optional/iceberg', 'secrets', 'clusters/threadripper']


def run(args, **kwargs):
    return subprocess.run(args, check=True, text=True, stdout=subprocess.PIPE, **kwargs).stdout


class KubernetesLoader(yaml.SafeLoader):
    pass


KubernetesLoader.yaml_implicit_resolvers = {
    key: [(tag, pattern) for tag, pattern in rules if tag not in
          ('tag:yaml.org,2002:value', 'tag:yaml.org,2002:timestamp')]
    for key, rules in yaml.SafeLoader.yaml_implicit_resolvers.items()}


def load_all(text):
    return [x for x in yaml.load_all(text, Loader=KubernetesLoader) if isinstance(x, dict) and x.get('kind')]


def omit_null(value):
    # Kubernetes decoding treats explicit null optional object fields as absent.
    if isinstance(value, dict):
        return {k: omit_null(v) for k, v in value.items() if v is not None}
    if isinstance(value, list):
        return [omit_null(v) for v in value]
    return value


def normalize(schema):
    if isinstance(schema, list):
        return [normalize(x) for x in schema]
    if not isinstance(schema, dict):
        return schema
    result = {k: normalize(v) for k, v in schema.items()}
    if schema.get('x-kubernetes-int-or-string') is True or schema.get('format') == 'int-or-string':
        result.pop('type', None)
        result['anyOf'] = [{'type': 'integer'}, {'type': 'string'}]
    return result


def policy(documents):
    clusters = [d for d in documents if d['kind'] == 'Cluster']
    assert {c['metadata']['name'] for c in clusters} == {'econgraph-app', 'econgraph-data', 'econgraph-keycloak'}
    for cluster in clusters:
        spec = cluster['spec']
        assert spec['instances'] >= 2, 'CNPG needs at least two instances'
        assert spec['affinity']['podAntiAffinityType'] == 'required', 'CNPG replicas must spread'
        assert spec['imageName'] == LOCK['postgres_image'], 'PostgreSQL must use verified image digest'
    for d in documents:
        if d['kind'] == 'Deployment' and d['metadata']['name'] in ['econ-graph-backend', 'econ-graph-data-backend', 'econ-graph-frontend', 'crawler-worker']:
            container = d['spec']['template']['spec']['containers'][0]
            assert re.fullmatch(r'[^\s]+@sha256:[0-9a-f]{64}', container['image']), 'image digest interpolation is invalid'
            assert container['imagePullPolicy'] != 'Never', 'Talos must pull feature images'
        if d['kind'] == 'ObjectStore':
            assert 'retentionPolicy' not in d['spec'], 'automatic backup deletion is forbidden'
        if d['kind'] == 'ScheduledBackup':
            assert d['spec']['backupOwnerReference'] == 'none', 'backup records must survive schedules'
        if d['kind'] == 'Secret':
            assert 'sops' in d, 'plaintext Secret is forbidden'
    dag = [d for d in documents if d['kind'] == 'Kustomization' and d['apiVersion'].startswith('kustomize.toolkit')]
    graph = {d['metadata']['name']: [x['name'] for x in d['spec'].get('dependsOn', [])] for d in dag}
    def visit(name, parents):
        assert name in graph, 'unknown dependency ' + name
        assert name not in parents, 'cyclic Flux dependencies'
        for child in graph[name]:
            visit(child, parents | {name})
    for d in dag:
        visit(d['metadata']['name'], set())
        assert d['spec']['prune'] is False and d['spec']['deletionPolicy'] == 'Orphan', 'primary cleanup prohibited'
        path = ROOT / d['spec']['path']
        assert path.is_dir(), 'missing Flux source path ' + str(path)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cache', type=Path, required=True)
    parser.add_argument('--fetch', action='store_true', help='download only checksum-locked public artifacts')
    parser.add_argument('--helm', default='helm')
    parser.add_argument('--kubectl', default='kubectl')
    parser.add_argument('--report', type=Path)
    args = parser.parse_args()
    args.cache.mkdir(parents=True, exist_ok=True)
    charts = args.cache / 'charts'
    charts.mkdir(exist_ok=True)
    for entry in [*LOCK['charts'].values(), *LOCK['assets'].values()]:
        destination = (charts / entry['package']) if 'package' in entry else args.cache / entry['file']
        if not destination.exists() and args.fetch:
            if entry['url'].startswith('oci://'):
                output = run([args.helm, 'pull', entry['url'], '--version', entry['version'], '--destination', str(charts)], stderr=subprocess.STDOUT)
                assert entry['oci_digest'] in output, 'OCI manifest digest mismatch'
            else:
                with urllib.request.urlopen(entry['url'], timeout=120) as response:
                    content = response.read(64 * 1024 * 1024 + 1)
                assert len(content) <= 64 * 1024 * 1024, 'artifact too large'
                destination.write_bytes(content)
        if not destination.exists():
            raise RuntimeError(f'missing locked artifact {destination}; use --fetch')
        assert hashlib.sha256(destination.read_bytes()).hexdigest() == entry['sha256'], f'artifact checksum mismatch: {destination}'
    settings = yaml.safe_load((ROOT / 'platform/clusters/threadripper/settings.example.yaml').read_text())['data']
    # Validate exact final digest interpolation too: no inherited Kustomize tag suffix.
    for key in ['BACKEND_IMAGE', 'FRONTEND_IMAGE', 'CRAWLER_IMAGE']:
        settings[key] = 'ghcr.io/econgraph/' + key.lower() + '@sha256:' + 'a' * 64
    documents, chart_docs = [], []
    for root in ROOTS:
        rendered = run([args.kubectl, 'kustomize', str(ROOT / 'platform' / root)])
        for document in load_all(rendered):
            if document.get('metadata', {}).get('annotations', {}).get('kustomize.toolkit.fluxcd.io/substitute') != 'disabled':
                text = re.sub(r'\$\{([A-Z][A-Z0-9_]*)\}', lambda m: str(settings[m[1]]), yaml.safe_dump(document))
                document = yaml.safe_load(text)
            documents.append(document)
    for name, entry in LOCK['charts'].items():
        release = next(d for d in documents if d['kind'] == 'HelmRelease' and d['metadata']['name'] == name)
        if name == 'cilium':
            assert release['spec']['releaseName'] == 'cilium'
            assert release['spec']['targetNamespace'] == release['spec']['storageNamespace'] == 'kube-system', 'Cilium bootstrap and Flux Helm history must share identity'
        assert release['spec']['chart']['spec']['version'] == entry['version'], 'unlocked chart version'
        with tempfile.NamedTemporaryFile(mode='w', suffix='.yaml') as values:
            values.write(yaml.safe_dump(release['spec'].get('values', {})))
            values.flush()
            rendered = run([args.helm, 'template', release['metadata']['name'], str(charts / entry['package']),
                            '--namespace', entry['namespace'], '--kube-version', LOCK['kubernetes'],
                            '--include-crds', '--values', values.name])
        if name == 'cilium':
            assert 'enable-host-legacy-routing: \"true\"' in rendered, 'Talos HostDNS requires Cilium legacy host routing'
        chart_docs.extend(load_all(rendered))
    schemas = {}
    swagger = normalize(json.loads((args.cache / 'kubernetes-openapi.json').read_text()))
    swagger['definitions']['io.k8s.apimachinery.pkg.api.resource.Quantity']['type'] = ['string', 'number']
    for name, schema in swagger['definitions'].items():
        for gvk in schema.get('x-kubernetes-group-version-kind', []):
            version = (gvk['group'] + '/' if gvk['group'] else '') + gvk['version']
            schemas[(version, gvk['kind'])] = {'$ref': '#/definitions/' + name}
    definitions = [*chart_docs, *load_all((args.cache / 'flux-install.yaml').read_text()),
                   *load_all((args.cache / 'cilium-pool-crd.yaml').read_text()),
                   *load_all((args.cache / 'cilium-l2-crd.yaml').read_text())]
    for definition in definitions:
        if definition['kind'] == 'CustomResourceDefinition':
            for version in definition['spec']['versions']:
                schemas[(definition['spec']['group'] + '/' + version['name'], definition['spec']['names']['kind'])] = normalize(version['schema']['openAPIV3Schema'])
    validated = 0
    with warnings.catch_warnings():
        warnings.simplefilter('ignore', DeprecationWarning)
        resolver = jsonschema.RefResolver.from_schema(swagger)
    for document in [*documents, *chart_docs]:
        key = document['apiVersion'], document['kind']
        if key not in schemas:
            raise RuntimeError('no published schema for ' + str(key))
        schema = schemas[key]
        validator = jsonschema.Draft4Validator(schema, resolver=resolver)
        errors = list(validator.iter_errors(omit_null(document)))
        if errors:
            prefix = f"{key} {document['metadata']['name']}"
            raise RuntimeError(prefix + ': ' + '; '.join('/'.join(map(str, e.path)) + ' ' + e.message for e in errors[:4]))
        validated += 1
    # Policies operate on one rendering of each logical source, excluding chart-generated dependencies.
    unique = {(d['apiVersion'], d['kind'], d['metadata'].get('namespace'), d['metadata']['name']): d for d in documents}
    policy(list(unique.values()))
    result = {'status': 'passed', 'kubernetes': LOCK['kubernetes'], 'charts': len(LOCK['charts']),
              'schema_validated_resources': validated, 'live_cluster_contacted': False,
              'limits': ['No Talos boot, CNPG restore, failover, node-drain or release E2E performed.']}
    if args.report:
        args.report.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    try:
        main()
    except (AssertionError, RuntimeError, subprocess.CalledProcessError) as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
