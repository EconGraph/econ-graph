#!/usr/bin/env python3
# qa-scenario: branch-isolation
# qa-mutates: yes
# qa-profiles: lab
# qa-timeout: 900
# qa-description: A branch namespace shows the shared data plane's series and keeps its users and annotations in its own app database (v4.1.0 exit criterion 1).
"""A branch environment reads the primary data plane and writes only its own app database.

Run against a branch deployed with scripts/deploy/deploy-branch.sh (QA_TARGET is its
http://<slug>.localhost). Inputs:
  QA_KUBE_CONTEXT          the kind context (e.g. kind-econ-graph); skipped without it.
  QA_BRANCH_NAMESPACE      the branch's namespace; skipped without it.
  QA_USERNAME, QA_PASSWORD or QA_PASSWORD_FILE
                           a user of the branch's own Keycloak realm; skipped without.
  QA_DATA_NAMESPACE        the primary data plane's namespace (default econ-graph): data
                           database econ_graph in postgresql-0, production's app database
                           econ_graph_app in app-postgres-0.
  QA_SEARCH_QUERY          the search (default "GDP").

Checks:
  branch namespace     labelled econgraph.io/env=branch and data-mode=primary, with no data
                       Postgres of its own.
  real data            an anonymous /data/graphql search on the branch finds series, and every
                       series it returned is a row of the primary data database.
  branch backend reads the data plane
                       the same search through the branch's own backend (/graphql) returns
                       the same series. (The branch's /data/graphql route goes to the
                       primary's backend, so this is the check that the branch's backend
                       reaches the data plane.)
  sign in              as the QA account, through the branch's realm.
  stored in the branch the annotation it creates is a row of the branch's app database
                       (econ_graph_app in its app-postgres-0), and so is the user it belongs
                       to.
  not in production    the annotation is not in production's app database.
  not in <namespace>   nor in any other branch's app database. A branch whose app database
                       can't be read is recorded as skipped, not failed.
The annotation's user_id references users in the same database, so an annotation found only
in the branch puts its user there too; user ids are generated per database, so they are
not compared across databases. The annotation is private, titled "QA scenario <uuid>", and
deleted at the end, also when a check fails or the create's response is lost. The
databases are only read.
"""

import os
import uuid

from qa import (GraphQLError, Kube, KubeError, check, data_or_raise, fail, finish, graphql,
                signed_in_token, skip, skip_check, sql_literal)

BRANCH = os.environ.get('QA_BRANCH_NAMESPACE', '').strip()
DATA_NAMESPACE = os.environ.get('QA_DATA_NAMESPACE', 'econ-graph')
QUERY = os.environ.get('QA_SEARCH_QUERY', 'GDP')
APP = ('app-postgres-0', 'postgres', 'econ_graph_app')

SEARCH = 'query($q: String!) { searchSeries(query: $q, first: 5) { series { id title } } }'
CREATE = """
mutation Create($input: CreateAnnotationInput!) {
  createAnnotation(input: $input) { id seriesId title visibility }
}"""
DELETE = 'mutation($id: ID!) { deleteAnnotation(input: {annotationId: $id}) }'

if not BRANCH:
    skip('no branch: set QA_BRANCH_NAMESPACE to the namespace deploy-branch.sh created')
if BRANCH == DATA_NAMESPACE:
    fail('branch namespace', f'{BRANCH} is the primary namespace, not a branch')
kube = Kube()

try:
    namespace = kube.items('namespaces', '-l', 'econgraph.io/env=branch')
    namespace = next((n for n in namespace if n['metadata']['name'] == BRANCH), None)
    if namespace is None:
        fail('branch namespace', f'{BRANCH} is not a namespace labelled econgraph.io/env=branch')
    mode = namespace['metadata'].get('labels', {}).get('econgraph.io/data-mode')
    own_data = kube.items('statefulsets', '-n', BRANCH, '--field-selector', 'metadata.name=postgresql')
    check('branch namespace', mode == 'primary' and not own_data,
          f'{BRANCH}: data-mode={mode}' + (', with its own data Postgres' if own_data else ''))
except KubeError as error:
    fail('branch namespace', str(error))

