"""Helpers for the Python QA scenarios in scripts/qa/scenarios/ (standard library only).

A scenario records each check with `check()`, then calls `finish()`, which exits 0 when every
check passed and 1 otherwise; `skip()` exits 77 with a reason. The runner
(scripts/qa/run_scenarios.py) sets QA_TARGET, QA_PROFILE, QA_EVIDENCE_FILE and QA_WORK_DIR, and
redacts secrets from what a scenario writes; scenarios still never print a password or token.

When the runner stops a scenario that overran its qa-timeout, it sends SIGTERM to the
scenario's process group. Importing this module turns that into SystemExit, after stopping every
command started with `run()`, so a scenario's `finally` blocks still clean up.
"""

from __future__ import annotations

import base64
import hashlib
import html.parser
import http.cookiejar
import json
import os
import secrets
import signal
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

EXIT_SKIP = 77

TARGET = os.environ.get('QA_TARGET', '').rstrip('/')
PROFILE = os.environ.get('QA_PROFILE', '')
WORK_DIR = Path(os.environ.get('QA_WORK_DIR') or '.')
REALM = 'econ-graph'
WEB_CLIENT_ID = 'econ-graph-web'

_failures = 0

# Process groups of the commands run() has running.
_children: set[int] = set()


def _signal_group(pgid: int, sig: int) -> None:
    try:
        os.killpg(pgid, sig)
    except (ProcessLookupError, PermissionError):
        pass


# When the runner asked this scenario to stop, and how long it allows for cleanup after that.
STOP_GRACE = float(os.environ.get('QA_TIMEOUT_GRACE_SECONDS') or 60)
stopped_at: float | None = None


def cleanup_deadline(normal: float) -> float:
    """A time.monotonic() deadline for a wait during cleanup: `normal` seconds from now, or,
    once the runner has asked the scenario to stop, half its grace period from then, leaving
    the rest for the cleanup itself."""
    if stopped_at is not None:
        return min(time.monotonic() + normal, stopped_at + STOP_GRACE / 2)
    return time.monotonic() + normal


def _on_sigterm(signum, frame):
    global stopped_at
    if stopped_at is not None:
        return
    stopped_at = time.monotonic()
    for pgid in list(_children):
        _signal_group(pgid, signal.SIGTERM)
    raise SystemExit(f'stopped by signal {signum}')


signal.signal(signal.SIGTERM, _on_sigterm)


def run(command: list[str], timeout: float, env: dict | None = None,
        capture: bool = True) -> subprocess.CompletedProcess:
    """Run a command in its own process group, killing the whole group if it overruns.

    Raises subprocess.TimeoutExpired after `timeout` seconds, once the group is gone. Output
    is captured as text unless `capture` is false, when it goes to this scenario's output.
    """
    pipe = subprocess.PIPE if capture else None
    proc = subprocess.Popen(command, env=env, stdin=subprocess.DEVNULL, stdout=pipe, stderr=pipe,
                            text=True, start_new_session=True)
    _children.add(proc.pid)
    try:
        out, err = proc.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        _signal_group(proc.pid, signal.SIGKILL)
        proc.communicate()
        raise
    finally:
        _children.discard(proc.pid)
        _signal_group(proc.pid, signal.SIGKILL)
    return subprocess.CompletedProcess(command, proc.returncode, out, err)


def _record(entry: dict) -> None:
    path = os.environ.get('QA_EVIDENCE_FILE')
    line = json.dumps(entry, default=str)
    if path:
        with open(path, 'a', encoding='utf-8') as handle:
            handle.write(line + '\n')


def check(name: str, ok: bool, detail: str = '', **data) -> bool:
    """Record one check's outcome, with evidence, and print it."""
    global _failures
    if not ok:
        _failures += 1
    entry = {'check': name, 'ok': bool(ok), 'detail': detail}
    if data:
        entry['data'] = data
    _record(entry)
    print(f'  {"ok  " if ok else "FAIL"}  {name}{": " + detail if detail else ""}', flush=True)
    return bool(ok)


def skip_check(name: str, reason: str) -> None:
    """Record a check that was not run, and why: neither a pass nor a failure."""
    _record({'check': name, 'ok': None, 'skipped': True, 'detail': reason})
    print(f'  skip  {name}: {reason}', flush=True)


def skip(reason: str) -> None:
    """End the scenario as skipped."""
    _record({'skip': reason})
    print(f'  skip  {reason}', flush=True)
    sys.exit(EXIT_SKIP)


def fail(name: str, detail: str) -> None:
    """Record a failed check and end the scenario now."""
    check(name, False, detail)
    sys.exit(1)


