import React from 'react';
import { render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import App from '../../App';
import { DASHBOARD_SERIES } from '../../config/dashboardSeries';
import { GET_SERIES_LATEST_OBSERVATION } from '../../hooks/useLatestObservation';
import { QUERIES } from '../../utils/graphql';
import { auditPage } from './audit';

type State = 'loaded' | 'empty' | 'error' | 'loading';
let state: State;
let requests: string[];
let unexpected: string[];
const compact = (query: string) => query.replace(/\s/g, '');
const notice = {
  citation: 'Synthetic source attribution',
  notice: 'Economic data supplied for an accessibility fixture.',
  termsUrl: 'https://example.invalid/terms',
  retrievalDateRequired: true,
};
const sources = [
  {
    id: 'bea',
    name: 'Bureau of Economic Analysis (BEA)',
    description: 'National economic accounts',
    baseUrl: 'https://example.invalid/bea',
    apiKeyRequired: true,
    rateLimitPerMinute: 60,
    seriesCount: 12,
    lifecycleCounts: { discontinued: 1, missingFromCatalog: 0, retired: 0, superseded: 0 },
    createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-02T00:00:00Z',
    notice,
  },
];

// Stub the transport only: render the real App, providers, hooks, cards and notices.
// Unexpected calls are asserted separately, so a rejected fixture cannot masquerade
// as the deliberate error-state test.
beforeEach(() => {
  state = 'loaded';
  requests = [];
  unexpected = [];
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, options: { body?: unknown }) => {
      const body = JSON.parse(String(options.body));
      let data: unknown;
      if (url === '/graphql' && compact(body.query) === compact(GET_SERIES_LATEST_OBSERVATION)) {
        const entry = DASHBOARD_SERIES.find(
          item =>
            item.sourceName === body.variables.sourceName &&
            item.externalId === body.variables.externalId
        );
        if (!entry) {
          unexpected.push(JSON.stringify(body.variables));
          throw new Error('Unknown dashboard series');
        }
        requests.push(entry.id);
        data = {
          seriesByExternalId:
            state === 'empty'
              ? null
              : {
                  id: `fixture-${entry.id}`,
                  units: entry.units,
                  lastUpdated: '2026-01-02T00:00:00Z',
                  source: { name: entry.sourceName, notice },
                  latestObservation: {
                    date: '2026-01-01',
                    value: entry.id === 'gdp' ? '30000000' : '4.2',
                    revisionDate: '2026-01-02',
                  },
                },
        };
      } else if (url === '/graphql' && compact(body.query) === compact(QUERIES.GET_DATA_SOURCES)) {
        requests.push('sources');
        data = { dataSources: state === 'empty' ? [] : sources };
      } else {
        unexpected.push(`${url}: ${body.query}`);
        throw new Error('Unexpected accessibility fixture request');
      }
      if (state === 'loading') return new Promise<Response>(() => {});
      if (state === 'error') return new Response('Fixture unavailable', { status: 503 });
      return new Response(JSON.stringify({ data }), {
        headers: { 'Content-Type': 'application/json' },
      });
    })
  );
});
afterEach(() => {
  vi.unstubAllGlobals();
  expect(unexpected).toEqual([]);
});

async function withApp(path: string, run: () => Promise<void>) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, retryDelay: 0 } } });
  const { unmount } = render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={[path]}>
        <App />
      </MemoryRouter>
    </QueryClientProvider>
  );
  try {
    await run();
  } finally {
    unmount();
    client.clear();
  }
}

describe('Current public data pages accessibility', () => {
  it.each<State>(['loaded', 'empty', 'error', 'loading'])(
    'audits the BEA/BLS dashboard in its %s state',
    async selected => {
      state = selected;
      await withApp('/', async () => {
        expect(screen.getByRole('heading', { name: 'Economic Dashboard', level: 1 })).toBeVisible();
        await waitFor(() =>
          expect(new Set(requests)).toEqual(new Set(DASHBOARD_SERIES.map(e => e.id)))
        );
        if (selected === 'loaded') {
          for (const entry of DASHBOARD_SERIES) {
            const card = screen.getByRole('article', { name: entry.label });
            expect(await within(card).findByRole('link', { name: entry.label })).toHaveAttribute(
              'href',
              `/series/fixture-${entry.id}`
            );
            expect(within(card).getByRole('link', { name: 'Terms of use' })).toHaveAttribute(
              'href',
              notice.termsUrl
            );
            expect(
              screen.getByRole('region', { name: `Source notice for ${entry.label}` })
            ).toBeVisible();
          }
        } else if (selected === 'loading') {
          expect(screen.getAllByRole('progressbar', { name: 'Loading' })).toHaveLength(
            DASHBOARD_SERIES.length
          );
        } else {
          const message = selected === 'empty' ? 'No data yet' : "Couldn't load this series";
          await waitFor(() =>
            expect(screen.getAllByText(message)).toHaveLength(DASHBOARD_SERIES.length)
          );
          for (const entry of DASHBOARD_SERIES) {
            expect(
              within(screen.getByRole('article', { name: entry.label })).queryByRole('link')
            ).toBeNull();
          }
        }
        await auditPage(`dashboard-${selected}`);
      });
    }
  );

  it.each<State>(['loaded', 'empty', 'error', 'loading'])(
    'audits the sources page in its %s state',
    async selected => {
      state = selected;
      await withApp('/sources', async () => {
        await waitFor(() => expect(requests).toContain('sources'));
        if (selected === 'loaded') {
          const source = await screen.findByRole('region', { name: sources[0].name });
          expect(within(source).getByRole('link', { name: 'Browse Series' })).toHaveAttribute(
            'href',
            '/explore?source=bea'
          );
          expect(within(source).getByText('12')).toBeVisible();
        } else if (selected === 'loading') {
          expect(screen.getByRole('progressbar', { name: 'Loading data sources' })).toBeVisible();
        } else if (selected === 'error') {
          expect(await screen.findByRole('alert')).toHaveTextContent('Could not load data sources');
        } else {
          await screen.findByRole('heading', { name: 'Data Sources', level: 1 });
          expect(screen.queryByRole('link', { name: 'Browse Series' })).toBeNull();
        }
        await auditPage(`sources-${selected}`);
      });
    }
  );
});
