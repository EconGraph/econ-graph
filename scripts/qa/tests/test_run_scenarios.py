#!/usr/bin/env python3
"""Tests for the QA scenario runner (scripts/qa/run_scenarios.py) and its scenarios.

Offline: the guard tests use stand-in scenarios, and the scenario tests run against a stub
site on 127.0.0.1 (GraphQL, Keycloak's sign-in form and token endpoint, MCP).

  python3 -m unittest discover -s scripts/qa/tests -v
"""

from __future__ import annotations

import base64
import contextlib
import hashlib
import io
import json
import os
import re
import signal
import stat
import subprocess
import sys
import tempfile
import threading
import time
import unittest
import urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

QA_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(QA_DIR))

import run_scenarios as runner  # noqa: E402

SCENARIOS = runner.discover(runner.DEFAULT_SCENARIO_DIR)
BY_NAME = {s.name: s for s in SCENARIOS}


def assert_stopped(test: unittest.TestCase, pid: int, wait: float = 5) -> None:
    """Fail unless PID has exited. A process orphaned when its parent died is reparented to the
    nearest subreaper (init, or systemd) and stays a zombie until it is reaped, which can take a
    moment after it has exited; a zombie has stopped running, so it counts as stopped. A live
    process fails after WAIT seconds."""
    deadline = time.monotonic() + wait
    while True:
        try:
            os.kill(pid, 0)
        except (ProcessLookupError, PermissionError):
            return  # gone (PermissionError: the PID now belongs to another user's process)
        try:
            state = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()[0]
        except (OSError, IndexError):
            state = None  # no /proc (macOS), or reaped while reading: poll again
        if state in ('Z', 'X'):
            return
        if time.monotonic() >= deadline:
            test.fail(f'process {pid} is still running (state {state})')
        time.sleep(0.05)


def write_scenario(directory: Path, name: str, header: str, body: str) -> Path:
    path = directory / name
    path.write_text(f'#!/usr/bin/env bash\n{header}\n{body}\n', encoding='utf-8')
    path.chmod(path.stat().st_mode | stat.S_IXUSR)
    return path


def run_main(argv, env, scenario_dir=runner.DEFAULT_SCENARIO_DIR):
    """main()'s exit status, stdout and stderr."""
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        try:
            code = runner.main(argv, env, scenario_dir)
        except SystemExit as exit_:
            code = exit_.code
    return code, out.getvalue(), err.getvalue()


class DeclarationTest(unittest.TestCase):
    def test_every_scenario_declares_what_it_changes(self):
        self.assertGreaterEqual(len(SCENARIOS), 12)
        for scenario in SCENARIOS:
            with self.subTest(scenario.name):
                self.assertIsNone(scenario.declaration_problem)
                self.assertTrue(scenario.description)
                self.assertTrue(scenario.profiles)

    def test_state_changing_scenarios(self):
        mutating = {s.name for s in SCENARIOS if s.mutates}
        self.assertEqual(mutating, {'annotation-round-trip', 'catalog-roles', 'backup-restore',
                                    'crawl-sample', 'branch-isolation', 'data-survives-redeploy'})

    def test_lab_only_scenarios(self):
        lab_only = {s.name for s in SCENARIOS if s.profiles == ('lab',)}
        self.assertEqual(lab_only, {'backup-restore', 'conversion-checks', 'crawl-sample',
                                    'branch-isolation', 'data-survives-redeploy'})

    def test_a_missing_or_unclear_declaration_counts_as_changing_state(self):
        with tempfile.TemporaryDirectory() as tmp:
            for name, line in (('silent', ''), ('vague', '# qa-mutates: maybe')):
                path = write_scenario(Path(tmp), name, f'# qa-profiles: production\n{line}', 'true')
                scenario = runner.parse_scenario(path)
                self.assertTrue(scenario.mutates, name)
                self.assertIn('treated as yes', scenario.declaration_problem)


class ProductionGuardTest(unittest.TestCase):
    """The production profile refuses every state-changing scenario but the QA round trip."""

    def qa_env(self, username='qa-alice', password='s3cret-qa'):
        return {'QA_USERNAME': username, 'QA_PASSWORD': password}

    def test_production_runs_only_read_only_scenarios_without_a_qa_account(self):
        for scenario in SCENARIOS:
            with self.subTest(scenario.name):
                why = runner.admit(scenario, 'production', {})
                if scenario.mutates:
                    self.assertIsNotNone(why)
                    self.assertTrue(why.startswith('refused'), why)
                elif 'production' in scenario.profiles:
                    self.assertIsNone(why)

    def test_a_qa_account_admits_only_the_annotation_round_trip(self):
        env = self.qa_env()
        for scenario in SCENARIOS:
            with self.subTest(scenario.name):
                why = runner.admit(scenario, 'production', env)
                if scenario.mutates and scenario.name != 'annotation-round-trip':
                    self.assertTrue(why and why.startswith('refused'), why)
        self.assertIsNone(runner.admit(BY_NAME['annotation-round-trip'], 'production', env))

    def test_the_round_trip_needs_a_complete_dedicated_qa_account(self):
        round_trip = BY_NAME['annotation-round-trip']
        for env in ({}, {'QA_USERNAME': 'qa-alice'}, {'QA_PASSWORD': 'x'},
                    self.qa_env(username='alice'), self.qa_env(username='joe@example.com'),
                    {'QA_USERNAME': 'qa-alice', 'QA_PASSWORD_FILE': '/nonexistent/password'}):
            with self.subTest(env=sorted(env)):
                why = runner.admit(round_trip, 'production', env)
                self.assertTrue(why and why.startswith('refused'), why)

    def test_a_password_file_counts(self):
        with tempfile.NamedTemporaryFile('w', delete=False) as handle:
            handle.write('from-a-file\n')
        try:
            env = {'QA_USERNAME': 'qa-bob', 'QA_PASSWORD_FILE': handle.name}
            self.assertIsNone(runner.admit(BY_NAME['annotation-round-trip'], 'production', env))
        finally:
            os.unlink(handle.name)

    def test_no_scenario_grants_itself_the_exception(self):
        # A mutating scenario that declares the production profile is still refused unless the
        # runner's own list names it.
        with tempfile.TemporaryDirectory() as tmp:
            path = write_scenario(Path(tmp), 'sneaky', '# qa-mutates: yes\n'
                                  '# qa-profiles: production\n# qa-description: x', 'true')
            why = runner.admit(runner.parse_scenario(path), 'production', self.qa_env())
            self.assertTrue(why and why.startswith('refused'), why)

    def test_a_copy_of_the_round_trip_elsewhere_gets_no_exception(self):
        # The exception is the file scripts/qa/scenarios/annotation-round-trip.py, not the name.
        with tempfile.TemporaryDirectory() as tmp:
            for name in ('annotation-round-trip', 'annotation-round-trip.py'):
                path = write_scenario(Path(tmp), name, '# qa-scenario: annotation-round-trip\n'
                                      '# qa-mutates: yes\n# qa-profiles: lab, ci, production\n'
                                      '# qa-description: spoof', 'true')
                spoof = runner.parse_scenario(path)
                self.assertEqual(spoof.name, 'annotation-round-trip')
                why = runner.admit(spoof, 'production', self.qa_env())
                self.assertTrue(why and why.startswith('refused'), why)

    def test_lab_and_ci_run_mutating_scenarios(self):
        self.assertIsNone(runner.admit(BY_NAME['catalog-roles'], 'ci', {}))
        self.assertIsNone(runner.admit(BY_NAME['backup-restore'], 'lab', {}))
        self.assertIn('not in the ci profile', runner.admit(BY_NAME['backup-restore'], 'ci', {}))


