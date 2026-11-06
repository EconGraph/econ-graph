#!/usr/bin/env python3
"""Temporary fixed-sample cold-history count probes, never fetch/budget acceptance.

Reports contain only fixed IDs, static enums, canonical dates and bounded numbers.
No provider bodies, notes, observation values, URLs or exception text are exported.
"""
import argparse
import datetime as dt
import json
import os
from pathlib import Path
import re
import signal
import time
import urllib.error
import urllib.parse
import urllib.request

IDS = ('GDP', 'GDPC1', 'CPIAUCSL', 'UNRATE', 'PAYEMS', 'DGS10', 'DEXUSEU', 'DCOILWTICO')
PATHS = {'metadata': '/fred/series', 'vintages': '/fred/series/vintagedates',
         'observations': '/fred/series/observations'}
FIRST, LAST = '1776-07-04', '9999-12-31'
LIMITS = {'http_attempts': 256, 'http_attempts_per_series': 64, 'vintage_pages_per_series': 10,
          'window_nodes_per_series': 64, 'response_bytes': 1024 * 1024,
          'total_response_bytes': 32 * 1024 * 1024, 'report_bytes': 16 * 1024 * 1024,
          'request_seconds': 20, 'run_seconds': 900, 'wrapper_seconds': 930,
          'requests_per_minute': 30, 'listed_vintages_per_window': 1000,
          'production_observation_page_size': 100000, 'production_pages_per_window': 100}
CODES = {'missing_key', 'invalid_json', 'invalid_shape', 'invalid_date', 'invalid_count',
         'redirect_refused', 'transport_error', 'http_error', 'vintage_limit',
         'vintage_limit_unsplittable', 'vintage_page_cap', 'window_node_cap',
         'production_page_guard', 'restricted_now', 'global_request_cap',
         'series_request_cap', 'response_byte_cap', 'total_byte_cap', 'time_cap',
         'report_byte_cap', 'internal_error', 'interrupted'}
GLOBAL_ERRORS = {'global_request_cap', 'response_byte_cap', 'total_byte_cap', 'time_cap',
                 'report_byte_cap', 'internal_error', 'interrupted'}
MARKERS = ('prior written permission', 'may not be reproduced',
           'reproduction, retransmission, or other use is prohibited', 'all rights reserved',
           'copyright', 'reprinted with permission', 'used with permission')


class ProfileError(Exception):
    def __init__(self, code, status=None):
        if code not in CODES or (status is not None and (type(status) is not int or not 100 <= status <= 599)):
            raise ValueError('invalid internal failure classification')
        self.code, self.status = code, status
        super().__init__(code)


def count(value):
    if type(value) is not int or not 0 <= value <= 1000000000:
        raise ProfileError('invalid_count')
    return value


def ascii_lower(value):
    return value.translate(str.maketrans('ABCDEFGHIJKLMNOPQRSTUVWXYZ', 'abcdefghijklmnopqrstuvwxyz'))


def date(value):
    if not isinstance(value, str) or not re.fullmatch('[0-9]{4}-[0-9]{2}-[0-9]{2}', value):
        raise ProfileError('invalid_date')
    try:
        parsed = dt.date.fromisoformat(value)
    except ValueError:
        raise ProfileError('invalid_date') from None
    if parsed.isoformat() != value or not FIRST <= value <= LAST:
        raise ProfileError('invalid_date')
    return value


def previous(value):
    parsed = dt.date.fromisoformat(value)
    return (parsed - dt.timedelta(days=1)).isoformat() if parsed > dt.date.min else value


def split_window(window, vintages, per_window=1000):
    """RealtimeWindow::split parity: later starts overlap the prior end by one day."""
    width = max(1, per_window)
    groups = [vintages[i:i + width] for i in range(0, len(vintages), width)]
    if len(groups) <= 1:
        return [{**window, 'vintages': list(vintages)}]
    return [{'start': window['start'] if i == 0 else previous(group[0]),
             'end': previous(groups[i + 1][0]) if i + 1 < len(groups) else window['end'],
             'keep_from': window['keep_from'] if i == 0 else group[0],
             'vintages': list(group)} for i, group in enumerate(groups)]


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def decode(body):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ProfileError('invalid_json')
            result[key] = value
        return result
    def invalid_constant(_):
        raise ProfileError('invalid_json')
    try:
        value = json.loads(body.decode('utf-8'), object_pairs_hook=unique, parse_constant=invalid_constant)
    except (ValueError, UnicodeError, RecursionError):
        raise ProfileError('invalid_json') from None
    if not isinstance(value, dict):
        raise ProfileError('invalid_shape')
    return value


