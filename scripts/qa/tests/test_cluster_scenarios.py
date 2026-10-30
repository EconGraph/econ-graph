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
    for ns, needle, answer in state.get('sql', []):
        if ns == namespace and needle in sql:
            if answer == 'COUNT_IDS':
                answer = str(len(re.findall(r"'[^']*'", sql.partition(' IN ')[2])))
            print(answer.replace('|', '\x1f'))
            break
    else:
        sys.exit(f'stub has no answer for {namespace}/{pod}: {sql[:80]}')
elif args[0] == 'wait':
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
        cases = {
            'no Ingress': {'ingress': [meta('old', 'econ-graph')]},
            'no IngressClass': {'ingressclass': [meta('nginx')]},
            'no ingress-nginx': {'pods': [meta('controller', 'kube-system',
                                               {'app.kubernetes.io/name': 'ingress-nginx'})]},
            'only HTTPRoutes': {'grpcroutes': [meta('rpc', 'econ-graph')]},
            'HTTPRoutes accepted': {'httproutes': [accepted_route('frontend'),
                                                   accepted_route('detached', ok='False')]},
        }
        for failing, change in cases.items():
            with self.subTest(failing):
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


class DataSurvivesRedeployTest(ClusterScenarioTest):
    def data_state(self, counts, crawler_ready=0):
        return {'resources': {'deployments': [meta('crawler-worker', 'econ-graph', {'app': 'crawler-worker'},
                                                   status={'readyReplicas': crawler_ready})]},
                'sql': [['econ-graph', TABLES_SQL, 'data_points|t\neconomic_series|t\ncountries|f'],
                        ['econ-graph', COUNTS_SQL, counts]]}

    BEFORE = 'data_points|1000|123456\neconomic_series|10|987\ncountries|3|55'

    def redeploy_to(self, counts):
        after = self.dir / 'after.json'
        after.write_text(json.dumps(self.data_state(counts)))
        return {'QA_DATA_REDEPLOY_COMMAND': f'echo redeployed >> "{self.dir}/redeploy.log"; '
                                            f'cp "{after}" "{self.state}"'}

    def test_unchanged_rows_pass(self):
        self.set_state(**self.data_state(self.BEFORE))
        code, result, checks = self.run_scenario('data-survives-redeploy', extra=self.redeploy_to(self.BEFORE))
        self.assertEqual(code, 0, result)
        self.assertEqual(checks, {'crawler paused': True, 'before': True, 'redeploy': True, 'same tables': True,
                                  'countries unchanged': True, 'data_points unchanged': True,
                                  'economic_series unchanged': True})
        self.assertEqual((self.dir / 'redeploy.log').read_text(), 'redeployed\n')
        # Once before and once after the redeploy, which may have recreated the cluster.
        self.assertEqual((self.dir / 'helper.log').read_text().split(), [CONTEXT, CONTEXT])
        sql = [json.loads(line)[-1] for line in self.log.read_text().splitlines() if '"exec"' in line]
        counts = [s for s in sql if COUNTS_SQL in s]
        self.assertEqual(len(counts), 2)
        # The id for tables that have one, the whole row for one that doesn't.
        self.assertIn('hashtextextended("data_points".id::text, 0)', counts[0])
        self.assertIn('hashtextextended("countries"::text, 0)', counts[0])
        self.assert_only_the_context()

    def test_a_changed_count_or_checksum_or_table_fails(self):
        for label, after, failing in (
                ('count', self.BEFORE.replace('1000|', '999|'), {'data_points unchanged'}),
                ('checksum', self.BEFORE.replace('|987', '|988'), {'economic_series unchanged'}),
                ('table', 'data_points|1000|123456\neconomic_series|10|987', {'same tables'})):
            with self.subTest(label):
                self.set_state(**self.data_state(self.BEFORE))
                code, result, checks = self.run_scenario('data-survives-redeploy', extra=self.redeploy_to(after))
                self.assertEqual(code, 1, result)
                self.assertEqual({k for k, ok in checks.items() if not ok}, failing)

    def test_a_running_crawler_fails_before_the_redeploy(self):
        self.set_state(**self.data_state(self.BEFORE, crawler_ready=1))
        code, result, checks = self.run_scenario('data-survives-redeploy', extra=self.redeploy_to(self.BEFORE))
        self.assertEqual(code, 1)
        self.assertEqual(checks, {'crawler paused': False})
        self.assertFalse((self.dir / 'redeploy.log').exists())
        code, result, checks = self.run_scenario('data-survives-redeploy', extra=dict(
            self.redeploy_to(self.BEFORE), QA_DATA_ALLOW_RUNNING_CRAWLER='1'))
        self.assertEqual(code, 0, result)

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


