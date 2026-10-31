#!/usr/bin/env python3
"""Measure the v4.1 first-deploy effects of the catalog lifecycle (ECO-803), read-only.

Usage:
  scripts/deploy/measure-v41-first-deploy.sh --database-url URL [--statement-timeout SECONDS]
      [--crawler COMMAND] [--i-know-this-is-production]

Prints a Markdown report to paste into ECO-803:
  1. Inactive BEA series, in total and by each rule of the crawler's BEA catalog selection
     (backend/crates/econ-graph-crawler/data/catalog_selection/bea.toml, read at run time), as
     the crawler matches stored series to rules: the external id as SeriesKey::parse reads it,
     the table prefix, `exclude`, the rule's frequencies (NIPA) or a state-level area
     (Regional); plus series known only from series_metadata that a rule matches.
  2. FRED: `crawler check-selection --source FRED --json --count-uncapped` against the live
     API: every series the rules select, counted past max_series when the selection is over
     it, and the requests one full fetch of them takes. Run only when FRED_API_KEY is set;
     otherwise the report says it was skipped. A report without a count is a failed
     measurement. The crawler gets only PATH, HOME, the Cargo/Rust variables and
     FRED_API_KEY: no database URL.
  ECO-807. The (series, date) pairs whose only two rows are one original release published
     before the date and one synthetic legacy row (the SQL of synthetic_legacy_row in
     backend/crates/econ-graph-core/src/models/data_point.rs), by source.
  ECO-738. The data migration 2026-11-01-000100_world_bank_first_release: the World Bank rows
     it would relabel (later real originals of a (series, date), ranked as the migration ranks
     them) and the revision twins that would make it refuse, with its estimated run time
     (about 25 s per million rows relabelled) and WAL (about 0.9 KB per row) against the
     crawler-migrate Job's activeDeadlineSeconds (k8s/manifests/crawler-migrate-job.yaml),
     which every pending data migration shares. Once the migration is applied the report
     says so instead: measure a copy restored before the release is deployed.

--database-url is required and names the data database to measure: the rehearsal copy or a
restored dump. Give the password in PGPASSWORD (or PGPASSFILE) rather than in the URL, where
the process list would show it; either way it reaches psql as PGPASSWORD and is never printed.

The script refuses a target that may reach the production cluster's data database unless
--i-know-this-is-production is given, failing closed:
  - an in-cluster host name (*.svc, data-postgres-service, postgres-service, postgresql,
    postgresql-0), or a host that does not resolve;
  - a port on this machine with a listener this user cannot see (another user's or root's),
    or when `ss` fails;
  - a port on this machine served by anything but a local Postgres server, a container port
    (docker-proxy, rootlesskit, rootlessport, slirp4netns, pasta) or a port-forward into
    another cluster or namespace. A port-forward into namespace econ-graph (or its context's
    default) of the kind cluster econ-graph, or of a cluster it cannot tell from --context or
    the kind-env.sh kubeconfig-<cluster>, is refused.
What it trusts, and why that holds today: an address that is not this machine's, because
production's data database is a ClusterIP Service in the kind cluster econ-graph, with no
NodePort or hostPort (kind maps only ports 80, 443, 3000 and 8080), so from this host it is
reachable only through an in-cluster name or a local forward; and a port-forward's cluster by
name, because kind names a cluster's context kind-<cluster> and kind-env.sh its kubeconfig
kubeconfig-<cluster>, so one naming another cluster reaches that cluster.

All queries run in one psql session and one transaction: SET TRANSACTION ISOLATION LEVEL
REPEATABLE READ, READ ONLY (one snapshot for the whole report; default_transaction_read_only
is on as well), with a statement timeout (default 600 seconds) and a 5-second lock timeout.
Idempotent: it writes nothing anywhere.

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
MIGRATE_JOB = ROOT / 'k8s/manifests/crawler-migrate-job.yaml'
DEFAULT_CRAWLER = (f'cargo run -q --manifest-path {ROOT}/backend/Cargo.toml '
                   '-p econ-graph-crawler --bin crawler --')
# The kind cluster and namespace that hold production's data database.
PRODUCTION_CLUSTER = 'econ-graph'
PRODUCTION_NAMESPACE = 'econ-graph'
IN_CLUSTER_HOSTS = ('data-postgres-service', 'postgres-service', 'postgresql-0', 'postgresql')
# The crawler's data source for BEA (data_source_template(SourceId::Bea) in econ-graph-crawler).
BEA_SOURCE = "ds.name = 'Bureau of Economic Analysis (BEA)'"
# Dataset codes in BEA external ids: bea_nipa/{table}.{line}.{frequency} and
# bea_regional/{table}.{line}.{area} (econ-graph-crawler/src/sources/bea.rs).
DATASET_CODES = {'NIPA': 'bea_nipa', 'Regional': 'bea_regional'}

EXIT_FAILED, EXIT_USAGE, EXIT_REFUSED = 1, 64, 65
LOCK_TIMEOUT = 5
# Precedes each query's output in the psql session.
MARKER = '@@measure-v41:'


def md(value) -> str:
    """A value for a Markdown table cell."""
    return str(value).replace('|', '\\|')


def literal(value: str) -> str:
    return "'" + value.replace("'", "''") + "'"


# --- The target --------------------------------------------------------------------------------


def parse_target(url: str):
    """(host, port, database, user, password) of a postgres:// URL. Errors never quote the URL,
    which may hold a password."""
    bad = ValueError('--database-url must be postgres://[user@]host[:port]/database with one '
                     'host (give the password in PGPASSWORD)')
    try:
        parts = urllib.parse.urlsplit(url)
        port = parts.port or 5432
    except ValueError:
        raise bad from None
    database = urllib.parse.unquote(parts.path.lstrip('/'))
    if (parts.scheme not in ('postgres', 'postgresql') or not parts.hostname or not database
            or ',' in parts.netloc or '/' in database):
        raise bad
    return (parts.hostname, port, database,
            urllib.parse.unquote(parts.username or ''), urllib.parse.unquote(parts.password or ''))


def redacted(url: str) -> str:
    parts = urllib.parse.urlsplit(url)
    netloc = parts.hostname or ''
    if parts.port:
        netloc += f':{parts.port}'
    if parts.username:
        netloc = f'{urllib.parse.unquote(parts.username)}@{netloc}'
    return urllib.parse.urlunsplit((parts.scheme, netloc, parts.path, '', ''))


def resolve(host: str):
    """getaddrinfo of `host`, or None when it does not resolve (including a malformed name,
    such as a label longer than 63 characters)."""
    try:
        return socket.getaddrinfo(host, None, proto=socket.IPPROTO_TCP)
    except (socket.gaierror, UnicodeError, ValueError):
        return None


def is_local(host: str) -> bool:
    """Whether `host` resolves to an address of this machine (one a local socket can bind)."""
    infos = resolve(host)
    if not infos:
        return False
    for family, _, _, _, address in infos:
        try:
            with socket.socket(family, socket.SOCK_STREAM) as probe:
                probe.bind((address[0], 0))
                return True
        except OSError:
            continue
    return False


def listeners(port: int) -> tuple[bool, list[int]] | None:
    """(whether any listening socket on TCP `port` has an owner this user cannot see, the pids
    of the owners it can see), or None when `ss` can't be run. One process often listens
    twice, on 127.0.0.1 and [::1]."""
    try:
        done = subprocess.run(['ss', '-Hltnp', f'sport = :{port}'], capture_output=True,
                              text=True, timeout=10)
    except (OSError, subprocess.TimeoutExpired):
        return None
    if done.returncode:
        return None
    lines = [line for line in done.stdout.splitlines() if line.strip()]
    hidden = any('pid=' not in line for line in lines)
    return hidden, sorted({int(pid) for pid in re.findall(r'pid=(\d+)', done.stdout)})


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


# Local listeners that serve a database on this machine rather than forward to one elsewhere:
# Postgres itself, and the processes Docker and Podman publish container ports with.
LOCAL_SERVERS = ('postgres', 'docker-proxy', 'rootlesskit', 'rootlessport', 'slirp4netns', 'pasta')


def production_reason(host: str, port: int) -> str | None:
    """Why the target may be the production cluster's data database, or None when it is known
    not to be. Fails closed: a local port whose listener can't be identified counts."""
    name = host.lower().rstrip('.')
    if '.svc' in name or name.split('.')[0] in IN_CLUSTER_HOSTS:
        return f'{host} is an in-cluster name of the data database'
    if not resolve(host):
        return f'{host} does not resolve, so it cannot be checked'
    if not is_local(host):
        return None
    found = listeners(port)
    if found is None:
        return f'cannot list what listens on port {port} (`ss` failed)'
    hidden, pids = found
    if hidden:
        return f'port {port} is served by a process this user cannot see (another user, or root)'
    for pid in pids:
        argv = proc_argv(pid)
        program = Path(argv[0]).name if argv else ''
        if 'port-forward' in argv:
            # Unknown counts as production: without -n it uses its context's namespace.
            namespace = option(argv, '-n', '--namespace')
            cluster = port_forward_cluster(argv, proc_env(pid))
            if namespace not in (None, PRODUCTION_NAMESPACE) or cluster not in (None, PRODUCTION_CLUSTER):
                continue
            where = f'cluster {cluster}' if cluster else 'a cluster it cannot tell'
            return (f'port {port} on {host} is a port-forward ({program}, pid {pid}) into '
                    f'namespace {namespace or "(the context default)"} of {where}')
        if program in LOCAL_SERVERS:
            continue
        return (f'port {port} on {host} is served by {program or "an unknown process"} '
                f'(pid {pid}), which may forward to production')
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

    def run(self, queries: list[tuple[str, str]]):
        """Run named SELECTs in one psql session and one READ ONLY, REPEATABLE READ transaction
        (one snapshot), under the statement and lock timeouts. Returns ({name: rows} for the
        queries that finished, the name of the one that failed or None, its error or None)."""
        script = ['BEGIN;', 'SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY;',
                  f"SET LOCAL statement_timeout = '{self.timeout}s';",
                  f"SET LOCAL lock_timeout = '{LOCK_TIMEOUT}s';"]
        for name, sql in queries:
            script += [f'\\echo {MARKER}{name}', sql.rstrip().rstrip(';') + ';']
        script += [f'\\echo {MARKER}', 'COMMIT;']
        try:
            done = subprocess.run(['psql', '-X', '-q', '-A', '-t', '-F', '\x1f', '-v', 'ON_ERROR_STOP=1'],
                                  input='\n'.join(script) + '\n', capture_output=True, text=True,
                                  env=self.env, timeout=self.timeout * len(queries) + 60)
        except subprocess.TimeoutExpired:
            return {}, queries[0][0] if queries else None, 'psql did not finish in time'
        results, current, ended = {}, None, False
        for line in done.stdout.splitlines():
            if line.startswith(MARKER):
                current = line[len(MARKER):] or None
                ended = current is None
                if current:
                    results[current] = []
            elif current and line:
                results[current].append(line.split('\x1f'))
        if done.returncode and ended:
            # Every query finished; only COMMIT failed.
            lines = done.stderr.strip().splitlines()
            return results, 'commit', (lines or [f'psql exited {done.returncode}'])[-1]
        if done.returncode or not ended:
            lines = done.stderr.strip().splitlines()
            # psql follows the ERROR line with the statement and a caret: report the ERROR.
            error = next((line.strip() for line in lines if 'ERROR:' in line or 'FATAL:' in line),
                         lines[-1] if lines else f'psql exited {done.returncode}')
            failed = current or (queries[0][0] if queries else None)
            results.pop(failed, None)
            return results, failed, error
        return results, None, None


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


