#!/usr/bin/env python3
"""Offline ownership and fixture-evidence regressions. No Docker daemon is contacted."""
from pathlib import Path
import json
import os
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]

class IsolatedRunner(unittest.TestCase):
    def check_cleanup(self, matching_owner, failing_start=False):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, content in {
                'cargo': '#!/bin/sh\nexit 42\n',
                'docker': '''#!/usr/bin/env python3
import os, sys
from pathlib import Path
args=sys.argv[2:]
log=Path(os.environ['TEST_DOCKER_LOG'])
with log.open('a') as f: f.write(' '.join(args)+'\\n')
if args[0]=='run':
    owner=args[args.index('--name')+1]
    Path(os.environ['TEST_OWNER']).write_text(owner)
    if os.environ.get('TEST_START_FAIL') == '1': sys.exit(42)
    print('fake-container')
elif args[0]=='inspect':
    print(Path(os.environ['TEST_OWNER']).read_text() if os.environ['TEST_MATCH']=='1' else 'someone-else')
elif args[0]=='port': print('127.0.0.1:12345')
''',
            }.items():
                path = root / name
                path.write_text(content)
                path.chmod(0o755)
            env = {**os.environ, 'PATH': str(root) + os.pathsep + os.environ['PATH'],
                   'CI': '', 'TEST_MATCH': '1' if matching_owner else '0',
                   'TEST_START_FAIL': '1' if failing_start else '0',
                   'TEST_OWNER': str(root / 'owner'), 'TEST_DOCKER_LOG': str(root / 'log')}
            result = subprocess.run(['bash', str(ROOT / 'scripts/train3/check-compatibility.sh')],
                                    env=env, text=True, capture_output=True)
            self.assertEqual(result.returncode, 42, result.stderr)
            log = (root / 'log').read_text()
            self.assertIn('--publish 127.0.0.1::5432', log)
            self.assertIn('--tmpfs /var/lib/postgresql:rw,size=512m', log)
            resource = (root / 'owner').read_text() if failing_start else 'fake-container'
            self.assertEqual('rm --force ' + resource in log, matching_owner)

    def test_failed_build_removes_only_owned_container(self):
        self.check_cleanup(True)

    def test_partial_startup_cleans_up_owned_name(self):
        self.check_cleanup(True, failing_start=True)

    def test_changed_owner_is_never_deleted(self):
        self.check_cleanup(False)

    def test_fixture_evidence_covers_recorded_files_and_migration_planes(self):
        result = subprocess.check_output(['python3', str(ROOT / 'scripts/train3/fixture-manifest.py')], text=True)
        evidence = json.loads(result)
        self.assertEqual(evidence['format'], 1)
        self.assertEqual(set(evidence['migrations']), {'app', 'data'})
        import hashlib
        for path, digest in evidence['fixtures'].items():
            self.assertEqual(digest, hashlib.sha256((ROOT / path).read_bytes()).hexdigest())
        seed = 'backend/crates/econ-graph-crawler/tests/fixtures/e2e-seed.json'
        for series in json.loads((ROOT / seed).read_text())['series']:
            for route in series.get('discovery', []) + series['routes']:
                self.assertIn(str((ROOT / seed).parent.joinpath(route['fixture']).relative_to(ROOT)), evidence['fixtures'])

if __name__ == '__main__': unittest.main()
