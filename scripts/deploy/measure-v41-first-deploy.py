#!/usr/bin/env python3
"""Measure the v4.1 first-deploy effects of the catalog lifecycle (ECO-803), read-only.

Usage:
  scripts/deploy/measure-v41-first-deploy.sh --database-url URL [--statement-timeout SECONDS]
      [--crawler COMMAND] [--i-know-this-is-production]

Prints a Markdown report to paste into ECO-803:
  1. Inactive BEA series, in total and by each rule of the crawler's BEA catalog selection
     (backend/crates/econ-graph-crawler/data/catalog_selection/bea.toml, read at run time), as
     the crawler matches stored series to rules: the table prefix, `exclude`, the rule's
     frequencies (NIPA) or a state-level area (Regional).
  2. FRED: `crawler check-selection --source FRED --json` against the live API: the series the
     rules select, against max_series, and the requests one full fetch of them takes. Run only
     when FRED_API_KEY is set; otherwise the report says it was skipped. The crawler gets only
     PATH, HOME, the Cargo/Rust variables and FRED_API_KEY: no database URL.
  ECO-807. The (series, date) pairs whose only two rows are one original release published
     before the date and one synthetic legacy row (the SQL of synthetic_legacy_row in
     backend/crates/econ-graph-core/src/models/data_point.rs), by source.

--database-url is required and names the data database to measure: the rehearsal copy or a
restored dump. Give the password in PGPASSWORD (or PGPASSFILE) rather than in the URL, where
the process list would show it; either way it reaches psql as PGPASSWORD and is never printed. The script refuses
a target that reaches the production cluster's data database unless --i-know-this-is-production
is given: an in-cluster host name (*.svc, data-postgres-service, postgres-service, postgresql-0),
or a port on this machine served by `kubectl port-forward` into namespace econ-graph (or the
context's default) of the kind cluster econ-graph (or a cluster it cannot tell). Every statement runs in a READ ONLY
transaction (SET TRANSACTION READ ONLY, with default_transaction_read_only on as well) under a
statement timeout (default 600 seconds). Idempotent: it writes nothing anywhere.

--crawler is the command that runs the crawler CLI, split on spaces (default: cargo run of
econ-graph-crawler's crawler binary from backend/).

Exit status: 0 report printed; 1 a measurement failed (the report says which); 64 usage;
65 refused: the target looks like production.
"""

import argparse
import json
import os
import re
import shlex
import socket
import subprocess
import sys
import tomllib
import urllib.parse
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BEA_RULES = ROOT / 'backend/crates/econ-graph-crawler/data/catalog_selection/bea.toml'
DATA_POINT_RS = ROOT / 'backend/crates/econ-graph-core/src/models/data_point.rs'
DEFAULT_CRAWLER = (f'cargo run -q --manifest-path {ROOT}/backend/Cargo.toml '
                   '-p econ-graph-crawler --bin crawler --')
# The kind cluster and namespace that hold production's data database.
PRODUCTION_CLUSTER = 'econ-graph'
PRODUCTION_NAMESPACE = 'econ-graph'
IN_CLUSTER_HOSTS = ('data-postgres-service', 'postgres-service', 'postgresql-0', 'postgresql')
BEA_SOURCE = "ds.name ILIKE '%Bureau of Economic Analysis%'"
# Dataset codes in BEA external ids: bea_nipa/{table}.{line}.{frequency} and
# bea_regional/{table}.{line}.{area} (econ-graph-crawler/src/sources/bea.rs).
DATASET_CODES = {'NIPA': 'bea_nipa', 'Regional': 'bea_regional'}

EXIT_FAILED, EXIT_USAGE, EXIT_REFUSED = 1, 64, 65


class Refused(Exception):
    pass


def literal(value: str) -> str:
    return "'" + value.replace("'", "''") + "'"


# --- The target --------------------------------------------------------------------------------


def parse_target(url: str):
    """(host, port, database, user, password) of a postgres:// URL."""
    parts = urllib.parse.urlsplit(url)
    if parts.scheme not in ('postgres', 'postgresql') or not parts.hostname:
        raise ValueError('--database-url must be a postgres:// or postgresql:// URL with a host')
    database = urllib.parse.unquote(parts.path.lstrip('/'))
    if not database:
        raise ValueError('--database-url must name a database')
    return (parts.hostname, parts.port or 5432, database,
            urllib.parse.unquote(parts.username or ''), urllib.parse.unquote(parts.password or ''))


