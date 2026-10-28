#!/usr/bin/env python3
# qa-scenario: annotation-round-trip
# qa-mutates: yes
# qa-profiles: lab, ci, production
# qa-timeout: 180
# qa-description: Sign in as the QA account; create a private annotation, retry it with the same idempotency key, check privacy, delete it.
"""The annotation round trip, as the QA account.

Inputs: QA_TARGET; QA_USERNAME and QA_PASSWORD or QA_PASSWORD_FILE (skipped without);
QA_OIDC_ISSUER (default TARGET/idp/realms/econ-graph); QA_OIDC_REDIRECT_URI (default
TARGET/auth/callback); QA_SEARCH_QUERY (default "GDP") picks the series.

Creates one private annotation, titled "QA scenario <uuid>", and deletes it again, also when
a check fails partway. The production profile runs this only with a qa-* account.
"""

import os
import uuid

from qa import GraphQLError, check, data_or_raise, error_codes, fail, finish, graphql, signed_in_token

QUERY = os.environ.get('QA_SEARCH_QUERY', 'GDP')

SEARCH = 'query($q: String!) { searchSeries(query: $q, first: 1) { series { id title endDate } } }'
CREATE = """
mutation Create($input: CreateAnnotationInput!) {
  createAnnotation(input: $input) { id seriesId title visibility }
}"""
LIST = 'query($id: String!) { annotationsForSeries(seriesId: $id) { id title visibility } }'
DELETE = 'mutation($id: ID!) { deleteAnnotation(input: {annotationId: $id}) }'

token = signed_in_token()

try:
    series = data_or_raise(*graphql('/graphql', SEARCH, {'q': QUERY}), 'searchSeries')['searchSeries']['series']
except (GraphQLError, OSError) as error:
    fail('find a series', str(error))
if not series:
    fail('find a series', f'"{QUERY}" found nothing to annotate')
series = series[0]

title = f'QA scenario {uuid.uuid4()}'
key = str(uuid.uuid4())
annotation = {
    'seriesId': series['id'],
    'annotationDate': series.get('endDate') or '2020-01-01',
    'title': title,
    'content': 'Written and deleted by scripts/qa/scenarios/annotation-round-trip.py.',
    'annotationType': 'note',
    'isPublic': False,
    'idempotencyKey': key,
}


def listed(with_token):
    status, body = graphql('/graphql', LIST, {'id': series['id']}, token=with_token)
    rows = data_or_raise(status, body, 'annotationsForSeries')['annotationsForSeries']
    return [row for row in rows if row['title'] == title]


created_ids = set()
try:
    first = data_or_raise(*graphql('/graphql', CREATE, {'input': annotation}, token=token),
                          'createAnnotation')['createAnnotation']
    created_ids.add(first['id'])
    check('create', first['title'] == title and first['visibility'] == 'PRIVATE',
          f'private annotation {first["id"]} on {series["title"]}', series_id=series['id'])

    # The retry a client makes after losing the response: same key, same input.
    again = data_or_raise(*graphql('/graphql', CREATE, {'input': annotation}, token=token),
                          'createAnnotation retry')['createAnnotation']
    created_ids.add(again['id'])
    check('idempotent retry', again['id'] == first['id'],
          f'the retry returned {again["id"]}, the first create {first["id"]}')

    mine = listed(token)
    check('stored once', len(mine) == 1, f'{len(mine)} annotation(s) titled {title} for the QA account')

    status, body = graphql('/graphql', LIST, {'id': series['id']})
    if status == 200 and not body.get('errors'):
        anonymous = [r for r in body['data']['annotationsForSeries'] if r['title'] == title]
        check('private to its author', not anonymous, f'an anonymous reader sees {len(anonymous)}')
    else:
        check('private to its author', False, f'anonymous annotationsForSeries answered {status} '
              f'{error_codes(body)}')

    deleted = data_or_raise(*graphql('/graphql', DELETE, {'id': first['id']}, token=token),
                            'deleteAnnotation')['deleteAnnotation']
    if deleted:
        created_ids.discard(first['id'])
    check('delete', deleted is True and not listed(token), f'annotation {first["id"]} deleted')
except (GraphQLError, OSError, KeyError, TypeError) as error:
    check('round trip', False, str(error))
finally:
    # Whatever a failure left behind.
    for leftover in sorted(created_ids):
        try:
            status, body = graphql('/graphql', DELETE, {'id': leftover}, token=token)
            removed = data_or_raise(status, body, 'deleteAnnotation')['deleteAnnotation'] is True
        except (GraphQLError, OSError, KeyError, TypeError) as error:
            check('clean up', False, f'leftover {leftover} not deleted: {error}')
            continue
        check('clean up', removed, f'leftover {leftover} ' + ('deleted' if removed else 'NOT deleted'))

finish()