try:
    found = data_or_raise(*graphql('/data/graphql', SEARCH, {'q': QUERY}), 'searchSeries')['searchSeries']['series']
    if not found:
        fail('real data', f'"{QUERY}" found no series through the branch')
    ids = ', '.join(sql_literal(s['id']) for s in found)
    known = kube.psql(DATA_NAMESPACE, 'postgresql-0', 'postgresql', 'econ_graph',
                      f'SELECT count(*) FROM economic_series WHERE id::text IN ({ids})')
    known = int(known[0][0])
    check('real data', known == len(found),
          f'{known} of the {len(found)} "{QUERY}" series are in the primary data database')
except (GraphQLError, OSError, KeyError, TypeError, ValueError, IndexError, KubeError) as error:
    fail('real data', str(error))

try:
    own = data_or_raise(*graphql('/graphql', SEARCH, {'q': QUERY}), 'searchSeries on /graphql')['searchSeries']['series']
    same = [s['id'] for s in own] == [s['id'] for s in found]
    check('branch backend reads the data plane', same,
          f'/graphql and /data/graphql found the same {len(found)} series' if same
          else f'/graphql found {[s["id"] for s in own]}, /data/graphql {[s["id"] for s in found]}')
except (GraphQLError, OSError, KeyError, TypeError) as error:
    check('branch backend reads the data plane', False, str(error))

token = signed_in_token()


def annotations_titled(namespace, title):
    """(id, user_id, whether that user is a row there) of each annotation titled `title`."""
    return kube.psql(namespace, *APP,
                     'SELECT a.id, a.user_id, EXISTS (SELECT 1 FROM users u WHERE u.id = a.user_id) '
                     f'FROM chart_annotations a WHERE a.title = {sql_literal(title)}')


title = f'QA scenario {uuid.uuid4()}'
created = None
attempted = False
try:
    attempted = True
    series = found[0]
    created = data_or_raise(*graphql('/graphql', CREATE, {'input': {
        'seriesId': series['id'], 'annotationDate': '2020-01-01',
        'title': title, 'content': 'Written and deleted by scripts/qa/scenarios/branch-isolation.py.',
        'annotationType': 'note', 'isPublic': False, 'idempotencyKey': str(uuid.uuid4())}},
        token=token), 'createAnnotation')['createAnnotation']['id']

    stored = annotations_titled(BRANCH, title)
    check('stored in the branch', len(stored) == 1 and stored[0][2] == 't',
          f'{len(stored)} annotation(s) in {BRANCH}'
          + (f', its user {"is" if stored[0][2] == "t" else "is NOT"} there' if stored else ''))

    found_in = annotations_titled(DATA_NAMESPACE, title)
    check('not in production', not found_in, f'{len(found_in)} annotation(s) in {DATA_NAMESPACE}')
    for other in [n['metadata']['name'] for n in kube.items('namespaces', '-l', 'econgraph.io/env=branch')
                  if n['metadata']['name'] != BRANCH]:
        if not kube.items('pods', '-n', other, '--field-selector', 'metadata.name=app-postgres-0'):
            continue
        try:
            found_in = annotations_titled(other, title)
        except KubeError as error:
            # Another branch that is broken is that branch's problem, not this one's.
            skip_check(f'not in {other}', f'its app database could not be read: {error}')
            continue
        check(f'not in {other}', not found_in, f'{len(found_in)} annotation(s) in {other}')
except (GraphQLError, OSError, KeyError, TypeError, ValueError, IndexError, KubeError) as error:
    check('branch app database', False, str(error))
finally:
    if attempted and not created:
        # The create may have succeeded with its response lost: find it by its title.
        try:
            leftover = annotations_titled(BRANCH, title)
            created = leftover[0][0] if leftover else None
        except KubeError as error:
            check('clean up', False, f'could not look for an annotation titled {title}: {error}')
    if created:
        try:
            removed = data_or_raise(*graphql('/graphql', DELETE, {'id': created}, token=token),
                                    'deleteAnnotation')['deleteAnnotation'] is True
        except (GraphQLError, OSError, KeyError, TypeError) as error:
            removed, why = False, str(error)
        else:
            why = ''
        check('clean up', removed, f'annotation {created} ' + ('deleted' if removed else f'NOT deleted {why}'))

finish()