class Client:
    def __init__(self, key, report, opener=None, clock=time.monotonic, sleep=time.sleep):
        if not isinstance(key, str) or not re.fullmatch('[a-z0-9]{32}', key):
            raise ProfileError('missing_key')
        self.key, self.report, self.clock, self.sleep = key, report, clock, sleep
        self.opener = opener or urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
        self.started, self.next_request = clock(), clock()
        self.per_series = {id: 0 for id in IDS}

    def get(self, id, endpoint, parameters):
        if id not in IDS or endpoint not in PATHS:
            raise ProfileError('internal_error')
        now = self.clock()
        if now - self.started >= LIMITS['run_seconds']:
            raise ProfileError('time_cap')
        if self.report['http_attempts'] >= LIMITS['http_attempts']:
            raise ProfileError('global_request_cap')
        if self.per_series[id] >= LIMITS['http_attempts_per_series']:
            raise ProfileError('series_request_cap')
        wait = max(0, self.next_request - now)
        if now + wait - self.started >= LIMITS['run_seconds']:
            raise ProfileError('time_cap')
        self.sleep(wait)
        if self.clock() - self.started >= LIMITS['run_seconds']:
            raise ProfileError('time_cap')
        self.report['http_attempts'] += 1
        self.per_series[id] += 1
        self.report['series'][id]['http_attempts'] += 1
        query = {'series_id': id, 'api_key': self.key, 'file_type': 'json', **parameters}
        request = urllib.request.Request('https://api.stlouisfed.org' + PATHS[endpoint] + '?' + urllib.parse.urlencode(query),
            headers={'Accept': 'application/json', 'Accept-Encoding': 'identity',
                     'User-Agent': 'EconGraph/4.0.0 (+https://econgraph.com; support@econgraph.com)'})
        response = None
        try:
            try:
                self.next_request = self.clock() + 2
                response = self.opener.open(request, timeout=min(LIMITS['request_seconds'],
                    LIMITS['run_seconds'] - (self.clock() - self.started)))
            except urllib.error.HTTPError as error:
                response = error
            status = response.getcode()
            if type(status) is not int or not 100 <= status <= 599:
                raise ProfileError('invalid_shape')
            if 300 <= status < 400:
                raise ProfileError('redirect_refused', status)
            chunks, received = [], 0
            while True:
                if self.clock() - self.started >= LIMITS['run_seconds']:
                    raise ProfileError('time_cap')
                remaining = min(LIMITS['response_bytes'] - received,
                                LIMITS['total_response_bytes'] - self.report['response_bytes_read'])
                if remaining <= 0:
                    raise ProfileError('total_byte_cap' if self.report['response_bytes_read'] >= LIMITS['total_response_bytes'] else 'response_byte_cap')
                chunk = response.read(min(65536, remaining))
                received += len(chunk)
                self.report['response_bytes_read'] += len(chunk)
                if not chunk:
                    break
                chunks.append(chunk)
            body = b''.join(chunks)
            if status != 200:
                try:
                    value = decode(body)
                except ProfileError:
                    raise ProfileError('http_error', status) from None
                message = value.get('error_message')
                if status == 400 and endpoint == 'observations' and isinstance(message, str) and 'maximum number of vintage dates' in ascii_lower(message):
                    raise ProfileError('vintage_limit', status)
                raise ProfileError('http_error', status)
            return decode(body)
        except ProfileError:
            raise
        except Exception:
            raise ProfileError('transport_error') from None
        finally:
            if response is not None:
                response.close()


def unrestricted_metadata(client, id):
    body = client.get(id, 'metadata', {})
    rows = body.get('seriess')
    if not isinstance(rows, list) or len(rows) != 1 or not isinstance(rows[0], dict) or rows[0].get('id') != id or not isinstance(rows[0].get('title'), str):
        raise ProfileError('invalid_shape')
    notes = rows[0].get('notes')
    if notes is not None and not isinstance(notes, str):
        raise ProfileError('invalid_shape')
    if any(marker in ascii_lower(' '.join((notes or '').split())) for marker in MARKERS):
        raise ProfileError('restricted_now')