def finish() -> None:
    sys.exit(1 if _failures else 0)


def secret(name: str) -> str:
    """The value of `name`, else the content of the file `name_FILE` names, else ''."""
    value = os.environ.get(name, '')
    if value:
        return value
    path = os.environ.get(f'{name}_FILE', '')
    if path:
        try:
            return Path(path).read_text(encoding='utf-8').strip()
        except OSError as error:
            fail('configuration', f'{name}_FILE could not be read: {error.strerror}')
    return ''


def url(path: str) -> str:
    """`path` on the target, unless it is already absolute."""
    return path if '://' in path else f'{TARGET}{path}'


class Response:
    def __init__(self, status: int, headers, body: bytes, final_url: str):
        self.status = status
        self.headers = headers
        self.body = body
        self.url = final_url

    @property
    def text(self) -> str:
        return self.body.decode('utf-8', errors='replace')

    def json(self):
        return json.loads(self.body)

    def is_json(self) -> bool:
        return 'json' in (self.headers.get('Content-Type') or '')


class _NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None


class Http:
    """A small HTTP client with an optional cookie jar; `follow_redirects=False` returns 3xx as is."""

    def __init__(self, follow_redirects: bool = True, jar: http.cookiejar.CookieJar | None = None,
                 timeout: float = 30):
        handlers = []
        if jar is not None:
            handlers.append(urllib.request.HTTPCookieProcessor(jar))
        if not follow_redirects:
            handlers.append(_NoRedirect())
        self.opener = urllib.request.build_opener(*handlers)
        self.timeout = timeout

    def request(self, method: str, target: str, body=None, headers=None) -> Response:
        data = None
        headers = dict(headers or {})
        if isinstance(body, (dict, list)) and headers.get('Content-Type') != 'application/x-www-form-urlencoded':
            data = json.dumps(body).encode()
            headers.setdefault('Content-Type', 'application/json')
        elif isinstance(body, dict):
            data = urllib.parse.urlencode(body).encode()
        elif isinstance(body, (bytes, str)):
            data = body.encode() if isinstance(body, str) else body
        headers.setdefault('User-Agent', 'econgraph-qa-scenarios')
        req = urllib.request.Request(url(target), data=data, headers=headers, method=method)
        try:
            with self.opener.open(req, timeout=self.timeout) as resp:
                return Response(resp.status, resp.headers, resp.read(), resp.url)
        except urllib.error.HTTPError as error:
            return Response(error.code, error.headers, error.read(), error.url)

    def get(self, target: str, headers=None) -> Response:
        return self.request('GET', target, headers=headers)

    def post_form(self, target: str, fields: dict, headers=None) -> Response:
        headers = dict(headers or {})
        headers['Content-Type'] = 'application/x-www-form-urlencoded'
        return self.request('POST', target, body=fields, headers=headers)


class GraphQLError(Exception):
    pass


def graphql(path: str, query: str, variables: dict | None = None, token: str | None = None,
            client: Http | None = None) -> tuple[int, dict]:
    """POST a GraphQL request; its HTTP status and decoded body (`{}` when not JSON)."""
    headers = {'Accept': 'application/json'}
    if token:
        headers['Authorization'] = f'Bearer {token}'
    response = (client or Http()).request(
        'POST', path, body={'query': query, 'variables': variables or {}}, headers=headers
    )
    try:
        body = response.json()
    except (ValueError, UnicodeDecodeError):
        body = {}
    return response.status, body if isinstance(body, dict) else {}


def data_or_raise(status: int, body: dict, what: str) -> dict:
    """The `data` of a successful GraphQL response, else GraphQLError naming `what`."""
    if status != 200 or body.get('errors') or not isinstance(body.get('data'), dict):
        messages = '; '.join(e.get('message', '?') for e in body.get('errors') or [])
        raise GraphQLError(f'{what}: HTTP {status}' + (f', {messages}' if messages else ''))
    return body['data']


def error_codes(body: dict) -> list[str]:
    return [
        str((e.get('extensions') or {}).get('code', ''))
        for e in body.get('errors') or []
    ]


# --- Sign-in -------------------------------------------------------------------------------


def issuer() -> str:
    """The realm's issuer: QA_OIDC_ISSUER, else Keycloak under /idp on the target."""
    return os.environ.get('QA_OIDC_ISSUER', '').rstrip('/') or f'{TARGET}/idp/realms/{REALM}'


def account() -> tuple[str, str]:
    """The QA account from QA_USERNAME and QA_PASSWORD (or QA_PASSWORD_FILE); skips without."""
    username = os.environ.get('QA_USERNAME', '').strip()
    password = secret('QA_PASSWORD')
    if not username or not password:
        skip('no QA account: set QA_USERNAME and QA_PASSWORD or QA_PASSWORD_FILE')
    return username, password


