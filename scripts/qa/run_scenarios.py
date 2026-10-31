#!/usr/bin/env python3
"""Run the QA scenario suite against a deployed EconGraph site (see docs/release/qa-scenarios.md).

Usage:
  scripts/qa/run-scenarios.sh --target BASE_URL --profile {lab,ci,production}
                              [--only NAME,...] [--require-pass NAME,...] [--json OUT.json]
  scripts/qa/run-scenarios.sh --list

Each scenario is an executable in scripts/qa/scenarios/ whose header declares it:

  # qa-scenario: annotation-round-trip
  # qa-mutates: yes
  # qa-profiles: lab, ci, production
  # qa-timeout: 300
  # qa-description: Sign in, create an annotation, retry it with the same key, delete it.

A scenario whose `qa-mutates` is missing or anything but `no` counts as changing state.

The production profile runs only scenarios that change nothing, plus the annotation round
trip, from this directory only, when a dedicated QA account (a `qa-` username) is configured.
The lab and ci profiles run only against hosts named in QA_LAB_TARGET_OK, and never against a
production host. --require-pass names scenarios that must pass, not skip. Credentials come from the environment or from files named in it, never
from arguments, and are redacted from everything the runner prints or writes.

Exit status: 0 when no scenario failed, 1 when one did, 2 for a usage error or a refusal.
"""

from __future__ import annotations

import argparse
import dataclasses
import datetime as dt
import json
import os
import re
import secrets
import signal
import subprocess
import sys
import tempfile
import threading
import time
import urllib.parse
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_SCENARIO_DIR = Path(__file__).resolve().parent / 'scenarios'

PROFILES = ('lab', 'ci', 'production')

# The only state-changing scenario the production profile may run, and only with a QA account:
# its name, and the one file that may carry it. Fixed here rather than declared by a scenario,
# so no scenario, and no file elsewhere declaring the same name, can grant itself an exception.
PRODUCTION_MUTATING_EXCEPTIONS = {
    'annotation-round-trip': (DEFAULT_SCENARIO_DIR / 'annotation-round-trip.py').resolve(),
}

# The QA account's username must start with this for the production exception to apply: the
# dedicated QA users (config/keycloak/qa/) are named qa-*, real accounts are not.
QA_ACCOUNT_PREFIX = 'qa-'

# Hosts the lab and CI profiles refuse: they run state-changing scenarios. QA_PRODUCTION_HOSTS
# (comma-separated) adds more.
PRODUCTION_HOSTS = frozenset({'econgraph.com', 'www.econgraph.com'})

# A scenario exits with this to say it was skipped (the automake convention).
EXIT_SKIP = 77

DEFAULT_TIMEOUT = 600
# How long an overrunning scenario gets to clean up after SIGTERM (QA_TIMEOUT_GRACE_SECONDS).
DEFAULT_GRACE = 60

# Environment variable names whose values are secrets, and *_FILE variables naming a file
# whose content is one.
SECRET_NAME = re.compile(r'(PASSWORD|SECRET|TOKEN|CREDENTIAL)', re.IGNORECASE)
# A JSON Web Token, a bearer header, and the password in a URL's userinfo.
JWT = re.compile(r'eyJ[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]*')
BEARER = re.compile(r'(Bearer\s+)[A-Za-z0-9._~+/=-]+', re.IGNORECASE)
URL_PASSWORD = re.compile(r'(\w+://[^/\s:@]*:)[^@\s/]+@')

REDACTED = '[redacted]'
OUTPUT_TAIL_LINES = 40


@dataclasses.dataclass(frozen=True)
class Scenario:
    """One scenario's declaration, read from its header."""

    name: str
    path: Path
    mutates: bool
    profiles: tuple[str, ...]
    timeout: int
    description: str
    # Why the declaration is suspect (e.g. no qa-mutates line); the scenario is then treated
    # as changing state.
    declaration_problem: str | None = None


HEADER = re.compile(r'^\s*(?:#|//)\s*qa-([a-z]+):\s*(.*?)\s*$')


