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
#   - the sign-in page must refuse a http://localhost redirect URI;
#   - before the gateway cut-over, ingress-nginx's 308s with its own page are the known
#     ECO-709 case: https on the alternate name with :443, and locally plain http to
#     https://MAIN_HOST with or without :443;
#   - MAIN_HOST and MAIN_HOST_PERMANENT come from the deploy config for the same base
#     domain when neither --main-host nor the environment gives them.
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
local = False
for arg in args:
    if arg == '-o': out = next(args)
    elif arg == '-D': headers = next(args)
    elif arg == '-H':
        header = next(args)
        if header.startswith('Origin: '): origin = header[8:]
        if header.startswith('X-Forwarded-Port:'): spoof = True
    elif arg == '-X': method = next(args)
    elif arg == '--connect-to': next(args); local = True
    elif arg in ('-w', '--max-time', '--data'): next(args)
    elif not arg.startswith('-'): url = arg
url = urlsplit(url)
main = os.environ['STUB_MAIN']
base = 'https://' + main
issuer = base + '/idp/realms/econ-graph'
scenario = os.environ.get('STUB_SCENARIO', '')
status, body, response = 200, '', {}
path = url.path + ('?' + url.query if url.query else '')
if url.scheme == 'http' and not local and scenario != 'cf-gateway-http':
    # Through Cloudflare, plain http never reaches the gateway: Always Use HTTPS
    # answers with a 301 to https on the same name.
    status = 301
    response['Location'] = 'https://' + url.hostname + path
    if scenario == 'cf-no-https': status, response = 200, {}
    if scenario == 'cf-other-host': response['Location'] = 'https://elsewhere.invalid' + path
