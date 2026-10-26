#!/usr/bin/env python3
"""Exercise release build/seed orchestration without Docker, Rust builds, or network."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


class ReleaseBuildTest(unittest.TestCase):
    def test_profiles_features_paths_and_offline_seed(self):
        source = Path(__file__).resolve().parents[1] / 'release-e2e.sh'
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            for name in ('scripts', 'backend', 'frontend', 'bin'):
                (repo / name).mkdir()
            shutil.copy(source, repo / 'scripts/release-e2e.sh')
            # The same stub records each external invocation. Cargo creates a fixture-loader
            # executable, so the real script must resolve and run its actual output path.
            stub = '''#!/usr/bin/env python3
import json, os, pathlib, sys
name = pathlib.Path(sys.argv[0]).name
with open(os.environ['CALL_LOG'], 'a') as log:
    log.write(json.dumps({'name': name, 'args': sys.argv[1:],
        'flags': os.getenv('FLAGS_PROFILE'), 'backend': os.getenv('RELEASE_BACKEND_BIN'),
        'proxy': os.getenv('HTTPS_PROXY'), 'no_proxy': os.getenv('NO_PROXY'),
        'app': os.getenv('APP_DATABASE_URL'), 'data': os.getenv('DATA_DATABASE_URL')}) + '\\n')
if name == 'cargo' and 'seed-fixtures' in sys.argv:
    target = pathlib.Path(os.environ['CARGO_TARGET_DIR']) / 'debug'
    target.mkdir(parents=True)
    seed = target / 'seed-fixtures'
    seed.write_text(pathlib.Path(sys.argv[0]).read_text())
    seed.chmod(0o755)
'''
            for name in ('docker', 'cargo', 'npx'):
                path = repo / 'bin' / name
                path.write_text(stub)
                path.chmod(0o755)
            log = repo / 'calls.jsonl'
            # Relative target path is resolved from backend/, not the caller's cwd.
            env = dict(os.environ, PATH=f'{repo / "bin"}:{os.environ["PATH"]}',
                       CARGO_TARGET_DIR='custom-target', CALL_LOG=str(log),
                       RELEASE_E2E_DATABASE_URL='postgres://fixture-only',
                       APP_DATABASE_URL='postgres://do-not-touch',
                       DATA_DATABASE_URL='postgres://do-not-touch')
            env.pop('RELEASE_BACKEND_BIN', None)
            subprocess.run(['bash', str(repo / 'scripts/release-e2e.sh'), 'smoke.spec.ts'],
                           cwd=repo, env=env, check=True)
            calls = [json.loads(line) for line in log.read_text().splitlines()]
            backend, seed_build = [call for call in calls if call['name'] == 'cargo']
            self.assertEqual(backend['args'], ['build', '--release', '--locked', '-p',
                             'econ-graph-backend', '--bin', 'econ-graph-backend'])
            self.assertEqual(seed_build['args'], ['build', '--locked', '-p',
                             'econ-graph-crawler', '--features',
                             'econ-graph-crawler/testkit', '--bin', 'seed-fixtures'])
            self.assertEqual(backend['flags'], 'release')
            self.assertEqual(seed_build['flags'], 'release')
            seed = next(call for call in calls if call['name'] == 'seed-fixtures')
            self.assertEqual(seed['proxy'], 'http://127.0.0.1:9')
            self.assertEqual(seed['no_proxy'], '127.0.0.1,localhost')
            self.assertEqual(seed['app'], 'postgres://fixture-only')
            self.assertEqual(seed['data'], 'postgres://fixture-only')
            playwright = next(call for call in calls if call['name'] == 'npx')
            self.assertEqual(playwright['backend'],
                             str(repo / 'backend/custom-target/release/econ-graph-backend'))
            self.assertEqual(playwright['args'], ['playwright', 'test',
                             '--config=playwright.release.config.ts', 'smoke.spec.ts'])


if __name__ == '__main__':
    unittest.main()
