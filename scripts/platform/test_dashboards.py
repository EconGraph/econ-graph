#!/usr/bin/env python3
"""Static guards for the platform Grafana dashboards; no Kubernetes or Grafana access.

Each dashboard in platform/observability/dashboards must parse, be provisioned by the
observability kustomization with the sidecar label and Flux substitution disabled, reference
only datasource uids the monitoring HelmRelease provisions, and query only econgraph_* /
crawler_* metrics that the Rust code registers.
"""
import json
from pathlib import Path
import re
import unittest
import yaml

ROOT = Path(__file__).resolve().parents[2]
DASHBOARDS = ROOT / 'platform/observability/dashboards'
SELECTOR = re.compile(r'([a-zA-Z_:][a-zA-Z0-9_:]*)[{\[]')
HISTOGRAM_SUFFIX = re.compile(r'_(bucket|sum|count)$')


def provisioned_uids():
    release = yaml.safe_load((ROOT / 'platform/addons/monitoring/release.yaml').read_text())
    grafana = release['spec']['values'].get('grafana', {})
    # kube-prometheus-stack provisions its Prometheus datasource with this uid by default.
    uids = {grafana.get('sidecar', {}).get('datasources', {}).get('uid', 'prometheus')}
    uids |= {ds['uid'] for ds in grafana.get('additionalDataSources', [])}
    return uids


def walk(value):
    if isinstance(value, dict):
        yield value
        for child in value.values():
            yield from walk(child)
    elif isinstance(value, list):
        for child in value:
            yield from walk(child)


def rust_sources():
    return '\n'.join(p.read_text(errors='ignore') for p in (ROOT / 'backend/crates').rglob('*.rs')
                     if 'target' not in p.parts)


class DashboardTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.files = sorted(DASHBOARDS.glob('*.json'))
        cls.dashboards = {p.name: json.loads(p.read_text()) for p in cls.files}

    def test_dashboards_exist_with_unique_uids(self):
        self.assertTrue(self.files)
        uids = [d['uid'] for d in self.dashboards.values()]
        self.assertEqual(len(uids), len(set(uids)), uids)

    def test_every_dashboard_is_provisioned_for_the_sidecar_without_substitution(self):
        kustomization = yaml.safe_load((ROOT / 'platform/observability/kustomization.yaml').read_text())
        provisioned = set()
        for generator in kustomization.get('configMapGenerator', []):
            files = [f for f in generator.get('files', []) if f.startswith('dashboards/')]
            if not files:
                continue
            options = generator.get('options', {})
            self.assertEqual(generator.get('namespace'), 'monitoring')
            self.assertEqual(options.get('labels', {}).get('grafana_dashboard'), '1')
            self.assertEqual(options.get('annotations', {}).get('kustomize.toolkit.fluxcd.io/substitute'), 'disabled')
            provisioned |= {Path(f).name for f in files}
        self.assertTrue(kustomization['generatorOptions']['disableNameSuffixHash'])
        self.assertEqual(provisioned, set(self.dashboards))

    def test_datasources_are_provisioned(self):
        uids = provisioned_uids()
        self.assertIn('loki', uids)
        for name, dashboard in self.dashboards.items():
            variables = {v['name'] for v in dashboard.get('templating', {}).get('list', []) if v.get('type') == 'datasource'}
            for node in walk(dashboard):
                ds = node.get('datasource')
                if ds is None:
                    continue
                self.assertIsInstance(ds, dict, f'{name}: datasource must be an object with a uid')
                uid = ds['uid']
                if uid.startswith('$'):
                    self.assertIn(uid.strip('${}'), variables, name)
                else:
                    self.assertIn(uid, uids, name)

    def test_queried_metrics_are_registered_in_rust(self):
        rust = rust_sources()
        for name, dashboard in self.dashboards.items():
            for node in walk(dashboard):
                if node.get('datasource', {}).get('type') != 'prometheus':
                    continue
                query = node.get('expr') or node.get('definition') or ''
                for metric in SELECTOR.findall(query):
                    if not metric.startswith(('econgraph_', 'crawler_')):
                        continue
                    base = HISTOGRAM_SUFFIX.sub('', metric)
                    self.assertTrue(f'"{metric}"' in rust or f'"{base}"' in rust,
                                    f'{name}: {metric} is not registered in backend/crates')


if __name__ == '__main__':
    unittest.main()
