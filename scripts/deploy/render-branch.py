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


def validate_data_image(image):
    """Disposable copies may run a release or immutable candidate, never latest."""
    if not image or not re.fullmatch(
            r'[A-Za-z0-9][A-Za-z0-9./:_-]*(?:@sha256:[0-9a-f]{64}|'
            r':(?:v\d+\.\d+\.\d+(?:[-.][A-Za-z0-9.-]+)?|sha-[0-9a-f]{40}))', image):
        raise ValueError('--data-image requires a release tag, full sha-<40 hex commit> tag, or sha256 digest')
    return image


def pull_policy(image):
    return 'IfNotPresent' if re.search(r'(?:@sha256:[0-9a-f]{64}|:sha-[0-9a-f]{40})$', image) else 'Always'


def render(branch, backend_image, frontend_image, local_data=False, data_image=None):
    if local_data:
        validate_data_image(data_image)
    elif data_image:
        raise ValueError('--data-image is only allowed with --local-data')
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
            # Restored queues are not permission to crawl the public internet.
            # Explicitly scale a local worker only for a bounded fixture run.
            d['spec']['replicas'] = 0 if name == 'crawler-worker' else 1
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
                c['imagePullPolicy'] = pull_policy(c['image'])
            if name in ('crawler-worker', 'crawler-migrate') and c['name'] in ('crawler-worker', 'crawler-migrate'):
                c['image'] = data_image
                c['imagePullPolicy'] = pull_policy(data_image)
                env_set(c, 'CRAWLER_SCHEDULER', 'false')
            if c['name'] == 'backend':
                env_set(c, 'OIDC_ISSUER', origin + '/idp/realms/econ-graph')
                env_set(c, 'DATA_GRAPHQL_URL', None if local_data else
                        'http://branch-data-proxy:8080/data/graphql')
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
        # Only this fixed-upstream proxy may reach the shared listener. A branch
        # backend must not select another branch/production route with Host.
        docs.append(policy('proxy-to-data-gateway', {'matchLabels': {'app': 'branch-data-proxy'}},
                           egress=[{'to': [proxy], 'ports': [{'port': 10080, 'protocol': 'TCP'}]}]))
        docs += data_proxy(frontend_image, host)
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


def data_proxy(image, host):
    # The branch frontend image already contains nginx. Use no browser assets,
    # arbitrary destination, credentials or caller-supplied upstream headers.
    config = '''pid /tmp/nginx.pid;
events { worker_connections 128; }
http {
  client_body_temp_path /tmp/client_temp;
  proxy_temp_path /tmp/proxy_temp;
  fastcgi_temp_path /tmp/fastcgi_temp;
  uwsgi_temp_path /tmp/uwsgi_temp;
  scgi_temp_path /tmp/scgi_temp;
  access_log /dev/stdout;
  error_log /dev/stderr;
  server {
    listen 8080;
    client_max_body_size 1m;
    location = /health { return 200 'ok'; }
    location = /data/graphql {
      proxy_pass http://econ-graph-gateway-proxy.envoy-gateway-system.svc.cluster.local/data/graphql;
      proxy_set_header Host HOST;
      proxy_set_header Authorization "";
      proxy_set_header Cookie "";
      proxy_set_header Forwarded "";
      proxy_set_header X-Forwarded-Host "";
      proxy_set_header X-Forwarded-For "";
      proxy_connect_timeout 5s;
      proxy_read_timeout 30s;
    }
    location / { return 403; }
  }
}
'''.replace('Host HOST;', 'Host ' + host + ';')
    return [object_('ConfigMap', 'branch-data-proxy', data={'nginx.conf': config}),
            object_('Service', 'branch-data-proxy', {'type': 'ClusterIP',
                'selector': {'app': 'branch-data-proxy'}, 'ports': [{'port': 8080, 'targetPort': 8080}]}),
            {'apiVersion': 'apps/v1', 'kind': 'Deployment', 'metadata': {'name': 'branch-data-proxy'},
             'spec': {'replicas': 1, 'selector': {'matchLabels': {'app': 'branch-data-proxy'}},
                'template': {'metadata': {'labels': {'app': 'branch-data-proxy'}}, 'spec': {
                    'automountServiceAccountToken': False,
                    'securityContext': {'runAsUser': 101, 'runAsNonRoot': True,
                                        'seccompProfile': {'type': 'RuntimeDefault'}},
                    'containers': [{'name': 'proxy', 'image': image, 'imagePullPolicy': pull_policy(image),
                        'command': ['nginx', '-c', '/branch/nginx.conf', '-g', 'daemon off;'],
                        'ports': [{'containerPort': 8080}],
                        'securityContext': {'allowPrivilegeEscalation': False,
                            'readOnlyRootFilesystem': True, 'capabilities': {'drop': ['ALL']}},
                        'resources': {'requests': {'cpu': '10m', 'memory': '32Mi'},
                                      'limits': {'cpu': '100m', 'memory': '64Mi'}},
                        'readinessProbe': {'httpGet': {'path': '/health', 'port': 8080}},
                        'volumeMounts': [{'name': 'config', 'mountPath': '/branch', 'readOnly': True},
                                         {'name': 'tmp', 'mountPath': '/tmp'}]}],
                    'volumes': [{'name': 'config', 'configMap': {'name': 'branch-data-proxy'}},
                                {'name': 'tmp', 'emptyDir': {'sizeLimit': '64Mi'}}]}}}}]


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