def redacted(url: str) -> str:
    parts = urllib.parse.urlsplit(url)
    netloc = parts.hostname or ''
    if parts.port:
        netloc += f':{parts.port}'
    if parts.username:
        netloc = f'{urllib.parse.unquote(parts.username)}@{netloc}'
    return urllib.parse.urlunsplit((parts.scheme, netloc, parts.path, '', ''))


def is_local(host: str) -> bool:
    """Whether `host` resolves to an address of this machine (one a local socket can bind)."""
    try:
        infos = socket.getaddrinfo(host, None, proto=socket.IPPROTO_TCP)
    except socket.gaierror:
        return False
    for family, _, _, _, address in infos:
        try:
            with socket.socket(family, socket.SOCK_STREAM) as probe:
                probe.bind((address[0], 0))
                return True
        except OSError:
            continue
    return False


def listener_pids(port: int) -> list[int]:
    """Pids of processes listening on TCP `port` that this user can see (ss -ltnp)."""
    try:
        out = subprocess.run(['ss', '-Hltnp', f'sport = :{port}'], capture_output=True,
                             text=True, timeout=10).stdout
    except (OSError, subprocess.TimeoutExpired):
        return []
    return sorted({int(pid) for pid in re.findall(r'pid=(\d+)', out)})


def proc_argv(pid: int) -> list[str]:
    try:
        raw = Path(f'/proc/{pid}/cmdline').read_bytes()
    except OSError:
        return []
    return [a.decode(errors='replace') for a in raw.split(b'\0') if a]


def proc_env(pid: int) -> dict[str, str]:
    try:
        raw = Path(f'/proc/{pid}/environ').read_bytes()
    except OSError:
        return {}
    pairs = (a.decode(errors='replace').partition('=') for a in raw.split(b'\0') if a)
    return {k: v for k, _, v in pairs}


def option(argv: list[str], *names: str) -> str | None:
    for i, arg in enumerate(argv):
        for name in names:
            if arg == name and i + 1 < len(argv):
                return argv[i + 1]
            if name.startswith('--') and arg.startswith(name + '='):
                return arg.split('=', 1)[1]
    return None


def port_forward_cluster(argv: list[str], env: dict[str, str]) -> str | None:
    """The kind cluster a `kubectl port-forward` reaches, or None when it can't tell: its
    --context (kind-<cluster>), else the kind-env.sh kubeconfig it runs with
    (kubeconfig-<cluster>)."""
    context = option(argv, '--context')
    if context:
        return context[len('kind-'):] if context.startswith('kind-') else context
    kubeconfig = option(argv, '--kubeconfig') or env.get('KUBECONFIG', '')
    match = re.search(r'kubeconfig-([a-z0-9][a-z0-9.-]*)$', kubeconfig)
    return match.group(1) if match else None


def production_reason(host: str, port: int) -> str | None:
    """Why the target looks like the production cluster's data database, or None."""
    name = host.lower().rstrip('.')
    if '.svc' in name or name.split('.')[0] in IN_CLUSTER_HOSTS:
        return f'{host} is an in-cluster name of the data database'
    if not is_local(host):
        return None
    for pid in listener_pids(port):
        argv = proc_argv(pid)
        if not argv or Path(argv[0]).name != 'kubectl' or 'port-forward' not in argv:
            continue
        # Unknown counts as production: a port-forward without -n uses its context's namespace.
        namespace = option(argv, '-n', '--namespace')
        cluster = port_forward_cluster(argv, proc_env(pid))
        if namespace not in (None, PRODUCTION_NAMESPACE) or cluster not in (None, PRODUCTION_CLUSTER):
            continue
        where = f'cluster {cluster}' if cluster else 'a cluster it cannot tell'
        return (f'port {port} on {host} is `kubectl port-forward` (pid {pid}) into namespace '
                f'{namespace or "(the context default)"} of {where}')
    return None


# --- psql --------------------------------------------------------------------------------------