class RunnerTest(unittest.TestCase):
    """The runner end to end, with stand-in scenarios that leave a marker when they run."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self.tmp.name)
        self.scenarios = self.dir / 'scenarios'
        self.scenarios.mkdir()
        self.marker = self.dir / 'ran'
        self.env = {'PATH': os.environ['PATH'], 'QA_WORK_ROOT': str(self.dir),
                    'QA_LAB_TARGET_OK': 'localhost,127.0.0.1,lab.example,site.example'}

    def tearDown(self):
        self.tmp.cleanup()

    def scenario(self, name, mutates='yes', profiles='lab, ci, production', body=None):
        header = f'# qa-scenario: {name}\n# qa-profiles: {profiles}\n# qa-description: {name}'
        if mutates is not None:
            header += f'\n# qa-mutates: {mutates}'
        write_scenario(self.scenarios, name, header,
                       body if body is not None else f'echo {name} >> "{self.marker}"')

    def ran(self):
        return self.marker.read_text().split() if self.marker.exists() else []

    def report(self, argv, env=None):
        out = self.dir / 'report.json'
        code, stdout, stderr = run_main(argv + ['--json', str(out)], env or self.env, self.scenarios)
        return code, (json.loads(out.read_text()) if out.exists() else None), stdout + stderr

    def test_production_does_not_run_a_mutating_scenario(self):
        self.scenario('writer')
        self.scenario('reader', mutates='no')
        self.scenario('undeclared', mutates=None)
        code, report, _ = self.report(['--target', 'https://site.example', '--profile', 'production'])
        self.assertEqual(code, 0)
        self.assertEqual(self.ran(), ['reader'])
        status = {r['name']: (r['status'], r['reason'] or '') for r in report['scenarios']}
        self.assertEqual(status['writer'][0], 'skip')
        self.assertTrue(status['writer'][1].startswith('refused'))
        self.assertTrue(status['undeclared'][1].startswith('refused'))

    def test_asking_production_for_a_mutating_scenario_by_name_is_an_error(self):
        self.scenario('writer')
        self.scenario('reader', mutates='no')
        code, report, output = self.report(['--target', 'https://site.example', '--profile',
                                            'production', '--only', 'reader,writer'])
        self.assertEqual(code, 2)
        self.assertIsNone(report)
        self.assertEqual(self.ran(), [])
        self.assertIn('writer: refused', output)

    def test_a_spoofed_round_trip_does_not_run_in_production(self):
        # A file elsewhere declaring the exception's name, with a QA account configured.
        self.scenario('annotation-round-trip')
        env = dict(self.env, QA_USERNAME='qa-alice', QA_PASSWORD='pw-123456')
        argv = ['--target', 'https://site.example', '--profile', 'production']
        code, report, _ = self.report(argv, env)
        self.assertEqual(code, 0)
        self.assertEqual(self.ran(), [])
        self.assertTrue(report['scenarios'][0]['reason'].startswith('refused'))
        code, _, output = self.report(argv + ['--only', 'annotation-round-trip'], env)
        self.assertEqual(code, 2)
        self.assertEqual(self.ran(), [])

    def test_the_command_line_has_no_scenario_directory_override(self):
        self.assertNotIn('QA_SCENARIO_DIR', (QA_DIR / 'run_scenarios.py').read_text())

    def test_lab_and_ci_run_only_against_named_targets(self):
        self.scenario('writer')
        env = {k: v for k, v in self.env.items() if k != 'QA_LAB_TARGET_OK'}
        for target in ('http://localhost:18081', 'http://127.0.0.1', 'http://10.0.0.5',
                       'http://[::1]:8080', 'https://lab-1.example'):
            for profile in ('lab', 'ci'):
                with self.subTest(target=target, profile=profile):
                    code, _, output = self.report(['--target', target, '--profile', profile], env)
                    self.assertEqual(code, 2)
                    self.assertIn('QA_LAB_TARGET_OK', output)
        self.assertEqual(self.ran(), [])
        # Named, it runs; a production host stays refused even when named.
        named = dict(env, QA_LAB_TARGET_OK='lab-1.example., localhost')
        code, _, _ = self.report(['--target', 'https://LAB-1.example', '--profile', 'lab'], named)
        self.assertEqual(code, 0)
        code, _, _ = self.report(['--target', 'https://lab-1.example.', '--profile', 'lab'], named)
        self.assertEqual(code, 0)
        code, _, _ = self.report(['--target', 'http://localhost:18081', '--profile', 'ci'], named)
        self.assertEqual(code, 0)
        self.assertEqual(self.ran(), ['writer', 'writer', 'writer'])
        code, _, _ = self.report(['--target', 'https://econgraph.com', '--profile', 'ci'],
                                 dict(env, QA_LAB_TARGET_OK='econgraph.com'))
        self.assertEqual(code, 2)
        # The production profile needs no list.
        code, _, _ = self.report(['--target', 'https://site.example', '--profile', 'production'], env)
        self.assertEqual(code, 0)

    def test_naming_a_scenario_the_profile_does_not_run_is_an_error(self):
        self.scenario('lab-only', mutates='no', profiles='lab')
        code, report, output = self.report(['--target', 'http://localhost:1', '--profile', 'ci',
                                            '--only', 'lab-only'])
        self.assertEqual(code, 2)
        self.assertIsNone(report)
        self.assertIn('not in the ci profile', output)

    def test_duplicate_scenario_names_are_a_usage_error(self):
        self.scenario('one', mutates='no')
        write_scenario(self.scenarios, 'two', '# qa-scenario: one\n# qa-mutates: no\n'
                       '# qa-profiles: ci\n# qa-description: x', 'true')
        code, report, output = self.report(['--target', 'http://localhost:1', '--profile', 'ci'])
        self.assertEqual(code, 2)
        self.assertIn('duplicate scenario names: one', output)
        self.assertEqual(self.ran(), [])

    def test_required_scenarios_must_pass_not_skip(self):
        self.scenario('passes', mutates='no', body='echo fine')
        self.scenario('skips', mutates='no',
                      body='echo \'{"skip": "quietly"}\' >> "$QA_EVIDENCE_FILE"; exit 77')
        argv = ['--target', 'http://localhost:1', '--profile', 'ci']
        code, _, _ = self.report(argv + ['--require-pass', 'passes'])
        self.assertEqual(code, 0)
        code, report, output = self.report(argv + ['--require-pass', 'passes,skips'])
        self.assertEqual(code, 1)
        self.assertEqual(report['counts']['fail'], 0)
        self.assertIn('required to pass but did not: skips', output)
        code, _, _ = self.report(argv + ['--require-pass', 'nonexistent'])
        self.assertEqual(code, 2)
        code, _, _ = self.report(argv + ['--only', 'passes', '--require-pass', 'skips'])
        self.assertEqual(code, 2)

    def test_a_skipped_check_is_neither_a_pass_nor_a_failure(self):
        self.scenario('partial', mutates='no', body=(
            'echo \'{"check": "a", "ok": true}\' >> "$QA_EVIDENCE_FILE"; '
            'echo \'{"check": "redeploy", "ok": null, "skipped": true}\' >> "$QA_EVIDENCE_FILE"'))
        code, report, output = self.report(['--target', 'http://localhost:1', '--profile', 'ci'])
        self.assertEqual(code, 0)
        result = report['scenarios'][0]
        self.assertEqual((result['status'], result['reason']), ('pass', 'skipped checks: redeploy'))
        self.assertIn('skipped checks: redeploy', output)

    def test_lab_and_ci_refuse_the_production_site(self):
        self.scenario('writer')
        for profile in ('lab', 'ci'):
            for target in ('https://econgraph.com', 'https://WWW.econgraph.com/',
                           'https://econgraph.com.', 'https://www.econgraph.com.:443/'):
                with self.subTest(profile=profile, target=target):
                    code, report, output = self.report(['--target', target, '--profile', profile])
                    self.assertEqual(code, 2)
                    self.assertIn('refusing', output)
        env = dict(self.env, QA_PRODUCTION_HOSTS='prod.example.')
        code, _, _ = self.report(['--target', 'https://prod.example', '--profile', 'ci'], env)
        self.assertEqual(code, 2)
        self.assertEqual(self.ran(), [])
        code, _, _ = self.report(['--target', 'https://econgraph.com', '--profile', 'production'])
        self.assertEqual(code, 0)

    def test_a_target_with_credentials_is_refused(self):
        self.scenario('reader', mutates='no')
        code, _, output = self.report(['--target', 'https://u:p@site.example', '--profile', 'ci'])
        self.assertEqual(code, 2)
        self.assertEqual(self.ran(), [])

    def test_credentials_are_not_arguments(self):
        for flag in ('--password', '--username', '--token'):
            with self.subTest(flag):
                code, _, _ = run_main(['--target', 'https://s.example', '--profile', 'ci', flag, 'x'],
                                      self.env)
                self.assertEqual(code, 2)

    def test_statuses_and_exit_code(self):
        self.scenario('passes', mutates='no', body='echo fine')
        self.scenario('fails', mutates='no', body='echo broken; exit 3')
        self.scenario('skips', mutates='no',
                      body='echo \'{"skip": "nothing to do here"}\' >> "$QA_EVIDENCE_FILE"; exit 77')
        self.scenario('lies', mutates='no',
                      body='echo \'{"check": "c", "ok": false, "detail": "bad"}\' >> "$QA_EVIDENCE_FILE"')
        self.scenario('lab-only', mutates='no', profiles='lab')
        # A check that does not say it passed did not.
        self.scenario('unsaid', mutates='no',
                      body='echo \'{"check": "c", "detail": "no verdict"}\' >> "$QA_EVIDENCE_FILE"')
        # A skip after a failed check is still a failure.
        self.scenario('halfskip', mutates='no',
                      body='echo \'{"check": "c", "ok": false}\' >> "$QA_EVIDENCE_FILE"; '
                           'echo \'{"skip": "later"}\' >> "$QA_EVIDENCE_FILE"; exit 77')
        code, report, output = self.report(['--target', 'http://localhost:1', '--profile', 'ci'])
        self.assertEqual(code, 1)
        status = {r['name']: r for r in report['scenarios']}
        self.assertEqual(status['passes']['status'], 'pass')
        self.assertEqual(status['fails']['status'], 'fail')
        self.assertEqual(status['fails']['output_tail'], ['broken'])
        self.assertEqual((status['skips']['status'], status['skips']['reason']), ('skip', 'nothing to do here'))
        self.assertEqual(status['lies']['status'], 'fail')
        self.assertEqual(status['lab-only']['status'], 'skip')
        self.assertEqual(status['halfskip']['status'], 'fail')
        self.assertEqual(status['unsaid']['status'], 'fail')
        self.assertEqual(report['counts'], {'pass': 1, 'fail': 4, 'skip': 2})
        self.assertIn('1 passed, 4 failed, 2 skipped', output)
        for result in report['scenarios']:
            self.assertIsInstance(result['duration_seconds'], float)

    def test_a_scenario_that_overruns_fails(self):
        self.scenario('slow', mutates='no', body='sleep 5')
        (self.scenarios / 'slow').write_text((self.scenarios / 'slow').read_text().replace(
            '# qa-description', '# qa-timeout: 1\n# qa-description'))
        code, report, _ = self.report(['--target', 'http://localhost:1', '--profile', 'ci'])
        self.assertEqual(code, 1)
        self.assertEqual(report['scenarios'][0]['reason'], 'timed out after 1s')

    def test_an_overrun_stops_everything_the_scenario_started(self):
        pid_file = self.dir / 'child.pid'
        self.scenario('spawner', mutates='no', body=f'sleep 300 & echo $! > "{pid_file}"; wait')
        (self.scenarios / 'spawner').write_text((self.scenarios / 'spawner').read_text().replace(
            '# qa-description', '# qa-timeout: 1\n# qa-description'))
        code, report, _ = self.report(['--target', 'http://localhost:1', '--profile', 'ci'],
                                      dict(self.env, QA_TIMEOUT_GRACE_SECONDS='2'))
        self.assertEqual(code, 1)
        self.assertEqual(report['scenarios'][0]['reason'], 'timed out after 1s')
        assert_stopped(self, int(pid_file.read_text()))

    def test_an_overrunning_python_scenario_still_cleans_up(self):
        cleaned = self.dir / 'cleaned'
        path = self.scenarios / 'tidy'
        path.write_text(f'''#!/usr/bin/env python3
# qa-scenario: tidy
# qa-mutates: no
# qa-profiles: ci
# qa-timeout: 1
# qa-description: tidy
import sys, time
import qa
try:
    qa.run(['sleep', '300'], timeout=600)
finally:
    open({str(cleaned)!r}, 'w').write('yes')
''')
        path.chmod(0o755)
        code, report, _ = self.report(['--target', 'http://localhost:1', '--profile', 'ci'],
                                      dict(self.env, QA_TIMEOUT_GRACE_SECONDS='10'))
        self.assertEqual(code, 1)
        self.assertEqual(cleaned.read_text(), 'yes')
        self.assertLess(report['scenarios'][0]['duration_seconds'], 10)

    def test_an_overrun_kills_and_reports_what_escaped_the_process_group(self):
        # setsid moves the child out of the scenario's process group, so the group kill misses
        # it; the runner finds it by the run's marker.
        pid_file = self.dir / 'escaped.pid'
        self.scenario('escaper', mutates='no',
                      body=f'setsid sleep 300 >/dev/null 2>&1 & echo $! > "{pid_file}"; wait')
        (self.scenarios / 'escaper').write_text((self.scenarios / 'escaper').read_text().replace(
            '# qa-description', '# qa-timeout: 1\n# qa-description'))
        code, report, _ = self.report(['--target', 'http://localhost:1', '--profile', 'ci'],
                                      dict(self.env, QA_TIMEOUT_GRACE_SECONDS='2'))
        self.assertEqual(code, 1)
        pid = int(pid_file.read_text())
        self.assertEqual(report['scenarios'][0]['reason'],
                         f'timed out after 1s; left running, now killed: sleep ({pid})')
        assert_stopped(self, pid)

    def test_a_scenario_that_leaves_a_process_running_fails(self):
        pid_file = self.dir / 'left.pid'
        # Exit only once the child has its own session (its session id is its pid), so the
        # group kill can't catch it before setsid has run.
        self.scenario('leaver', mutates='no',
                      body=f'setsid sleep 300 >/dev/null 2>&1 & echo $! > "{pid_file}"\n'
                           'until [ "$(cut -d" " -f6 /proc/$!/stat)" = "$!" ]; do sleep 0.05; done')
        code, report, _ = self.report(['--target', 'http://localhost:1', '--profile', 'ci'])
        self.assertEqual(code, 1)
        pid = int(pid_file.read_text())
        self.assertEqual((report['scenarios'][0]['status'], report['scenarios'][0]['reason']),
                         ('fail', f'left running, now killed: sleep ({pid})'))
        assert_stopped(self, pid)

    def test_a_cleanup_command_ends_within_the_grace_period(self):
        # qa.run starts commands in their own session, outside the group the runner kills, so
        # one started in cleanup gets only what is left of the grace period.
        pid_file = self.dir / 'cleanup.pid'
        path = self.scenarios / 'slow-cleanup'
        path.write_text(f'''#!/usr/bin/env python3
# qa-scenario: slow-cleanup
# qa-mutates: no
# qa-profiles: ci
# qa-timeout: 1
# qa-description: slow cleanup
import qa
try:
    qa.run(['sleep', '300'], timeout=600)
finally:
    qa.run(['sh', '-c', 'echo $$ > {str(pid_file)}; exec sleep 300'], timeout=1200)
''')
        path.chmod(0o755)
        code, report, _ = self.report(['--target', 'http://localhost:1', '--profile', 'ci'],
                                      dict(self.env, QA_TIMEOUT_GRACE_SECONDS='4'))
        self.assertEqual(code, 1)
        result = report['scenarios'][0]
        self.assertEqual(result['reason'], 'timed out after 1s')
        self.assertLess(result['duration_seconds'], 1 + 4 + 2)
        assert_stopped(self, int(pid_file.read_text()))

    def test_stopping_the_runner_lets_the_running_scenario_clean_up(self):
        started, cleaned = self.dir / 'started', self.dir / 'cleaned'
        child_pid = self.dir / 'child.pid'
        path = self.scenarios / 'a-tidy'
        path.write_text(f'''#!/usr/bin/env python3
# qa-scenario: a-tidy
# qa-mutates: no
# qa-profiles: ci
# qa-description: tidy
import qa
open({str(started)!r}, 'w').write('yes')
try:
    qa.run(['sh', '-c', 'echo $$ > {str(child_pid)}; exec sleep 300'], timeout=600)
finally:
    open({str(cleaned)!r}, 'w').write(str(qa.cleanup_deadline(1000) - qa.stopped_at))
''')
        path.chmod(0o755)
        self.scenario('b-later', mutates='no', profiles='ci')
        report = self.dir / 'report.json'
        env = dict(self.env, QA_TIMEOUT_GRACE_SECONDS='8')
        launch = ('import sys; from pathlib import Path; sys.path.insert(0, sys.argv[1]); '
                  'import run_scenarios as r; sys.exit(r.main(sys.argv[3:], None, Path(sys.argv[2])))')
        runner_proc = subprocess.Popen(
            [sys.executable, '-c', launch, str(QA_DIR), str(self.scenarios),
             '--target', 'http://localhost:1', '--profile', 'ci', '--json', str(report)],
            env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        try:
            deadline = time.monotonic() + 20
            while not child_pid.exists() and time.monotonic() < deadline:
                time.sleep(0.1)
            runner_proc.send_signal(signal.SIGTERM)
            runner_proc.communicate(timeout=30)
        finally:
            if runner_proc.poll() is None:
                runner_proc.kill()
                runner_proc.communicate()
        self.assertEqual(runner_proc.returncode, 1)
        # The scenario's own command is gone too.
        assert_stopped(self, int(child_pid.read_text()))
        # Cleanup ran, with half the grace period for its waits.
        self.assertAlmostEqual(float(cleaned.read_text()), 4.0, delta=0.5)
        self.assertEqual(self.ran(), [])
        results = {r['name']: r for r in json.loads(report.read_text())['scenarios']}
        self.assertIn('runner was stopped', results['b-later']['reason'])

    def test_secrets_never_reach_the_output(self):
        password_file = self.dir / 'pw'
        password_file.write_text('file-held-secret\n')
        jwt = 'eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiJxYSJ9.c2lnbmF0dXJl'
        self.scenario('leaky', mutates='no', body=f'''
echo "password $QA_PASSWORD and $(cat "$QA_PASSWORD_FILE")"
echo "token {jwt}"
echo "Authorization: Bearer opaque-token-value"
echo "url postgres://app:db-password@db:5432/x"
echo '{{"check": "c", "ok": false, "detail": "env-held-secret {jwt}"}}' >> "$QA_EVIDENCE_FILE"
exit 1''')
        env = dict(self.env, QA_PASSWORD='env-held-secret', QA_PASSWORD_FILE=str(password_file))
        code, report, output = self.report(['--target', 'http://localhost:1', '--profile', 'ci'], env)
        text = json.dumps(report) + output
        self.assertEqual(code, 1)
        for secret in ('env-held-secret', 'file-held-secret', jwt, 'opaque-token-value', 'db-password'):
            self.assertNotIn(secret, text)
        self.assertIn('[redacted]', text)


# --- A stub site for the scenarios ----------------------------------------------------------

SERIES_ID = '11111111-1111-1111-1111-111111111111'
PASSWORD = 'stub-password-123'


class StubSite:
    """GraphQL, Keycloak's sign-in and MCP, enough for the scenarios; records what it saw."""

    def __init__(self):
        self.annotations: dict[str, dict] = {}
        self.keys: dict[str, str] = {}
        self.codes: dict[str, str] = {}
        self.authorizations: list[tuple[str, str | None]] = []
        self.data_graphql_tokens: list[str | None] = []
        self.refuse_deletes = 0
        site = self

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def send(self, status, body, content_type='application/json', headers=()):
                data = body if isinstance(body, bytes) else json.dumps(body).encode()
                self.send_response(status)
                self.send_header('Content-Type', content_type)
                self.send_header('Content-Length', str(len(data)))
                for name, value in headers:
                    self.send_header(name, value)
                self.end_headers()
                self.wfile.write(data)

            def body(self):
                return self.rfile.read(int(self.headers.get('Content-Length') or 0))

            def do_GET(self):
                site.get(self)

            def do_POST(self):
                site.post(self)

        self.server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        self.base = f'http://127.0.0.1:{self.server.server_port}'
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)

    def __enter__(self):
        self.thread.start()
        return self

    def __exit__(self, *exc):
        self.server.shutdown()
        self.server.server_close()

    issuer_path = '/idp/realms/econ-graph'

    def token_user(self, handler):
        auth = handler.headers.get('Authorization') or ''
        return 'qa-alice' if auth == 'Bearer ' + self.access_token else None

    access_token = 'eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiJxYS1hbGljZSJ9.c3R1Yi1zaWduYXR1cmU'

    def get(self, h):
        path, _, query = h.path.partition('?')
        if path == f'{self.issuer_path}/.well-known/openid-configuration':
            return h.send(200, {'issuer': self.base + self.issuer_path,
                                'authorization_endpoint': f'{self.base}{self.issuer_path}/auth',
                                'token_endpoint': f'{self.base}{self.issuer_path}/token'})
        if path == f'{self.issuer_path}/auth':
            params = urllib.parse.parse_qs(query)
            self.pending = params
            page = (f'<html><form id="kc-form-login" action="{self.issuer_path}/login?session=abc&amp;tab=1" '
                    'method="post"><input name="username" type="text"/>'
                    '<input name="password" type="password"/>'
                    '<input type="hidden" name="credentialId" value=""/></form></html>')
            return h.send(200, page.encode(), 'text/html',
                          [('Set-Cookie', f'AUTH_SESSION_ID=s1; Path={self.issuer_path}/; Secure; HttpOnly')])
        h.send(404, b'not found', 'text/plain')

    def post(self, h):
        path, _, query = h.path.partition('?')
        raw = h.body()
        if path == f'{self.issuer_path}/login':
            form = urllib.parse.parse_qs(raw.decode())
            if ('AUTH_SESSION_ID=s1' not in (h.headers.get('Cookie') or '') or query != 'session=abc&tab=1'
                    or form.get('username') != ['qa-alice'] or form.get('password') != [PASSWORD]):
                return h.send(200, b'<html>Invalid username or password.</html>', 'text/html')
            code = 'code-123'
            self.codes[code] = self.pending['code_challenge'][0]
            redirect = (f'{self.pending["redirect_uri"][0]}?state={self.pending["state"][0]}'
                        f'&code={code}')
            return h.send(302, b'', 'text/html', [('Location', redirect)])
        if path == f'{self.issuer_path}/token':
            form = urllib.parse.parse_qs(raw.decode())
            verifier = form.get('code_verifier', [''])[0]
            challenge = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).rstrip(b'=').decode()
            if self.codes.get(form.get('code', [''])[0]) != challenge:
                return h.send(400, {'error': 'invalid_grant'})
            return h.send(200, {'access_token': self.access_token, 'token_type': 'Bearer'})
        if path in ('/graphql', '/data/graphql'):
            if path == '/data/graphql':
                self.data_graphql_tokens.append(h.headers.get('Authorization'))
            return h.send(200, self.graphql(json.loads(raw), self.token_user(h)))
        if path == '/mcp':
            return self.mcp(h, json.loads(raw))
        if path == '/broken-mcp':
            return h.send(502, b'<html>Bad Gateway</html>', 'text/html')
        h.send(404, b'not found', 'text/plain')

    def graphql(self, request, user):
        q, v = request['query'], request.get('variables') or {}
        series = {'id': SERIES_ID, 'title': 'Gross Domestic Product', 'externalId': 'GDP',
                  'endDate': '2024-01-01', 'units': 'USD', 'frequency': 'Quarterly',
                  'source': {'name': 'FRED'}}
        if 'createAnnotation' in q:
            if not user:
                return {'errors': [{'message': 'sign in', 'extensions': {'code': 'UNAUTHENTICATED'}}]}
            key = v['input']['idempotencyKey']
            if key not in self.keys:
                annotation_id = f'a{len(self.annotations)}'
                self.keys[key] = annotation_id
                self.annotations[annotation_id] = {
                    'id': annotation_id, 'seriesId': v['input']['seriesId'], 'title': v['input']['title'],
                    'visibility': 'PUBLIC' if v['input'].get('isPublic') else 'PRIVATE', 'user': user}
            row = self.annotations[self.keys[key]]
            return {'data': {'createAnnotation': {k: row[k] for k in ('id', 'seriesId', 'title', 'visibility')}}}
        if 'deleteAnnotation' in q:
            if self.refuse_deletes:
                self.refuse_deletes -= 1
                return {'data': {'deleteAnnotation': False}}
            return {'data': {'deleteAnnotation': self.annotations.pop(v['id'], None) is not None}}
        if 'annotationsForSeries' in q:
            rows = [{k: a[k] for k in ('id', 'title', 'visibility')} for a in self.annotations.values()
                    if a['visibility'] == 'PUBLIC' or a['user'] == user]
            return {'data': {'annotationsForSeries': rows}}
        if 'searchSeries' in q:
            return {'data': {'searchSeries': {'totalCount': 1, 'series': [series]}}}
        if 'seriesData' in q:
            nodes = [{'date': f'202{i}-01-01', 'value': str(100 + i)} for i in range(4)]
            return {'data': {'seriesData': {'totalCount': 4, 'nodes': nodes}}}
        if 'datasets' in q:
            return {'data': {'datasets': [{'id': 'd1', 'code': 'wdi', 'dimensions': [
                {'name': 'area', 'codelist': 'countries', 'codes': [{'code': 'USA'}]},
                {'name': 'indicator', 'codelist': None, 'codes': [{'code': 'NY.GDP.PCAP.CD'}]}]}]}}
        if 'crossSection' in q:
            return {'data': {'crossSection': [
                {'key': 'USA', 'date': '2023-01-01', 'value': '82769.4', 'area': {'kind': 'COUNTRY', 'iso3': 'USA'}},
                {'key': 'WLD', 'date': '2023-01-01', 'value': '1', 'area': {'kind': 'AGGREGATE', 'iso3': None}}]}}
        if 'series(' in q:
            return {'data': {'series': series}}
        return {'errors': [{'message': f'stub cannot answer {q[:60]}'}]}

    def mcp(self, h, request):
        if not self.token_user(h):
            return h.send(401, {'jsonrpc': '2.0', 'id': request.get('id'),
                                'error': {'code': -32001, 'message': 'Authentication required'}})
        method, params = request['method'], request.get('params') or {}

        def text(value):
            return {'content': [{'type': 'text', 'text': json.dumps(value)}], 'is_error': False}

        if method == 'tools/list':
            result = {'tools': [{'name': n} for n in ('search_economic_series', 'get_series_data',
                                                       'get_series_metadata', 'create_data_visualization')]}
        elif params.get('name') == 'search_economic_series':
            result = text({'data': {'searchSeries': {'series': [{'id': SERIES_ID, 'name': 'GDP'}]}}})
        elif params.get('name') == 'get_series_metadata':
            result = text({'data': {'series': {'id': SERIES_ID, 'name': 'GDP'}}})
        else:
            result = text({'data': {'series': {'id': SERIES_ID, 'dataPoints': [{'date': '2024-01-01'}]}}})
        h.send(200, {'jsonrpc': '2.0', 'id': request.get('id'), 'result': result})