class _LoginForm(html.parser.HTMLParser):
    """Finds the form that has a password field: its action and hidden inputs."""

    def __init__(self):
        super().__init__()
        self.forms: list[dict] = []
        self._current: dict | None = None

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == 'form':
            self._current = {'action': attrs.get('action') or '', 'fields': {}, 'password': False}
            self.forms.append(self._current)
        elif tag == 'input' and self._current is not None:
            name = attrs.get('name')
            if attrs.get('type') == 'password' or name == 'password':
                self._current['password'] = True
            if name and attrs.get('type') == 'hidden':
                self._current['fields'][name] = attrs.get('value') or ''

    def handle_endtag(self, tag):
        if tag == 'form':
            self._current = None


class SignInError(Exception):
    pass


class _BrowserCookies(http.cookiejar.DefaultCookiePolicy):
    """Sends Secure cookies to http://localhost too, as browsers do (a local stack's Keycloak
    marks its session cookies Secure)."""

    def return_ok_secure(self, cookie, request):
        host = urllib.parse.urlsplit(request.get_full_url()).hostname
        return host in ('localhost', '127.0.0.1', '::1') or super().return_ok_secure(cookie, request)


def sign_in(username: str, password: str, redirect_uri: str | None = None) -> str:
    """An access token for the account, through the web client's sign-in form.

    Authorization code with PKCE, as the browser app signs in, so it works where the password
    grant is off (every deployed realm). The final redirect to the app is not followed.
    """
    redirect_uri = redirect_uri or os.environ.get('QA_OIDC_REDIRECT_URI') or f'{TARGET}/auth/callback'
    client_id = os.environ.get('QA_OIDC_CLIENT_ID') or WEB_CLIENT_ID
    discovery = Http().get(f'{issuer()}/.well-known/openid-configuration')
    if discovery.status != 200:
        raise SignInError(f'discovery at {issuer()} answered {discovery.status}')
    config = discovery.json()

    verifier = secrets.token_urlsafe(48)
    challenge = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).rstrip(b'=').decode()
    state = secrets.token_urlsafe(16)
    query = urllib.parse.urlencode({
        'client_id': client_id,
        'redirect_uri': redirect_uri,
        'response_type': 'code',
        'scope': 'openid',
        'state': state,
        'code_challenge': challenge,
        'code_challenge_method': 'S256',
    })
    jar = http.cookiejar.CookieJar(policy=_BrowserCookies())
    browser = Http(jar=jar)
    page = browser.get(f'{config["authorization_endpoint"]}?{query}')
    if page.status != 200:
        raise SignInError(f'the sign-in page answered {page.status}')
    parser = _LoginForm()
    parser.feed(page.text)
    form = next((f for f in parser.forms if f['password']), None)
    if form is None:
        raise SignInError('the sign-in page has no password form')

    submit = Http(follow_redirects=False, jar=jar)
    action = urllib.parse.urljoin(page.url, form['action'])
    fields = dict(form['fields'])
    fields.update(username=username, password=password)
    answer = submit.post_form(action, fields)
    location = answer.headers.get('Location') or ''
    if answer.status not in (302, 303) or not location.startswith(redirect_uri):
        if answer.status == 200:
            raise SignInError('Keycloak did not accept the QA account (wrong password, locked, '
                              'or a required action is pending)')
        raise SignInError(f'sign-in answered {answer.status} without redirecting to the app')
    params = urllib.parse.parse_qs(urllib.parse.urlsplit(location).query)
    if params.get('state', [''])[0] != state or 'code' not in params:
        raise SignInError(f'the redirect carried no code for this sign-in ({", ".join(sorted(params))})')

    token = Http().post_form(config['token_endpoint'], {
        'grant_type': 'authorization_code',
        'client_id': client_id,
        'code': params['code'][0],
        'redirect_uri': redirect_uri,
        'code_verifier': verifier,
    })
    if token.status != 200:
        raise SignInError(f'the token endpoint answered {token.status}')
    access = token.json().get('access_token')
    if not access:
        raise SignInError('the token response has no access_token')
    return access


def signed_in_token() -> str:
    """Sign in as the QA account, or skip without one; a failed sign-in fails the scenario."""
    username, password = account()
    try:
        token = sign_in(username, password)
    except (SignInError, OSError, ValueError, KeyError) as error:
        fail('sign in', f'{username}: {error}')
    check('sign in', True, f'{username} signed in through {issuer()}')
    return token
