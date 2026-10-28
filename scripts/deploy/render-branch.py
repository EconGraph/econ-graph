#!/usr/bin/env python3
"""Render disposable branch resources without contacting Kubernetes."""
import argparse
import hashlib
from pathlib import Path
import re
import sys

import yaml

ROOT = Path(__file__).resolve().parents[2]
SOURCES = [
    'configmap.yaml', 'backend-deployment.yaml', 'backend-service.yaml',
    'frontend-deployment.yaml', 'frontend-service.yaml', 'app-postgres-init.yaml',
    'app-postgres.yaml', 'keycloak/postgres.yaml', 'keycloak/deployment.yaml',
    'keycloak/realm-import-job.yaml', 'gateway/routes/httproutes.yaml',
    'gateway/routes/ratelimit-policies.yaml',
]


def source_objects():
    docs = []
    for source in SOURCES:
        for obj in yaml.safe_load_all((ROOT / 'k8s/manifests' / source).read_text()):
            if obj:
                obj['metadata'].pop('namespace', None)
                docs.append(obj)
    return docs


def identity(branch):
    if not branch or len(branch) > 256 or any(ord(c) < 32 for c in branch):
        raise ValueError('branch must contain 1 to 256 printable characters')
    prefix = re.sub('[^a-z0-9]+', '-', branch.lower()).strip('-')[:32].rstrip('-') or 'branch'
    slug = prefix + '-' + hashlib.sha256(branch.encode()).hexdigest()[:10]
    return slug, 'econ-graph-br-' + slug


def env_set(container, name, value):
    container['env'] = [e for e in container.get('env', []) if e['name'] != name]
    if value is not None:
        container['env'].append({'name': name, 'value': value})


def object_(kind, name, spec=None, **extra):
    obj = {'apiVersion': 'v1', 'kind': kind, 'metadata': {'name': name}}
    if spec is not None:
        obj['spec'] = spec
    obj.update(extra)
    return obj