# The parts of a BEA external id as SeriesKey::parse reads them (econ-graph-crawler's
# sources/bea.rs): the dataset code before the first '/', then exactly three '.'-separated
# parts after it, the table and line non-empty ASCII alphanumerics.
REST = "substr({t}.external_id, strpos({t}.external_id, '/') + 1)"
PARSES = ("(strpos({t}.external_id, '/') > 0 AND "
          + REST + " ~ '^[A-Za-z0-9]+\\.[A-Za-z0-9]+\\.[^.]*$')")
CODE = "split_part({t}.external_id, '/', 1)"
TABLE = "split_part(" + REST + ", '.', 1)"
LAST = "split_part(" + REST + ", '.', 3)"


def rule_predicate(rule, exclude, t: str = 'es') -> str:
    """SQL: the row aliased `t` (economic_series or series_metadata) is matched by `rule`, as the
    crawler matches stored series (SeriesKey::parse, then includes_stored_series)."""
    code, table, last = DATASET_CODES[rule['dataset']], TABLE.format(t=t), LAST.format(t=t)
    parts = [PARSES.format(t=t), f"{CODE.format(t=t)} = {literal(code)}",
             f"{table} LIKE {literal(rule['prefix'] + '%')}"]
    if exclude:
        parts.append(f"{table} NOT IN ({', '.join(literal(x) for x in exclude)})")
    if rule['dataset'] == 'NIPA':
        freqs = [f for f in rule['frequencies'] if f in ('A', 'Q', 'M')]
        parts.append(f"{last} IN ({', '.join(literal(f) for f in freqs) or 'NULL'})")
    else:
        # Regional series are annual, and only state-level areas (five digits ending 000).
        if 'A' not in rule['frequencies']:
            return 'FALSE'
        parts.append(f"{last} ~ '^[0-9]{{2}}000$'")
    return '(' + ' AND '.join(parts) + ')'