class Database:
    def __init__(self, url: str, timeout_seconds: int):
        self.host, self.port, self.name, self.user, password = parse_target(url)
        self.timeout = timeout_seconds
        # Only the URL picks the target; a password may come from PGPASSWORD or PGPASSFILE.
        keep = ('PGPASSWORD', 'PGPASSFILE')
        self.env = {k: v for k, v in os.environ.items()
                    if (k in keep or not k.startswith('PG'))
                    and k not in ('DATABASE_URL', 'DATA_DATABASE_URL')}
        self.env.update(PGHOST=self.host, PGPORT=str(self.port), PGDATABASE=self.name,
                        PGAPPNAME='measure-v41-first-deploy', PGCONNECT_TIMEOUT='15',
                        PGOPTIONS=f'-c default_transaction_read_only=on '
                                  f'-c statement_timeout={timeout_seconds}s')
        if self.user:
            self.env['PGUSER'] = self.user
        if password:
            self.env['PGPASSWORD'] = password

    def rows(self, sql: str) -> list[list[str]]:
        """Rows of one SELECT, run in a READ ONLY transaction under the statement timeout."""
        script = ('BEGIN;\nSET TRANSACTION READ ONLY;\n'
                  f"SET LOCAL statement_timeout = '{self.timeout}s';\n"
                  f'{sql.rstrip().rstrip(";")};\nCOMMIT;\n')
        done = subprocess.run(['psql', '-X', '-q', '-A', '-t', '-F', '\x1f', '-v', 'ON_ERROR_STOP=1'],
                              input=script, capture_output=True, text=True, env=self.env,
                              timeout=self.timeout + 60)
        if done.returncode:
            raise RuntimeError(done.stderr.strip().splitlines()[-1] if done.stderr.strip()
                               else f'psql exited {done.returncode}')
        return [line.split('\x1f') for line in done.stdout.splitlines() if line]


# --- 1. Inactive BEA series --------------------------------------------------------------------


def bea_rules():
    config = tomllib.loads(BEA_RULES.read_text(encoding='utf-8'))
    rules = []
    for rule in config.get('rule', []):
        dataset, prefix = rule['dataset'], rule['prefix']
        if dataset not in DATASET_CODES or not re.fullmatch(r'[A-Za-z0-9]+', prefix):
            raise ValueError(f'unexpected BEA rule {rule!r}')
        if 'title_contains' in rule:
            raise ValueError(f'BEA rule {prefix} has title_contains, which this report '
                             'cannot apply: update the script')
        rules.append({'dataset': dataset, 'prefix': prefix,
                      'frequencies': list(rule.get('frequencies', []))})
    exclude = list(config.get('exclude', []))
    for table in exclude:
        if not re.fullmatch(r'[A-Za-z0-9]+', table):
            raise ValueError(f'unexpected BEA exclude {table!r}')
    return rules, exclude


def rule_predicate(rule, exclude) -> str:
    """SQL: economic_series `es` is matched by `rule`, as the crawler matches stored series."""
    code = DATASET_CODES[rule['dataset']]
    table = "split_part(split_part(es.external_id, '/', 2), '.', 1)"
    last = "split_part(split_part(es.external_id, '/', 2), '.', 3)"
    parts = [f"split_part(es.external_id, '/', 1) = {literal(code)}",
             f"{table} LIKE {literal(rule['prefix'] + '%')}"]
    if exclude:
        parts.append(f"{table} NOT IN ({', '.join(literal(t) for t in exclude)})")
    if rule['dataset'] == 'NIPA':
        freqs = ', '.join(literal(f) for f in rule['frequencies']) or 'NULL'
        parts.append(f'{last} IN ({freqs})')
    else:
        # Regional series are annual, and only state-level areas (five digits ending 000).
        if 'A' not in rule['frequencies']:
            return 'FALSE'
        parts.append(f"{last} ~ '^[0-9]{{2}}000$'")
    return '(' + ' AND '.join(parts) + ')'