class BranchIsolationTest(ClusterScenarioTest):
    BRANCH = 'eg-br-feature'

    def branch_state(self, production='0|0', other='0|0', annotation='u-1', users='1', mode='primary',
                     own_data=False):
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
                [self.BRANCH, 'SELECT user_id FROM chart_annotations', annotation],
                [self.BRANCH, 'FROM users', f'1|{users}'],
                ['econ-graph', 'FROM users', production],
                ['eg-br-other', 'FROM users', other],
            ]}

    def run_branch(self, site, **extra):
        env = {'QA_BRANCH_NAMESPACE': self.BRANCH, 'QA_USERNAME': 'qa-alice', 'QA_PASSWORD': PASSWORD}
        env.update(extra)
        return self.run_scenario('branch-isolation', site.base, 'lab', env)

    def test_isolated_branch_passes_and_cleans_up(self):
        self.set_state(**self.branch_state())
        with StubSite() as site:
            code, result, checks = self.run_branch(site)
        self.assertEqual(code, 0, result)
        self.assertEqual(checks, {'branch namespace': True, 'real data': True, 'sign in': True,
                                  'stored in the branch': True, 'not in production': True,
                                  'not in eg-br-other': True, 'clean up': True})
        self.assertEqual(site.annotations, {})
        self.assertEqual(site.data_graphql_tokens, [None])
        sql = ' '.join(json.loads(line)[-1] for line in self.log.read_text().splitlines() if '"exec"' in line)
        self.assertIn(f"'{SERIES_ID}'", sql)
        self.assert_only_the_context()

    def test_rows_in_production_or_another_branch_fail(self):
        for where, change, failing in (('production', {'production': '1|1'}, 'not in production'),
                                       ('other branch', {'other': '0|1'}, 'not in eg-br-other')):
            with self.subTest(where):
                self.set_state(**self.branch_state(**change))
                with StubSite() as site:
                    code, result, checks = self.run_branch(site)
                self.assertEqual(code, 1, result)
                self.assertEqual({k for k, ok in checks.items() if not ok}, {failing})
                self.assertEqual(site.annotations, {})

    def test_an_annotation_missing_from_the_branch_fails(self):
        self.set_state(**self.branch_state(annotation=''))
        with StubSite() as site:
            code, _, checks = self.run_branch(site)
        self.assertEqual(code, 1)
        self.assertFalse(checks['stored in the branch'])
        self.assertTrue(checks['clean up'])
        self.assertEqual(site.annotations, {})

    def test_a_branch_with_its_own_data_fails(self):
        for label, state in (('local mode', self.branch_state(mode='local')),
                             ('own Postgres', self.branch_state(own_data=True))):
            with self.subTest(label):
                self.set_state(**state)
                with StubSite() as site:
                    code, _, checks = self.run_branch(site)
                self.assertEqual(code, 1)
                self.assertFalse(checks['branch namespace'])

    def test_series_missing_from_the_primary_fail(self):
        state = self.branch_state()
        state['sql'][0][2] = '0'
        self.set_state(**state)
        with StubSite() as site:
            code, _, checks = self.run_branch(site)
        self.assertEqual(code, 1)
        self.assertEqual({k for k, ok in checks.items() if not ok}, {'real data'})

    def test_refuses_an_unlabelled_or_primary_namespace(self):
        self.set_state(**self.branch_state())
        with StubSite() as site:
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
