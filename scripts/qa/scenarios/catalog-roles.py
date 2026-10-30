#!/usr/bin/env python3
# qa-scenario: catalog-roles
# qa-mutates: yes
# qa-profiles: lab, ci
# qa-timeout: 1800
# qa-description: Withdraw one catalog from anonymous visitors in Keycloak, check it takes effect, survives a redeploy, then restore it.
"""Catalog roles for anonymous visitors (config/keycloak/README.md, "Catalog access").

Inputs:
  QA_TARGET                    the site; anonymous reads go to its /graphql.
  QA_KEYCLOAK_URL              Keycloak's base URL, admin API included (default: the issuer
                               without /realms/econ-graph). The public route serves no admin
                               API, so a lab gives its port-forward here.
  QA_KEYCLOAK_ADMIN_USERNAME,  a master-realm admin; skipped without.
  QA_KEYCLOAK_ADMIN_PASSWORD
    (or _PASSWORD_FILE)
  QA_CATALOG_ROLE              the role to withdraw (default catalog.bls:read, a public catalog).
  QA_CATALOG_SOURCE            text in that catalog's data source name (default BLS).
  QA_ANONYMOUS_CLIENT_ID       the anonymous reader's client (default econgraph-anonymous).
  QA_CATALOG_WAIT_SECONDS      how long a change may take to reach the backend (default 420:
                               the backend caches anonymous grants for up to five minutes).
  QA_REDEPLOY_COMMAND          run with bash after the restriction is verified, e.g. the lab's
                               deploy.sh; the restriction must survive it. Without it the
                               redeploy and persistence checks are skipped: the scenario passes with
                               "skipped checks: redeploy" as its reason.
  QA_REDEPLOY_TIMEOUT_SECONDS  how long the redeploy may run (default 600).

Skipped when the realm has no catalog roles (before ECO-657). Whatever the outcome, the
anonymous reader's original role mappings are put back.
"""

import os
import subprocess
import time
import urllib.parse

from qa import (Http, check, data_or_raise, error_codes, fail, finish, graphql, issuer, secret,
                skip, GraphQLError)
from qa import cleanup_deadline, skip_check
from qa import run as run_bounded

REALM = 'econ-graph'
ROLE = os.environ.get('QA_CATALOG_ROLE', 'catalog.bls:read')
SOURCE_MATCH = os.environ.get('QA_CATALOG_SOURCE', 'BLS')
READER_CLIENT = os.environ.get('QA_ANONYMOUS_CLIENT_ID', 'econgraph-anonymous')
WAIT = float(os.environ.get('QA_CATALOG_WAIT_SECONDS', '420'))
POLL = float(os.environ.get('QA_CATALOG_POLL_SECONDS', '10'))
REDEPLOY = os.environ.get('QA_REDEPLOY_COMMAND', '')
REDEPLOY_TIMEOUT = float(os.environ.get('QA_REDEPLOY_TIMEOUT_SECONDS', '600'))
KEYCLOAK = (os.environ.get('QA_KEYCLOAK_URL') or issuer().rsplit(f'/realms/{REALM}', 1)[0]).rstrip('/')
ADMIN_USER = os.environ.get('QA_KEYCLOAK_ADMIN_USERNAME', '')
ADMIN_PASSWORD = secret('QA_KEYCLOAK_ADMIN_PASSWORD')
PUBLIC_COMPOSITE = 'catalog-public'

if not ADMIN_USER or not ADMIN_PASSWORD:
    skip('no Keycloak admin: set QA_KEYCLOAK_ADMIN_USERNAME and QA_KEYCLOAK_ADMIN_PASSWORD or '
         'QA_KEYCLOAK_ADMIN_PASSWORD_FILE')