elif url.scheme == 'http' or url.hostname != main:
    status = int(os.environ['STUB_REDIRECT_STATUS'])
    response['Location'] = base + path
    if scenario == 'query': response['Location'] = base + url.path
    if scenario == 'port': response['Location'] = 'https://' + main + ':443' + path
    if scenario == 'wrong-status': status = 302 if status == 301 else 301
    if scenario in ('nginx-http', 'nginx-http-443', 'nginx-http-page') and url.scheme == 'http' and local:
        # ingress-nginx's ssl redirect from plain http, before the gateway cut-over.
        status = 308
        response['Location'] = 'https://' + main + (':443' if scenario == 'nginx-http-443' else '') + path
        if scenario != 'nginx-http-page': body = '<html><center><h1>308 Permanent Redirect</h1></center><hr><center>nginx</center></html>'
    if scenario.startswith('nginx') and not scenario.startswith('nginx-http') and url.scheme == 'https':
        # ingress-nginx before the gateway cut-over: a 308 spelling out :443 (ECO-709).
        status = 308
        response['Location'] = 'https://' + main + ':443' + path
        if scenario == 'nginx': body = '<html><center><h1>308 Permanent Redirect</h1></center><hr><center>nginx</center></html>'
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
    # Local mode never sees Cloudflare's upgrade: the stub answers http there only as the
    # gateway does, so the passing runs above checked the gateway's direct redirect, and
    # the Cloudflare run checked the same-name 301 and then the https hop.
    via = run(via=True)
    assert via.returncode == 0 and 'redirects (301, Cloudflare) to https://www.econgraph.com, which serves the app' in via.stdout, via.stdout
    assert 'redirects (301, Cloudflare) to https://econgraph.com, then (302) to https://www.econgraph.com/series' in via.stdout, via.stdout
    via = run(main='econgraph.com', explicit=True, via=True)
    assert via.returncode == 0 and 'to https://www.econgraph.com, then (301) to https://econgraph.com/series' in via.stdout, via.stdout
    assert 'Cloudflare' not in run().stdout.replace('--via-cloudflare', '')
    result = run('cf-gateway-http', via=True)
    assert result.returncode == 0 and 'Cloudflare)' not in result.stdout, result.stdout
    for scenario in ['cf-no-https', 'cf-other-host']:
        result = run(scenario, via=True)
        assert result.returncode != 0 and 'FAIL  plain http://' in result.stdout, (scenario, result.stdout)
    for scenario in ['wrong-status', 'query']:
        result = run(scenario, via=True)
        assert result.returncode != 0, result.stdout
        assert 'FAIL  plain http://econgraph.com redirects (301, Cloudflare)' in result.stdout, (scenario, result.stdout)
    print('ok through Cloudflare: its same-name 301 to https, then the https hop to the main host')
    # ingress-nginx's 308 with :443, only with nginx's own page, is the known case.
    for via in (False, True):
        result = run('nginx', via=via)
        assert result.returncode == 0, result.stdout
        assert 'redirects (308) through ingress-nginx, with :443 (ECO-709' in result.stdout, result.stdout
        result = run('nginx-plain', via=via)
        assert result.returncode != 0 and 'econgraph.com redirects (308)' in result.stdout, result.stdout
    print('ok ingress-nginx 308 with :443 and its own page is the known ECO-709 case; without the page it fails')
    # Locally, before the cut-over, plain http answered by ingress-nginx's 308 to
    # https://MAIN_HOST (with or without :443) and its page is the same known case.
    for scenario, target in [('nginx-http', 'https://www.econgraph.com/series'),
                             ('nginx-http-443', 'https://www.econgraph.com:443/series')]:
        result = run(scenario)
        assert result.returncode == 0, result.stdout
        assert f'redirects (308) through ingress-nginx to {target}' in result.stdout, result.stdout
    result = run('nginx-http-page')
    assert result.returncode != 0 and 'FAIL  plain http://econgraph.com redirects (308)' in result.stdout, result.stdout
    print('ok locally, ingress-nginx 308 from plain http with its page is the known ECO-709 case; without the page it fails')
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
    # MAIN_HOST too: the deploy config's for the same base domain, so the bare command checks
    # the served host after a switch to the apex; the environment and --main-host win.
    deploy_env.write_text('PUBLIC_HOST=econgraph.com\nMAIN_HOST=econgraph.com\n')
    bare = lambda **extra: subprocess.run([str(check), '--host', 'econgraph.com'] + extra.pop('args', []),
                                         env=dict(env, STUB_SCENARIO='', **extra), text=True,
                                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    result = bare(STUB_MAIN='econgraph.com', STUB_REDIRECT_STATUS='301')
    assert result.returncode == 0 and 'redirects (301) to https://econgraph.com/series' in result.stdout, result.stdout
    result = bare(STUB_MAIN='www.econgraph.com', STUB_REDIRECT_STATUS='302', MAIN_HOST='www.econgraph.com')
    assert result.returncode == 0, result.stdout
    result = bare(STUB_MAIN='www.econgraph.com', STUB_REDIRECT_STATUS='302', args=['--main-host', 'www.econgraph.com'])
    assert result.returncode == 0, result.stdout
    deploy_env.write_text('PUBLIC_HOST=other.example\nMAIN_HOST=econgraph.com\n')
    result = bare(STUB_MAIN='www.econgraph.com', STUB_REDIRECT_STATUS='302')
    assert result.returncode == 0 and 'to https://www.econgraph.com/series' in result.stdout, result.stdout
    deploy_env.write_text('PUBLIC_HOST=econgraph.com\nMAIN_HOST=elsewhere.example\n')
    result = bare(STUB_MAIN='www.econgraph.com', STUB_REDIRECT_STATUS='302')
    assert result.returncode == 2 and 'must be econgraph.com or www.econgraph.com, not elsewhere.example' in result.stdout, result.stdout
    deploy_env.unlink()
    print('ok MAIN_HOST and MAIN_HOST_PERMANENT from the deploy config for the same base domain; the environment and --main-host win')
    result = run(main='other.invalid', explicit=True)
    assert result.returncode == 2 and 'must be econgraph.com or www.econgraph.com, not other.invalid' in result.stdout, result.stdout
    print('ok unrelated main host rejected before curl')
print('public site checks: all passed')
PY