def measure_bea(db: Database) -> list[str]:
    rules, exclude = bea_rules()
    inactive = (f'FROM economic_series es JOIN data_sources ds ON ds.id = es.source_id '
                f'WHERE {BEA_SOURCE} AND NOT es.is_active')
    columns = [f'count(*) FILTER (WHERE {rule_predicate(r, exclude)})' for r in rules]
    any_rule = ' OR '.join(rule_predicate(r, exclude) for r in rules) or 'FALSE'
    table = "split_part(split_part(es.external_id, '/', 2), '.', 1)"
    excluded = (f"count(*) FILTER (WHERE {table} IN ({', '.join(literal(t) for t in exclude)}))"
                if exclude else '0')
    row = db.rows(f'SELECT count(*), count(*) FILTER (WHERE {any_rule}), {excluded}'
                  f'{", " if columns else ""}{", ".join(columns)} {inactive}')[0]
    total, matched, in_excluded, per_rule = row[0], row[1], row[2], row[3:]
    lines = [f'- Inactive BEA series: **{total}**',
             f'- Matched by a current rule (could be enrolled again): **{matched}**',
             f'- Not matched by any rule: {int(total) - int(matched)}'
             f' (of which in excluded tables {", ".join(exclude) or "(none)"}: {in_excluded})',
             '', '| Dataset | Rule prefix | Frequencies | Inactive series |', '|---|---|---|---|']
    for rule, count in zip(rules, per_rule):
        lines.append(f"| {rule['dataset']} | {rule['prefix']} | "
                     f"{', '.join(rule['frequencies'])} | {count} |")
    return lines


# --- ECO-807 -----------------------------------------------------------------------------------


def synthetic_legacy_row(row: str, other: str) -> str:
    """synthetic_legacy_row in econ-graph-core/src/models/data_point.rs; the stub test checks
    that the two stay the same."""
    return (f"({row}.revision_date = {row}.date AND {row}.is_original_release "
            f"AND EXISTS ("
            f"SELECT 1 FROM data_points {other} "
            f"WHERE {other}.series_id = {row}.series_id "
            f"AND {other}.date = {row}.date "
            f"AND {other}.is_original_release "
            f"AND {other}.revision_date <> {row}.date"
            f"))")


ECO_807 = f"""
SELECT ds.name, count(*)
FROM data_points s
JOIN economic_series es ON es.id = s.series_id
JOIN data_sources ds ON ds.id = es.source_id
WHERE {synthetic_legacy_row('s', 'other_original')}
  AND (SELECT count(*) FROM data_points a WHERE a.series_id = s.series_id AND a.date = s.date) = 2
  AND EXISTS (SELECT 1 FROM data_points e WHERE e.series_id = s.series_id AND e.date = s.date
              AND e.is_original_release AND e.revision_date < s.date)
GROUP BY ds.name ORDER BY ds.name"""


def measure_eco_807(db: Database) -> list[str]:
    rows = db.rows(ECO_807)
    total = sum(int(n) for _, n in rows)
    lines = [f'- (series, date) pairs with exactly one original published before the date and '
             f'one synthetic legacy row, and no other row: **{total}**']
    if rows:
        lines += ['', '| Source | Pairs |', '|---|---|']
        lines += [f'| {name} | {n} |' for name, n in rows]
    return lines


# --- 2. FRED selection -------------------------------------------------------------------------


def measure_fred(crawler: str) -> tuple[list[str], bool]:
    """Report lines, and whether the check failed."""
    if not os.environ.get('FRED_API_KEY'):
        return ['- skipped: needs FRED_API_KEY'], False
    keep = ('PATH', 'HOME', 'CARGO_HOME', 'RUSTUP_HOME', 'CARGO_TARGET_DIR', 'RUSTUP_TOOLCHAIN')
    env = {k: os.environ[k] for k in keep if k in os.environ}
    env['FRED_API_KEY'] = os.environ['FRED_API_KEY']
    env['RUST_LOG'] = 'error'
    try:
        done = subprocess.run(shlex.split(crawler) + ['check-selection', '--source', 'FRED', '--json'],
                              capture_output=True, text=True, env=env, timeout=3600)
    except (OSError, subprocess.TimeoutExpired) as error:
        return [f'- FAILED: could not run the crawler: {error}'], True
    try:
        report = json.loads(done.stdout)
        fred = next(s for s in report['sources'] if s['source'] == 'FRED')
    except (ValueError, KeyError, StopIteration):
        tail = (done.stderr or done.stdout).strip().splitlines()[-1:] or [f'exit {done.returncode}']
        return [f'- FAILED: check-selection gave no FRED report: {tail[0][:300]}'], True
    if fred.get('status') in ('failed', 'skipped'):
        why = fred.get('error') or fred.get('skipped') or fred.get('status')
        return [f"- FAILED: check-selection {fred.get('status')}: {why}"], True
    bound = next((b for b in fred.get('bounds', []) if b.get('what') == 'selected series'), {})
    selected, limit = fred.get('selected'), bound.get('limit')
    lines = [f'- Series the FRED rules and include list select now: **{selected}**'
             f' (max_series {limit}; {"within" if selected is not None and limit is not None and selected <= limit else "OVER"} the cap)']
    refresh = fred.get('refresh')
    if refresh:
        lines.append(f"- First full fetch: about {refresh['requests']} requests (/series, "
                     f"/series/vintagedates and /series/observations per series), "
                     f"{refresh['minutes']:.0f} minutes at {refresh['requests_per_minute']:.0f} "
                     f"requests a minute. Vintage counts per series are not part of the check.")
    lines.append(f"- check-selection status: {fred.get('status')}, "
                 f"{len(fred.get('problems', []))} problems, {len(fred.get('notices', []))} notices")
    for problem in fred.get('problems', [])[:20]:
        lines.append(f"  - {problem.get('kind')}: {problem.get('entry', '')}: "
                     f"{problem.get('detail', '')}")
    return lines, False