def parse_scenario(path: Path) -> Scenario:
    """Read a scenario's declaration from the first 40 lines of its file."""
    fields: dict[str, str] = {}
    with path.open(encoding='utf-8') as handle:
        for _, line in zip(range(40), handle):
            match = HEADER.match(line)
            if match:
                fields.setdefault(match.group(1), match.group(2))
    name = fields.get('scenario') or path.stem
    problem = None
    mutates_text = fields.get('mutates', '').lower()
    if mutates_text not in ('yes', 'no'):
        problem = f'qa-mutates is {mutates_text!r}, not yes or no; treated as yes'
    mutates = mutates_text != 'no'
    profiles = tuple(p.strip() for p in fields.get('profiles', '').split(',') if p.strip())
    unknown = [p for p in profiles if p not in PROFILES]
    if unknown or not profiles:
        problem = problem or f'qa-profiles {fields.get("profiles")!r} names no known profile'
        profiles = tuple(p for p in profiles if p in PROFILES)
    try:
        timeout = int(fields.get('timeout', DEFAULT_TIMEOUT))
    except ValueError:
        timeout = DEFAULT_TIMEOUT
    return Scenario(
        name=name,
        path=path,
        mutates=mutates,
        profiles=profiles,
        timeout=timeout,
        description=fields.get('description', ''),
        declaration_problem=problem,
    )


def discover(directory: Path) -> list[Scenario]:
    """Every executable scenario in `directory`, in name order."""
    scenarios = [
        parse_scenario(path)
        for path in sorted(directory.iterdir())
        if path.is_file() and os.access(path, os.X_OK) and not path.name.startswith('.')
    ]
    names = [s.name for s in scenarios]
    duplicates = sorted({n for n in names if names.count(n) > 1})
    if duplicates:
        raise ValueError(f'duplicate scenario names: {", ".join(duplicates)}')
    return scenarios


def read_secret(env: dict[str, str], name: str) -> str:
    """The value of `name`, or the content of the file `name_FILE` names, or ''."""
    value = env.get(name, '')
    if value:
        return value
    file_name = env.get(f'{name}_FILE', '')
    if file_name:
        try:
            return Path(file_name).read_text(encoding='utf-8').strip()
        except OSError:
            return ''
    return ''


def qa_account(env: dict[str, str]) -> tuple[str, bool]:
    """The configured QA username, and whether it has both a username and a password."""
    username = env.get('QA_USERNAME', '').strip()
    return username, bool(username and read_secret(env, 'QA_PASSWORD'))


def admit(scenario: Scenario, profile: str, env: dict[str, str]) -> str | None:
    """Why `scenario` must not run under `profile`, or None when it may.

    The production profile refuses every scenario that changes state, or that does not
    declare clearly that it does not, except those in PRODUCTION_MUTATING_EXCEPTIONS when a
    dedicated QA account is configured.
    """
    if profile == 'production' and scenario.mutates:
        if PRODUCTION_MUTATING_EXCEPTIONS.get(scenario.name) != scenario.path.resolve():
            return 'refused: the production profile runs no scenario that changes state'
    if profile not in scenario.profiles:
        allowed = ', '.join(scenario.profiles) or 'none'
        return f'not in the {profile} profile (runs in: {allowed})'
    if profile != 'production' or not scenario.mutates:
        return None
    username, complete = qa_account(env)
    if not complete:
        return (
            'refused: the production profile runs this only as a dedicated QA account; '
            'set QA_USERNAME and QA_PASSWORD or QA_PASSWORD_FILE'
        )
    if not username.startswith(QA_ACCOUNT_PREFIX):
        return (
            f'refused: QA_USERNAME must name a dedicated QA account '
            f'(a {QA_ACCOUNT_PREFIX}* user) under the production profile'
        )
    return None


def production_hosts(env: dict[str, str]) -> set[str]:
    extra = {h.strip().rstrip('.').lower() for h in env.get('QA_PRODUCTION_HOSTS', '').split(',')
             if h.strip()}
    return set(PRODUCTION_HOSTS) | extra


def lab_targets(env: dict[str, str]) -> set[str]:
    """The hosts QA_LAB_TARGET_OK names: the only targets the lab and ci profiles accept."""
    return {h.strip().rstrip('.').lower() for h in env.get('QA_LAB_TARGET_OK', '').split(',')
            if h.strip()}


