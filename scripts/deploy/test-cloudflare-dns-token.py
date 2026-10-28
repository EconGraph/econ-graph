#!/usr/bin/env python3
"""Exercise DNS credential handoff against a fake kubectl, never a cluster."""
import json
import os
from pathlib import Path
import pty
import select
import subprocess
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[2]
SETUP = ROOT / 'scripts/deploy/setup-letsencrypt-cloudflare.sh'
SECRETS = ROOT / 'scripts/deploy/create-secrets.sh'
TOKEN = 'fake-cloudflare-dns-token-for-offline-tests'
STUB = r'''#!/usr/bin/env python3
import json, os, pathlib, stat, sys
args = sys.argv[1:]
with open(os.environ['STUB_LOG'], 'a') as log:
    log.write(json.dumps(args) + '\n')
if 'current-context' in args:
    print(os.environ.get('STUB_CURRENT_CONTEXT', 'test-context'))
elif 'view' in args and '--minify' in args:
    print('https://127.0.0.1:6443')
elif 'namespace' in args and 'kube-system' in args:
    print('11111111-2222-3333-4444-555555555555')
elif 'create' in args and 'secret' in args:
    assert 'cloudflare-api-token-secret' in args
    assert 'cert-manager' in args
    arg = next(a for a in args if a.startswith('--from-file=api-token='))
    path = pathlib.Path(arg.split('=', 2)[2])
    assert stat.S_IMODE(path.stat().st_mode) & 0o077 == 0
    assert stat.S_IMODE(path.parent.parent.stat().st_mode) & 0o077 == 0
    assert path.read_text() == os.environ['EXPECTED_TOKEN']
    print('kind: Secret\nmetadata:\n  name: cloudflare-api-token-secret')
elif 'apply' in args:
    sys.stdin.read()
elif 'certificate' in args and any(a.startswith('jsonpath=') for a in args):
    print('Ready')
'''


class DnsTokenTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        directory = Path(self.tmp.name)
        kubectl = directory / 'kubectl'
        kubectl.write_text(STUB)
        kubectl.chmod(0o700)
        self.log = directory / 'calls'
        self.env = dict(os.environ, KUBECTL=str(kubectl),
                        STUB_LOG=str(self.log), EXPECTED_TOKEN=TOKEN)
        for key in ('CLOUDFLARE_API_TOKEN', 'CLUSTER_ROLE_CONTEXT'):
            self.env.pop(key, None)

    def run_script(self, script, *args, token=None, current_context=None):
        env = dict(self.env)
        if token is not None:
            env['CLOUDFLARE_API_TOKEN'] = token
        if current_context is not None:
            env['STUB_CURRENT_CONTEXT'] = current_context
        return subprocess.run(['bash', '-x', str(script), *args],
                              env=env, stdin=subprocess.DEVNULL,
                              capture_output=True, text=True, timeout=15)

    def calls(self):
        return [json.loads(line) for line in self.log.read_text().splitlines()]

    def assert_private(self, result):
        self.assertNotIn(TOKEN, result.stdout + result.stderr)
        self.assertNotIn(TOKEN, self.log.read_text())
        for args in self.calls():
            if 'apply' in args and '--field-manager=create-secrets' in args:
                self.assertIn('--server-side', args)
            if args == ['config', 'current-context']:
                continue
            self.assertIn('--context', args)
            self.assertEqual(args[args.index('--context') + 1], 'test-context')

    def test_environment_token_reaches_only_dns_secret_without_argv_or_trace(self):
        # --context names a context other than the current one: the credential
        # and certificate writes all go to it, and the current one is not changed.
        result = self.run_script(SETUP, '--domain', 'econgraph.example.com',
                                 '--context', 'test-context', token=TOKEN,
                                 current_context='other-context')
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assert_private(result)
        self.assertFalse(any('use-context' in args for args in self.calls()))
        created = [args for args in self.calls() if 'create' in args]
        self.assertEqual(len(created), 1)
        self.assertTrue(all('ingress' not in ' '.join(args) for args in self.calls()))

    def test_argv_token_is_refused_without_echo_or_cluster_calls(self):
        for flag in ('-t', '--token'):
            result = self.run_script(SETUP, flag, TOKEN)
            self.assertEqual(result.returncode, 2)
            self.assertNotIn(TOKEN, result.stdout + result.stderr)
            self.assertFalse(self.log.exists())

    def test_noninteractive_missing_token_writes_nothing(self):
        result = self.run_script(SETUP)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.log.exists())

    def test_dns_only_dry_run_creates_nothing(self):
        result = self.run_script(SECRETS, '--cloudflare-dns-only', '--dry-run', token=TOKEN)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_private(result)
        self.assertFalse(any('create' in args or 'apply' in args for args in self.calls()))

    def test_explicit_context_replaces_inherited_one(self):
        # With KUBECTL and KUBECONFIG unset, --context=NAME uses the cached kind
        # kubectl even when a CLUSTER_ROLE_CONTEXT is inherited; plain kubectl
        # on PATH would fail.
        directory = Path(self.tmp.name)
        cache = directory / 'cache'
        (cache / 'econgraph' / 'bin').mkdir(parents=True)
        (directory / 'kubectl').rename(cache / 'econgraph' / 'bin' / 'kubectl')
        (cache / 'econgraph' / 'kubeconfig-econ-graph').write_text('apiVersion: v1\n')
        plain = directory / 'plain'
        plain.mkdir()
        (plain / 'kubectl').write_text('#!/bin/sh\nexit 1\n')
        (plain / 'kubectl').chmod(0o700)
        env = dict(self.env, CLUSTER_ROLE_CONTEXT='stale-context', XDG_CACHE_HOME=str(cache),
                   PATH=f"{plain}:{os.environ['PATH']}",
                   STUB_CURRENT_CONTEXT='other-context', CLOUDFLARE_API_TOKEN=TOKEN)
        env.pop('KUBECTL')
        env.pop('KUBECONFIG', None)
        result = subprocess.run(['bash', '-x', str(SECRETS), '--cloudflare-dns-only',
                                 '--context=test-context'],
                                env=env, stdin=subprocess.DEVNULL,
                                capture_output=True, text=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assert_private(result)
        self.assertEqual(len([args for args in self.calls() if 'create' in args]), 1)

    def test_unknown_argument_is_not_echoed(self):
        result = self.run_script(SECRETS, '--cloudflare-dns-only', '--token=' + TOKEN)
        self.assertEqual(result.returncode, 2)
        self.assertNotIn(TOKEN, result.stdout + result.stderr)
        self.assertFalse(self.log.exists())

    def test_hidden_prompt_does_not_echo_token(self):
        master, slave = pty.openpty()
        try:
            with subprocess.Popen(['bash', '-x', str(SETUP)], env=self.env,
                                  stdin=slave, stdout=slave, stderr=slave) as process:
                os.close(slave)
                slave = None
                output = b''
                sent = False
                deadline = time.monotonic() + 15
                while time.monotonic() < deadline:
                    if select.select([master], [], [], 0.1)[0]:
                        try:
                            chunk = os.read(master, 65536)
                        except OSError:
                            break
                        if not chunk:
                            break
                        output += chunk
                        if b'input hidden' in output and not sent:
                            os.write(master, TOKEN.encode() + b'\n')
                            sent = True
                    if process.poll() is not None:
                        break
                if process.poll() is None:
                    process.kill()
                self.assertTrue(sent, output.decode())
                self.assertEqual(process.wait(), 0, output.decode())
                self.assertNotIn(TOKEN.encode(), output)
                self.assertNotIn(TOKEN, self.log.read_text())
        finally:
            os.close(master)
            if slave is not None:
                os.close(slave)


if __name__ == '__main__':
    unittest.main()
