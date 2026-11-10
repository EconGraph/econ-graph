#!/usr/bin/env python3
"""Checks on the production kind configuration (terraform/k8s/).

- kind-config.yaml publishes only the gateway's ports 80 and 443, both on 127.0.0.1, from
  the control-plane node, and no other node publishes anything: without listenAddress,
  kind (Docker) publishes a port on every host interface, which bypasses Cloudflare.
- main.tf, which generates kind-config.yaml, declares the same mappings, mounts and node
  image, so a terraform apply cannot bring back what the YAML no longer has.
- The Terraform outputs advertise no port the configuration does not publish.

Needs python3 with PyYAML. Usage: python3 scripts/deploy/test-kind-config.py
"""

import re
import unittest
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
KIND_CONFIG = ROOT / 'terraform/k8s/kind-config.yaml'
MAIN_TF = ROOT / 'terraform/k8s/main.tf'
OUTPUTS_TF = ROOT / 'terraform/k8s/outputs.tf'
GATEWAY_MAPPINGS = [
    {'containerPort': 80, 'hostPort': 80, 'listenAddress': '127.0.0.1', 'protocol': 'TCP'},
    {'containerPort': 443, 'hostPort': 443, 'listenAddress': '127.0.0.1', 'protocol': 'TCP'},
]


def tf_blocks(text, key):
    """The bodies of every `KEY = [ ... ]` list in main.tf, as lists of {field: value}."""
    blocks = []
    for match in re.finditer(r'\b' + key + r'\s*=\s*\[', text):
        depth, i = 1, match.end()
        while depth:
            depth += {'[': 1, ']': -1}.get(text[i], 0)
            i += 1
        body = text[match.end():i - 1]
        items = []
        for item in re.findall(r'\{([^{}]*)\}', body):
            fields = {}
            for name, value in re.findall(r'(\w+)\s*=\s*("[^"]*"|\d+)', item):
                fields[name] = int(value) if value.isdigit() else value.strip('"')
            items.append(fields)
        blocks.append(items)
    return blocks


class KindConfigTests(unittest.TestCase):
    def setUp(self):
        self.config = yaml.safe_load(KIND_CONFIG.read_text())
        self.main_tf = MAIN_TF.read_text()

    def test_only_the_gateway_ports_on_loopback(self):
        nodes = self.config['nodes']
        control_planes = [n for n in nodes if n['role'] == 'control-plane']
        self.assertEqual(len(control_planes), 1)
        self.assertEqual(control_planes[0]['extraPortMappings'], GATEWAY_MAPPINGS)
        for node in nodes:
            if node['role'] != 'control-plane':
                self.assertNotIn('extraPortMappings', node, node['role'])

    def test_main_tf_declares_the_same_mappings(self):
        self.assertEqual(tf_blocks(self.main_tf, 'extraPortMappings'), [GATEWAY_MAPPINGS])

    def test_main_tf_declares_the_same_mounts(self):
        mounts = [m for n in self.config['nodes'] for m in n.get('extraMounts', [])]
        self.assertEqual(tf_blocks(self.main_tf, 'extraMounts'), [mounts])

    def test_the_node_image_comes_from_the_variable_terraform_pins(self):
        variables = (ROOT / 'terraform/k8s/variables.tf').read_text()
        default = re.search(r'variable\s+"node_image"\s*\{[^}]*default\s*=\s*"([^"]+)"', variables)
        self.assertIsNotNone(default, 'variables.tf has no node_image default')
        self.assertEqual({n['image'] for n in self.config['nodes']}, {default.group(1)})

    def test_outputs_advertise_no_unpublished_port(self):
        for path in (MAIN_TF, OUTPUTS_TF):
            for url in re.findall(r'https?://localhost(?::(\d+))?', path.read_text()):
                self.assertIn(url, ('', '80', '443'), f'{path.name} advertises localhost:{url}')


if __name__ == '__main__':
    unittest.main()
