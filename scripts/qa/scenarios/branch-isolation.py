#!/usr/bin/env python3
# qa-scenario: branch-isolation
# qa-mutates: yes
# qa-profiles: lab
# qa-timeout: 600
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
  sign in              as the QA account, through the branch's realm.
  stored in the branch the annotation it creates, and the user it belongs to, are rows of
                       the branch's app database (econ_graph_app in its app-postgres-0).
  not in production    neither is in production's app database.
  not in <namespace>   neither is in any other branch's app database.
The annotation is private, titled "QA scenario <uuid>", and deleted at the end, also when a
check fails. The databases are only read.
"""

import os
import uuid

from qa import (GraphQLError, Kube, KubeError, check, data_or_raise, fail, finish, graphql,
                signed_in_token, skip, sql_literal)

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

token = signed_in_token()


def app_rows(namespace, title, user_id):
    """(annotations titled `title`, users with id `user_id`) in `namespace`'s app database."""
    rows = kube.psql(namespace, *APP,
                     f'SELECT (SELECT count(*) FROM chart_annotations WHERE title = {sql_literal(title)}), '
                     f'(SELECT count(*) FROM users WHERE id::text = {sql_literal(user_id)})')
    return int(rows[0][0]), int(rows[0][1])


title = f'QA scenario {uuid.uuid4()}'
created = None
try:
    series = found[0]
    created = data_or_raise(*graphql('/graphql', CREATE, {'input': {
        'seriesId': series['id'], 'annotationDate': '2020-01-01',
        'title': title, 'content': 'Written and deleted by scripts/qa/scenarios/branch-isolation.py.',
        'annotationType': 'note', 'isPublic': False, 'idempotencyKey': str(uuid.uuid4())}},
        token=token), 'createAnnotation')['createAnnotation']['id']

    stored = kube.psql(BRANCH, *APP, f'SELECT user_id FROM chart_annotations WHERE title = {sql_literal(title)}')
    user_id = stored[0][0] if len(stored) == 1 else ''
    users = app_rows(BRANCH, title, user_id)[1] if user_id else 0
    check('stored in the branch', len(stored) == 1 and users == 1,
          f'{len(stored)} annotation(s) and {users} user(s) in {BRANCH}', user_id=user_id)

    # Without the user's id, the user check below would pass vacuously; the annotation's
    # title still identifies it everywhere.
    for other in [DATA_NAMESPACE] + [n['metadata']['name'] for n in kube.items(
            'namespaces', '-l', 'econgraph.io/env=branch') if n['metadata']['name'] != BRANCH]:
        # Production's app database must be there to check; a branch may run without one.
        if other != DATA_NAMESPACE and not kube.items('pods', '-n', other, '--field-selector',
                                                      'metadata.name=app-postgres-0'):
            continue
        annotations, users = app_rows(other, title, user_id)
        check('not in production' if other == DATA_NAMESPACE else f'not in {other}',
              annotations == 0 and users == 0, f'{annotations} annotation(s) and {users} user(s) in {other}')
except (GraphQLError, OSError, KeyError, TypeError, ValueError, IndexError, KubeError) as error:
    check('branch app database', False, str(error))
finally:
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
