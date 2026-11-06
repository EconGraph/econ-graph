import datetime as dt
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import urllib.parse

SCRIPT = Path(__file__).resolve().parents[1] / 'profile-fred-history-counts.py'
spec = importlib.util.spec_from_file_location('fred_history_profile', SCRIPT)
P = importlib.util.module_from_spec(spec)
spec.loader.exec_module(P)
REVISION = 'a' * 40
KEY = 'k' * 32  # Offline fixture only; tests never read runtime credentials.
MARKER = 'PRIVATE_PROVIDER_TEXT https://invalid.example/?api_key=' + KEY


class Clock:
    def __init__(self): self.now = 0
    def __call__(self): return self.now
    def sleep(self, seconds): self.now += seconds


class Response(io.BytesIO):
    def __init__(self, status, body):
        super().__init__(body if isinstance(body, bytes) else json.dumps(body).encode())
        self.status = status
    def getcode(self): return self.status


class Opener:
    def __init__(self, handler): self.handler, self.requests = handler, []
    def open(self, request, timeout):
        self.requests.append((request, timeout))
        return self.handler(request)


def query(request): return dict(urllib.parse.parse_qsl(urllib.parse.urlparse(request.full_url).query))
def dates(n): return [(dt.date(2000, 1, 1) + dt.timedelta(days=i)).isoformat() for i in range(n)]


class FakeFred:
    def __init__(self, vintage_dates=None, observation_count=100001):
        self.dates = dates(3) if vintage_dates is None else vintage_dates
        self.count = observation_count
        self.calls = []
    def __call__(self, request):
        q = query(request); path = urllib.parse.urlparse(request.full_url).path
        self.calls.append((path, q))
        if path == '/fred/series':
            body = {'seriess': [{'id': q['series_id'], 'title': MARKER, 'notes': MARKER}]}
        elif path == '/fred/series/vintagedates':
            offset = int(q['offset'])
            body = {'realtime_start': P.FIRST, 'realtime_end': P.LAST, 'count': len(self.dates),
                    'limit': 10000, 'offset': offset, 'vintage_dates': self.dates[offset:offset + 10000]}
        elif path == '/fred/series/observations':
            body = {'realtime_start': q['realtime_start'], 'realtime_end': q['realtime_end'],
                    'count': self.count, 'limit': 1, 'offset': 0, 'output_type': 1,
                    'observations': [] if self.count == 0 else [{'date': '2000-01-01',
                        'realtime_start': q['realtime_start'], 'realtime_end': q['realtime_end'], 'value': MARKER}]}
        else: raise AssertionError('unexpected endpoint')
        return Response(200, body)


def fixture(handler=None):
    clock = Clock(); result = P.report(REVISION); opener = Opener(handler or FakeFred())
    return P.Client(KEY, result, opener, clock, clock.sleep), result, opener, clock


class PlannerTests(unittest.TestCase):
    def test_actual_rust_windows_split_at_vintage_boundaries_golden(self):
        # Literal oracle from fred.rs::windows_split_at_vintage_boundaries at2f365b84.
        whole = {'start': P.FIRST, 'end': P.LAST, 'keep_from': None}
        v = ['2020-01-01', '2020-01-02', '2020-02-01', '2020-03-01', '2020-03-02']
        expected = [(P.FIRST, '2020-01-31', None, v[:2]),
                    ('2020-01-31', '2020-03-01', '2020-02-01', v[2:4]),
                    ('2020-03-01', P.LAST, '2020-03-02', v[4:])]
        windows = P.split_window(whole, v, 2)
        self.assertEqual([(w['start'], w['end'], w['keep_from'], w['vintages']) for w in windows], expected)
        for day in dates(1) + ['2020-01-31', '2020-02-01', '2020-03-01', '2020-03-02', P.LAST]:
            self.assertEqual(sum((w['keep_from'] is None or day >= w['keep_from']) and day <= w['end'] for w in windows), 1)
        for i in [1, 2]: self.assertEqual(windows[i - 1]['end'], windows[i]['start'])
        self.assertEqual(P.split_window(whole, [], 1000), [{**whole, 'vintages': []}])
        self.assertEqual(P.split_window(whole, v, 5), [{**whole, 'vintages': v}])

    def test_rust_incremental_golden_keeps_parent_boundary_but_not_profile_mode(self):
        v = ['2020-01-01', '2020-01-02', '2020-02-01', '2020-03-01', '2020-03-02']
        parent = {'start': '2020-01-01', 'end': P.LAST, 'keep_from': '2020-01-02'}
        w = P.split_window(parent, v, 3)
        self.assertEqual((w[0]['start'], w[0]['end'], w[0]['keep_from']), ('2020-01-01', '2020-02-29', '2020-01-02'))
        self.assertEqual((w[1]['start'], w[1]['keep_from']), ('2020-02-29', '2020-03-01'))
        self.assertIsNone(P.report(REVISION)['known_vintage'])


