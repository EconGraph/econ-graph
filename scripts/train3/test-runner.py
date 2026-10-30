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
    def check_cleanup(self, matching_owner, failing_start=False, readiness_retry=False, seed_refusal=None):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, content in {
                'cargo': '#!/bin/sh\necho cargo >> "$TEST_DOCKER_LOG"\nif [ -n "${TEST_SEED_EXIT:-}" ] && [ "$1" = build ]; then exit 0; fi\nexit 42\n',
                'sleep': '#!/bin/sh\nexit 0\n',
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
elif args[0]=='exec' and args[2]=='pg_isready' and os.environ['TEST_TCP_RETRY']=='1':
    # Init's Unix socket is ready immediately; final TCP starts on the second probe.
    if args[3:] == ['-h', '127.0.0.1', '-U', 'postgres', '-d', 'train3']:
        state=Path(os.environ['TEST_PROBE_STATE'])
        probes=int(state.read_text()) if state.exists() else 0
        state.write_text(str(probes+1))
        if probes == 0: sys.exit(1)
elif args[0]=='exec' and args[2]=='psql': print('{"format":1}')
elif args[0]=='port': print('127.0.0.1:12345')
''',
            }.items():
                path = root / name
                path.write_text(content)
                path.chmod(0o755)
            env = {**os.environ, 'PATH': str(root) + os.pathsep + os.environ['PATH'],
                   'CI': '', 'TEST_MATCH': '1' if matching_owner else '0',
                   'TEST_START_FAIL': '1' if failing_start else '0',
                   'TEST_TCP_RETRY': '1' if readiness_retry else '0',
                   'TEST_PROBE_STATE': str(root / 'probes'),
                   'TEST_OWNER': str(root / 'owner'), 'TEST_DOCKER_LOG': str(root / 'log')}
            if seed_refusal:
                code, message, _ = seed_refusal
                target = root / 'target/debug'
                target.mkdir(parents=True)
                seed = target / 'seed-fixtures'
                seed.write_text("""#!/usr/bin/env python3
import os, sys
from pathlib import Path
state=Path(os.environ['TEST_SEED_STATE'])
count=int(state.read_text()) if state.exists() else 0
state.write_text(str(count+1))
if count == 2:
    print(os.environ['TEST_SEED_MESSAGE'], file=sys.stderr)
    sys.exit(int(os.environ['TEST_SEED_EXIT']))
""")
                seed.chmod(0o755)
                env.update({'CARGO_TARGET_DIR': str(root / 'target'),
                            'TEST_SEED_STATE': str(root / 'seed-count'),
                            'TEST_SEED_EXIT': str(code), 'TEST_SEED_MESSAGE': message})
            result = subprocess.run(['bash', str(ROOT / 'scripts/train3/check-compatibility.sh')],
                                    env=env, text=True, capture_output=True)
            self.assertEqual(result.returncode, seed_refusal[2] if seed_refusal else 42, result.stderr)
            log = (root / 'log').read_text()
            self.assertIn('--publish 127.0.0.1::5432', log)
            self.assertIn('--tmpfs /var/lib/postgresql:rw,size=512m', log)
            resource = (root / 'owner').read_text() if failing_start else 'fake-container'
            self.assertEqual('rm --force ' + resource in log, matching_owner)
            return log

    def test_failed_build_removes_only_owned_container(self):
        self.check_cleanup(True)

    def test_startup_waits_for_final_tcp_database(self):
        commands = self.check_cleanup(True, readiness_retry=True).splitlines()
        expected = 'exec fake-container pg_isready -h 127.0.0.1 -U postgres -d train3'
        probes = [index for index, command in enumerate(commands) if command == expected]
        self.assertEqual(len(probes), 2, 'temporary socket readiness must not start the build')
        self.assertGreater(commands.index('cargo'), probes[-1])

    def test_populated_seed_requires_the_expected_guard_failure(self):
        guard = '--deterministic-empty requires an entirely empty disposable database; no migrations or identity changes were applied'
        for status, message, expected in [(1, guard, 42), (1, 'connection refused', 1), (139, guard, 1)]:
            with self.subTest(status=status, message=message):
                self.check_cleanup(True, seed_refusal=(status, message, expected))

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
