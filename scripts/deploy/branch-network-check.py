#!/usr/bin/env python3
"""Fail closed unless disposable branch NetworkPolicies enforce actual connections."""
import ipaddress
import subprocess


def verify(namespace, local_data, k, lookup, apply):
    label = {'econgraph.io/network-probe': 'true'}
    def pod(name, app, target=False):
        command = ['sh', '-ec', 'mkdir -p /tmp/www; echo branch-network-canary >/tmp/www/index.html; exec httpd -f -p 8080 -h /tmp/www'] if target else ['sleep', '600']
        container = {'name': 'probe', 'image': 'busybox:1.38', 'command': command,
            'resources': {'requests': {'cpu': '10m', 'memory': '16Mi'}, 'limits': {'cpu': '50m', 'memory': '32Mi'}},
            'securityContext': {'runAsNonRoot': True, 'runAsUser': 1000, 'allowPrivilegeEscalation': False,
                                'readOnlyRootFilesystem': True, 'capabilities': {'drop': ['ALL']}},
            'volumeMounts': [{'name': 'tmp', 'mountPath': '/tmp'}]}
        if target:
            container['readinessProbe'] = {'tcpSocket': {'port': 8080}, 'periodSeconds': 1}
        elif app in ('econ-graph-backend', 'branch-data-proxy'):
            # Never become an endpoint of the branch backend Service during a redeploy.
            container['readinessProbe'] = {'exec': {'command': ['false']}, 'periodSeconds': 1}
        return {'apiVersion': 'v1', 'kind': 'Pod', 'metadata': {'name': name, 'namespace': namespace,
                'labels': dict(label, app=app)}, 'spec': {'automountServiceAccountToken': False,
                'restartPolicy': 'Never', 'containers': [container], 'volumes': [{'name': 'tmp', 'emptyDir': {}}],
                'securityContext': {'seccompProfile': {'type': 'RuntimeDefault'}}}}
    names = ['branch-network-target', 'branch-network-allow', 'branch-network-deny', 'branch-network-backend',
             'branch-network-proxy']
    def connect(podname, address, port):
        ipaddress.ip_address(address)
        return k('-n', namespace, 'exec', podname, '--', 'nc', '-z', '-w', '3', address, str(port),
                 capture=True, check=False).returncode == 0
    def service(name, ns):
        obj = lookup('service', name, ns)
        if not obj or not obj.get('spec', {}).get('clusterIP'):
            raise ValueError(f'network preflight requires Service {ns}/{name}')
        ip = obj['spec']['clusterIP']
        ipaddress.ip_address(ip)
        return ip
    try:
        k('-n', namespace, 'delete', 'pods', *names, '--ignore-not-found', '--wait=true')
        apply([pod(names[0], names[0], True), pod(names[1], names[1]),
               pod(names[2], names[2]), pod(names[3], 'econ-graph-backend'), pod(names[4], 'branch-data-proxy')])
        for name in names:
            k('-n', namespace, 'wait', '--for=jsonpath={.status.phase}=Running', 'pod/'+name, '--timeout=120s')
        k('-n', namespace, 'wait', '--for=condition=Ready', 'pod/'+names[0], '--timeout=60s')
        target = lookup('pod', names[0], namespace)['status']['podIP']
        if not connect(names[1], target, 8080):
            raise ValueError('positive canary connection failed; isolation cannot be verified')
        if connect(names[2], target, 8080):
            raise ValueError('NetworkPolicy is not enforced; verify the CNI on a scratch cluster before deploying branches')
        primary = service('econ-graph-backend-service', 'econ-graph')
        database = service('data-postgres-service', 'econ-graph')
        proxy = service('econ-graph-gateway-proxy', 'envoy-gateway-system')
        # Direct primary access is denied for every role. Only the fixed-host
        # proxy may reach the shared gateway, and only in shared-data mode.
        for client in [names[1], names[3], names[4]]:
            if connect(client, primary, 9876) or connect(client, database, 5432):
                raise ValueError('a branch pod can directly reach primary data services; refusing deployment')
        if connect(names[1], proxy, 80):
            raise ValueError('a non-backend branch pod can reach the shared gateway')
        if connect(names[3], proxy, 80):
            raise ValueError('backend can reach the shared gateway and select another branch Host')
        proxy_open = connect(names[4], proxy, 80)
        if proxy_open != (not local_data):
            raise ValueError('fixed proxy gateway access does not match the selected branch data mode')
    finally:
        # Cleanup is always namespace-scoped, even if any probe failed.
        k('-n', namespace, 'delete', 'pods', *names, '--ignore-not-found', '--wait=false', check=False)