class ScenarioTest(unittest.TestCase):
    """The real scenarios, run by the runner against the stub site."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self.tmp.name)
        self.env = {'PATH': os.environ['PATH'], 'QA_WORK_ROOT': str(self.dir),
                    'QA_LAB_TARGET_OK': 'localhost,127.0.0.1,lab.example,site.example'}

    def tearDown(self):
        self.tmp.cleanup()

    def run_scenarios(self, target, profile, only, extra=None):
        out = self.dir / 'report.json'
        code, stdout, stderr = run_main(['--target', target, '--profile', profile, '--only', only,
                                         '--json', str(out)], dict(self.env, **(extra or {})))
        report = json.loads(out.read_text()) if out.exists() else None
        return code, {r['name']: r for r in report['scenarios']} if report else {}, stdout + stderr

    def checks(self, result):
        return {c['check']: c['ok'] for c in result['evidence']}

    def test_anonymous_api(self):
        with StubSite() as site:
            code, results, _ = self.run_scenarios(site.base, 'ci', 'anonymous-api')
        self.assertEqual(code, 0, results)
        self.assertEqual(self.checks(results['anonymous-api']), {
            'search': True, 'open series': True, 'chart data': True, 'world map': True,
            '/data/graphql without a token': True})
        self.assertEqual(site.data_graphql_tokens, [None, None])

    def test_annotation_round_trip_as_a_qa_account_in_production(self):
        password_file = self.dir / 'pw'
        password_file.write_text(PASSWORD)
        with StubSite() as site:
            code, results, output = self.run_scenarios(
                site.base, 'production', 'annotation-round-trip',
                {'QA_USERNAME': 'qa-alice', 'QA_PASSWORD_FILE': str(password_file)})
        self.assertEqual(code, 0, results)
        self.assertEqual(self.checks(results['annotation-round-trip']), {
            'sign in': True, 'create': True, 'idempotent retry': True, 'stored once': True,
            'private to its author': True, 'delete': True})
        self.assertEqual(site.annotations, {})
        self.assertEqual(len(site.keys), 1)
        text = json.dumps(results) + output
        self.assertNotIn(PASSWORD, text)
        self.assertNotIn(site.access_token, text)

    def test_a_delete_that_does_nothing_fails_and_is_retried(self):
        with StubSite() as site:
            site.refuse_deletes = 1
            code, results, _ = self.run_scenarios(site.base, 'ci', 'annotation-round-trip',
                                                  {'QA_USERNAME': 'qa-alice', 'QA_PASSWORD': PASSWORD})
        self.assertEqual(code, 1)
        checks = self.checks(results['annotation-round-trip'])
        self.assertFalse(checks['delete'])
        self.assertTrue(checks['clean up'])
        self.assertEqual(site.annotations, {})

    def test_a_wrong_password_fails_sign_in(self):
        with StubSite() as site:
            code, results, _ = self.run_scenarios(site.base, 'ci', 'annotation-round-trip',
                                                  {'QA_USERNAME': 'qa-alice', 'QA_PASSWORD': 'wrong-one'})
        self.assertEqual(code, 1)
        self.assertIn('did not accept', results['annotation-round-trip']['reason'])

    def test_annotation_round_trip_skips_without_an_account(self):
        with StubSite() as site:
            code, results, _ = self.run_scenarios(site.base, 'ci', 'annotation-round-trip')
        self.assertEqual(code, 0)
        self.assertEqual(results['annotation-round-trip']['status'], 'skip')

    def test_mcp_reads(self):
        with StubSite() as site:
            code, results, _ = self.run_scenarios(site.base, 'ci', 'mcp-reads',
                                                  {'QA_USERNAME': 'qa-alice', 'QA_PASSWORD': PASSWORD})
        self.assertEqual(code, 0, results)
        self.assertEqual(self.checks(results['mcp-reads']), {
            'refuses an anonymous caller': True, 'sign in': True, 'tools/list': True,
            'search_economic_series': True, 'get_series_metadata': True, 'get_series_data': True})

    def test_mcp_reads_fail_on_a_broken_endpoint(self):
        with StubSite() as site:
            code, results, _ = self.run_scenarios(site.base, 'ci', 'mcp-reads',
                                                  {'QA_MCP_URL': site.base + '/broken-mcp'})
        self.assertEqual(code, 1, results)
        self.assertEqual(results['mcp-reads']['status'], 'fail')

    def test_mcp_reads_skip_where_nothing_serves_mcp(self):
        with StubSite() as site:
            code, results, _ = self.run_scenarios(site.base, 'ci', 'mcp-reads',
                                                  {'QA_MCP_URL': site.base + '/no-mcp-here'})
        self.assertEqual(code, 0)
        self.assertEqual(results['mcp-reads']['status'], 'skip')

    def test_catalog_roles_skip_without_an_admin(self):
        code, results, _ = self.run_scenarios('http://127.0.0.1:9', 'ci', 'catalog-roles')
        self.assertEqual(code, 0)
        self.assertIn('Keycloak admin', results['catalog-roles']['reason'])

    def fake_check_public_site(self, exit_code):
        script = self.dir / 'check-public-site.sh'
        script.write_text(f'''#!/usr/bin/env bash
echo "$*" > "{self.dir}/args"
echo "==> Checking"
echo "  ok    GET / serves the app (200)"
echo "  {"FAIL" if exit_code else "ok  "}  /metrics not served (404)"
exit {exit_code}
''')
        script.chmod(0o755)
        return {'QA_CHECK_PUBLIC_SITE': str(script)}

    def test_public_site_calls_the_existing_check(self):
        for profile, route, route_args in (
                ('production', None, '--host site.example --via-cloudflare'),
                ('lab', None, '--host site.example --via-cloudflare'),
                ('lab', 'gateway', '--host site.example')):
            with self.subTest(profile=profile, route=route):
                extra = self.fake_check_public_site(0)
                if route:
                    extra['QA_PUBLIC_SITE_ROUTE'] = route
                code, results, _ = self.run_scenarios('https://site.example', profile, 'public-site',
                                                      extra)
                self.assertEqual(code, 0, results)
                self.assertEqual((self.dir / 'args').read_text().strip(), route_args)
                self.assertEqual(self.checks(results['public-site']),
                                 {'GET / serves the app (200)': True, '/metrics not served (404)': True})
        code, results, _ = self.run_scenarios('https://site.example', 'lab', 'public-site',
                                              self.fake_check_public_site(1))
        self.assertEqual(code, 1)
        self.assertFalse(self.checks(results['public-site'])['/metrics not served (404)'])

    def test_public_site_skips_a_plain_http_target(self):
        code, results, _ = self.run_scenarios('http://localhost:18081', 'ci', 'public-site',
                                              self.fake_check_public_site(0))
        self.assertEqual(code, 0)
        self.assertEqual(results['public-site']['status'], 'skip')
        self.assertFalse((self.dir / 'args').exists())

    def test_conversion_checks(self):
        report = self.dir / 'conversion.json'
        table = {'name': 'economic_series', 'before': {'rows': 3, 'checksum': 'md5:a'},
                 'after': {'rows': 3, 'checksum': 'md5:a'}}
        annotation = {'id': 'x', 'series_id_before': 's1', 'series_id_after': 's1'}
        report.write_text(json.dumps({'tables': [table], 'annotations': [annotation]}))
        code, results, _ = self.run_scenarios('http://lab.example', 'lab', 'conversion-checks',
                                              {'QA_CONVERSION_REPORT': str(report)})
        self.assertEqual(code, 0, results)
        moved = dict(annotation, series_id_after='s2')
        changed = dict(table, after={'rows': 3, 'checksum': 'md5:b'})
        report.write_text(json.dumps({'tables': [changed], 'annotations': [moved]}))
        code, results, _ = self.run_scenarios('http://lab.example', 'lab', 'conversion-checks',
                                              {'QA_CONVERSION_REPORT': str(report)})
        self.assertEqual(code, 1)
        self.assertEqual(self.checks(results['conversion-checks']), {
            'report lists tables': True, 'economic_series unchanged': False,
            'annotations on their original series': False})
        report.write_text(json.dumps({'tables': [table]}))
        code, results, _ = self.run_scenarios('http://lab.example', 'lab', 'conversion-checks',
                                              {'QA_CONVERSION_REPORT': str(report)})
        self.assertEqual(code, 1)
        self.assertIn('not a conversion report', results['conversion-checks']['reason'])

    def test_crawl_sample_runs_one_source_at_a_time(self):
        log = self.dir / 'crawl.log'
        hook = self.dir / 'crawl.sh'
        hook.write_text(f'''#!/usr/bin/env bash
echo "start $QA_CRAWL_SOURCE" >> "{log}"
n=1; [ "$QA_CRAWL_SOURCE" = BLS ] && n=0
echo "{{\\"source\\": \\"$QA_CRAWL_SOURCE\\", \\"series\\": \\"S1\\", \\"ok\\": true, \\"observations\\": $n}}"
echo "end $QA_CRAWL_SOURCE" >> "{log}"
''')
        hook.chmod(0o755)
        code, results, _ = self.run_scenarios('http://lab.example', 'lab', 'crawl-sample', {
            'QA_CRAWL_SAMPLE_COMMAND': str(hook), 'QA_CRAWL_SOURCES': 'FRED,BLS',
            'QA_CRAWL_INTERVAL_SECONDS': '0'})
        self.assertEqual(code, 1)
        self.assertEqual(self.checks(results['crawl-sample']), {'crawl FRED': True, 'crawl BLS': False})
        self.assertEqual(log.read_text().split('\n')[:4], ['start FRED', 'end FRED', 'start BLS', 'end BLS'])

    def test_crawl_sample_needs_a_source(self):
        code, results, _ = self.run_scenarios('http://lab.example', 'lab', 'crawl-sample', {
            'QA_CRAWL_SAMPLE_COMMAND': 'true', 'QA_CRAWL_SOURCES': ' , '})
        self.assertEqual(code, 1)
        self.assertIn('names no source', results['crawl-sample']['reason'])

    def test_a_crawl_hook_that_overruns_is_stopped(self):
        pid_file = self.dir / 'hook.pid'
        code, results, _ = self.run_scenarios('http://lab.example', 'lab', 'crawl-sample', {
            'QA_CRAWL_SAMPLE_COMMAND': f'sleep 300 & echo $! > "{pid_file}"; wait',
            'QA_CRAWL_SOURCES': 'FRED', 'QA_CRAWL_HOOK_TIMEOUT_SECONDS': '1'})
        self.assertEqual(code, 1)
        self.assertIn('overran', results['crawl-sample']['reason'])
        assert_stopped(self, int(pid_file.read_text()))

    def test_lab_only_scenarios_are_not_run_outside_labs(self):
        # Asked for by name, they are an error; in a full run, the runner skips them.
        code, results, output = self.run_scenarios('https://site.example', 'ci',
                                                   'backup-restore,conversion-checks,crawl-sample')
        self.assertEqual(code, 2)
        for name in ('backup-restore', 'conversion-checks', 'crawl-sample'):
            self.assertIn(f'{name}: not in the ci profile', output)
        for name in ('backup-restore', 'conversion-checks', 'crawl-sample'):
            self.assertIn('not in the ci profile', runner.admit(BY_NAME[name], 'ci', {}))

    def test_backup_restore_keeps_database_passwords_off_command_lines(self):
        # Stand-in psql, pg_dump and pg_restore that log their arguments and PGPASSWORD.
        bin_dir, log = self.dir / 'bin', self.dir / 'pg.log'
        bin_dir.mkdir()
        stub = f'''#!/usr/bin/env python3
import json, os, sys
name = os.path.basename(sys.argv[0])
with open({str(log)!r}, 'a') as out:
    out.write(json.dumps({{'name': name, 'argv': sys.argv[1:],
                          'password': os.environ.get('PGPASSWORD')}}) + '\\n')
if name == 'pg_dump':
    path = next(a for a in sys.argv if a.startswith('--file='))[len('--file='):]
    open(path, 'wb').write(b'dump')
if name == 'psql':
    sql = sys.argv[sys.argv.index('-c') + 1]
    if 'pg_get_userbyid' in sql:
        print('public\\x1fseries\\x1fapp')
    elif 'count(*)' in sql:
        print('0\\x1f5')
'''
        for tool in ('psql', 'pg_dump', 'pg_restore'):
            (bin_dir / tool).write_text(stub)
            (bin_dir / tool).chmod(0o755)
        code, results, output = self.run_scenarios('http://lab.example', 'lab', 'backup-restore', {
            'PATH': f'{bin_dir}:{os.environ["PATH"]}',
            'QA_BACKUP_SOURCE_DATABASE_URL': 'postgres://app:src-pw-1234@127.0.0.1:5439/app',
            'QA_SCRATCH_DATABASE_URL': 'postgres://admin:scr%40tch-pw@127.0.0.1:5439/app_scratch'})
        self.assertEqual(code, 0, results)
        calls = [json.loads(line) for line in log.read_text().splitlines()]
        self.assertEqual({c['name'] for c in calls}, {'psql', 'pg_dump', 'pg_restore'})
        for call in calls:
            argv = ' '.join(call['argv'])
            for secret in ('src-pw-1234', 'scr@tch-pw', 'scr%40tch-pw'):
                self.assertNotIn(secret, argv, call)
        by_db = {c['argv'][c['argv'].index('-d') + 1]: c['password'] for c in calls}
        self.assertEqual(by_db['postgres://app@127.0.0.1:5439/app'], 'src-pw-1234')
        self.assertEqual(by_db['postgres://admin@127.0.0.1:5439/app_scratch'], 'scr@tch-pw')
        self.assertEqual(by_db['postgres://admin@127.0.0.1:5439/postgres'], 'scr@tch-pw')
        self.assertNotIn('src-pw-1234', json.dumps(results) + output)

    def test_backup_restore_refuses_a_scratch_database_without_scratch_in_its_name(self):
        code, results, _ = self.run_scenarios('http://lab.example', 'lab', 'backup-restore', {
            'QA_BACKUP_SOURCE_DATABASE_URL': 'postgres://u:pw-abcdef@127.0.0.1:9/app',
            'QA_SCRATCH_DATABASE_URL': 'postgres://u:pw-abcdef@127.0.0.1:9/app_copy'})
        self.assertEqual(code, 1)
        self.assertIn('must contain "scratch"', results['backup-restore']['reason'])
        self.assertNotIn('pw-abcdef', json.dumps(results))


if __name__ == '__main__':
    unittest.main()