class Admin:
    """The Keycloak admin API for the econ-graph realm."""

    def __init__(self):
        self.http = Http()
        self.token = None

    def login(self):
        answer = self.http.post_form(f'{KEYCLOAK}/realms/master/protocol/openid-connect/token', {
            'grant_type': 'password', 'client_id': 'admin-cli',
            'username': ADMIN_USER, 'password': ADMIN_PASSWORD,
        })
        if answer.status != 200:
            raise RuntimeError(f'admin sign-in at {KEYCLOAK} answered {answer.status}')
        self.token = answer.json()['access_token']

    def call(self, method, path, body=None):
        if self.token is None:
            self.login()
        answer = self.http.request(method, f'{KEYCLOAK}/admin/realms/{REALM}{path}', body=body,
                                   headers={'Authorization': f'Bearer {self.token}'})
        if answer.status == 401:
            self.login()
            answer = self.http.request(method, f'{KEYCLOAK}/admin/realms/{REALM}{path}', body=body,
                                       headers={'Authorization': f'Bearer {self.token}'})
        if answer.status >= 300:
            raise RuntimeError(f'{method} {path} answered {answer.status}')
        return answer.json() if answer.body else None

    def client(self, client_id):
        found = self.call('GET', f'/clients?clientId={urllib.parse.quote(client_id)}')
        return found[0] if found else None


def reachable_admin(admin, deadline):
    """Wait for the admin API (Keycloak may be restarting after a redeploy), never past
    `deadline`: each attempt's HTTP timeout and each pause fit in the time left."""
    while True:
        left = deadline - time.monotonic()
        admin.http.timeout = max(1.0, min(30.0, left))
        try:
            admin.login()
            return
        except (RuntimeError, OSError) as error:
            left = deadline - time.monotonic()
            if left <= 0:
                raise RuntimeError(f'Keycloak admin API unreachable: {error}') from error
            time.sleep(min(POLL, left))
        finally:
            admin.http.timeout = 30.0


SERIES = 'query($id: ID!) { series(id: $id) { id } }'


def anonymous_can_read(series_id):
    """Whether an anonymous caller can read the series: True, False, or None when unknown."""
    try:
        status, body = graphql('/graphql', SERIES, {'id': series_id})
    except OSError:
        return None
    if status != 200:
        return None
    if 'FORBIDDEN' in error_codes(body):
        return False
    if body.get('errors'):
        return None
    return (body.get('data') or {}).get('series') is not None


def wait_for(series_id, readable, what):
    """Poll until anonymous access is `readable`; a check either way."""
    started = time.monotonic()
    while True:
        state = anonymous_can_read(series_id)
        elapsed = time.monotonic() - started
        if state is readable:
            return check(what, True, f'after {elapsed:.0f}s', seconds=round(elapsed))
        if elapsed > WAIT:
            return check(what, False, f'still {"un" if readable else ""}readable after {WAIT:.0f}s'
                         if state is not None else f'no clear answer after {WAIT:.0f}s')
        time.sleep(POLL)


admin = Admin()
try:
    reachable_admin(admin, time.monotonic() + 60)
    api = admin.client('econ-graph-api')
    if api is None:
        fail('realm', 'no econ-graph-api client')
    roles = {r['name']: r for r in admin.call('GET', f'/clients/{api["id"]}/roles')}
    catalog_roles = sorted(n for n in roles if n.startswith('catalog.') and n.endswith(':read'))
    if not catalog_roles:
        skip('this realm has no catalog roles (catalog.*:read, ECO-657); nothing to restrict')
    if ROLE not in roles:
        fail('realm', f'no {ROLE} role; the realm has {", ".join(catalog_roles)}')
    reader_client = admin.client(READER_CLIENT)
    if reader_client is None:
        skip(f'this realm has no {READER_CLIENT} client, so anonymous reads use no catalog roles')
    reader = admin.call('GET', f'/clients/{reader_client["id"]}/service-account-user')
except (RuntimeError, OSError, KeyError) as error:
    fail('keycloak admin', str(error))

# A series in the catalog, found while it is still readable.
try:
    sources = data_or_raise(*graphql('/graphql', '{ dataSources { id name } }'), 'dataSources')['dataSources']
    source = next((s for s in sources if SOURCE_MATCH.lower() in s['name'].lower()), None)
    if source is None:
        fail('catalog', f'no data source matching {SOURCE_MATCH!r} is readable anonymously')
    found = data_or_raise(*graphql(
        '/graphql', 'query($s: ID!) { dataSource(id: $s) { series(first: 1) { nodes { id title } } } }',
        {'s': source['id']}), 'dataSource')['dataSource']['series']['nodes']
except (GraphQLError, OSError, KeyError, TypeError) as error:
    fail('catalog', str(error))
if not found:
    fail('catalog', f'{source["name"]} has no series to check access with')
