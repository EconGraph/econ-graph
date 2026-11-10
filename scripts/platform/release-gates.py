#!/usr/bin/env python3
"""Threshold checks and an availability probe for the v4.2.0 live release gates.

The check subcommands read JSON or text the operator captured with the read-only
commands named on each gate's Linear issue. They never call kubectl, a cluster or
a host themselves. `probe` sends HTTP requests to the one URL it is given.

  cnpg-startup    CNPG operator and barman plugin running, all three Clusters healthy
  backup-history  every database kept a completed base backup within its limit, all window long
  resources       pod requests, peak node use (from Prometheus) and host disk below their limits
  probe           request one URL until stopped and record each result, for the drain gate

Each check prints a JSON report, writes it to --report when given, and exits 1 on failure.
"""
import argparse
import datetime as dt
import http.client
import importlib.util
import json
import os
from pathlib import Path
import re
import signal
import ssl
import sys
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
CLUSTERS = ('econgraph-app', 'econgraph-data', 'econgraph-keycloak')
HEALTHY = 'Cluster in healthy state'
PLUGIN = 'barman-cloud.cloudnative-pg.io'


def _backup_limits():
    # One source for the per-database limits: the in-cluster backup health check.
    spec = importlib.util.spec_from_file_location('backup_health', ROOT / 'platform/observability/backup-health.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return dict(module.LIMITS)


def items(path):
    value = json.loads(Path(path).read_text())
    return value['items'] if isinstance(value, dict) and 'items' in value else value


def condition(obj, kind):
    return any(c.get('type') == kind and c.get('status') == 'True'
               for c in obj.get('status', {}).get('conditions', []))


def timestamp(value):
    parsed = dt.datetime.fromisoformat(value.replace('Z', '+00:00'))
    if parsed.tzinfo is None:
        raise ValueError(f'timestamp without a time zone: {value}')
    return parsed


# cnpg-startup

def check_cnpg_startup(deployments, clusters):
    failures = []
    for wanted in ('cloudnative-pg', 'barman'):
        found = [d for d in deployments if wanted in d['metadata']['name']]
        if not found:
            failures.append(f'no {wanted} Deployment in cnpg-system')
        for d in found:
            want = d.get('spec', {}).get('replicas', 1)
            have = d.get('status', {}).get('availableReplicas', 0)
            if want < 1 or have < want or not condition(d, 'Available'):
                failures.append(f"{d['metadata']['name']}: {have}/{want} replicas available")
    by_name = {c['metadata']['name']: c for c in clusters}
    for name in CLUSTERS:
        c = by_name.get(name)
        if c is None:
            failures.append(f'{name}: Cluster missing')
            continue
        status = c.get('status', {})
        want = c.get('spec', {}).get('instances', 0)
        if want < 2 or status.get('readyInstances', 0) != want:
            failures.append(f"{name}: {status.get('readyInstances', 0)}/{want} instances ready")
        if status.get('phase') != HEALTHY:
            failures.append(f"{name}: phase is {status.get('phase')!r}")
        if not condition(c, 'ContinuousArchiving'):
            failures.append(f'{name}: WAL archiving is not confirmed healthy')
        archivers = [p for p in c.get('spec', {}).get('plugins', []) if p.get('isWALArchiver')]
        if [p.get('name') for p in archivers] != [PLUGIN]:
            failures.append(f'{name}: WAL archiver is not the barman plugin')
    return {'failures': failures}


# backup-history

def check_backup_history(backups, now, window, limits):
    failures, clusters = [], {}
    start = now - window
    for name, limit_seconds in limits.items():
        limit = dt.timedelta(seconds=limit_seconds)
        own = [b for b in backups if b.get('spec', {}).get('cluster', {}).get('name') == name
               and b.get('spec', {}).get('method') == 'plugin'
               and b.get('spec', {}).get('pluginConfiguration', {}).get('name') == PLUGIN]
        failed = [b['metadata']['name'] for b in own
                  if b.get('status', {}).get('phase', '').lower() == 'failed'
                  and timestamp(b['metadata']['creationTimestamp']) >= start]
        done = sorted(timestamp(b['status']['stoppedAt']) for b in own
                      if b.get('status', {}).get('phase') == 'completed' and b.get('status', {}).get('stoppedAt'))
        # A backup completed before the window still covers its start.
        before = [t for t in done if t <= start]
        inside = [t for t in done if start < t <= now]
        points = ([before[-1]] if before else []) + inside
        worst = None
        if not before:
            failures.append(f'{name}: no completed backup before the window starts; history is shorter than the window')
        elif not inside:
            failures.append(f'{name}: no completed backup inside the window')
        else:
            # The first gap runs from the last backup before the window, as backup-health.py measures age.
            edges = points + [now]
            worst = max(b - a for a, b in zip(edges, edges[1:]))
            if worst > limit:
                failures.append(f'{name}: longest time without a completed backup was {worst}, limit {limit}')
        if failed:
            failures.append(f'{name}: {len(failed)} failed backups in the window: {", ".join(sorted(failed))}')
        clusters[name] = {'completed_in_window': len(inside), 'failed_in_window': len(failed),
                          'longest_gap_seconds': worst.total_seconds() if worst else None,
                          'limit_seconds': limit_seconds}
    return {'window_start': start.isoformat(), 'window_end': now.isoformat(),
            'clusters': clusters, 'failures': failures}


# resources

UNITS = {'': 1, 'k': 10**3, 'M': 10**6, 'G': 10**9, 'T': 10**12, 'P': 10**15,
         'Ki': 2**10, 'Mi': 2**20, 'Gi': 2**30, 'Ti': 2**40, 'Pi': 2**50}


def cpu(value):
    value = str(value)
    return float(value[:-1]) / 1000 if value.endswith('m') else float(value)


def memory(value):
    match = re.fullmatch(r'([0-9.]+)([kMGTP]i?|)', str(value))
    if not match:
        raise ValueError(f'unrecognised memory quantity {value!r}')
    return float(match.group(1)) * UNITS[match.group(2)]


def pod_requests(pod):
    # Kubernetes schedules on max(sum of containers, largest init container) plus overhead.
    def total(containers, unit, parse):
        return [parse(c.get('resources', {}).get('requests', {}).get(unit, 0)) for c in containers]
    spec = pod.get('spec', {})
    result = {}
    for unit, parse in (('cpu', cpu), ('memory', memory)):
        main = sum(total(spec.get('containers', []), unit, parse))
        init = max(total(spec.get('initContainers', []), unit, parse), default=0)
        result[unit] = max(main, init) + parse(spec.get('overhead', {}).get(unit, 0))
    return result


def parse_usage(cpu_range, memory_range):
    """Prometheus query_range responses, one series per `node` label, into node -> unit -> values."""
    usage = {}
    for unit, response in (('cpu', cpu_range), ('memory', memory_range)):
        if response.get('status') != 'success' or response.get('data', {}).get('resultType') != 'matrix':
            raise ValueError(f'{unit}: not a successful Prometheus range-query response')
        for series in response['data']['result']:
            node = series.get('metric', {}).get('node')
            if not node:
                raise ValueError(f'{unit}: series without a node label; group the query by node')
            usage.setdefault(node, {}).setdefault(unit, []).extend(float(v) for _, v in series['values'])
    return usage


def parse_df(text):
    """`df -P -B1 PATH...` output."""
    result = {}
    for line in text.splitlines()[1:]:
        fields = line.split()
        if len(fields) >= 6:
            size, used = int(fields[1]), int(fields[2])
            result[fields[5]] = used / size if size else 1.0
    return result


def check_resources(nodes, pods, usage, disks, limits):
    failures, report = [], {}
    allocatable = {n['metadata']['name']: {'cpu': cpu(n['status']['allocatable']['cpu']),
                                            'memory': memory(n['status']['allocatable']['memory'])}
                   for n in nodes}
    requested = {name: {'cpu': 0.0, 'memory': 0.0} for name in allocatable}
    for pod in pods:
        node = pod.get('spec', {}).get('nodeName')
        if node in requested and pod.get('status', {}).get('phase') not in ('Succeeded', 'Failed'):
            for unit, amount in pod_requests(pod).items():
                requested[node][unit] += amount
    for name, alloc in sorted(allocatable.items()):
        entry = report[name] = {}
        for unit in ('cpu', 'memory'):
            ratio = requested[name][unit] / alloc[unit] if alloc[unit] else 1.0
            entry[f'{unit}_requested_ratio'] = round(ratio, 4)
            if ratio > limits['requests']:
                failures.append(f"{name}: {unit} requests are {ratio:.0%} of allocatable, limit {limits['requests']:.0%}")
            seen = usage.get(name, {}).get(unit, [])
            entry[f'{unit}_samples'] = len(seen)
            if len(seen) < limits['min_samples']:
                failures.append(f"{name}: {len(seen)} {unit} usage samples, at least {limits['min_samples']} required")
            if not seen:
                continue
            peak = max(seen) / alloc[unit] if alloc[unit] else 1.0
            entry[f'{unit}_peak_ratio'] = round(peak, 4)
            if peak > limits['usage']:
                failures.append(f"{name}: peak {unit} use was {peak:.0%} of allocatable, limit {limits['usage']:.0%}")
    for mount, ratio in sorted(disks.items()):
        report[f'disk:{mount}'] = {'used_ratio': round(ratio, 4)}
        if ratio > limits['disk']:
            failures.append(f"{mount}: {ratio:.0%} used, limit {limits['disk']:.0%}")
    return {'nodes': report, 'failures': failures}


# probe

def write_atomic(path, value):
    temporary = path.with_name(path.name + '.tmp')
    temporary.write_text(json.dumps(value, indent=2) + '\n')
    os.replace(temporary, path)


def request_once(url, body, timeout, context, graphql):
    started = dt.datetime.now(dt.timezone.utc)
    data = body.encode() if body is not None else None
    headers = {'Content-Type': 'application/json'} if data is not None else {}
    record = {'started_at': started.isoformat()}
    try:
        with urllib.request.urlopen(urllib.request.Request(url, data=data, headers=headers),
                                    timeout=timeout, context=context) as response:
            payload = response.read()
            record['status'] = response.status
            success = 200 <= response.status < 300
            if success and graphql:
                answer = json.loads(payload)
                if not isinstance(answer, dict) or answer.get('errors') or 'data' not in answer:
                    success, record['error'] = False, 'GraphQL errors in response'
    except urllib.error.HTTPError as error:
        record['status'], success, record['error'] = error.code, False, f'HTTP {error.code}'
    except (OSError, ValueError, http.client.HTTPException) as error:
        # Connection failures, timeouts, TLS errors, truncated or non-JSON bodies all count as failed requests.
        success, record['error'] = False, type(error).__name__
    record['completed_at'] = dt.datetime.now(dt.timezone.utc).isoformat()
    record['success'] = success
    return record


def probe(args, clock=time.monotonic, sleep=time.sleep, send=request_once):
    if not args.url.startswith(('https://', 'http://')):
        raise ValueError('--url must be an http(s) URL')
    body = args.json_body.read_text() if args.json_body else None
    if body is not None:
        json.loads(body)
    context = ssl.create_default_context(cafile=str(args.cafile) if args.cafile else None)
    stop = {'now': False}

    def finish(*_):
        stop['now'] = True
    previous = {s: signal.signal(s, finish) for s in (signal.SIGTERM, signal.SIGINT)}
    evidence = {'url': args.url, 'interval_seconds': args.interval, 'requests': []}
    deadline = clock() + args.duration
    try:
        # The file is rewritten after every request, so a reader mid-run sees a complete JSON document.
        while not stop['now'] and clock() < deadline:
            evidence['requests'].append(send(args.url, body, args.timeout, context, args.graphql))
            write_atomic(args.out, evidence)
            sleep(args.interval)
    finally:
        for number, handler in previous.items():
            signal.signal(number, handler)
    failed = [r for r in evidence['requests'] if not r['success']]
    summary = {'requests': len(evidence['requests']), 'failed': len(failed), 'out': str(args.out)}
    print(json.dumps(summary))
    return 1 if failed or not evidence['requests'] else 0


def emit(report, path):
    report['passed'] = not report['failures']
    text = json.dumps(report, indent=2, sort_keys=True) + '\n'
    if path:
        Path(path).write_text(text)
    print(text, end='')
    return 0 if report['passed'] else 1


def parser():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest='command', required=True)
    s = sub.add_parser('cnpg-startup')
    s.add_argument('--deployments', type=Path, required=True, help='kubectl get deployments -n cnpg-system -o json')
    s.add_argument('--clusters', type=Path, required=True, help='kubectl get clusters.postgresql.cnpg.io -n econ-graph -o json')
    s.add_argument('--report', type=Path)
    s = sub.add_parser('backup-history')
    s.add_argument('--backups', type=Path, required=True, help='kubectl get backups.postgresql.cnpg.io -n econ-graph -o json')
    s.add_argument('--window-days', type=float, default=7.0)
    s.add_argument('--now', help='UTC end of the window (default: now)')
    s.add_argument('--limit', action='append', default=[], metavar='CLUSTER=SECONDS',
                   help='override the backup-health.py limit for one cluster; repeatable')
    s.add_argument('--report', type=Path)
    s = sub.add_parser('resources')
    s.add_argument('--nodes', type=Path, required=True, help='kubectl get nodes -o json')
    s.add_argument('--pods', type=Path, required=True, help='kubectl get pods -A -o json')
    s.add_argument('--cpu-range', type=Path, required=True, help='Prometheus query_range JSON: CPU cores used, by node')
    s.add_argument('--memory-range', type=Path, required=True, help='Prometheus query_range JSON: working-set bytes, by node')
    s.add_argument('--df', type=Path, help='host df -P -B1 PATH... output')
    s.add_argument('--max-request-ratio', type=float, default=0.85)
    s.add_argument('--max-usage-ratio', type=float, default=0.80)
    s.add_argument('--max-disk-ratio', type=float, default=0.80)
    s.add_argument('--min-samples', type=int, default=10)
    s.add_argument('--report', type=Path)
    s = sub.add_parser('probe')
    s.add_argument('--url', required=True)
    s.add_argument('--json-body', type=Path, help='POST this JSON body instead of a GET')
    s.add_argument('--graphql', action='store_true', help='also fail a response carrying GraphQL errors')
    s.add_argument('--cafile', type=Path, help='trust this CA bundle instead of the system store')
    s.add_argument('--interval', type=float, default=2.0)
    s.add_argument('--timeout', type=float, default=5.0)
    s.add_argument('--duration', type=float, default=3600.0, help='seconds; SIGTERM or Ctrl-C stops it sooner')
    s.add_argument('--out', type=Path, required=True)
    return p


