#!/usr/bin/env python3
"""Offline tests for the scenarios that read the kind cluster: branch-isolation,
data-survives-redeploy and gateway-routes-only.

A stand-in for lib/kind-kubectl.sh hands the scenarios a stand-in kubectl, which answers
`get`, `exec ... psql` and `wait` from a JSON state file and logs every call. No cluster,
Docker or database is reached.

  python3 -m unittest discover -s scripts/qa/tests -v
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from test_run_scenarios import PASSWORD, SERIES_ID, StubSite, run_main  # noqa: E402

CONTEXT = 'kind-qa-test'

FAKE_KUBECTL = r'''#!/usr/bin/env python3
"""Answers from the state file in FAKE_KUBE_STATE; logs argv to FAKE_KUBE_LOG."""
import json, os, re, sys
args = sys.argv[1:]
with open(os.environ['FAKE_KUBE_LOG'], 'a') as log:
    log.write(json.dumps(args) + '\n')
state = json.load(open(os.environ['FAKE_KUBE_STATE']))
if args[:4] != ['--kubeconfig', os.environ['FAKE_KUBECONFIG'], '--context', os.environ['FAKE_CONTEXT']]:
    sys.exit('called without the explicit kubeconfig and context')
args = args[4:]

def option(name):
    return args[args.index(name) + 1] if name in args else None

if args[0] == 'get':
    kind = args[1]
    if kind in state.get('missing', []):
        sys.exit(f'error: the server doesn\'t have a resource type "{kind}"')
    items = state.get('resources', {}).get(kind, [])
    namespace = option('-n')
    if namespace:
        items = [i for i in items if i['metadata'].get('namespace') == namespace]
    if option('-l'):
        key, _, value = option('-l').partition('=')
        items = [i for i in items if i['metadata'].get('labels', {}).get(key) == value]
    if option('--field-selector'):
        name = option('--field-selector').partition('=')[2]
        items = [i for i in items if i['metadata']['name'] == name]
    print(json.dumps({'items': items}))
elif args[0] == 'exec':
    namespace, pod, sql = option('-n'), args[3], args[-1]
    expected = {'postgresql-0': ('postgresql', 'econ_graph'), 'app-postgres-0': ('postgres', 'econ_graph_app')}
    if (option('-c'), option('-d'), option('-U')) != (*expected.get(pod, ('?', '?')), 'postgres'):
        sys.exit(f'no container {option("-c")} with database {option("-d")} in pod {pod}')
    for rule in state.get('sql', []):
        ns, needle, answer = rule
        if ns == namespace and needle in sql:
            if isinstance(answer, list):
                # A sequence: each call takes the next answer, the last one repeats.
                if len(answer) > 1:
                    rule[2] = answer[1:]
                    json.dump(state, open(os.environ['FAKE_KUBE_STATE'], 'w'))
                answer = answer[0]
            if answer == 'ERROR':
                sys.exit(f'error: pod {pod} is not running')
            if answer == 'COUNT_IDS':
                answer = str(len(re.findall(r"'[^']*'", sql.partition(' IN ')[2])))
            print(answer.replace('|', '\x1f'))
            break
    else:
        sys.exit(f'stub has no answer for {namespace}/{pod}: {sql[:80]}')
elif args[0] == 'rollout':
    pass
else:
    sys.exit(f'stub cannot {args[0]}')
'''


def meta(name, namespace=None, labels=None, **rest):
    item = {'metadata': {'name': name, **({'namespace': namespace} if namespace else {}),
                         **({'labels': labels} if labels else {})}}
    item.update(rest)
    return item


def accepted_route(name, namespace='econ-graph', ok='True'):
    return meta(name, namespace, status={'parents': [{'conditions': [
        {'type': 'Accepted', 'status': ok}, {'type': 'ResolvedRefs', 'status': 'True'}]}]})


class ClusterScenarioTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self.tmp.name)
        self.state = self.dir / 'state.json'
        self.log = self.dir / 'kubectl.log'
        kubectl, helper = self.dir / 'kubectl', self.dir / 'helper.sh'
        kubectl.write_text(FAKE_KUBECTL)
        helper.write_text(f'#!/usr/bin/env bash\necho "$1" >> "{self.dir}/helper.log"\n'
                          f'echo "{kubectl}"\necho "{self.dir}/kubeconfig"\n')
        for path in (kubectl, helper):
            path.chmod(0o755)
        self.env = {'PATH': os.environ['PATH'], 'QA_WORK_ROOT': str(self.dir),
                    'QA_LAB_TARGET_OK': 'localhost,127.0.0.1,lab.example',
                    'QA_KUBE_CONTEXT': CONTEXT, 'QA_KIND_KUBECTL_HELPER': str(helper),
                    'FAKE_KUBE_STATE': str(self.state), 'FAKE_KUBE_LOG': str(self.log),
                    'FAKE_KUBECONFIG': f'{self.dir}/kubeconfig', 'FAKE_CONTEXT': CONTEXT}

    def tearDown(self):
        self.tmp.cleanup()

    def set_state(self, **state):
        self.state.write_text(json.dumps(state))

    def run_scenario(self, name, target='http://lab.example', profile='lab', extra=None):
        out = self.dir / 'report.json'
        env = dict(self.env, **(extra or {}))
        code, stdout, stderr = run_main(['--target', target, '--profile', profile, '--only', name,
                                         '--json', str(out)], env)
        result = json.loads(out.read_text())['scenarios'][0]
        checks = {c['check']: c['ok'] for c in result['evidence'] if 'check' in c}
        return code, result, checks

    def assert_only_the_context(self):
        self.assertEqual(set((self.dir / 'helper.log').read_text().split()), {CONTEXT})


class GatewayRoutesOnlyTest(ClusterScenarioTest):
    def clean(self, **changes):
        state = {'resources': {'namespaces': [meta('econ-graph'), meta('envoy-gateway-system')],
                               'httproutes': [accepted_route('frontend'), accepted_route('graphql')]},
                 'missing': ['tlsroutes', 'tcproutes', 'udproutes']}
        state['resources'].update(changes)
        return state

    def test_passes_with_only_accepted_httproutes(self):
        self.set_state(**self.clean())
        code, result, checks = self.run_scenario('gateway-routes-only')
        self.assertEqual(code, 0, result)
        self.assertEqual(checks, {'no Ingress': True, 'no IngressClass': True, 'no ingress-nginx': True,
                                  'only HTTPRoutes': True, 'HTTPRoutes accepted': True})
        self.assert_only_the_context()
        calls = [json.loads(line) for line in self.log.read_text().splitlines()]
        self.assertTrue(all(call[0] == '--kubeconfig' for call in calls))
        self.assertNotIn('exec', [call[4] for call in calls])

    def test_runs_in_production_read_only(self):
        self.set_state(**self.clean())
        code, result, _ = self.run_scenario('gateway-routes-only', 'https://econgraph.com', 'production')
        self.assertEqual(code, 0, result)

    def test_each_leftover_fails(self):
        nginx = {'app.kubernetes.io/name': 'ingress-nginx'}
        cases = [
            ('no Ingress', {'ingress': [meta('old', 'econ-graph')]}),
            ('no IngressClass', {'ingressclass': [meta('nginx')]}),
            ('no ingress-nginx', {'pods': [meta('controller', 'kube-system', nginx)]}),
            ('no ingress-nginx', {'validatingwebhookconfigurations': [meta('ingress-nginx-admission')]}),
            ('no ingress-nginx', {'clusterroles': [meta('nginx', labels={'app.kubernetes.io/part-of':
                                                                          'ingress-nginx'})]}),
            ('no ingress-nginx', {'deployments': [meta('ingress-nginx-controller', 'web')]}),
            ('only HTTPRoutes', {'grpcroutes': [meta('rpc', 'econ-graph')]}),
            ('HTTPRoutes accepted', {'httproutes': [accepted_route('frontend'),
                                                    accepted_route('detached', ok='False')]}),
        ]
        for failing, change in cases:
            with self.subTest(failing, change=next(iter(change))):
                self.set_state(**self.clean(**change))
                code, result, checks = self.run_scenario('gateway-routes-only')
                self.assertEqual(code, 1, result)
                self.assertEqual({k for k, ok in checks.items() if not ok}, {failing})

    def test_an_ingress_nginx_namespace_fails(self):
        state = self.clean()
        state['resources']['namespaces'].append(meta('ingress-nginx'))
        self.set_state(**state)
        code, _, checks = self.run_scenario('gateway-routes-only')
        self.assertEqual(code, 1)
        self.assertFalse(checks['no ingress-nginx'])

    def test_no_httproute_at_all_fails(self):
        self.set_state(**self.clean(httproutes=[]))
        code, result, checks = self.run_scenario('gateway-routes-only')
        self.assertEqual(code, 1)
        self.assertIn('no HTTPRoute', json.dumps(result['evidence']))

    def test_a_route_without_status_fails(self):
        self.set_state(**self.clean(httproutes=[meta('new', 'econ-graph')]))
        code, _, checks = self.run_scenario('gateway-routes-only')
        self.assertEqual(code, 1)
        self.assertFalse(checks['HTTPRoutes accepted'])

    def test_skips_without_a_context(self):
        env = dict(self.env)
        del env['QA_KUBE_CONTEXT']
        self.env = env
        code, result, _ = self.run_scenario('gateway-routes-only')
        self.assertEqual(code, 0)
        self.assertEqual(result['status'], 'skip')
        self.assertIn('QA_KUBE_CONTEXT', result['reason'])

    def test_a_cluster_it_cannot_reach_fails(self):
        helper = self.dir / 'helper.sh'
        helper.write_text('#!/usr/bin/env bash\necho "error: cannot reach kind cluster" >&2\nexit 1\n')
        code, result, _ = self.run_scenario('gateway-routes-only')
        self.assertEqual(code, 1)
        self.assertIn('cannot reach', result['reason'])


class KindKubectlHelperTest(unittest.TestCase):
    def test_refuses_a_context_that_is_not_kind(self):
        helper = Path(__file__).resolve().parents[1] / 'lib' / 'kind-kubectl.sh'
        for context in ('', 'microk8s', 'kind-', 'kind-a b', 'prod-kind-x'):
            with self.subTest(context=context):
                done = subprocess.run([str(helper), context], capture_output=True, text=True, timeout=30)
                self.assertEqual(done.returncode, 1)
                self.assertIn('expected a kind context', done.stderr)
                self.assertEqual(done.stdout, '')


TABLES_SQL = 'information_schema.tables'
COUNTS_SQL = 'UNION ALL'


QUEUE_SQL = 'FROM crawl_queue'


class DataSurvivesRedeployTest(ClusterScenarioTest):
    BEFORE = 'data_points|1000|123456\neconomic_series|10|987\ndatasets|4|44\ncrawl_queue|7|70'
    TABLES = 'data_points\neconomic_series\ndatasets\ncrawl_queue'

    def worker(self, uid='w1', scheduler='false', args=None, ready=True, value_from=False):
        env = [] if scheduler is None else [
            {'name': 'CRAWLER_SCHEDULER', **({'valueFrom': {'configMapKeyRef': {'name': 'c', 'key': 'k'}}}
                                             if value_from else {'value': scheduler})}]
        container = {'name': 'crawler-worker', 'env': env, **({'args': args} if args else {})}
        pod = meta(f'crawler-worker-{uid}', 'econ-graph', {'app': 'crawler-worker'},
                   spec={'containers': [container]}, status={'containerStatuses': [{'ready': ready}]})
        pod['metadata']['uid'] = uid
        return pod

    def data_state(self, counts, queued='0', workers=None):
        return {'resources': {'pods': [self.worker()] if workers is None else workers},
                'sql': [['econ-graph', QUEUE_SQL, queued],
                        ['econ-graph', TABLES_SQL, self.TABLES],
                        ['econ-graph', COUNTS_SQL, counts]]}

    def redeploy_to(self, counts, **after_state):
        after = self.dir / 'after.json'
        after.write_text(json.dumps(self.data_state(counts, **after_state)))
        return {'QA_DATA_REDEPLOY_COMMAND': f'echo redeployed >> "{self.dir}/redeploy.log"; '
                                            f'cp "{after}" "{self.state}"'}

    def redeployed(self):
        return (self.dir / 'redeploy.log').exists()

    def run_data(self, after=None, before_state=None, **after_state):
        self.set_state(**(before_state or self.data_state(self.BEFORE)))
        return self.run_scenario('data-survives-redeploy',
                                 extra=self.redeploy_to(self.BEFORE if after is None else after, **after_state))

    def assert_inconclusive(self, code, result, when, says):
        self.assertEqual(code, 0, result)
        self.assertEqual(result['status'], 'skip')
        self.assertTrue(result['reason'].startswith(f'inconclusive: the crawler is active {when}'),
                        result['reason'])
        self.assertIn(says, result['reason'])
        self.assertIn('quiet window', result['reason'])

    def test_unchanged_rows_pass(self):
        # crawl_queue changed too: the worker's own queue is not compared.
        code, result, checks = self.run_data(self.BEFORE.replace('crawl_queue|7|70', 'crawl_queue|3|31'))
        self.assertEqual(code, 0, result)
        self.assertEqual(checks, {'quiet before the first reading': True, 'quiet after the first reading': True,
                                  'before': True, 'redeploy': True, 'quiet before the second reading': True,
                                  'quiet after the second reading': True, 'same tables': True,
                                  'data_points unchanged': True, 'economic_series unchanged': True,
                                  'datasets unchanged': True})
        self.assertEqual((self.dir / 'redeploy.log').read_text(), 'redeployed\n')
        # Once before and once after the redeploy, which may have recreated the cluster.
        self.assertEqual((self.dir / 'helper.log').read_text().split(), [CONTEXT, CONTEXT])
        sql = [json.loads(line)[-1] for line in self.log.read_text().splitlines() if '"exec"' in line]
        counts = [s for s in sql if COUNTS_SQL in s]
        self.assertEqual(len(counts), 2)
        # The whole row, so a changed value shows, not only an added or removed id.
        for table in ('data_points', 'economic_series', 'datasets'):
            self.assertIn(f'hashtextextended("{table}"::text, 0)', counts[0])
        self.assertNotIn('.id::text', counts[0])
        # The queue is read around each reading, and counts every kind of unfinished job.
        queue = [s for s in sql if QUEUE_SQL in s]
        self.assertEqual(len(queue), 4)
        for state in ("'pending'", "'processing'", "'retrying'", 'locked_by IS NOT NULL'):
            self.assertIn(state, queue[0])
        self.assert_only_the_context()

    def test_a_stopped_crawler_and_every_off_value_are_quiet(self):
        for label, workers in (('no worker pods', []),
                               ('0', [self.worker(scheduler='0')]),
                               ('Off', [self.worker(scheduler=' Off ')]),
                               ('no', [self.worker(scheduler='no')]),
                               ('args win', [self.worker(scheduler='true', args=['--scheduler=false'])]),
                               ('args pair', [self.worker(scheduler=None, args=['--scheduler', 'f'])])):
            with self.subTest(label):
                state = self.data_state(self.BEFORE, workers=workers)
                code, result, _ = self.run_data(before_state=state, workers=workers)
                self.assertEqual((code, result['status']), (0, 'pass'), result)

    def test_a_changed_count_checksum_or_table_fails(self):
        for label, after, failing in (
                ('count', self.BEFORE.replace('1000|', '999|'), {'data_points unchanged'}),
                ('checksum', self.BEFORE.replace('|987', '|988'), {'economic_series unchanged'}),
                ('table', 'data_points|1000|123456\neconomic_series|10|987\ndatasets|4|44', {'same tables'})):
            with self.subTest(label):
                code, result, checks = self.run_data(after)
                self.assertEqual((code, result['status']), (1, 'fail'), result)
                self.assertEqual({k for k, ok in checks.items() if not ok}, failing)

    def test_an_active_crawler_before_the_redeploy_is_inconclusive(self):
        for label, state, says in (
                ('scheduler on', dict(workers=[self.worker(scheduler='true')]), 'CRAWLER_SCHEDULER=false'),
                ('scheduler unset', dict(workers=[self.worker(scheduler=None)]), 'CRAWLER_SCHEDULER=false'),
                ('from a ConfigMap', dict(workers=[self.worker(value_from=True)]), 'CRAWLER_SCHEDULER=false'),
                ('args override', dict(workers=[self.worker(args=['--scheduler', 'true'])]), 'CRAWLER_SCHEDULER=false'),
                ('one of two', dict(workers=[self.worker(), self.worker('w2', scheduler='true')]), 'crawler-worker-w2'),
                ('starting', dict(workers=[self.worker(ready=False)]), 'still starting'),
                ('jobs queued', dict(queued='3'), '3 pending, processing, retrying or locked jobs')):
            with self.subTest(label):
                code, result, checks = self.run_data(before_state=self.data_state(self.BEFORE, **state))
                self.assert_inconclusive(code, result, 'before the first reading', says)
                self.assertFalse(self.redeployed())
                self.assertNotIn(False, checks.values())

    def test_jobs_queued_during_the_first_reading_are_inconclusive(self):
        code, result, _ = self.run_data(before_state=self.data_state(self.BEFORE, queued=['0', '2']))
        self.assert_inconclusive(code, result, 'after the first reading', '2 pending')
        self.assertFalse(self.redeployed())

    def test_an_active_crawler_after_the_redeploy_is_inconclusive_not_a_failure(self):
        # The rows changed too, but with the crawler active that proves nothing either way.
        changed = self.BEFORE.replace('1000|', '1200|')
        for label, state, when, says in (
                ('scheduler on', dict(workers=[self.worker('w2', scheduler='true')]), 'before the second reading',
                 'CRAWLER_SCHEDULER=false'),
                ('jobs queued', dict(queued='5'), 'before the second reading', '5 pending'),
                ('worker still starting', dict(workers=[self.worker('w2', ready=False)]),
                 'before the second reading', 'still starting'),
                ('jobs during the reading', dict(queued=['0', '4']), 'after the second reading', '4 pending')):
            with self.subTest(label):
                code, result, checks = self.run_data(changed, **state)
                self.assert_inconclusive(code, result, when, says)
                self.assertTrue(self.redeployed())
                self.assertNotIn('data_points unchanged', checks)
                (self.dir / 'redeploy.log').unlink()

    def test_startup_writes_after_a_worker_restart_are_inconclusive(self):
        datasets_changed = self.BEFORE.replace('datasets|4|44', 'datasets|4|45')
        code, result, checks = self.run_data(datasets_changed, workers=[self.worker('w2')])
        self.assertEqual((code, result['status']), (0, 'skip'), result)
        self.assertTrue(result['reason'].startswith('inconclusive: only datasets changed'), result['reason'])
        self.assertNotIn(False, checks.values())

    def test_startup_tables_changed_without_a_restart_fail(self):
        code, result, checks = self.run_data(self.BEFORE.replace('datasets|4|44', 'datasets|4|45'))
        self.assertEqual((code, result['status']), (1, 'fail'), result)
        self.assertEqual({k for k, ok in checks.items() if not ok}, {'datasets unchanged'})

    def test_other_tables_changed_after_a_restart_fail(self):
        after = self.BEFORE.replace('datasets|4|44', 'datasets|4|45').replace('1000|', '999|')
        code, result, checks = self.run_data(after, workers=[self.worker('w2')])
        self.assertEqual((code, result['status']), (1, 'fail'), result)
        self.assertEqual({k for k, ok in checks.items() if not ok}, {'datasets unchanged', 'data_points unchanged'})

    def test_inconclusive_does_not_satisfy_require_pass(self):
        self.set_state(**self.data_state(self.BEFORE, queued='1'))
        env = dict(self.env, **self.redeploy_to(self.BEFORE))
        code, _, _ = run_main(['--target', 'http://lab.example', '--profile', 'lab', '--only',
                               'data-survives-redeploy', '--require-pass', 'data-survives-redeploy'], env)
        self.assertEqual(code, 1)

    def test_a_failed_redeploy_fails(self):
        self.set_state(**self.data_state(self.BEFORE))
        code, result, _ = self.run_scenario('data-survives-redeploy', extra={'QA_DATA_REDEPLOY_COMMAND': 'exit 3'})
        self.assertEqual(code, 1)
        self.assertIn('exited 3', result['reason'])

    def test_skips_without_a_redeploy(self):
        self.set_state(**self.data_state(self.BEFORE))
        code, result, _ = self.run_scenario('data-survives-redeploy')
        self.assertEqual((code, result['status']), (0, 'skip'))
        self.assertFalse(self.log.exists())

    def test_lab_only(self):
        code, _, _ = run_main(['--target', 'http://lab.example', '--profile', 'ci', '--only',
                               'data-survives-redeploy'], self.env)
        self.assertEqual(code, 2)


class BranchSite(StubSite):
    """The stub site, able to answer /graphql's search differently from /data/graphql's, and
    to lose the response of a create it carried out."""

    def __init__(self, own_series_id=None, lose_create=False):
        super().__init__()
        self.own_series_id, self.lose_create = own_series_id, lose_create

    def post(self, h):
        self.path = h.path
        super().post(h)

    def graphql(self, request, user):
        answer = super().graphql(request, user)
        if 'searchSeries' in request['query'] and self.path == '/graphql' and self.own_series_id:
            answer['data']['searchSeries']['series'][0]['id'] = self.own_series_id
        if 'createAnnotation' in request['query'] and self.lose_create:
            return {'errors': [{'message': 'upstream connect error'}]}
        return answer


ANNOTATIONS_SQL = 'FROM chart_annotations a WHERE a.title'


class BranchIsolationTest(ClusterScenarioTest):
    BRANCH = 'eg-br-feature'

    def branch_state(self, production='', other='', stored='a0|u-1|t', mode='primary', own_data=False):
        branch_labels = {'econgraph.io/env': 'branch', 'econgraph.io/data-mode': mode}
        return {
            'resources': {
                'namespaces': [meta('econ-graph'), meta(self.BRANCH, labels=branch_labels),
                               meta('eg-br-other', labels={'econgraph.io/env': 'branch',
                                                           'econgraph.io/data-mode': 'primary'})],
                'statefulsets': [meta('postgresql', self.BRANCH)] if own_data else [],
                'pods': [meta('app-postgres-0', 'eg-br-other')],
            },
            'sql': [
                ['econ-graph', 'FROM economic_series', 'COUNT_IDS'],
                [self.BRANCH, ANNOTATIONS_SQL, stored],
                ['econ-graph', ANNOTATIONS_SQL, production],
                ['eg-br-other', ANNOTATIONS_SQL, other],
            ]}

    def run_branch(self, site, **extra):
        env = {'QA_BRANCH_NAMESPACE': self.BRANCH, 'QA_USERNAME': 'qa-alice', 'QA_PASSWORD': PASSWORD}
        env.update(extra)
        return self.run_scenario('branch-isolation', site.base, 'lab', env)

    def failing(self, checks):
        return {k for k, ok in checks.items() if ok is False}

    def test_isolated_branch_passes_and_cleans_up(self):
        self.set_state(**self.branch_state())
        with BranchSite() as site:
            code, result, checks = self.run_branch(site)
        self.assertEqual(code, 0, result)
        self.assertEqual(checks, {'branch namespace': True, 'real data': True,
                                  'branch backend reads the data plane': True, 'sign in': True,
                                  'stored in the branch': True, 'not in production': True,
                                  'not in eg-br-other': True, 'clean up': True})
        self.assertEqual(site.annotations, {})
        self.assertEqual(site.data_graphql_tokens, [None])
        sql = ' '.join(json.loads(line)[-1] for line in self.log.read_text().splitlines() if '"exec"' in line)
        self.assertIn(f"'{SERIES_ID}'", sql)
        self.assert_only_the_context()

    def test_an_annotation_in_production_or_another_branch_fails(self):
        for where, change, failing in (('production', {'production': 'a0|u-9|t'}, 'not in production'),
                                       ('other branch', {'other': 'a0|u-9|t'}, 'not in eg-br-other')):
            with self.subTest(where):
                self.set_state(**self.branch_state(**change))
                with BranchSite() as site:
                    code, result, checks = self.run_branch(site)
                self.assertEqual(code, 1, result)
                self.assertEqual(self.failing(checks), {failing})
                self.assertEqual(site.annotations, {})

    def test_an_annotation_or_its_user_missing_from_the_branch_fails(self):
        for label, stored in (('no annotation', ''), ('no user', 'a0|u-1|f'), ('two', 'a0|u-1|t\na1|u-1|t')):
            with self.subTest(label):
                self.set_state(**self.branch_state(stored=stored))
                with BranchSite() as site:
                    code, _, checks = self.run_branch(site)
                self.assertEqual(code, 1)
                self.assertEqual(self.failing(checks), {'stored in the branch'})
                self.assertEqual(site.annotations, {})

    def test_another_branch_that_cannot_be_read_is_skipped_not_failed(self):
        self.set_state(**self.branch_state(other='ERROR'))
        with BranchSite() as site:
            code, result, checks = self.run_branch(site)
        self.assertEqual(code, 0, result)
        self.assertIsNone(checks['not in eg-br-other'])

    def test_production_that_cannot_be_read_fails(self):
        self.set_state(**self.branch_state(production='ERROR'))
        with BranchSite() as site:
            code, _, checks = self.run_branch(site)
        self.assertEqual(code, 1)
        self.assertEqual(self.failing(checks), {'branch app database'})
        self.assertEqual(site.annotations, {})

    def test_a_lost_create_response_is_still_cleaned_up(self):
        self.set_state(**self.branch_state())
        with BranchSite(lose_create=True) as site:
            code, _, checks = self.run_branch(site)
        self.assertEqual(code, 1)
        self.assertTrue(checks['clean up'])
        self.assertEqual(site.annotations, {})

    def test_a_branch_backend_that_answers_differently_fails(self):
        self.set_state(**self.branch_state())
        with BranchSite(own_series_id='22222222-2222-2222-2222-222222222222') as site:
            code, _, checks = self.run_branch(site)
        self.assertEqual(code, 1)
        self.assertEqual(self.failing(checks), {'branch backend reads the data plane'})

    def test_a_branch_with_its_own_data_fails(self):
        for label, state in (('local mode', self.branch_state(mode='local')),
                             ('own Postgres', self.branch_state(own_data=True))):
            with self.subTest(label):
                self.set_state(**state)
                with BranchSite() as site:
                    code, _, checks = self.run_branch(site)
                self.assertEqual(code, 1)
                self.assertEqual(self.failing(checks), {'branch namespace'})

    def test_series_missing_from_the_primary_fail(self):
        state = self.branch_state()
        state['sql'][0][2] = '0'
        self.set_state(**state)
        with BranchSite() as site:
            code, _, checks = self.run_branch(site)
        self.assertEqual(code, 1)
        self.assertEqual(self.failing(checks), {'real data'})

    def test_refuses_an_unlabelled_or_primary_namespace(self):
        self.set_state(**self.branch_state())
        with BranchSite() as site:
            for namespace, reason in (('econ-graph', 'primary namespace'), ('eg-br-nope', 'not a namespace')):
                with self.subTest(namespace):
                    code, result, _ = self.run_branch(site, QA_BRANCH_NAMESPACE=namespace)
                    self.assertEqual(code, 1)
                    self.assertIn(reason, result['reason'])

    def test_skips_without_a_branch(self):
        code, result, _ = self.run_scenario('branch-isolation')
        self.assertEqual((code, result['status']), (0, 'skip'))
        self.assertIn('QA_BRANCH_NAMESPACE', result['reason'])


if __name__ == '__main__':
    unittest.main()