series = found[0]
check('readable before', anonymous_can_read(series['id']) is True,
      f'{series["title"]} ({source["name"]}) is readable anonymously', series_id=series['id'])

user_path = f'/users/{reader["id"]}/role-mappings'


def effective_roles():
    return {r['name'] for r in admin.call('GET', f'{user_path}/clients/{api["id"]}/composite')}


try:
    realm_direct = {r['name']: r for r in admin.call('GET', f'{user_path}/realm')}
    client_direct = {r['name'] for r in admin.call('GET', f'{user_path}/clients/{api["id"]}')}
    client_effective = effective_roles()
except (RuntimeError, OSError, KeyError) as error:
    fail('keycloak admin', str(error))
check('reader holds the role', ROLE in client_effective,
      f'{READER_CLIENT} holds {ROLE} ({"through " + PUBLIC_COMPOSITE if PUBLIC_COMPOSITE in realm_direct else "directly"})')

# What the restriction changes, so the restore undoes exactly that.
removed_public = PUBLIC_COMPOSITE in realm_direct
removed_direct = ROLE in client_direct
added = sorted(n for n in client_effective
               if n.startswith('catalog.') and n != ROLE and n not in client_direct)
restored = False


def restore():
    if added:
        admin.call('DELETE', f'{user_path}/clients/{api["id"]}', [roles[n] for n in added])
    if removed_direct:
        admin.call('POST', f'{user_path}/clients/{api["id"]}', [roles[ROLE]])
    if removed_public:
        admin.call('POST', f'{user_path}/realm', [realm_direct[PUBLIC_COMPOSITE]])


try:
    # The README's anonymous-only restriction: drop catalog-public from the reader and grant
    # the other catalogs individually.
    if added:
        admin.call('POST', f'{user_path}/clients/{api["id"]}', [roles[n] for n in added])
    if removed_direct:
        admin.call('DELETE', f'{user_path}/clients/{api["id"]}', [roles[ROLE]])
    if removed_public:
        admin.call('DELETE', f'{user_path}/realm', [realm_direct[PUBLIC_COMPOSITE]])
    effective = effective_roles()
    check('restrict', ROLE not in effective and set(added) <= effective,
          f'{READER_CLIENT} no longer holds {ROLE}; keeps {len(effective & set(catalog_roles))} catalog roles')
    wait_for(series['id'], False, 'restriction takes effect')

    if not REDEPLOY:
        skip_check('redeploy', 'QA_REDEPLOY_COMMAND is unset, so persistence across a redeploy is unchecked')
    else:
        started = time.monotonic()
        try:
            redeploy = run_bounded(['bash', '-c', REDEPLOY], REDEPLOY_TIMEOUT, capture=False)
            check('redeploy', redeploy.returncode == 0,
                  f'QA_REDEPLOY_COMMAND exited {redeploy.returncode} after {time.monotonic() - started:.0f}s')
        except subprocess.TimeoutExpired:
            check('redeploy', False, f'QA_REDEPLOY_COMMAND overran {REDEPLOY_TIMEOUT:.0f}s and was stopped')
        reachable_admin(admin, time.monotonic() + WAIT)
        effective = effective_roles()
        check('restriction persisted in Keycloak', ROLE not in effective,
              f'after the redeploy {READER_CLIENT} {"still holds" if ROLE in effective else "lacks"} {ROLE}')
        wait_for(series['id'], False, 'restriction persisted for visitors')
except (RuntimeError, OSError, KeyError) as error:
    check('restrict', False, str(error))
finally:
    try:
        # Keycloak may still be restarting after a redeploy that failed or overran.
        reachable_admin(admin, cleanup_deadline(WAIT))
        restore()
        restored = True
    except (RuntimeError, OSError) as error:
        check('restore', False, f'{error}; put {PUBLIC_COMPOSITE} back on {READER_CLIENT} by hand')

if restored:
    try:
        effective = effective_roles()
    except (RuntimeError, OSError) as error:
        check('restore', False, f'could not read the restored mappings: {error}')
    else:
        check('restore', effective == client_effective,
              f'{READER_CLIENT} has its original roles back' if effective == client_effective
              else f'{READER_CLIENT} differs from before: {sorted(effective ^ client_effective)}')
        wait_for(series['id'], True, 'readable again')

finish()
