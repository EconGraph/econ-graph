#!/usr/bin/env python3
# qa-scenario: anonymous-api
# qa-mutates: no
# qa-profiles: lab, ci, production
# qa-timeout: 180
# qa-description: Without a token: search, a series, its observations, the world map's cross-section, and /data/graphql.
"""Anonymous reads through the app's /graphql and the token-free /data/graphql.

Inputs: QA_TARGET; QA_SEARCH_QUERY (default "GDP"), the search that must find a series with
observations. Reads only; sends no Authorization header.
"""

import os
import re

from qa import GraphQLError, check, data_or_raise, fail, finish, graphql

QUERY = os.environ.get('QA_SEARCH_QUERY', 'GDP')

SEARCH = """
query Search($query: String!) {
  searchSeries(query: $query, first: 5) { totalCount series { id title externalId } }
}"""
SERIES = 'query Series($id: ID!) { series(id: $id) { id title units frequency source { name } } }'
DATA = """
query Data($id: ID!) {
  seriesData(seriesId: $id, first: 200) { totalCount nodes { date value } }
}"""
DATASETS = 'query { datasets { id code dimensions { name codelist codes { code } } } }'
CROSS_SECTION = """
query Map($datasetId: ID!, $filter: [DimensionFilterInput!]!, $across: String!) {
  crossSection(datasetId: $datasetId, filter: $filter, across: $across, latest: true) {
    key date value area { kind iso3 }
  }
}"""

ISO_DATE = re.compile(r'^\d{4}-\d{2}-\d{2}$')


def is_number(value) -> bool:
    try:
        float(value)
    except (TypeError, ValueError):
        return False
    return True


def search(path):
    data = data_or_raise(*graphql(path, SEARCH, {'query': QUERY}), f'searchSeries on {path}')
    return data['searchSeries']


try:
    found = search('/graphql')
    check('search', found['totalCount'] > 0 and bool(found['series']),
          f'"{QUERY}" matched {found["totalCount"]} series', total=found['totalCount'])
    if not found['series']:
        fail('search', f'"{QUERY}" found nothing to open')

    # The first result with observations: a catalog can list series not yet fetched.
    chosen, points = None, None
    for candidate in found['series']:
        data = data_or_raise(*graphql('/graphql', DATA, {'id': candidate['id']}), 'seriesData')
        if data['seriesData']['totalCount'] > 0:
            chosen, points = candidate, data['seriesData']
            break
    if chosen is None:
        fail('series data', f'none of the first {len(found["series"])} "{QUERY}" results has observations')

    series = data_or_raise(*graphql('/graphql', SERIES, {'id': chosen['id']}), 'series')['series']
    check('open series', bool(series) and series['id'] == chosen['id'],
          f'{series["title"]} ({series["source"]["name"] if series.get("source") else "?"})',
          series_id=chosen['id'], external_id=chosen['externalId'])

    nodes = points['nodes']
    dates = [n['date'] for n in nodes]
    numeric = [n for n in nodes if n['value'] is not None]
    parsable = all(is_number(n['value']) for n in numeric)
    check('chart data',
          bool(nodes) and all(ISO_DATE.match(d) for d in dates) and dates == sorted(dates)
          and bool(numeric) and parsable,
          f'{points["totalCount"]} observations, first page {dates[0]} to {dates[-1]}' if dates else 'no points',
          total=points['totalCount'], newest_on_page=dates[-1] if dates else None)

    datasets = data_or_raise(*graphql('/graphql', DATASETS), 'datasets')['datasets']
    wdi = next((d for d in datasets if d['code'] == 'wdi'), None)
    if wdi is None:
        check('world map', False, 'no wdi dataset, so the world map has nothing to show')
    else:
        area = next((d for d in wdi['dimensions'] if d['codelist'] == 'countries'), None)
        others = [d for d in wdi['dimensions'] if d is not area]
        if area is None or len(others) != 1 or not others[0]['codes']:
            check('world map', False, 'the wdi dataset lacks its area and indicator dimensions')
        else:
            codes = [c['code'] for c in others[0]['codes']]
            indicator = 'NY.GDP.PCAP.CD' if 'NY.GDP.PCAP.CD' in codes else codes[0]
            entries = data_or_raise(*graphql('/graphql', CROSS_SECTION, {
                'datasetId': wdi['id'],
                'filter': [{'dimension': others[0]['name'], 'value': indicator}],
                'across': area['name'],
            }), 'crossSection')['crossSection']
            countries = [e for e in entries
                         if (e.get('area') or {}).get('kind') == 'COUNTRY' and e['value'] is not None]
            check('world map', bool(countries),
                  f'{indicator}: {len(countries)} countries with a latest value',
                  indicator=indicator, countries=len(countries))

    # /data/graphql: the data operations' endpoint, which never carries a token.
    try:
        found_data = search('/data/graphql')
        data_path = data_or_raise(*graphql('/data/graphql', DATA, {'id': chosen['id']}),
                                  'seriesData on /data/graphql')['seriesData']
    except GraphQLError as error:
        check('/data/graphql without a token', False, str(error))
    else:
        check('/data/graphql without a token',
              found_data['totalCount'] == found['totalCount']
              and data_path['totalCount'] == points['totalCount'],
              f'search {found_data["totalCount"]} and observations {data_path["totalCount"]} '
              f'match /graphql ({found["totalCount"]}, {points["totalCount"]})')
except GraphQLError as error:
    fail('graphql', str(error))
except OSError as error:
    fail('reach the target', str(error))

finish()