def check_target(target: str, profile: str, env: dict[str, str]) -> str | None:
    """Why `target` is unusable under `profile`, or None."""
    parsed = urllib.parse.urlsplit(target)
    if parsed.scheme not in ('http', 'https') or not parsed.hostname:
        return f'--target must be an http(s) base URL, not {target!r}'
    if parsed.username or parsed.password:
        return '--target must not carry credentials; use QA_USERNAME and QA_PASSWORD_FILE'
    if parsed.query or parsed.fragment:
        return '--target must be a base URL without a query or fragment'
    # `econgraph.com.` (the fully qualified form) is the same host.
    host = parsed.hostname.rstrip('.').lower()
    if profile == 'production':
        return None
    if host in production_hosts(env):
        return (
            f'refusing the {profile} profile against {parsed.hostname}: it runs state-changing '
            'scenarios; use --profile production for the production site'
        )
    # By name only: localhost or an address on the production host reaches production as
    # surely as its public name does, so the lab and ci profiles accept only hosts named for them.
    if host not in lab_targets(env):
        return (
            f'refusing the {profile} profile against {parsed.hostname}: it runs state-changing '
            f'scenarios, so its target must be named in QA_LAB_TARGET_OK '
            f'(e.g. QA_LAB_TARGET_OK={host} for a lab or the release e2e stack)'
        )
    return None


class Redactor:
    """Removes secret values from text: those in the environment, tokens and URL passwords."""

    def __init__(self, env: dict[str, str]):
        values = set()
        for name, value in env.items():
            if not SECRET_NAME.search(name):
                continue
            if name.endswith('_FILE'):
                try:
                    value = Path(value).read_text(encoding='utf-8').strip()
                except OSError:
                    continue
            if len(value) >= 4:
                values.add(value)
        # Longest first, so a secret containing another is removed whole.
        self.values = sorted(values, key=len, reverse=True)

    def __call__(self, text: str) -> str:
        for value in self.values:
            text = text.replace(value, REDACTED)
        text = JWT.sub(REDACTED, text)
        text = BEARER.sub(lambda m: m.group(1) + REDACTED, text)
        return URL_PASSWORD.sub(lambda m: m.group(1) + REDACTED + '@', text)

    def deep(self, value):
        """Redact every string in a JSON-like value."""
        if isinstance(value, str):
            return self(value)
        if isinstance(value, list):
            return [self.deep(v) for v in value]
        if isinstance(value, dict):
            return {self(str(k)): self.deep(v) for k, v in value.items()}
        return value


def failed(check: dict) -> bool:
    """A check failed unless it says ok or skipped."""
    return not check.get('ok', False) and not check.get('skipped', False)


def read_evidence(path: Path) -> tuple[list, str | None]:
    """A scenario's evidence file: JSON lines, each a check or a `{"skip": reason}`."""
    checks, skip_reason = [], None
    if not path.exists():
        return checks, skip_reason
    for line in path.read_text(encoding='utf-8', errors='replace').splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            entry = json.loads(line)
        except json.JSONDecodeError:
            checks.append({'check': 'evidence', 'ok': False, 'detail': f'unparsable line: {line[:200]}'})
            continue
        if isinstance(entry, dict) and 'skip' in entry:
            skip_reason = str(entry['skip'])
        else:
            checks.append(entry)
    return checks, skip_reason


# The running scenario's process group, and whether the runner was asked to stop.
_current_group: int | None = None
_last_group: int | None = None
_stopping = False


_grace = float(DEFAULT_GRACE)


def _stop_group(group: int) -> None:
    """SIGTERM a scenario's group so it cleans up, and SIGKILL it after the grace period."""
    _signal_group(group, signal.SIGTERM)
    timer = threading.Timer(_grace, _signal_group, (group, signal.SIGKILL))
    timer.daemon = True
    timer.start()


def _on_sigterm(signum, frame):
    """Stop (SIGTERM or SIGINT): stop the running scenario and run nothing more.

    `_stopping` is set before `_current_group` is read, and run_bounded sets `_current_group`
    before it reads `_stopping`, so a scenario starting as the signal arrives is stopped by one
    side or the other (a second SIGTERM is harmless)."""
    global _stopping
    if _stopping:
        return
    _stopping = True
    group = _current_group
    if group is not None:
        _stop_group(group)