# --- Main --------------------------------------------------------------------------------------


class Parser(argparse.ArgumentParser):
    def error(self, message):
        self.print_usage(sys.stderr)
        print(f'{self.prog}: error: {message}', file=sys.stderr)
        sys.exit(EXIT_USAGE)


def main(argv=None) -> int:
    parser = Parser(description=__doc__.split('\n\n')[0],
                                     formatter_class=argparse.RawDescriptionHelpFormatter,
                                     epilog=__doc__.split('\n\n', 1)[1])
    parser.add_argument('--database-url', required=True)
    parser.add_argument('--statement-timeout', type=int, default=600)
    parser.add_argument('--crawler', default=DEFAULT_CRAWLER)
    parser.add_argument('--i-know-this-is-production', action='store_true')
    args = parser.parse_args(argv)
    if args.statement_timeout <= 0:
        parser.error('--statement-timeout takes a positive number of seconds')
    try:
        db = Database(args.database_url, args.statement_timeout)
    except ValueError as error:
        parser.error(str(error))

    reason = production_reason(db.host, db.port)
    if reason and not args.i_know_this_is_production:
        print(f'refusing: {reason}. Measure the rehearsal copy or a restored dump; pass '
              '--i-know-this-is-production only to measure production itself.', file=sys.stderr)
        return EXIT_REFUSED

    failed = False
    out = ['## v4.1 first-deploy measurements (ECO-803)', '',
           f"- Measured {datetime.now(timezone.utc):%Y-%m-%d %H:%M} UTC against "
           f"`{redacted(args.database_url)}`" + (' (production)' if reason else '')]
    try:
        commit = subprocess.run(['git', '-C', str(ROOT), 'rev-parse', '--short', 'HEAD'],
                                capture_output=True, text=True, timeout=10).stdout.strip()
    except OSError:
        commit = ''
    if commit:
        out[-1] += f', rules from commit {commit}'
    try:
        version = db.rows('SHOW server_version')[0][0]
        out.append(f'- Server: PostgreSQL {version}; every statement READ ONLY, '
                   f'statement timeout {args.statement_timeout}s')
    except (RuntimeError, IndexError, subprocess.TimeoutExpired) as error:
        print(f'cannot query {redacted(args.database_url)}: {error}', file=sys.stderr)
        return EXIT_FAILED

    for title, measure in (('### 1. Inactive BEA series by rule', lambda: measure_bea(db)),
                           ('### ECO-807: projection-only dates with a synthetic row',
                            lambda: measure_eco_807(db))):
        out += ['', title, '']
        try:
            out += measure()
        except (RuntimeError, ValueError, OSError, KeyError, subprocess.TimeoutExpired) as error:
            out.append(f'- FAILED: {error}')
            failed = True
    out += ['', '### 2. FRED selection (live API)', '']
    lines, fred_failed = measure_fred(args.crawler)
    out += lines
    failed = failed or fred_failed
    print('\n'.join(out))
    return EXIT_FAILED if failed else 0


if __name__ == '__main__':
    sys.exit(main())
