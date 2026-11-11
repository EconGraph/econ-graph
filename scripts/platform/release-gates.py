#!/usr/bin/env python3
"""Threshold checks and an availability probe for the v4.2.0 live release gates.

The check subcommands read JSON or text the operator captured with the read-only
commands named on each gate's Linear issue. They never call kubectl, a cluster or
a host themselves. `probe` sends HTTP requests to the one URL it is given.

  cnpg-startup    CNPG operator and barman plugin running, all three Clusters healthy
  backup-history  every database kept a completed base backup within its limit, all window long
  resources       pod requests, peak node use (from Prometheus) and host disk below their limits
  probe           request one URL until stopped and record each result and its timing, for the drain gate

Each check prints a JSON report, writes it to --report when given, and exits 1 on failure.
"""
import argparse
import datetime as dt
import http.client
import importlib.util
import json
import math
import os
from pathlib import Path
import re
import signal
import ssl
import sys
import threading
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

def rollout_failures(d):
    """Why a Deployment's rollout is not complete, by `kubectl rollout status`'s rules.

    During a surge the old pod stays Available while the new one starts, so availability alone
    passes too early: every replica must be updated, no old replica may remain, and every
    updated replica must be available.
    """
    name, want = d['metadata']['name'], d.get('spec', {}).get('replicas', 1)
    status = d.get('status', {})
    updated, total = status.get('updatedReplicas', 0), status.get('replicas', 0)
    available = status.get('availableReplicas', 0)
    failures = []
    if (status.get('observedGeneration') or 0) < (d['metadata'].get('generation') or 0):
        failures.append(f"{name}: controller has not observed generation {d['metadata'].get('generation')}")
    if want < 1 or updated != want:
        failures.append(f'{name}: {updated}/{want} replicas updated')
    if total != updated:
        failures.append(f'{name}: {total - updated} old replicas still running')
    if available != updated or not condition(d, 'Available'):
        failures.append(f'{name}: {available}/{updated} updated replicas available')
    return failures


def check_cnpg_startup(deployments, clusters):
    failures = []
    for wanted in ('cloudnative-pg', 'barman'):
        found = [d for d in deployments if wanted in d['metadata']['name']]
        if not found:
            failures.append(f'no {wanted} Deployment in cnpg-system')
        for d in found:
            failures += rollout_failures(d)
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

