/**
 * Where `executeGraphQL` sends each operation, and with which token (ECO-427).
 *
 * Data operations go to `VITE_DATA_GRAPHQL_URL` (falling back to the GraphQL URL) and never
 * carry `Authorization`. App, admin, unnamed and unclassified operations, and every mutation,
 * go to the GraphQL URL with the signed-in user's token. The endpoints are read when
 * `utils/graphql.ts` loads, so each case stubs the environment and imports it afresh.
 */
import { afterEach, beforeEach, describe, expect, it, vi, type MockInstance } from 'vitest';
import { MUTATIONS, QUERIES, type GraphQLRequest } from '../graphql';
import { GET_SERIES_LATEST_OBSERVATION } from '../../hooks/useLatestObservation';
import {
  GET_MAP_CROSS_SECTION,
  GET_MAP_DATASETS,
  GET_MAP_SERIES_META,
} from '../../components/global/hooks/useWorldMapData';

/** The data operations a release build sends, with the documents their call sites use. */
const SHIPPED_DATA_OPERATIONS: Array<[string, string]> = [
  ['GetSeriesDetail', QUERIES.GET_SERIES_DETAIL],
  ['GetSeriesData', QUERIES.GET_SERIES_DATA],
  ['SearchSeries', QUERIES.SEARCH_SERIES],
  ['GetSeriesList', QUERIES.GET_SERIES_LIST],
  ['GetDataSources', QUERIES.GET_DATA_SOURCES],
  ['GetSeriesLatestObservation', GET_SERIES_LATEST_OBSERVATION],
  ['GetMapDatasets', GET_MAP_DATASETS],
  ['GetMapCrossSection', GET_MAP_CROSS_SECTION],
  ['GetMapSeriesMeta', GET_MAP_SERIES_META],
];

/** App and admin operations in `utils/graphql.ts`. */
const APP_OPERATIONS: Array<[string, string]> = [
  ['GetAnnotationsForSeries', QUERIES.GET_ANNOTATIONS_FOR_SERIES],
  ['GetCommentsForAnnotation', QUERIES.GET_COMMENTS_FOR_ANNOTATION],
  ['GetChartCollaborators', QUERIES.GET_CHART_COLLABORATORS],
  ['GetUser', QUERIES.GET_USER],
  ['GetCrawlerStatus', QUERIES.GET_CRAWLER_STATUS],
  ['CreateAnnotation', MUTATIONS.CREATE_ANNOTATION],
  ['UpdateAnnotation', MUTATIONS.UPDATE_ANNOTATION],
  ['DeleteAnnotation', MUTATIONS.DELETE_ANNOTATION],
  ['AddComment', MUTATIONS.ADD_COMMENT],
  ['ShareChart', MUTATIONS.SHARE_CHART],
  ['TriggerCrawl', MUTATIONS.TRIGGER_CRAWL],
];

/** Requests that are not data operations, whatever names they mention. */
const NOT_DATA_REQUESTS: Array<[string, GraphQLRequest]> = [
  ['an unnamed shorthand query', { query: '{ ok }' }],
  ['an unnamed query', { query: 'query { dataSources { id } }' }],
  ['an unnamed query with variables', { query: 'query ($id: ID!) { series(id: $id) { id } }' }],
  ['an unclassified named query', { query: 'query GetSomethingNew { ok }' }],
  [
    'a mutation named like a data operation',
    { query: 'mutation GetSeriesDetail { ok }', operationName: 'GetSeriesDetail' },
  ],
  ['a subscription named like a data operation', { query: 'subscription GetSeriesData { ok }' }],
  [
    'a data query whose operationName names something else',
    { query: QUERIES.GET_SERIES_DETAIL, operationName: 'GetAnnotationsForSeries' },
  ],
  [
    'a document with a data query and an app query',
    {
      query: `${QUERIES.GET_SERIES_DETAIL}\n${QUERIES.GET_ANNOTATIONS_FOR_SERIES}`,
      operationName: 'GetSeriesDetail',
    },
  ],
  [
    'a document with a data query and a mutation',
    { query: `${QUERIES.GET_DATA_SOURCES}\n${MUTATIONS.CREATE_ANNOTATION}` },
  ],
  [
    'a document with a data query and an anonymous query',
    { query: `${QUERIES.GET_DATA_SOURCES}\n{ ok }` },
  ],
];