def vintage_dates(client, id, series, checkpoint):
    received, total = 0, None
    for _ in range(LIMITS['vintage_pages_per_series']):
        body = client.get(id, 'vintages', {'realtime_start': FIRST, 'realtime_end': LAST,
                                         'limit': 10000, 'offset': received})
        n = count(body.get('count'))
        rows = body.get('vintage_dates')
        if (count(body.get('offset')) != received or count(body.get('limit')) != 10000
                or body.get('realtime_start') != FIRST or body.get('realtime_end') != LAST
                or body.get('sort_order', 'asc') != 'asc' or (total is not None and n != total)
                or not isinstance(rows, list) or len(rows) != min(10000, max(0, n - received))):
            raise ProfileError('invalid_shape')
        clean = [date(value) for value in rows]
        if clean != sorted(set(clean)) or (clean and series['vintage_dates'] and clean[0] <= series['vintage_dates'][-1]):
            raise ProfileError('invalid_shape')
        series['vintage_dates'].extend(clean)
        series['vintage_date_pages'] += 1
        received += len(clean)
        total = n
        checkpoint()
        if received == total:
            series['vintages_complete'] = True
            return series['vintage_dates']
    raise ProfileError('vintage_page_cap')


def observation_count(client, id, window):
    body = client.get(id, 'observations', {'realtime_start': window['start'], 'realtime_end': window['end'],
                                         'limit': 1, 'offset': 0})
    n = count(body.get('count'))
    rows = body.get('observations')
    if (count(body.get('offset')) != 0 or count(body.get('limit')) != 1
            or body.get('realtime_start') != window['start'] or body.get('realtime_end') != window['end']
            or count(body.get('output_type', 1)) != 1 or not isinstance(rows, list) or len(rows) != min(1, n)):
        raise ProfileError('invalid_shape')
    for row in rows:
        if not isinstance(row, dict) or not isinstance(row.get('value'), str):
            raise ProfileError('invalid_shape')
        date(row.get('date')); date(row.get('realtime_start')); date(row.get('realtime_end'))
        # The value is never parsed numerically, retained, hashed or printed.
    return n