def bea_queries(rules, exclude) -> list[tuple[str, str]]:
    inactive = (f'FROM economic_series es JOIN data_sources ds ON ds.id = es.source_id '
                f'WHERE {BEA_SOURCE} AND NOT es.is_active')
    columns = [f'count(*) FILTER (WHERE {rule_predicate(r, exclude)})' for r in rules]
    any_rule = ' OR '.join(rule_predicate(r, exclude) for r in rules) or 'FALSE'
    table = TABLE.format(t='es')
    excluded = (f"count(*) FILTER (WHERE {PARSES.format(t='es')} AND {table} IN "
                f"({', '.join(literal(t) for t in exclude)}))" if exclude else '0')
    # stored_scope_members also enrols series_metadata rows with no economic_series row yet.
    metadata_any = ' OR '.join(rule_predicate(r, exclude, 'sm') for r in rules) or 'FALSE'
    return [
        ('bea', f'SELECT count(*), count(*) FILTER (WHERE {any_rule}), {excluded}'
                f'{", " if columns else ""}{", ".join(columns)} {inactive}'),
        ('bea_metadata', f'SELECT count(*) FROM series_metadata sm JOIN data_sources ds '
                         f'ON ds.id = sm.source_id WHERE {BEA_SOURCE} AND ({metadata_any}) '
                         f'AND NOT EXISTS (SELECT 1 FROM economic_series e WHERE e.source_id = '
                         f'sm.source_id AND e.external_id = sm.external_id)'),
    ]