/**
 * Data queries whose syntax a naive scan gets wrong. Each is still exactly one data query, so it
 * goes to the data URL; each would be miscounted (and sent as an app operation) if the scanner
 * stopped handling comments, strings, fragments or object values in a header.
 */
const TRICKY_DATA_REQUESTS: Array<[string, GraphQLRequest]> = [
  [
    'a comment that reads like a mutation',
    { query: '# mutation Evil { x }\nquery GetDataSources { dataSources { id } }' },
  ],
  [
    'a string argument holding braces and an operation',
    { query: 'query SearchSeries { searchSeries(query: "} query Foo { }") { id } }' },
  ],
  [
    'a block string argument holding braces and an operation',
    // Spans lines: a plain string can't, so only block-string handling reads this right.
    { query: 'query SearchSeries { searchSeries(query: """\n} query Foo {\n""") { id } }' },
  ],
  [
    'a fragment',
    {
      query:
        'query GetSeriesDetail { series(id: "1") { ...Fields } }\n' +
        'fragment Fields on EconomicSeriesType { id title }',
      operationName: 'GetSeriesDetail',
    },
  ],
  [
    'an object default value for a variable',
    {
      query:
        'query GetSeriesList($filter: SeriesFilter = {sourceId: "x"}) ' +
        '{ seriesList(filter: $filter) { totalCount } }',
    },
  ],
  [
    'an object value in a directive argument on the operation',
    { query: 'query GetDataSources @d(a: {b: 1}) { dataSources { id } }' },
  ],
];

/** Documents a broken block-string scan would read as one data query; each holds a mutation. */
const HIDDEN_OPERATION_REQUESTS: Array<[string, GraphQLRequest]> = [
  [
    'an escaped \\""" in a block string, followed by a mutation',
    {
      query:
        'query GetDataSources { dataSources(x: """ \\""" """) { id } }\n' +
        'mutation CreateAnnotation { createAnnotation { id } }',
    },
  ],
];

const TOKEN = 'token-alice';

let fetchSpy: MockInstance<typeof fetch>;

/**
 * Loads `utils/graphql.ts` with the given data URL and a signed-in user.
 * @param dataUrl - `VITE_DATA_GRAPHQL_URL`, or undefined to leave it unset.
 * @returns The freshly loaded `executeGraphQL`.
 */
async function loadWith(dataUrl: string | undefined) {
  vi.resetModules();
  vi.stubEnv('VITE_GRAPHQL_URL', '');
  vi.stubEnv('VITE_DATA_GRAPHQL_URL', dataUrl ?? '');
  const { setAccessTokenProvider } = await import('../../auth/accessToken');
  setAccessTokenProvider(async () => TOKEN);
  const { executeGraphQL } = await import('../graphql');
  return executeGraphQL;
}

/** The URL and `Authorization` header of the only request sent. */
function sent(): { url: string; authorization: string | undefined } {
  expect(fetchSpy).toHaveBeenCalledTimes(1);
  const [url, init] = fetchSpy.mock.calls[0];
  return {
    url: String(url),
    authorization: (init?.headers as Record<string, string>).Authorization,
  };
}

beforeEach(() => {
  vi.spyOn(console, 'log').mockImplementation(() => {});
  fetchSpy = vi
    .spyOn(globalThis, 'fetch')
    .mockImplementation(async () => ({ ok: true, json: async () => ({ data: {} }) }) as unknown as Awaited<ReturnType<typeof fetch>>);
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllEnvs();
  vi.resetModules();
});