class ProbeTests(unittest.TestCase):
    def test_late_time_cap_after_final_series_progress_cannot_green_wrap_failure(self):
        client, result, _, _ = fixture(FakeFred(observation_count=1))
        original = P.profile_series
        def late_deadline(client, id, series, checkpoint):
            original(client, id, series, checkpoint)
            if id == P.IDS[-1]:
                # SIGALRM may raise here after complete=True, before run resumes.
                raise P.ProfileError('time_cap')
        with patch.object(P, 'profile_series', side_effect=late_deadline):
            status = P.run(client, result, lambda: None)
        self.assertEqual(status, 1)
        self.assertFalse(result['complete'])
        self.assertEqual(result['status'], 'partial')
        self.assertEqual(result['failure']['code'], 'time_cap')
        self.assertFalse(result['series'][P.IDS[-1]]['complete'])
        self.assertEqual(result['series'][P.IDS[-1]]['status'], 'partial')
        self.assertIn('estimated_production_cold_logical_requests', result['series'][P.IDS[-1]])

    def test_all_eight_count_without_exporting_values_notes_urls_or_credentials(self):
        client, result, opener, clock = fixture()
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            self.assertEqual(P.run(client, result, lambda: P.write_report(output, result)), 0)
            content = (output / 'profile.json').read_bytes()
            for forbidden in [MARKER.encode(), KEY.encode(), b'https://', b'api_key', b'PRIVATE_PROVIDER_TEXT']:
                self.assertNotIn(forbidden, content)
            self.assertEqual(len(opener.requests), 24)
            self.assertEqual(result['http_attempts'], 24)
            self.assertEqual(clock.now, 46)
            self.assertTrue(result['complete'])
            for row in result['series'].values():
                self.assertEqual(row['http_attempts'], 3)
                self.assertEqual(row['estimated_production_cold_logical_requests'], 4)
                self.assertEqual(row['observed_count_sum_with_overlap'], 100001)
                self.assertTrue(row['fresh_metadata_unrestricted'])
            for flag in ['production_budget_accepted', 'full_fetch_executed', 'production_bytes_estimated',
                         'statistically_representative', 'limit1_equivalent_to_full_fetch_verified']:
                self.assertFalse(result[flag])
            for request, timeout in opener.requests:
                self.assertLessEqual(timeout, 20)
                q = query(request)
                self.assertEqual(q['file_type'], 'json')
                self.assertEqual(q['api_key'], KEY)
                self.assertNotIn('observation_start', q)
                self.assertNotIn('vintage_dates', q)
                self.assertNotIn('output_type', q)

    def test_recognized_400_odd_split_counts_rejected_parent_and_overlap(self):
        v = ['2020-01-01', '2020-01-02', '2020-02-01', '2020-03-01', '2020-03-02']
        fake = FakeFred(v, 1)
        def handler(request):
            q = query(request)
            if '/observations?' in request.full_url and q['realtime_start'] == P.FIRST and q['realtime_end'] == P.LAST:
                return Response(400, {'error_message': 'Bad Request. maximum number of vintage dates ' + MARKER})
            return fake(request)
        client, result, _, _ = fixture(handler)
        series = result['series']['GDP']
        P.profile_series(client, 'GDP', series, lambda: None)
        self.assertEqual([w['status'] for w in series['windows']], ['vintage_limit_split', 'counted', 'counted'])
        self.assertEqual([(w['start'], w['end'], w['keep_from'], w['listed_vintage_count']) for w in series['windows']],
                         [(P.FIRST, P.LAST, None, 5), (P.FIRST, '2020-02-29', None, 3),
                          ('2020-02-29', P.LAST, '2020-03-01', 2)])
        self.assertEqual(series['estimated_production_cold_logical_requests'], 5)
        self.assertEqual(series['observed_count_sum_with_overlap'], 2)
        self.assertNotIn(MARKER, json.dumps(result))

    def test_empty_vintages_still_probe_one_window_and_zero_count_costs_one_page(self):
        client, result, opener, _ = fixture(FakeFred([], 0))
        P.profile_series(client, 'GDP', result['series']['GDP'], lambda: None)
        self.assertEqual(len(opener.requests), 3)
        self.assertEqual(result['series']['GDP']['windows'][0]['estimated_production_pages'], 1)

    def test_complete_10001_vintage_dates_use_two_pages_and_eleven_windows(self):
        client, result, opener, _ = fixture(FakeFred(dates(10001), 1))
        series = result['series']['GDP']
        P.profile_series(client, 'GDP', series, lambda: None)
        self.assertEqual((series['vintage_date_pages'], len(series['vintage_dates']), len(series['windows'])), (2, 10001, 11))
        self.assertEqual([query(r)['offset'] for r, _ in opener.requests if '/vintagedates?' in r.full_url], ['0', '10000'])
        self.assertEqual(series['estimated_production_cold_logical_requests'], 14)

    def test_restriction_recheck_prevents_vintage_and_observation_requests(self):
        client, result, opener, _ = fixture(lambda r: Response(200, {'seriess': [{'id': query(r)['series_id'],
            'title': MARKER, 'notes': 'Prior written\npermission is needed ' + MARKER}]}))
        self.assertEqual(P.run(client, result, lambda: None), 1)
        self.assertEqual(len(opener.requests), 8)
        self.assertTrue(all(row['failure']['code'] == 'restricted_now' for row in result['series'].values()))
        self.assertNotIn(MARKER, json.dumps(result))

    def test_production_page_guard_is_not_accepted_as_a_complete_estimate(self):
        client, result, _, _ = fixture(FakeFred(observation_count=10000001))
        self.assertEqual(P.run(client, result, lambda: None), 1)
        self.assertEqual(result['series']['GDP']['failure']['code'], 'production_page_guard')
        self.assertNotIn('estimated_production_cold_logical_requests', result['series']['GDP'])

    def test_unrecognized_400_and_non400_do_not_trigger_splitting(self):
        for status, message in [(400, MARKER), (500, 'maximum number of vintage dates ' + MARKER)]:
            client, result, opener, _ = fixture(lambda _: Response(status, {'error_message': message}))
            with self.assertRaises(P.ProfileError) as error:
                client.get('GDP', 'observations', {'limit': 1})
            self.assertEqual(error.exception.code, 'http_error')
            self.assertEqual(len(opener.requests), 1)
            self.assertNotIn(MARKER, json.dumps(result))
        client, result, _, _ = fixture(lambda _: Response(403, MARKER.encode()))
        with self.assertRaises(P.ProfileError) as error: client.get('GDP', 'metadata', {})
        self.assertEqual((error.exception.code, error.exception.status), ('http_error', 403))

    def test_single_vintage_limit_failure_is_unsplittable(self):
        fake = FakeFred(dates(1))
        def handler(r):
            return Response(400, {'error_message': 'maximum number of vintage dates'}) if '/observations?' in r.full_url else fake(r)
        client, result, _, _ = fixture(handler)
        self.assertEqual(P.run(client, result, lambda: None), 1)
        self.assertEqual(result['series']['GDP']['failure']['code'], 'vintage_limit_unsplittable')

    def test_invalid_numeric_counts_dates_shapes_fail_closed(self):
        for bad in [True, False, -1, 1.5, '1', None, 1000000001]:
            with self.subTest(bad=bad), self.assertRaises(P.ProfileError): P.count(bad)
        for bad in ['2020-1-01', '2020-02-30', '1776-07-03', MARKER, None]:
            with self.subTest(bad=bad), self.assertRaises(P.ProfileError): P.date(bad)
        for raw in [b'{"count":1,"count":2}', b'{"count":NaN}', b'[]', b'not-json', b'\xff']:
            with self.subTest(raw=raw), self.assertRaises(P.ProfileError): P.decode(raw)

    def test_wrong_observation_shape_does_not_supply_count_estimate(self):
        fake = FakeFred()
        for mutation in [lambda b:b.update(limit=True), lambda b:b.update(offset=1),
                         lambda b:b.update(observations=[]), lambda b:b.update(output_type=True)]:
            def handler(r):
                reply = fake(r)
                if '/observations?' in r.full_url:
                    body = json.loads(reply.getvalue()); mutation(body); return Response(200, body)
                return reply
            client, result, _, _ = fixture(handler)
            self.assertEqual(P.run(client, result, lambda: None), 1)
            self.assertFalse(result['series']['GDP']['complete'])

    def test_vintage_drift_duplicates_and_partial_pages_rejected(self):
        fake = FakeFred(dates(10001))
        for mutation in [lambda b:b.update(offset=2), lambda b:b.update(count=10002),
                         lambda b:b.update(vintage_dates=['2000-01-01'])]:
            def handler(r):
                reply = fake(r)
                if '/vintagedates?' in r.full_url and query(r)['offset'] == '10000':
                    body = json.loads(reply.getvalue()); mutation(body); return Response(200, body)
                return reply
            client, result, _, _ = fixture(handler)
            self.assertEqual(P.run(client, result, lambda: None), 1)
            self.assertFalse(result['series']['GDP']['vintages_complete'])