def profile_series(client, id, series, checkpoint):
    unrestricted_metadata(client, id)
    series['fresh_metadata_unrestricted'] = True
    checkpoint()
    dates = vintage_dates(client, id, series, checkpoint)
    pending = list(reversed(split_window({'start': FIRST, 'end': LAST, 'keep_from': None}, dates)))
    rejected = 0
    while pending:
        if len(series['windows']) >= LIMITS['window_nodes_per_series']:
            raise ProfileError('window_node_cap')
        window = pending.pop()
        record = {'start': window['start'], 'end': window['end'], 'keep_from': window['keep_from'],
                  'listed_vintage_count': len(window['vintages']), 'status': 'in_progress'}
        series['windows'].append(record)
        checkpoint()
        try:
            n = observation_count(client, id, window)
        except ProfileError as error:
            if error.code != 'vintage_limit':
                raise
            dates = window['vintages']
            if len(dates) < 2:
                record['status'] = 'vintage_limit_unsplittable'
                raise ProfileError('vintage_limit_unsplittable', 400)
            children = split_window(window, dates, min(1000, (len(dates) + 1) // 2))
            if len(children) < 2:
                record['status'] = 'vintage_limit_unsplittable'
                raise ProfileError('vintage_limit_unsplittable', 400)
            record['status'] = 'vintage_limit_split'
            rejected += 1
            pending.extend(reversed(children))
        else:
            pages = max(1, (n + 99999) // 100000)
            record.update(status='counted', observation_count=n, estimated_production_pages=pages)
            if pages > 100:
                record['status'] = 'production_page_guard'
                raise ProfileError('production_page_guard')
        checkpoint()
    series.update(complete=True, status='complete',
        estimated_production_cold_logical_requests=1 + series['vintage_date_pages'] + rejected + sum(w['estimated_production_pages'] for w in series['windows'] if w['status'] == 'counted'),
        observed_count_sum_with_overlap=sum(w['observation_count'] for w in series['windows'] if w['status'] == 'counted'))


def report(revision):
    if not re.fullmatch('[0-9a-f]{40}', revision):
        raise ProfileError('invalid_shape')
    return {'format': 1, 'execution_revision': revision, 'mode': 'cold_window_count_sample',
        'planning_source_revision': '2f365b8422fdbf86a45c2f7797df5617ca7b2856',
        'temporary_base_revision': '09fed0357cb61233e70e5721ee9bd70459d4500b',
        'sample_membership_revision': '52c3e8c80030c7b44b0752cd05c41b442b62444a',
        'sample_membership_date': '2026-11-05',
        'sample_membership_artifact_sha256': '0ed6a7e3ad5cc691cd32777cf0227030c06f1237039264b0e113fb593c8866ac',
        'started_at': dt.datetime.now(dt.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ'),
        'status': 'in_progress', 'complete': False, 'http_attempts': 0, 'response_bytes_read': 0,
        'limits': dict(LIMITS), 'production_budget_accepted': False, 'full_fetch_executed': False,
        'statistically_representative': False, 'production_bytes_estimated': False,
        'observation_values_retained': False, 'production_policy_changed': False,
        'counts_include_overlap_rows': True, 'production_network_retries_estimated': False,
        'limit1_equivalent_to_full_fetch_verified': False, 'known_vintage': None,
        'series': {id: {'status': 'unvisited', 'complete': False, 'http_attempts': 0, 'fresh_metadata_unrestricted': False,
                        'vintages_complete': False, 'vintage_date_pages': 0, 'vintage_dates': [], 'windows': []} for id in IDS}}


def run(client, result, checkpoint):
    started = time.monotonic()
    checkpoint()
    for id in IDS:
        series = result['series'][id]
        series['status'] = 'in_progress'
        try:
            profile_series(client, id, series, checkpoint)
        except ProfileError as error:
            series.update(complete=False, status='partial', failure={'code': error.code, 'http_status': error.status})
            if error.code in GLOBAL_ERRORS:
                result['failure'] = {'code': error.code, 'http_status': error.status}
                break
        except Exception:
            series.update(complete=False, status='partial', failure={'code': 'internal_error', 'http_status': None})
            result['failure'] = {'code': 'internal_error', 'http_status': None}
            break
        checkpoint()
    result['complete'] = not result.get('failure') and all(
        series['complete'] and not series.get('failure') for series in result['series'].values())
    result['status'] = 'complete' if result['complete'] else 'partial'
    result['elapsed_milliseconds'] = max(0, int((time.monotonic() - started) * 1000))
    result['finished_at'] = dt.datetime.now(dt.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')
    checkpoint()
    return 0 if result['complete'] else 1


def write_report(output, result):
    content = json.dumps(result, ensure_ascii=True, sort_keys=True, separators=(',', ':'), allow_nan=False).encode() + b'\n'
    if len(content) > LIMITS['report_bytes']:
        raise ProfileError('report_byte_cap')
    pending = output / 'profile.pending'
    pending.write_bytes(content)
    pending.replace(output / 'profile.json')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--revision', required=True)
    args = parser.parse_args()
    if not args.output.is_absolute():
        return 2
    args.output.mkdir(parents=True, exist_ok=False)
    result = report(args.revision)
    checkpoint = lambda: write_report(args.output, result)
    checkpoint()
    status = 1
    def deadline(_signal, _frame):
        raise ProfileError('time_cap')
    signal.signal(signal.SIGALRM, deadline)
    signal.setitimer(signal.ITIMER_REAL, LIMITS['run_seconds'])
    try:
        client = Client(os.environ.get('FRED_API_KEY', ''), result)
        status = run(client, result, checkpoint)
    except ProfileError as error:
        status = 1
        result.update(status='partial', complete=False, failure={'code': error.code, 'http_status': error.status})
    except BaseException:
        status = 1
        result.update(status='partial', complete=False, failure={'code': 'interrupted', 'http_status': None})
    finally:
        signal.setitimer(signal.ITIMER_REAL, 0)
        checkpoint()
        (args.output / 'exit-status.txt').write_text(str(status) + '\n')
    return status


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except SystemExit:
        raise
    except BaseException:
        raise SystemExit(2) from None