describe('with VITE_DATA_GRAPHQL_URL=/data/graphql, signed in', () => {
  it.each(SHIPPED_DATA_OPERATIONS)(
    '%s goes to /data/graphql with no Authorization header',
    async (_name, query) => {
      const executeGraphQL = await loadWith('/data/graphql');
      await executeGraphQL({ query, variables: {} });
      expect(sent()).toEqual({ url: '/data/graphql', authorization: undefined });
    }
  );

  it.each(SHIPPED_DATA_OPERATIONS)(
    '%s with its operationName goes to /data/graphql with no Authorization header',
    async (name, query) => {
      const executeGraphQL = await loadWith('/data/graphql');
      await executeGraphQL({ query, operationName: name });
      expect(sent()).toEqual({ url: '/data/graphql', authorization: undefined });
    }
  );

  it.each(APP_OPERATIONS)('%s goes to /graphql with the bearer token', async (_name, query) => {
    const executeGraphQL = await loadWith('/data/graphql');
    await executeGraphQL({ query });
    expect(sent()).toEqual({ url: '/graphql', authorization: `Bearer ${TOKEN}` });
  });

  it.each(NOT_DATA_REQUESTS)('%s goes to /graphql with the bearer token', async (_name, request) => {
    const executeGraphQL = await loadWith('/data/graphql');
    await executeGraphQL(request);
    expect(sent()).toEqual({ url: '/graphql', authorization: `Bearer ${TOKEN}` });
  });

  it.each(TRICKY_DATA_REQUESTS)(
    'a data query with %s goes to /data/graphql with no Authorization header',
    async (_name, request) => {
      const executeGraphQL = await loadWith('/data/graphql');
      await executeGraphQL(request);
      expect(sent()).toEqual({ url: '/data/graphql', authorization: undefined });
    }
  );

  it.each(HIDDEN_OPERATION_REQUESTS)(
    'a document with %s goes to /graphql with the bearer token',
    async (_name, request) => {
      const executeGraphQL = await loadWith('/data/graphql');
      await executeGraphQL(request);
      expect(sent()).toEqual({ url: '/graphql', authorization: `Bearer ${TOKEN}` });
    }
  );

  it('does not ask for a token for a data operation', async () => {
    const executeGraphQL = await loadWith('/data/graphql');
    const { setAccessTokenProvider } = await import('../../auth/accessToken');
    const provider = vi.fn(async () => TOKEN);
    setAccessTokenProvider(provider);
    await executeGraphQL({ query: QUERIES.GET_SERIES_DETAIL });
    expect(provider).not.toHaveBeenCalled();
  });
});

describe('with VITE_DATA_GRAPHQL_URL unset, signed in', () => {
  it.each(SHIPPED_DATA_OPERATIONS)(
    '%s goes to /graphql as before, with no Authorization header',
    async (_name, query) => {
      const executeGraphQL = await loadWith(undefined);
      await executeGraphQL({ query });
      expect(sent()).toEqual({ url: '/graphql', authorization: undefined });
    }
  );

  it.each(APP_OPERATIONS)('%s goes to /graphql with the bearer token', async (_name, query) => {
    const executeGraphQL = await loadWith(undefined);
    await executeGraphQL({ query });
    expect(sent()).toEqual({ url: '/graphql', authorization: `Bearer ${TOKEN}` });
  });

  it.each(NOT_DATA_REQUESTS)('%s goes to /graphql with the bearer token', async (_name, request) => {
    const executeGraphQL = await loadWith(undefined);
    await executeGraphQL(request);
    expect(sent()).toEqual({ url: '/graphql', authorization: `Bearer ${TOKEN}` });
  });

  it('sends data operations to VITE_GRAPHQL_URL when only that is set', async () => {
    vi.resetModules();
    vi.stubEnv('VITE_GRAPHQL_URL', 'http://backend.test/graphql');
    vi.stubEnv('VITE_DATA_GRAPHQL_URL', '');
    const { executeGraphQL } = await import('../graphql');
    await executeGraphQL({ query: QUERIES.GET_DATA_SOURCES });
    expect(sent().url).toBe('http://backend.test/graphql');
  });
});
