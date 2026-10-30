#!/usr/bin/env python3
# qa-scenario: gateway-routes-only
# qa-mutates: no
# qa-profiles: lab, production
# qa-timeout: 300
# qa-description: No Ingress or ingress-nginx left in the cluster; every route is an HTTPRoute that its Gateway accepted (v4.1.0 exit criterion 5).
"""No ingress-nginx left; all routes are HTTPRoutes. Read-only.

Inputs: QA_KUBE_CONTEXT, the kind context (e.g. kind-econ-graph); skipped without it.

Checks, across all namespaces:
  no Ingress           `get ingress -A` lists nothing.
  no IngressClass      no IngressClass is left.
  no ingress-nginx     nothing of ingress-nginx's: no namespace, deployment, daemonset,
                       service or pod labelled app.kubernetes.io/name=ingress-nginx or
                       named like it, and no such ClusterRole, ClusterRoleBinding or
                       admission webhook configuration (the cluster-scoped leftovers of
                       deleting only its namespace).
  only HTTPRoutes      no GRPCRoute, TLSRoute, TCPRoute or UDPRoute (a type the cluster
                       doesn't have counts as none).
  HTTPRoutes accepted  at least one HTTPRoute, and every parent of every HTTPRoute reports
                       Accepted and ResolvedRefs true.
"""

from qa import Kube, KubeError, check, fail, finish

kube = Kube()


def names(items):
    return [f'{i["metadata"].get("namespace", "")}/{i["metadata"]["name"]}'.lstrip('/') for i in items]


NAMESPACED = {'deployments', 'daemonsets', 'services', 'pods'}


def is_ingress_nginx(item):
    labels = item['metadata'].get('labels') or {}
    return 'ingress-nginx' in item['metadata']['name'] or 'ingress-nginx' in (
        labels.get('app.kubernetes.io/name'), labels.get('app.kubernetes.io/part-of'))


def none_left(name, items, what):
    found = names(items)
    check(name, not found, f'none in {kube.context}' if not found
          else f'{len(found)} {what}: {", ".join(found[:5])}', found=found[:20])


try:
    none_left('no Ingress', kube.items('ingress', '-A'), 'Ingress objects')
    none_left('no IngressClass', kube.items('ingressclass'), 'IngressClasses')
    leftovers = []
    for kind in ('namespaces', 'deployments', 'daemonsets', 'services', 'pods', 'clusterroles',
                 'clusterrolebindings', 'validatingwebhookconfigurations', 'mutatingwebhookconfigurations'):
        items = kube.items(kind, '-A') if kind in NAMESPACED else kube.items(kind)
        leftovers += [f'{kind}:{name}' for item, name in zip(items, names(items)) if is_ingress_nginx(item)]
    check('no ingress-nginx', not leftovers, f'none in {kube.context}' if not leftovers
          else f'{len(leftovers)} left: {", ".join(leftovers[:5])}', found=leftovers[:20])

    others = []
    for kind in ('grpcroutes', 'tlsroutes', 'tcproutes', 'udproutes'):
        others += [f'{kind}:{n}' for n in names(kube.items(kind, '-A'))]
    check('only HTTPRoutes', not others, 'no other route kinds' if not others
          else f'{len(others)} other routes: {", ".join(others[:5])}')

    routes = kube.items('httproutes', '-A')
    unaccepted = []
    for route in routes:
        parents = (route.get('status') or {}).get('parents') or []
        conditions = [{c['type']: c['status'] for c in p.get('conditions') or []} for p in parents]
        if not conditions or any(c.get('Accepted') != 'True' or c.get('ResolvedRefs') != 'True'
                                 for c in conditions):
            unaccepted.append(names([route])[0])
    check('HTTPRoutes accepted', bool(routes) and not unaccepted,
          f'{len(routes)} HTTPRoutes, all accepted' if routes and not unaccepted
          else 'no HTTPRoute at all' if not routes
          else f'{len(unaccepted)} of {len(routes)} not accepted: {", ".join(unaccepted[:5])}',
          routes=len(routes))
except KubeError as error:
    fail('cluster', str(error))

finish()
