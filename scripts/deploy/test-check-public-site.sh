#!/usr/bin/env bash
# Offline smoke-check tests: every curl request is answered by a stub.
# Includes canonical-host rollback and rejects redirect loops, spoofed redirects,
# dropped queries, stale issuers, and public/refused-origin CORS regressions.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
python3 - "$ROOT" <<'PY'
import os
from pathlib import Path
import subprocess
import sys
import tempfile

root = Path(sys.argv[1])
with tempfile.TemporaryDirectory() as tmp:
    work = Path(tmp)
    stub = work / 'curl'
    stub.write_text('''#!/usr/bin/env python3
import json, os, sys
from urllib.parse import urlsplit
args = iter(sys.argv[1:])
out = headers = None
origin = ''
method = 'GET'
spoof = False
for arg in args:
    if arg == '-o': out = next(args)
    elif arg == '-D': headers = next(args)
    elif arg == '-H':
        header = next(args)
        if header.startswith('Origin: '): origin = header[8:]
        if header.startswith('X-Forwarded-Port:'): spoof = True
    elif arg == '-X': method = next(args)
    elif arg in ('-w', '--max-time', '--connect-to', '--data'): next(args)
    elif not arg.startswith('-'): url = arg
url = urlsplit(url)
main = os.environ['STUB_MAIN']
base = 'https://' + main
issuer = base + '/idp/realms/econ-graph'
scenario = os.environ.get('STUB_SCENARIO', '')
status, body, response = 200, '', {}
path = url.path + ('?' + url.query if url.query else '')
if url.scheme == 'http' or url.hostname != main:
    status = 301
    response['Location'] = base + path
    if scenario == 'query': response['Location'] = base + url.path
    if scenario == 'port': response['Location'] = 'https://' + main + ':443' + path
    if scenario == 'temporary': status = 302
    if scenario == 'spoof' and spoof: response['Location'] = 'https://spoofed.invalid' + path
elif path == '/' or path.startswith('/series/smoke-check'):
    body = '<html><div id="root"></div></html>'
    if scenario == 'loop':
        status = 301
        response['Location'] = 'https://econgraph.com/'
elif method == 'OPTIONS':
    if origin in ('https://econgraph.com', 'https://www.econgraph.com'):
        if not (scenario == 'missing-apex-cors' and origin == 'https://econgraph.com'):
            response['Access-Control-Allow-Origin'] = origin
        if scenario == 'failed-cors': status = 403
    elif scenario == 'refused-cors': response['Access-Control-Allow-Origin'] = origin
elif path == '/graphql': body = json.dumps({'data': {'searchSeries': {'totalCount': 1}}})
elif '/.well-known/' in path and '/realms/econ-graph/' in path and '..' not in path and '%' not in path:
    body = json.dumps({'issuer': issuer, 'authorization_endpoint': issuer + '/protocol/openid-connect/auth'})
    if scenario == 'issuer': body = json.dumps({'issuer': 'https://old.invalid/idp/realms/econ-graph'})
elif '/protocol/openid-connect/auth?' in path and 'client_id=econ-graph-web' in path:
    if 'redirect_uri=http%3A%2F%2Flocalhost' in path:
        status, body = 400, 'Invalid parameter: redirect_uri'
        if scenario == 'localhost-login': status, body = 200, '<form id="kc-form-login">'
    else: body = '<form id="kc-form-login" action="' + base + '/idp/login">'
else: status = 404
if out: open(out, 'w').write(body)
if headers: open(headers, 'w').write('HTTP/1.1 ' + str(status) + '\\r\\n' + ''.join(k + ': ' + v + '\\r\\n' for k,v in response.items()) + '\\r\\n')
print(status, end='')
''')
    stub.chmod(0o755)
    env = dict(os.environ, PATH=str(work) + os.pathsep + os.environ['PATH'])
    env.pop('MAIN_HOST', None)
    check = root / 'scripts/deploy/check-public-site.sh'
    def run(scenario='', main='www.econgraph.com', explicit=False, via=False):
        args = [str(check), '--host', 'econgraph.com']
        if explicit: args += ['--main-host', main]
        if via: args += ['--via-cloudflare']
        return subprocess.run(args, env=dict(env, STUB_MAIN=main, STUB_SCENARIO=scenario),
                              text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    for main, explicit, via in [('www.econgraph.com', False, False), ('www.econgraph.com', True, True),
                                ('econgraph.com', True, False)]:
        result = run(main=main, explicit=explicit, via=via)
        assert result.returncode == 0, result.stdout
        print('ok canonical matrix, issuer, login and CORS:', main, 'Cloudflare' if via else 'local')
    for scenario in ['query', 'port', 'temporary', 'spoof', 'loop', 'issuer', 'missing-apex-cors',
                     'failed-cors', 'refused-cors', 'localhost-login']:
        result = run(scenario)
        assert result.returncode != 0 and 'FAIL' in result.stdout, (scenario, result.stdout)
        print('ok regression refused:', scenario)
    result = run(main='other.invalid', explicit=True)
    assert result.returncode == 2 and '--main-host must be' in result.stdout, result.stdout
    print('ok unrelated main host rejected before curl')
print('public site checks: all passed')
PY