def _signal_group(pgid: int, sig: int) -> None:
    try:
        os.killpg(pgid, sig)
    except (ProcessLookupError, PermissionError):
        pass


# Set to a value unique to each scenario run in its environment. Every process the scenario starts
# inherits it, whatever process group or session it moves to, so the runner can find whatever is
# left once the scenario has ended.
RUN_MARKER = 'QA_RUN_MARKER'


def _proc_stat(pid: str) -> tuple[str, int, str] | None:
    """(command name, process group, state) of a /proc pid, or None once it has gone."""
    try:
        stat = Path(f'/proc/{pid}/stat').read_text()
    except OSError:
        return None
    name = stat[stat.index('(') + 1:stat.rindex(')')]
    fields = stat[stat.rindex(')') + 2:].split()
    return name, int(fields[2]), fields[0]


def stop_leftovers(marker: str, group: int) -> list[str]:
    """Kill every process still carrying this run's marker (see RUN_MARKER) outside the
    scenario's process group, which run_bounded has already killed. Those escaped it: commands
    started in their own session, such as qa.run's, that outlived the scenario, or a setsid.
    Returns them as `name (pid)`. Without /proc (macOS) finds nothing."""
    needle = f'{RUN_MARKER}={marker}'.encode()
    found = []
    for entry in Path('/proc').glob('[0-9]*') if Path('/proc').is_dir() else ():
        pid = entry.name
        try:
            environ = (entry / 'environ').read_bytes()
        except OSError:
            continue
        stat = _proc_stat(pid)
        if needle not in environ.split(b'\0') or stat is None or stat[1] == group or stat[2] in 'ZX':
            continue
        try:
            os.kill(int(pid), signal.SIGKILL)
        except ProcessLookupError:
            continue
        found.append(f'{stat[0]} ({pid})')
    return found


def run_bounded(command: list[str], env: dict[str, str], timeout: float,
                grace: float) -> tuple[int, bytes, bool]:
    """Run a scenario in its own process group: its exit status, output, and whether it overran.

    On overrunning, the group gets SIGTERM (Python scenarios then stop their own commands and run
    their cleanup), and after `grace` seconds SIGKILL, so nothing it started keeps running.
    """
    global _current_group, _last_group
    proc = subprocess.Popen(command, env=env, cwd=REPO_ROOT, stdin=subprocess.DEVNULL,
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                            start_new_session=True)
    _current_group = _last_group = proc.pid
    if _stopping:
        _stop_group(proc.pid)
    try:
        return _wait_bounded(proc, timeout, grace)
    finally:
        _current_group = None


def _wait_bounded(proc: subprocess.Popen, timeout: float, grace: float) -> tuple[int, bytes, bool]:
    try:
        output, _ = proc.communicate(timeout=timeout)
        _signal_group(proc.pid, signal.SIGKILL)
        return proc.returncode, output, False
    except subprocess.TimeoutExpired:
        pass
    _signal_group(proc.pid, signal.SIGTERM)
    try:
        output, _ = proc.communicate(timeout=grace)
    except subprocess.TimeoutExpired:
        _signal_group(proc.pid, signal.SIGKILL)
        try:
            output, _ = proc.communicate(timeout=10)
        except subprocess.TimeoutExpired:
            # Something outside the group still holds the output pipe.
            proc.kill()
            output = b''
    # Whatever of the group outlived the scenario itself.
    _signal_group(proc.pid, signal.SIGKILL)
    return -1, output or b'', True