class BoundaryTests(unittest.TestCase):
    def test_request_rate_global_series_time_and_byte_caps(self):
        client, result, opener, clock = fixture()
        with patch.dict(P.LIMITS, http_attempts=1):
            client.get('GDP', 'metadata', {})
            with self.assertRaises(P.ProfileError) as error: client.get('GDPC1', 'metadata', {})
            self.assertEqual(error.exception.code, 'global_request_cap')
        client, result, _, clock = fixture()
        with patch.dict(P.LIMITS, http_attempts_per_series=1):
            client.get('GDP', 'metadata', {})
            with self.assertRaises(P.ProfileError) as error: client.get('GDP', 'metadata', {})
            self.assertEqual(error.exception.code, 'series_request_cap')
            client.get('GDPC1', 'metadata', {})
        clock.now = 900
        with self.assertRaises(P.ProfileError) as error: client.get('GDPC1', 'metadata', {})
        self.assertEqual(error.exception.code, 'time_cap')
        for limits, expected in [({'response_bytes':32}, 'response_byte_cap'),
                                 ({'total_response_bytes':32}, 'total_byte_cap')]:
            client, result, _, _ = fixture(lambda _: Response(200, b'x' * 100))
            with patch.dict(P.LIMITS, **limits), self.assertRaises(P.ProfileError) as error:
                client.get('GDP', 'metadata', {})
            self.assertEqual(error.exception.code, expected)
            self.assertEqual(result['response_bytes_read'], 32)

    def test_window_and_vintage_page_caps_leave_partial_status(self):
        fake = FakeFred(dates(10001), 1)
        for limits, expected in [({'vintage_pages_per_series':1}, 'vintage_page_cap'),
                                 ({'window_nodes_per_series':1}, 'window_node_cap')]:
            client, result, _, _ = fixture(fake)
            with patch.dict(P.LIMITS, **limits): self.assertEqual(P.run(client, result, lambda: None), 1)
            self.assertEqual(result['series']['GDP']['failure']['code'], expected)
            self.assertNotIn('estimated_production_cold_logical_requests', result['series']['GDP'])

    def test_redirects_and_transport_errors_never_export_urls(self):
        self.assertIsNone(P.NoRedirect().redirect_request(None, None, 302, MARKER, {}, MARKER))
        client, result, opener, _ = fixture(lambda _: Response(302, MARKER.encode()))
        with self.assertRaises(P.ProfileError) as error: client.get('GDP', 'metadata', {})
        self.assertEqual(error.exception.code, 'redirect_refused')
        self.assertEqual(len(opener.requests), 1)
        def fail(_): raise OSError(MARKER)
        client, result, _, _ = fixture(fail)
        self.assertEqual(P.run(client, result, lambda: None), 1)
        self.assertEqual(result['series']['GDP']['failure']['code'], 'transport_error')
        self.assertNotIn(MARKER, json.dumps(result))

    def test_missing_key_cli_preserves_safe_partial_artifact_and_actual_failure(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / 'out'
            process = subprocess.run([sys.executable, str(SCRIPT), '--output', str(output), '--revision', REVISION],
                env={'PATH': os.defpath}, capture_output=True)
            self.assertEqual(process.returncode, 1)
            self.assertEqual(process.stdout + process.stderr, b'')
            saved = json.loads((output / 'profile.json').read_bytes())
            self.assertFalse(saved['complete'])
            self.assertEqual(saved['failure']['code'], 'missing_key')
            self.assertEqual(saved['http_attempts'], 0)
            self.assertEqual((output / 'exit-status.txt').read_text(), '1\n')

    def test_workflow_serializes_key_use_and_preserves_actual_exit_without_log_artifacts(self):
        text = (SCRIPT.parents[1] / '.github/workflows/crawler-integration-test.yml').read_text()
        self.assertIn('group: fred-count-only-audit\n  cancel-in-progress: false', text)
        self.assertEqual(text.count('secrets.FRED_API_KEY'), 1)
        self.assertLess(text.index('Run offline profile tests without credentials'), text.index('secrets.FRED_API_KEY'))
        self.assertIn('persist-credentials: false', text)
        self.assertIn('contents: read', text)
        self.assertIn('timeout --signal=TERM --kill-after=5s 930s', text)
        self.assertIn('env -i PATH=', text)
        self.assertIn('status=$?', text)
        self.assertIn('exit "$status"', text)
        self.assertNotIn('stdout.txt', text)
        self.assertNotIn('stderr.txt', text)


if __name__ == '__main__': unittest.main()
