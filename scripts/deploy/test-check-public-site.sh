#!/usr/bin/env bash
# Offline tests for scripts/deploy/check-public-site.sh: a stub curl first on PATH
# answers every request, so nothing reaches the network.
#   - www and apex main hosts, locally and through Cloudflare, pass; the redirects to
#     www are temporary (302) until MAIN_HOST_PERMANENT names it, then permanent (301),
#     and the redirects to the apex are permanent;
#   - the wrong redirect status either way, a dropped query, a port, a spoofed
#     redirect, a redirect loop and a stale issuer fail;
#   - CORS: both public origins must be allowed; http://localhost:3000 and
#     https://evil.example must get the backend's 403 with no Access-Control-Allow-Origin,
#     so an allowed refused origin, a 2xx without the header, and a 502 all fail;
#   - the sign-in page must refuse a http://localhost redirect URI.
#
# Usage: scripts/deploy/test-check-public-site.sh
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
    status = int(os.environ['STUB_REDIRECT_STATUS'])
    response['Location'] = base + path
    if scenario == 'query': response['Location'] = base + url.path
    if scenario == 'port': response['Location'] = 'https://' + main + ':443' + path
    if scenario == 'wrong-status': status = 302 if status == 301 else 301
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
    elif scenario == 'refused-cors' or (scenario == 'cors-old' and origin == 'http://localhost:3000'):
        status = 204
        response['Access-Control-Allow-Origin'] = origin
    elif scenario == 'cors-down': status = 502
    elif scenario == 'cors-no-403': status = 204
    else: status = 403
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
    env.pop('MAIN_HOST_PERMANENT', None)
    # The deploy config is a scratch file, never the operator's.
    deploy_env = work / 'deploy.env'
    env['ECONGRAPH_DEPLOY_ENV'] = str(deploy_env)
    check = root / 'scripts/deploy/check-public-site.sh'
    def run(scenario='', main='www.econgraph.com', explicit=False, via=False, permanent=None):
        args = [str(check), '--host', 'econgraph.com']
        if explicit: args += ['--main-host', main]
        if via: args += ['--via-cloudflare']
        code = '301' if main == 'econgraph.com' or permanent == main else '302'
        extra = {'MAIN_HOST_PERMANENT': permanent} if permanent else {}
        return subprocess.run(args, env=dict(env, STUB_MAIN=main, STUB_SCENARIO=scenario,
                                             STUB_REDIRECT_STATUS=code, **extra),
                              text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    for main, explicit, via, permanent in [
            ('www.econgraph.com', False, False, None), ('www.econgraph.com', True, True, None),
            ('www.econgraph.com', True, False, 'www.econgraph.com'), ('econgraph.com', True, False, None),
            ('econgraph.com', True, False, 'www.econgraph.com')]:
        result = run(main=main, explicit=explicit, via=via, permanent=permanent)
        assert result.returncode == 0, result.stdout
        code = '301' if main == 'econgraph.com' or permanent == main else '302'
        assert f'redirects ({code}) to https://{main}/series/smoke-check?from=alternate' in result.stdout, result.stdout
        assert 'refused (403)' in result.stdout, result.stdout
        print('ok canonical matrix, issuer, login and CORS:', main, 'Cloudflare' if via else 'local',
              'permanent' if code == '301' else 'temporary')
    for scenario in ['query', 'port', 'wrong-status', 'spoof', 'loop', 'issuer', 'missing-apex-cors',
                     'failed-cors', 'refused-cors', 'cors-old', 'cors-down', 'cors-no-403', 'localhost-login']:
        result = run(scenario)
        assert result.returncode != 0 and 'FAIL' in result.stdout, (scenario, result.stdout)
        print('ok regression refused:', scenario)
    for scenario, expected in [('cors-old', "CORS preflight from http://localhost:3000 returned 204 with Access-Control-Allow-Origin 'http://localhost:3000'"),
                               ('cors-down', "CORS preflight from http://localhost:3000 returned 502, expected the backend's 403"),
                               ('cors-no-403', "CORS preflight from https://evil.example returned 204, expected the backend's 403")]:
        assert expected in run(scenario).stdout, scenario
    print('ok refused origins need the backend 403 without an allowed origin')
    for permanent in (None, 'www.econgraph.com'):
        result = run('wrong-status', permanent=permanent)
        assert 'expected a ' + ('301' if permanent else '302') in result.stdout, result.stdout
    print('ok a premature 301, or a 302 after finalize, fails')
    # Without MAIN_HOST_PERMANENT in the environment, the deploy config's is used, for
    # the same base domain only; the environment, even empty, wins.
    deploy_env.write_text('PUBLIC_HOST=econgraph.com\nMAIN_HOST_PERMANENT=www.econgraph.com\n')
    result = subprocess.run([str(check), '--host', 'econgraph.com'], env=dict(env, STUB_MAIN='www.econgraph.com',
                            STUB_SCENARIO='', STUB_REDIRECT_STATUS='301'), text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    assert result.returncode == 0, result.stdout
    result = subprocess.run([str(check), '--host', 'econgraph.com'], env=dict(env, STUB_MAIN='www.econgraph.com',
                            STUB_SCENARIO='', STUB_REDIRECT_STATUS='302', MAIN_HOST_PERMANENT=''), text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    assert result.returncode == 0, result.stdout
    deploy_env.write_text('PUBLIC_HOST=other.example\nMAIN_HOST_PERMANENT=www.econgraph.com\n')
    result = subprocess.run([str(check), '--host', 'econgraph.com'], env=dict(env, STUB_MAIN='www.econgraph.com',
                            STUB_SCENARIO='', STUB_REDIRECT_STATUS='302'), text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    assert result.returncode == 0, result.stdout
    deploy_env.unlink()
    print('ok MAIN_HOST_PERMANENT from the deploy config for the same base domain; the environment wins')
    result = run(main='other.invalid', explicit=True)
    assert result.returncode == 2 and '--main-host must be' in result.stdout, result.stdout
    print('ok unrelated main host rejected before curl')
print('public site checks: all passed')
PY