def render(branch, backend_image, frontend_image, local_data=False, data_image=None):
    slug, namespace = identity(branch)
    host = slug + '.localhost'
    origin = 'http://' + host
    labels = {'econgraph.io/env': 'branch', 'econgraph.io/branch': slug,
              'econgraph.io/data-mode': 'local' if local_data else 'primary',
              'econgraph.io/gateway-access': 'true'}
    docs = list(yaml.safe_load_all((ROOT/'k8s/base/application.yaml').read_text()))
    if local_data:
        for source in ['postgres-init.yaml', 'postgres-deployment.yaml', 'postgres.yaml',
                       'crawler-worker.yaml', 'crawler-migrate-job.yaml']:
            docs.extend(d for d in yaml.safe_load_all((ROOT/'k8s/manifests'/source).read_text()) if d)
    for d in docs:
        d['metadata']['namespace'] = namespace
        kind, name = d['kind'], d['metadata']['name']
        if kind == 'Service':
            d['spec']['type'] = 'ClusterIP'
            for port in d['spec']['ports']:
                port.pop('nodePort', None)
        if kind == 'ConfigMap' and name == 'econ-graph-config':
            d['data']['CORS_ORIGIN'] = origin
            d['data']['CHART_API_SERVICE_URL'] = ''
            # Legacy browser values are not frontend build configuration.
            for key in list(d['data']):
                if key.startswith('REACT_APP_'):
                    d['data'].pop(key)
        if kind == 'HTTPRoute':
            d['spec']['hostnames'] = [host]
            if name == 'econ-graph-data-graphql' and not local_data:
                d['spec']['rules'][0]['backendRefs'][0]['namespace'] = 'econ-graph'
        if kind not in ('Deployment', 'StatefulSet', 'Job'):
            continue
        # Prometheus runs in econ-graph, which branch-isolation keeps out, and the
        # backend's alert rules have no namespace filter: branch pods are not scrape targets.
        template_meta = d['spec']['template'].get('metadata', {})
        annotations = {key: value for key, value in template_meta.get('annotations', {}).items()
                       if not key.startswith('prometheus.io/')}
        if annotations:
            template_meta['annotations'] = annotations
        else:
            template_meta.pop('annotations', None)
        pod = d['spec']['template']['spec']
        pod['automountServiceAccountToken'] = False
        if kind == 'Deployment':
            d['spec']['replicas'] = 1
        if kind == 'StatefulSet':
            pod.pop('nodeSelector', None)
            # Local-path volumes need the same chown initContainer as production,
            # but never bind an existing production PV.
            for claim in d['spec']['volumeClaimTemplates']:
                claim['spec'].pop('volumeName', None)
                claim['spec']['storageClassName'] = 'standard'
                claim['spec']['resources']['requests']['storage'] = '2Gi'
        for c in pod.get('initContainers', []) + pod.get('containers', []):
            for i, arg in enumerate(c.get('args', [])):
                if isinstance(arg, str):
                    c['args'][i] = arg.replace('postgres-service.econ-graph.svc.cluster.local',
                                              'data-postgres-service' if local_data else 'app-postgres-service')
            if c['name'] in ('backend', 'frontend'):
                c['image'] = backend_image if c['name'] == 'backend' else frontend_image
                c['imagePullPolicy'] = 'IfNotPresent'
            if name in ('crawler-worker', 'crawler-migrate') and c['name'] in ('crawler-worker', 'crawler-migrate'):
                c['image'] = data_image
                c['imagePullPolicy'] = 'IfNotPresent'
                env_set(c, 'CRAWLER_SCHEDULER', 'false')
            if c['name'] == 'backend':
                env_set(c, 'OIDC_ISSUER', origin + '/idp/realms/econ-graph')
                env_set(c, 'DATA_GRAPHQL_URL', None if local_data else
                        'http://econ-graph-gateway-proxy.envoy-gateway-system.svc.cluster.local/data/graphql')
                env_set(c, 'DATA_GRAPHQL_HOST', None if local_data else host)
                if not local_data:
                    env_set(c, 'DATA_DATABASE_URL', None)
            if c['name'] == 'frontend':
                c['env'] = []
            if name in ('keycloak', 'keycloak-realm-import'):
                c['env'] = [e for e in c.get('env', []) if not e['name'].startswith(('KC_GOOGLE', 'KC_FACEBOOK'))]
                if c['name'] == 'keycloak':
                    env_set(c, 'KC_HOSTNAME', origin + '/idp')
                if c['name'] in ('keycloak', 'render-realm'):
                    env_set(c, 'KC_WEB_BASE_URL', origin)
                    env_set(c, 'KC_WEB_E2E_BASE_URL', origin)
    docs += [object_('ResourceQuota', 'branch-budget', {'hard': {
        'requests.cpu': '4', 'requests.memory': '8Gi', 'limits.cpu': '12',
        'limits.memory': '16Gi', 'requests.storage': '8Gi', 'persistentvolumeclaims': '4', 'pods': '16'}}),
        object_('LimitRange', 'branch-defaults', {'limits': [{'type': 'Container',
            'default': {'cpu': '500m', 'memory': '512Mi'},
            'defaultRequest': {'cpu': '50m', 'memory': '64Mi'}}]})]
    # NamespaceSelector and podSelector in ONE peer are ANDed. Separate peers
    # would accidentally admit all pods from every namespace.
    own = {'namespaceSelector': {'matchLabels': {'kubernetes.io/metadata.name': namespace}}}
    proxy = {'namespaceSelector': {'matchLabels': {'kubernetes.io/metadata.name': 'envoy-gateway-system'}},
             'podSelector': {'matchLabels': {'app.kubernetes.io/name': 'envoy',
                 'gateway.envoyproxy.io/owning-gateway-name': 'econ-graph-gateway',
                 'gateway.envoyproxy.io/owning-gateway-namespace': 'econ-graph-gateway'}}}
    dns = {'namespaceSelector': {'matchLabels': {'kubernetes.io/metadata.name': 'kube-system'}},
           'podSelector': {'matchLabels': {'k8s-app': 'kube-dns'}}}
    def policy(name, selector, ingress=None, egress=None):
        spec = {'podSelector': selector, 'policyTypes': []}
        for typ, rules in [('Ingress', ingress), ('Egress', egress)]:
            if rules is not None:
                spec['policyTypes'].append(typ)
                spec[typ.lower()] = rules
        p = object_('NetworkPolicy', name, spec)
        p['apiVersion'] = 'networking.k8s.io/v1'
        return p
    docs += [policy('branch-isolation', {'matchExpressions': [{'key': 'app', 'operator': 'NotIn', 'values': ['branch-network-deny']}]}, ingress=[{'from': [own]}], egress=[
                {'to': [own]}, {'to': [dns], 'ports': [{'port': 53, 'protocol': 'UDP'}, {'port': 53, 'protocol': 'TCP'}]}]),
             policy('gateway-to-app', {'matchExpressions': [{'key': 'app', 'operator': 'In',
                    'values': ['econ-graph-backend', 'econ-graph-frontend', 'keycloak']}]},
                    ingress=[{'from': [proxy], 'ports': [{'port': p, 'protocol': 'TCP'} for p in [3000, 8080, 9876]]}])]
    docs.append(policy('branch-probe-deny', {'matchLabels': {'app': 'branch-network-deny'}}, ingress=[], egress=[]))
    if not local_data:
        docs.append(policy('backend-to-data-gateway', {'matchLabels': {'app': 'econ-graph-backend'}},
                           egress=[{'to': [proxy], 'ports': [{'port': 10080, 'protocol': 'TCP'}]}]))
    if local_data:
        docs.append(policy('crawler-public-https', {'matchLabels': {'app': 'crawler-worker'}},
            egress=[{'to': [{'ipBlock': {'cidr': '0.0.0.0/0', 'except': [
                '0.0.0.0/8', '10.0.0.0/8', '100.64.0.0/10', '127.0.0.0/8',
                '169.254.0.0/16', '172.16.0.0/12', '192.168.0.0/16', '224.0.0.0/4', '240.0.0.0/4']}}],
                'ports': [{'port': 443, 'protocol': 'TCP'}]}]))
    for d in docs:
        d['metadata']['namespace'] = namespace
    ns = object_('Namespace', namespace)
    ns['metadata'].update(labels=labels, annotations={'econgraph.io/branch-name': branch})
    docs.insert(0, ns)
    return docs