def run_scenario(
    scenario: Scenario,
    target: str,
    profile: str,
    env: dict[str, str],
    redact: Redactor,
    work_root: Path,
) -> dict:
    """Run one admitted scenario; its result, redacted."""
    work = work_root / scenario.name
    work.mkdir(parents=True, exist_ok=True)
    evidence_file = work / 'evidence.jsonl'
    child_env = dict(env)
    child_env.update(
        QA_TARGET=target.rstrip('/'),
        QA_PROFILE=profile,
        QA_SCENARIO=scenario.name,
        QA_REPO_ROOT=str(REPO_ROOT),
        QA_EVIDENCE_FILE=str(evidence_file),
        QA_WORK_DIR=str(work),
        # How long the scenario has to clean up after SIGTERM.
        QA_TIMEOUT_GRACE_SECONDS=str(_grace),
        **{RUN_MARKER: f'{scenario.name}-{secrets.token_hex(8)}'},
        PYTHONPATH=os.pathsep.join(
            p for p in (str(Path(__file__).resolve().parent / 'lib'), env.get('PYTHONPATH', '')) if p
        ),
    )
    started = time.monotonic()
    code, raw, timed_out = run_bounded([str(scenario.path)], child_env, scenario.timeout, _grace)
    # run_bounded returns once the scenario's process group is gone; anything it started outside
    # that group is still running.
    leftovers = stop_leftovers(child_env[RUN_MARKER], _last_group)
    output = raw.decode('utf-8', errors='replace')
    duration = round(time.monotonic() - started, 2)

    checks, skip_reason = read_evidence(evidence_file)
    tail = output.splitlines()[-OUTPUT_TAIL_LINES:]
    if timed_out:
        status, reason = 'fail', f'timed out after {scenario.timeout}s'
    elif code == 0:
        failures = [c for c in checks if failed(c)]
        skipped = [c['check'] for c in checks if c.get('skipped')]
        if failures:
            status, reason = 'fail', f'exited 0 but {len(failures)} check(s) failed'
        else:
            status = 'pass'
            reason = f'skipped checks: {", ".join(skipped)}' if skipped else None
    elif code == EXIT_SKIP and not any(failed(c) for c in checks):
        status = 'skip'
        reason = skip_reason or (tail[-1] if tail else 'skipped')
    else:
        failures = [c for c in checks if failed(c)]
        status = 'fail'
        reason = (
            f'{failures[0].get("check")}: {failures[0].get("detail", "")}'
            if failures
            else f'exited {code}' + (f': {tail[-1]}' if tail else '')
        )
    if leftovers:
        # A scenario must stop everything it starts, also when stopped for overrunning.
        left = f'left running, now killed: {", ".join(leftovers)}'
        status, reason = 'fail', f'{reason}; {left}' if reason and status == 'fail' else left
    return redact.deep(
        {
            'name': scenario.name,
            'description': scenario.description,
            'mutates': scenario.mutates,
            'status': status,
            'reason': reason,
            'duration_seconds': duration,
            'exit_code': code,
            'evidence': checks,
            'output_tail': tail,
        }
    )


def not_run(scenario: Scenario, reason: str) -> dict:
    return {
        'name': scenario.name,
        'description': scenario.description,
        'mutates': scenario.mutates,
        'status': 'skip',
        'reason': reason,
        'duration_seconds': 0.0,
        'exit_code': None,
        'evidence': [],
        'output_tail': [],
    }


def summary_lines(report: dict) -> list[str]:
    lines = [f'QA scenarios: {report["target"]} (profile {report["profile"]})']
    width = max((len(s['name']) for s in report['scenarios']), default=10)
    for result in report['scenarios']:
        line = f'  {result["status"].upper():4}  {result["name"]:{width}}  {result["duration_seconds"]:7.1f}s'
        if result['reason']:
            line += f'  {result["reason"]}'
        lines.append(line)
        if result['status'] == 'fail':
            for check in result['evidence']:
                if failed(check):
                    lines.append(f'          - {check.get("check")}: {check.get("detail", "")}')
    counts = report['counts']
    lines.append(f'{counts["pass"]} passed, {counts["fail"]} failed, {counts["skip"]} skipped')
    return lines