def check_backup_history(backups, now, window, limits, grace=dt.timedelta(hours=2)):
    failures, clusters = [], {}
    start = now - window
    for name, limit_seconds in limits.items():
        limit = dt.timedelta(seconds=limit_seconds)
        own = [b for b in backups if b.get('spec', {}).get('cluster', {}).get('name') == name
               and b.get('spec', {}).get('method') == 'plugin'
               and b.get('spec', {}).get('pluginConfiguration', {}).get('name') == PLUGIN]
        failed = []
        for b in own:
            status = b.get('status', {})
            created = timestamp(b['metadata']['creationTimestamp'])
            stopped = timestamp(status['stoppedAt']) if status.get('stoppedAt') else None
            phase = status.get('phase') or ''
            if phase == 'completed':
                if not stopped:
                    failed.append(f"{b['metadata']['name']} (completed without stoppedAt)")
                continue
            # Skip only what provably ended before the window. Without stoppedAt there is no
            # end to place, so a pre-window backup that is unfinished or failed still counts.
            if created < start and stopped and stopped <= start:
                continue
            if phase.lower() in ('failed', 'walarchivingfailing') or now - created > grace:
                # Failed outright, or still running, pending or unknown past the grace period.
                failed.append(f"{b['metadata']['name']} ({phase or 'no phase'})")
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
            failures.append(f'{name}: {len(failed)} backups in the window did not complete: {", ".join(sorted(failed))}')
        clusters[name] = {'completed_in_window': len(inside), 'failed_in_window': len(failed),
                          'longest_gap_seconds': worst.total_seconds() if worst is not None else None,
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
    """The pod's effective request, as the Kubernetes scheduler computes it.

    Init containers with restartPolicy: Always are native sidecars (the barman plugin's is one):
    they keep running, so they add to the regular containers and to every init container
    started after them. The pod needs max(regular containers + all sidecars, peak while
    initialising) plus overhead.
    """
    spec = pod.get('spec', {})
    result = {}
    for unit, parse in (('cpu', cpu), ('memory', memory)):
        def request(container):
            return parse(container.get('resources', {}).get('requests', {}).get(unit, 0))
        sidecars, init_peak = 0.0, 0.0
        for container in spec.get('initContainers', []):
            if container.get('restartPolicy') == 'Always':
                sidecars += request(container)
                init_peak = max(init_peak, sidecars)
            else:
                init_peak = max(init_peak, sidecars + request(container))
        main = sum(request(c) for c in spec.get('containers', [])) + sidecars
        result[unit] = max(main, init_peak) + parse(spec.get('overhead', {}).get(unit, 0))
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
            # Prometheus writes NaN or Inf for gaps and division by zero; they are not measurements.
            values = [float(v) for _, v in series['values']]
            usage.setdefault(node, {}).setdefault(unit, []).extend(v for v in values if math.isfinite(v))
    return usage


def parse_df(text):
    """`df -P -B1 PATH...` output, as mount -> used / (used + available), the ratio df prints."""
    result = {}
    for line in text.splitlines()[1:]:
        fields = line.split()
        if len(fields) >= 6:
            used, available = int(fields[2]), int(fields[3])
            result[' '.join(fields[5:])] = used / (used + available) if used + available else 1.0
    return result


def check_resources(nodes, pods, usage, disks, mounts, limits):
    failures, report = [], {}
    if not nodes:
        failures.append('no nodes in the node capture')
    if not disks:
        failures.append('the df capture lists no filesystems')
    for mount in mounts:
        if mount not in disks:
            failures.append(f'{mount}: expected filesystem missing from the df capture')
    for n in nodes:
        if not condition(n, 'Ready'):
            failures.append(f"{n['metadata']['name']}: node is not Ready")
        if n.get('spec', {}).get('unschedulable'):
            failures.append(f"{n['metadata']['name']}: node is cordoned")
    allocatable = {n['metadata']['name']: {'cpu': cpu(n['status']['allocatable']['cpu']),
                                            'memory': memory(n['status']['allocatable']['memory'])}
                   for n in nodes}
    requested = {name: {'cpu': 0.0, 'memory': 0.0} for name in allocatable}
    for pod in pods:
        node = pod.get('spec', {}).get('nodeName')
        if not node and pod.get('status', {}).get('phase') == 'Pending':
            # A pod that fits on no node is the clearest sign of too little capacity.
            meta = pod.get('metadata', {})
            failures.append(f"{meta.get('namespace')}/{meta.get('name')}: Pending and not scheduled")
        if node and node not in requested and pod.get('status', {}).get('phase') not in ('Succeeded', 'Failed'):
            meta = pod.get('metadata', {})
            failures.append(f"{meta.get('namespace')}/{meta.get('name')}: on node {node}, which is not in the node capture")
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
    clock = time.monotonic()
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
    record['duration_seconds'] = round(time.monotonic() - clock, 3)
    record['success'] = success
    return record


ABANDONED = []  # request threads still running after their deadline


def request_bounded(url, body, timeout, context, graphql):
    """request_once under one overall deadline of `timeout` seconds.

    The socket timeout bounds each read, not the request: a slow DNS lookup or a body that
    drips a byte at a time can run far past it. A request still running at the deadline is
    recorded as failed and left to finish in a daemon thread.
    """
    result, started, clock = {}, dt.datetime.now(dt.timezone.utc), time.monotonic()
    worker = threading.Thread(target=lambda: result.update(request_once(url, body, timeout, context, graphql)),
                              daemon=True)
    worker.start()
    worker.join(timeout)
    if worker.is_alive() or not result:
        if worker.is_alive():
            ABANDONED.append(worker)
        return {'started_at': started.isoformat(), 'completed_at': dt.datetime.now(dt.timezone.utc).isoformat(),
                'duration_seconds': round(time.monotonic() - clock, 3), 'success': False,
                'error': f'no response within the {timeout}s deadline'}
    return dict(result)


def probe(args, clock=time.monotonic, sleep=time.sleep, send=request_bounded):
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
    previous_done, previous_wall = None, None
    limit = args.interval + args.timeout
    max_abandoned = getattr(args, 'max_abandoned', 5)
    try:
        # The file is rewritten after every request, so a reader mid-run sees a complete JSON document.
        while not stop['now'] and clock() < deadline:
            record = send(args.url, body, args.timeout, context, args.graphql)
            done = clock()
            if record.get('duration_seconds', 0) > args.timeout:
                record['success'], record['error'] = False, f"took {record['duration_seconds']}s, limit {args.timeout}s"
            # A stalled probe leaves a stretch with no observation, which must not pass as available.
            # The monotonic clock stops while the host is suspended, so also compare wall-clock times.
            wall = timestamp(record['completed_at']) if record.get('completed_at') else None
            gaps = []
            if previous_done is not None:
                gaps.append(done - previous_done)
            if previous_wall is not None and wall is not None:
                gaps.append((wall - previous_wall).total_seconds())
            if gaps and max(gaps) > limit:
                record['success'] = False
                record['error'] = f'{max(gaps):.1f}s since the previous request completed, limit {limit}s'
            previous_done, previous_wall = done, wall or previous_wall
            leaked = sum(t.is_alive() for t in ABANDONED)
            if leaked > max_abandoned:
                record['success'] = False
                record['error'] = f'{leaked} requests still running past their deadline; stopping the probe'
                stop['now'] = True
            evidence['requests'].append(record)
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
    s.add_argument('--grace-minutes', type=float, default=120.0,
                   help='a backup still not completed this long after creation counts as failed')
    s.add_argument('--report', type=Path)
    s = sub.add_parser('resources')
    s.add_argument('--nodes', type=Path, required=True, help='kubectl get nodes -o json')
    s.add_argument('--pods', type=Path, required=True, help='kubectl get pods -A -o json')
    s.add_argument('--cpu-range', type=Path, required=True, help='Prometheus query_range JSON: CPU cores used, by node')
    s.add_argument('--memory-range', type=Path, required=True, help='Prometheus query_range JSON: working-set bytes, by node')
    s.add_argument('--df', type=Path, required=True, help='host df -P -B1 PATH... output')
    s.add_argument('--mount', action='append', required=True,
                   help='a mount point that must appear in the df capture; repeatable')
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
    s.add_argument('--max-abandoned', type=int, default=5,
                   help='stop, failing, when more requests than this are still running past their deadline')
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
                                         limits, dt.timedelta(minutes=args.grace_minutes)), args.report)
    if args.command == 'resources':
        limits = {'requests': args.max_request_ratio, 'usage': args.max_usage_ratio,
                  'disk': args.max_disk_ratio, 'min_samples': args.min_samples}
        disks = parse_df(args.df.read_text())
        usage = parse_usage(json.loads(args.cpu_range.read_text()), json.loads(args.memory_range.read_text()))
        return emit(check_resources(items(args.nodes), items(args.pods), usage, disks, args.mount, limits), args.report)
    return probe(args)


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (ValueError, KeyError, OSError) as error:
        print(f'release-gates: {error}', file=sys.stderr)
        sys.exit(2)