def grant(branch):
    slug, namespace = identity(branch)
    return {'apiVersion': 'gateway.networking.k8s.io/v1beta1', 'kind': 'ReferenceGrant',
            'metadata': {'name': 'data-graphql-from-' + slug, 'namespace': 'econ-graph',
                         'labels': {'econgraph.io/branch': slug}},
            'spec': {'from': [{'group': 'gateway.networking.k8s.io', 'kind': 'HTTPRoute', 'namespace': namespace}],
                     'to': [{'group': '', 'kind': 'Service', 'name': 'econ-graph-backend-service'}]}}


def refresh_example():
    """Keep the inspectable Kustomize example on the exact runtime renderer."""
    base = source_objects()
    docs = render('example', 'econ-graph-backend:branch-example', 'econ-graph-frontend:branch-example')
    objects = {(d['kind'], d['metadata']['name']): d for d in docs}
    patches = []
    for original in base:
        rendered = objects[(original['kind'], original['metadata']['name'])]
        operations = [{'op': 'add', 'path': '/metadata/namespace', 'value': rendered['metadata']['namespace']}]
        for field in ['spec', 'data']:
            if field in rendered and original.get(field) != rendered[field]:
                operations.append({'op': 'replace', 'path': '/' + field, 'value': rendered[field]})
        group, _, version = original['apiVersion'].rpartition('/')
        target = {'kind': original['kind'], 'name': original['metadata']['name'],
                  'version': version or original['apiVersion']}
        if group:
            target['group'] = group
        patches.append({'target': target, 'patch': yaml.safe_dump(operations, sort_keys=False)})
    base_keys = {(d['kind'], d['metadata']['name']) for d in base}
    extras = [d for d in docs if (d['kind'], d['metadata']['name']) not in base_keys]
    overlay = ROOT/'k8s/overlays/branch'
    (overlay/'extras.yaml').write_text(yaml.safe_dump_all(extras, sort_keys=False))
    kustomization = {'apiVersion': 'kustomize.config.k8s.io/v1beta1', 'kind': 'Kustomization',
                     'resources': ['../../base', 'extras.yaml'], 'patches': patches}
    (overlay/'kustomization.yaml').write_text(
        '# Example branch overlay. deploy-branch.sh renders the actual branch identity and images.\n' +
        yaml.safe_dump(kustomization, sort_keys=False))


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--refresh-base', action='store_true')
    p.add_argument('--branch', default='example')
    p.add_argument('--backend-image', default='econ-graph-backend:branch-example')
    p.add_argument('--frontend-image', default='econ-graph-frontend:branch-example')
    p.add_argument('--local-data', action='store_true')
    p.add_argument('--data-image')
    args = p.parse_args()
    if args.refresh_base:
        (ROOT/'k8s/base/application.yaml').write_text('# Generated by scripts/deploy/render-branch.py --refresh-base; parity tested against manifests.\n' +
                                                    yaml.safe_dump_all(source_objects(), sort_keys=False))
        refresh_example()
        return
    if args.local_data and not args.data_image:
        p.error('--local-data requires a tagged --data-image')
    try:
        yaml.safe_dump_all(render(args.branch, args.backend_image, args.frontend_image,
                                  args.local_data, args.data_image), sys.stdout, sort_keys=False)
    except ValueError as e:
        p.error(str(e))


if __name__ == '__main__':
    main()