def main(argv=None):
    args = parser().parse_args(argv)
    if args.command == 'cnpg-startup':
        return emit(check_cnpg_startup(items(args.deployments), items(args.clusters)), args.report)
    if args.command == 'backup-history':
        now = timestamp(args.now) if args.now else dt.datetime.now(dt.timezone.utc)
        limits = _backup_limits()
        for override in args.limit:
            name, _, seconds = override.partition('=')
            if name not in limits or not seconds.isdigit() or int(seconds) <= 0:
                raise ValueError(f'--limit wants one of {", ".join(limits)}=SECONDS, got {override!r}')
            limits[name] = int(seconds)
        return emit(check_backup_history(items(args.backups), now, dt.timedelta(days=args.window_days),
                                         limits), args.report)
    if args.command == 'resources':
        limits = {'requests': args.max_request_ratio, 'usage': args.max_usage_ratio,
                  'disk': args.max_disk_ratio, 'min_samples': args.min_samples}
        disks = parse_df(args.df.read_text()) if args.df else {}
        usage = parse_usage(json.loads(args.cpu_range.read_text()), json.loads(args.memory_range.read_text()))
        return emit(check_resources(items(args.nodes), items(args.pods), usage, disks, limits), args.report)
    return probe(args)


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (ValueError, KeyError, OSError) as error:
        print(f'release-gates: {error}', file=sys.stderr)
        sys.exit(2)
