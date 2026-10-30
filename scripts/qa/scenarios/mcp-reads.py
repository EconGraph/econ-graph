#!/usr/bin/env python3
# qa-scenario: mcp-reads
# qa-mutates: no
# qa-profiles: lab, ci, production
# qa-timeout: 120
# qa-description: As the QA account, list the MCP tools and read a series through them: search, metadata, data points.
"""MCP data reads.

Inputs: QA_TARGET; QA_MCP_URL (default TARGET/mcp); the QA account (QA_USERNAME, QA_PASSWORD
or QA_PASSWORD_FILE, and the sign-in settings of annotation-round-trip.py); QA_SEARCH_QUERY
(default "GDP").

Skipped when nothing answers MCP at QA_MCP_URL: release builds compile /mcp out (the `mcp`
build flag, config/flags/README.md), and the gateway routes no /mcp path. A refused token or a
JSON-RPC error fails. Calls only the read tools.
"""

import json
import os

from qa import Http, check, fail, finish, signed_in_token, skip, url

QUERY = os.environ.get('QA_SEARCH_QUERY', 'GDP')
ENDPOINT = os.environ.get('QA_MCP_URL') or url('/mcp')
READ_TOOLS = {'search_economic_series', 'get_series_metadata', 'get_series_data'}

client = Http()
next_id = 0


def rpc(method, params=None, token=None):
    """POST one JSON-RPC request; the HTTP response."""
    global next_id
    next_id += 1
    headers = {'Accept': 'application/json'}
    if token:
        headers['Authorization'] = f'Bearer {token}'
    return client.request('POST', ENDPOINT, body={
        'jsonrpc': '2.0', 'id': next_id, 'method': method, 'params': params or {},
    }, headers=headers)


def result(response, what):
    if response.status != 200 or not response.is_json():
        fail(what, f'HTTP {response.status} ({response.headers.get("Content-Type")})')
    body = response.json()
    if body.get('error'):
        fail(what, f'JSON-RPC error {body["error"].get("code")}: {body["error"].get("message")}')
    return body.get('result')


def text_payload(tool_result):
    """The JSON a tool returned, from its MCP `content` text, or the result itself."""
    if isinstance(tool_result, dict) and isinstance(tool_result.get('content'), list):
        for item in tool_result['content']:
            if item.get('type') == 'text':
                try:
                    return json.loads(item['text'])
                except (ValueError, TypeError):
                    return item['text']
    return tool_result


def dig(value, *keys):
    """value[k1][k2]..., or None where a level is missing."""
    for key in keys:
        if not isinstance(value, dict):
            return None
        value = value.get(key)
    return value


# Without a token first: an endpoint that answers is one that is served.
probe = rpc('tools/list')
# Absent: 404/405, or the frontend's HTML fallback answering 200. Any other non-JSON answer
# (a 502 page, a bare 401) is a served endpoint misbehaving, and fails below.
if probe.status in (404, 405) or (200 <= probe.status < 300 and not probe.is_json()):
    skip(f'no MCP endpoint at {ENDPOINT} (HTTP {probe.status}); release builds compile /mcp out')
check('refuses an anonymous caller', probe.status == 401 and probe.is_json(),
      f'HTTP {probe.status} ({probe.headers.get("Content-Type")}) without a token')

token = signed_in_token()

tools = result(rpc('tools/list', token=token), 'tools/list')
names = {t.get('name') for t in (tools or {}).get('tools', [])}
check('tools/list', READ_TOOLS <= names, f'{len(names)} tools; read tools present: {sorted(READ_TOOLS & names)}')

found = text_payload(result(rpc('tools/call', {
    'name': 'search_economic_series', 'arguments': {'query': QUERY, 'limit': 3},
}, token), 'search_economic_series'))
series = dig(found, 'data', 'searchSeries', 'series') or []
check('search_economic_series', bool(series), f'"{QUERY}" returned {len(series)} series')
if not series:
    finish()

series_id = series[0].get('id')
meta = text_payload(result(rpc('tools/call', {
    'name': 'get_series_metadata', 'arguments': {'series_id': series_id},
}, token), 'get_series_metadata'))
meta = dig(meta, 'data', 'series') or {}
check('get_series_metadata', meta.get('id') == series_id, f'metadata for {meta.get("name")}',
      series_id=series_id)

points = text_payload(result(rpc('tools/call', {
    'name': 'get_series_data', 'arguments': {'series_id': series_id, 'limit': 10},
}, token), 'get_series_data'))
rows = dig(points, 'data', 'series', 'dataPoints') or []
check('get_series_data', bool(rows), f'{len(rows)} data points for {series_id}')

finish()