def render_bea(rules, exclude, results) -> list[str]:
    row = results['bea'][0]
    total, matched, in_excluded, per_rule = row[0], row[1], row[2], row[3:]
    metadata_only = results['bea_metadata'][0][0]
    lines = [f'- Inactive BEA series: **{total}**',
             f'- Matched by a current rule (could be enrolled again): **{matched}**',
             f'- Not matched by any rule: {int(total) - int(matched)}'
             f' (of which in excluded tables {", ".join(exclude) or "(none)"}: {in_excluded})',
             f'- BEA series known only from series_metadata (no economic_series row) that a '
             f'rule matches: {metadata_only}',
             '', '| Dataset | Rule prefix | Frequencies | Inactive series |', '|---|---|---|---|']
    for rule, count in zip(rules, per_rule):
        lines.append(f"| {md(rule['dataset'])} | {md(rule['prefix'])} | "
                     f"{md(', '.join(rule['frequencies']))} | {md(count)} |")
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
SELECT translate(ds.name, E'\\n\\r\\x1f', '   '), count(*)
FROM data_points s
JOIN economic_series es ON es.id = s.series_id
JOIN data_sources ds ON ds.id = es.source_id
WHERE {synthetic_legacy_row('s', 'other_original')}
  AND (SELECT count(*) FROM data_points a WHERE a.series_id = s.series_id AND a.date = s.date) = 2
  AND EXISTS (SELECT 1 FROM data_points e WHERE e.series_id = s.series_id AND e.date = s.date
              AND e.is_original_release AND e.revision_date < s.date)