def run_all(selected, args, env, redact, work_parent) -> list[dict]:
    """Run the admitted scenarios one at a time; each one's result."""
    results = []
    with tempfile.TemporaryDirectory(prefix='qa-scenarios-', dir=work_parent) as work_root:
        for scenario in selected:
            if _stopping:
                results.append(not_run(scenario, 'not run: the runner was stopped'))
                continue
            why = admit(scenario, args.profile, env)
            if why:
                results.append(not_run(scenario, why))
                continue
            print(f'==> {scenario.name}', flush=True)
            result = run_scenario(scenario, args.target, args.profile, env, redact, Path(work_root))
            if scenario.declaration_problem:
                result['declaration_problem'] = scenario.declaration_problem
            results.append(result)
            print(f'    {result["status"]} ({result["duration_seconds"]}s)', flush=True)
    return results


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        prog='run-scenarios.sh',
        allow_abbrev=False,
        description='Run the QA scenario suite against a deployed site.',
        epilog='Credentials come from the environment (QA_USERNAME, QA_PASSWORD or '
        'QA_PASSWORD_FILE, ...), never from arguments. See docs/release/qa-scenarios.md.',
    )
    parser.add_argument('--target', help='base URL of the site, e.g. https://lab-1.example')
    parser.add_argument('--profile', choices=PROFILES)
    parser.add_argument('--only', default='', help='comma-separated scenario names')
    parser.add_argument('--json', dest='json_out', help='write the JSON report here')
    parser.add_argument('--require-pass', default='',
                        help='comma-separated scenarios that must pass; a skip of one fails the run')
    parser.add_argument('--list', action='store_true', help='list the scenarios and exit')
    args = parser.parse_args(argv)
    if not args.list and (not args.target or not args.profile):
        parser.error('--target and --profile are required')
    return args


def main(argv: list[str] | None = None, env: dict[str, str] | None = None,
         scenario_dir: Path = DEFAULT_SCENARIO_DIR) -> int:
    """The command line. `scenario_dir` exists for the runner's own tests; the command line always
    runs scripts/qa/scenarios/."""
    global _grace, _stopping
    args = parse_args(sys.argv[1:] if argv is None else argv)
    env = dict(os.environ if env is None else env)
    try:
        scenarios = discover(scenario_dir)
    except ValueError as error:
        print(f'error: {error}', file=sys.stderr)
        return 2

    if args.list:
        for s in scenarios:
            print(
                f'{s.name:24} mutates={"yes" if s.mutates else "no ":3} '
                f'profiles={",".join(s.profiles):22} {s.description}'
            )
        return 0

    refusal = check_target(args.target, args.profile, env)
    if refusal:
        print(f'error: {refusal}', file=sys.stderr)
        return 2

    by_name = {s.name: s for s in scenarios}
    only = [n.strip() for n in args.only.split(',') if n.strip()]
    required = [n.strip() for n in args.require_pass.split(',') if n.strip()]
    unknown = [n for n in only + required if n not in by_name]
    if unknown:
        print(f'error: unknown scenario(s): {", ".join(unknown)}', file=sys.stderr)
        return 2
    selected = [by_name[n] for n in only] if only else scenarios

    # Asking for a scenario the profile won't run, by --only or --require-pass, is an error,
    # before anything runs.
    asked = [by_name[n] for n in dict.fromkeys(only + required)]
    refused = [(s, admit(s, args.profile, env)) for s in asked]
    refused = [(s, why) for s, why in refused if why]
    if refused:
        for s, why in refused:
            print(f'error: {s.name}: {why}', file=sys.stderr)
        return 2
    missing = [n for n in required if n not in {s.name for s in selected}]
    if missing:
        print(f'error: --require-pass names scenarios --only leaves out: {", ".join(missing)}',
              file=sys.stderr)
        return 2

    redact = Redactor(env)
    started_at = dt.datetime.now(dt.timezone.utc)
    work_parent = env.get('QA_WORK_ROOT') or None
    _stopping = False
    _grace = float(env.get('QA_TIMEOUT_GRACE_SECONDS') or DEFAULT_GRACE)
    previous = {sig: signal.signal(sig, _on_sigterm) for sig in (signal.SIGTERM, signal.SIGINT)}
    try:
        results = run_all(selected, args, env, redact, work_parent)
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)

    counts = {k: sum(r['status'] == k for r in results) for k in ('pass', 'fail', 'skip')}
    report = {
        'target': args.target,
        'profile': args.profile,
        'started_at': started_at.isoformat(timespec='seconds'),
        'finished_at': dt.datetime.now(dt.timezone.utc).isoformat(timespec='seconds'),
        'counts': counts,
        'scenarios': results,
    }
    if args.json_out:
        Path(args.json_out).write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print('\n'.join(redact(line) for line in summary_lines(report)))
    status = {r['name']: r['status'] for r in results}
    not_passed = [n for n in required if status.get(n) != 'pass']
    if not_passed:
        print(f'required to pass but did not: {", ".join(not_passed)}')
    return 1 if counts['fail'] or not_passed or _stopping else 0


if __name__ == '__main__':
    sys.exit(main())
