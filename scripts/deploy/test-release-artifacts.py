#!/usr/bin/env python3
"""Offline contract checks: every app image deploy.sh applies ships as a release artifact.

Reads manifests, release.yml and the release loader's image table. No Docker,
registry, cluster or upstream API is contacted. Run with python3; requires PyYAML.
"""
from pathlib import Path
import re
import unittest

import yaml

ROOT = Path(__file__).resolve().parents[2]


class ReleaseArtifactsTest(unittest.TestCase):
    def setUp(self):
        self.workflow = yaml.safe_load((ROOT / '.github/workflows/release.yml').read_text())
        self.steps = [step for job in self.workflow['jobs'].values() for step in job['steps']]
        self.library = (ROOT / 'scripts/deploy/lib/release-images.sh').read_text()
        self.names = set(re.search(r'^RI_NAMES=\(([^)]+)\)', self.library, re.M)[1].split())

    def test_deployed_app_images_are_all_release_artifacts(self):
        deploy = (ROOT / 'scripts/deploy/deploy.sh').read_text()
        paths = set()
        for line in deploy.splitlines():
            line = line.strip()
            if line.startswith('#'):
                continue
            if 'kubectl apply -f' in line or 'render_manifest ' in line:
                paths.update(re.findall(r'k8s/manifests/[\w./-]+\.yaml', line))
        images = set()
        for path in paths:
            for doc in yaml.safe_load_all((ROOT / path).read_text()):
                if not doc or doc.get('kind') not in ('Deployment', 'Job'):
                    continue
                spec = doc['spec']['template']['spec']
                for container in spec.get('containers', []) + spec.get('initContainers', []):
                    ref = container.get('image', '')
                    if ref.startswith('econ-graph-'):
                        self.assertEqual(container.get('imagePullPolicy'), 'Never', ref)
                        images.add(ref.split(':')[0].removeprefix('econ-graph-'))
        self.assertEqual(images, self.names)
        self.assertIn('chart-api', images)

    def test_loader_maps_cover_each_deployed_container(self):
        tables = {}
        for kind in ('DEPLOYMENT', 'CONTAINER', 'MANIFEST'):
            table = re.search(r'declare -A RI_' + kind + r'=\((.*?)\)', self.library, re.S)[1]
            tables[kind] = dict(re.findall(r'\[([\w-]+)\]=([^\s]+)', table))
            self.assertEqual(set(tables[kind]), self.names)
        for name in self.names:
            manifest = ROOT / tables['MANIFEST'][name]
            deployment, = [d for d in yaml.safe_load_all(manifest.read_text())
                           if d and d.get('kind') == 'Deployment']
            self.assertEqual(deployment['metadata']['name'], tables['DEPLOYMENT'][name])
            self.assertEqual(deployment['spec']['template']['metadata']['labels']['app'],
                             tables['DEPLOYMENT'][name])
            container, = [c for c in deployment['spec']['template']['spec']['containers']
                          if c['name'] == tables['CONTAINER'][name]]
            self.assertTrue(container['image'].startswith('econ-graph-' + name + ':'))

    def test_every_artifact_has_build_metadata_and_digest_promotion(self):
        metadata = {step['id'].removeprefix('meta-'): step for step in self.steps
                    if step.get('id', '').startswith('meta-')}
        self.assertEqual(set(metadata), self.names)
        builds = [step for step in self.steps if step.get('uses', '').startswith('docker/build-push-action@')]
        self.assertEqual(len(builds), len(self.names))
        for name, step in metadata.items():
            values = step['with']
            self.assertTrue(values['images'].endswith('/econ-graph-' + name))
            self.assertEqual(values['tags'], 'type=raw,value=sha-${{ steps.sha.outputs.sha }}')
            self.assertEqual(values['labels'], 'org.opencontainers.image.revision=${{ steps.sha.outputs.sha }}')
            self.assertEqual(values['flavor'], 'latest=false')
            build, = [s for s in builds if s['with']['tags'] == '${{ steps.meta-' + name + '.outputs.tags }}']
            self.assertTrue(build['with']['push'])
            self.assertEqual(build['with']['labels'], '${{ steps.meta-' + name + '.outputs.labels }}')
            self.assertEqual(build['if'], "steps.check.outputs.econ_graph_" + name.replace('-', '_') + " != 'true'")
            self.assertTrue((ROOT / build['with']['context'] / 'Dockerfile').is_file())
        chart, = [s for s in builds if s['with']['context'] == './chart-api-service']
        self.assertEqual(chart['with']['tags'], '${{ steps.meta-chart-api.outputs.tags }}')
        # Existence guard, rc tag promotion, and final digest resolution each
        # enumerate the complete set; final promotion consumes each resolved digest.
        loops = []
        for step in self.steps:
            loops += re.findall(r'for image in (econ-graph-[^;]+); do', step.get('run', ''))
        self.assertEqual(len(loops), 3)
        for loop in loops:
            self.assertEqual(set(loop.split()), {'econ-graph-' + n for n in self.names})
        promotion, = [s['run'] for s in self.steps if s.get('name') == 'Retag the resolved digests as the release']
        for name in self.names:
            self.assertIn('[econ-graph-' + name + ']="${{ steps.digests.outputs.econ_graph_' +
                          name.replace('-', '_') + ' }}"', promotion)


if __name__ == '__main__':
    unittest.main()