GROUP BY ds.name ORDER BY 1"""


def render_eco_807(results) -> list[str]:
    rows = results['eco_807']
    total = sum(int(n) for _, n in rows)
    lines = [f'- (series, date) pairs with exactly one original published before the date and '
             f'one synthetic legacy row, and no other row: **{total}**']
    if rows:
        lines += ['', '| Source | Pairs |', '|---|---|']
        lines += [f'| {md(name)} | {md(n)} |' for name, n in rows]
    return lines


# --- ECO-738 -----------------------------------------------------------------------------------

# The crawler's World Bank data source (data_sources has no source code column).
WORLD_BANK_SOURCE = 'World Bank Open Data'
FIRST_RELEASE_VERSION = '20261101000100'
# Measured on a copy with 3.6M relabels: 91 s and about 0.9 KB of WAL per row (each relabelled
# row is rewritten; the updated_at trigger fires), rounded up for the estimate.
SECONDS_PER_MILLION_RELABELS = 25
WAL_KB_PER_RELABEL = 0.9

# The rows 2026-11-01-000100_world_bank_first_release/up.sql relabels: every real original of
# a World Bank (series, date) after its earliest (by revision_date, then id), synthetic legacy
# rows left out; and of those, the ones with a revision twin, which make it refuse.
ECO_738 = f"""
WITH world_bank AS (SELECT id FROM data_sources WHERE name = {literal(WORLD_BANK_SOURCE)}),
later AS (
  SELECT ranked.series_id, ranked.date, ranked.revision_date
  FROM (
    SELECT dp.series_id, dp.date, dp.revision_date,
      ROW_NUMBER() OVER (PARTITION BY dp.series_id, dp.date ORDER BY dp.revision_date, dp.id)
        AS release_number
    FROM data_points dp
    JOIN economic_series es ON es.id = dp.series_id
    WHERE es.source_id IN (SELECT id FROM world_bank)
      AND dp.is_original_release
      AND NOT {synthetic_legacy_row('dp', 'other_original')}
  ) ranked
  WHERE ranked.release_number > 1
)
SELECT (SELECT count(*) FROM __diesel_schema_migrations
        WHERE version = {literal(FIRST_RELEASE_VERSION)}),
       (SELECT count(*) FROM world_bank),
       (SELECT count(*) FROM later),
       (SELECT count(*) FROM later l WHERE EXISTS (
          SELECT 1 FROM data_points r WHERE r.series_id = l.series_id AND r.date = l.date
          AND r.revision_date = l.revision_date AND NOT r.is_original_release))"""


def migrate_deadline() -> int:
    """activeDeadlineSeconds of the crawler-migrate Job."""
    match = re.search(r'^\s*activeDeadlineSeconds:\s*(\d+)\s*$', MIGRATE_JOB.read_text(), re.M)
    if not match:
        raise ValueError(f'no activeDeadlineSeconds in {MIGRATE_JOB.relative_to(ROOT)}')
    return int(match.group(1))


def render_eco_738(results) -> list[str]:
    applied, sources, relabel, twins = (int(v) for v in results['eco_738'][0])
    if applied:
        return [f'- Already applied here ({FIRST_RELEASE_VERSION} in __diesel_schema_migrations): '
                f'nothing is left to relabel. Measure a copy restored before the release is '
                f'deployed.']
    if sources != 1:
        raise ValueError(f'expected one data_sources row named {WORLD_BANK_SOURCE}, found '
                         f'{sources}; the migration refuses this database')
    seconds = relabel / 1_000_000 * SECONDS_PER_MILLION_RELABELS
    wal_gb = relabel * WAL_KB_PER_RELABEL / 1024 / 1024
    deadline = migrate_deadline()
    lines = [f'- World Bank rows to relabel as revisions: **{relabel}**',
             f'- Revision twins (the migration refuses while any exist): **{twins}**',
             f'- Estimated run time: about {seconds:.0f} s at {SECONDS_PER_MILLION_RELABELS} s per '
             f'million rows; the crawler-migrate Job allows {deadline} s for every pending data '
             f'migration ({"within" if seconds <= deadline else "OVER"} it)',
             f'- Estimated WAL: about {wal_gb:.1f} GB ({WAL_KB_PER_RELABEL} KB per row), and '
             f'{relabel} dead tuples for autovacuum']
    if twins:
        lines.append('- The migration refuses this database until the twins are resolved')
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
        done = subprocess.run(shlex.split(crawler) + ['check-selection', '--source', 'FRED', '--json',
                                                      '--count-uncapped'],
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
    if not isinstance(selected, int) or not isinstance(limit, int):
        details = '; '.join(f"{p.get('entry')}: {p.get('detail')}" for p in fred.get('problems', []))
        return [f'- FAILED: count unknown: check-selection reported no selected-series count'
                f'{" (" + details[:300] + ")" if details else ""}'], True
    lines = [f'- Series the FRED rules and include list select now: **{selected}**'
             f' (max_series {limit}; {"within" if selected <= limit else "OVER"} the cap)']
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
           f"`{redacted(args.database_url)}`"]
    if reason:
        out[-1] += f' (production guard overridden: {reason})'
    try:
        commit = subprocess.run(['git', '-C', str(ROOT), 'rev-parse', '--short', 'HEAD'],
                                capture_output=True, text=True, timeout=10).stdout.strip()
    except OSError:
        commit = ''
    if commit:
        out[-1] += f', rules from commit {commit}'

    sections = []
    try:
        rules, exclude = bea_rules()
        bea = bea_queries(rules, exclude)
        sections.append(('### 1. Inactive BEA series by rule', [n for n, _ in bea],
                         lambda r: render_bea(rules, exclude, r)))
    except (ValueError, OSError, KeyError) as error:
        bea = []
        sections.append(('### 1. Inactive BEA series by rule', [], error))
    sections.append(('### ECO-807: projection-only dates with a synthetic row', ['eco_807'],
                     render_eco_807))
    sections.append(('### ECO-738: World Bank first-release relabel', ['eco_738'],
                     render_eco_738))
    queries = [('version', 'SHOW server_version')] + bea + [('eco_807', ECO_807),
                                                             ('eco_738', ECO_738)]
    results, failed_query, error = db.run(queries)
    if 'version' not in results:
        print(f'cannot query {redacted(args.database_url)}: {error}', file=sys.stderr)
        return EXIT_FAILED
    out.append(f"- Server: PostgreSQL {results['version'][0][0]}; one READ ONLY, REPEATABLE "
               f"READ transaction (one snapshot), statement timeout {args.statement_timeout}s, "
               f"lock timeout {LOCK_TIMEOUT}s")
    for title, names, render in sections:
        out += ['', title, '']
        if isinstance(render, Exception):
            out.append(f'- FAILED: {render}')
            failed = True
        elif all(name in results for name in names):
            try:
                out += render(results)
            except (ValueError, IndexError, KeyError) as render_error:
                out.append(f'- FAILED: unexpected query output: {render_error}')
                failed = True
        else:
            why = error if failed_query in names else f'not run: {failed_query} failed first'
            out.append(f'- FAILED: {why}')
            failed = True
    if failed_query == 'commit':
        out += ['', f'- FAILED: the read-only transaction did not commit cleanly: {error}']
        failed = True
    out += ['', '### 2. FRED selection (live API)', '']
    lines, fred_failed = measure_fred(args.crawler)
    out += lines
    failed = failed or fred_failed
    print('\n'.join(out))
    return EXIT_FAILED if failed else 0


if __name__ == '__main__':
    sys.exit(main())
