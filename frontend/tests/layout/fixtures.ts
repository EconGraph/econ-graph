import { readFileSync } from 'node:fs';
import { test as base, expect, type Locator } from '@playwright/test';

export const PUBLIC_URL = 'http://127.0.0.1:18181';
export const ADMIN_URL = 'http://127.0.0.1:18182/admin/';

// Layout fixtures deliberately do not exercise a live crawler or authentication service.
// Unexpected API operations fail the test instead of quietly producing an empty UI.
const responses: Record<string, object> = {
  SearchSeries: { searchSeries: [] },
  SearchSeriesFulltext: { searchSeriesFulltext: [] },
  CrawlerAdminOverview: {
    crawlerStatus: {
      isRunning: true,
      activeWorkers: 3,
      lastCrawl: '2026-06-01T10:00:00Z',
      nextScheduledCrawl: '2026-06-01T14:00:00Z',
      sources: [],
    },
    queueStatistics: {
      totalItems: 100,
      pendingItems: 20,
      processingItems: 5,
      completedItems: 70,
      failedItems: 3,
      retryingItems: 2,
    },
    crawlerSources: [
      {
        source: 'FRED',
        name: 'FRED',
        enabled: true,
        crawlFrequencyHours: 24,
        requiresAdminApproval: false,
      },
    ],
  },
};

const fonts = [300, 400, 500, 700].map(weight => ({
  weight: String(weight),
  data: readFileSync(
    new URL(`./fonts/roboto-latin-${weight}-normal.woff2`, import.meta.url)
  ).toString('base64'),
}));

export const test = base.extend({
  page: async ({ page }, use) => {
    // Supply the app's real Roboto family from pinned local assets, before it renders.
    await page.addInitScript(fonts => {
      for (const { weight, data } of fonts) {
        const face = new FontFace('Roboto', `url(data:font/woff2;base64,${data})`, { weight });
        document.fonts.add(face);
        void face.load();
      }
    }, fonts);
    const unexpected: string[] = [];
    const errors: string[] = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.clock.setFixedTime(new Date('2026-06-01T12:00:00Z'));
    await page.route('**/*', async route => {
      const url = new URL(route.request().url());
      if (url.hostname === 'fonts.googleapis.com') {
        // The identical font family is supplied by the pinned local assets above.
        return route.fulfill({ contentType: 'text/css', body: '' });
      }
      if (!['127.0.0.1', 'localhost'].includes(url.hostname)) {
        unexpected.push(route.request().url());
        return route.abort();
      }
      if (url.pathname === '/graphql') {
        const body = route.request().postDataJSON();
        const operation = /\bquery\s+(\w+)/.exec(body.query)?.[1];
        if (operation === 'GetSeriesLatestObservation') {
          const entries: Record<string, { source: string; value: string }> = {
            'bea_nipa/T10105.1.Q': {
              source: 'Bureau of Economic Analysis (BEA)',
              value: '30000000000000',
            },
            LNS14000000: { source: 'Bureau of Labor Statistics (BLS)', value: '4.2' },
            CUSR0000SA0: { source: 'Bureau of Labor Statistics (BLS)', value: '321.123' },
            CES0000000001: { source: 'Bureau of Labor Statistics (BLS)', value: '159000' },
          };
          const entry = entries[body.variables?.externalId];
          if (entry && entry.source === body.variables.sourceName) {
            return route.fulfill({
              json: {
                data: {
                  seriesByExternalId: {
                    id: body.variables.externalId,
                    units: null,
                    lastUpdated: '2026-06-01T10:00:00Z',
                    source: { name: entry.source, notice: null },
                    latestObservation: {
                      date: '2026-03-01',
                      value: entry.value,
                      revisionDate: '2026-05-01',
                    },
                  },
                },
              },
            });
          }
        }
        if (!operation || !responses[operation]) {
          unexpected.push(`GraphQL operation: ${operation ?? body.query}`);
          return route.fulfill({
            status: 400,
            json: { errors: [{ message: 'Unconfigured fixture' }] },
          });
        }
        return route.fulfill({ json: { data: responses[operation] } });
      }
      if (url.pathname.startsWith('/api/')) {
        unexpected.push(url.pathname);
        return route.abort();
      }
      return route.continue();
    });
    await use(page);
    expect(
      unexpected,
      'All external requests and API operations must have explicit fixtures'
    ).toEqual([]);
    expect(errors, 'The production page must render without JavaScript errors').toEqual([]);
  },
});

export { expect };

export async function box(locator: Locator) {
  await expect(locator).toBeVisible();
  const bounds = await locator.boundingBox();
  expect(bounds).not.toBeNull();
  return bounds!;
}

export async function above(first: Locator, second: Locator, gap = 0) {
  const a = await box(first);
  const b = await box(second);
  expect(
    b.y - (a.y + a.height),
    'Text/control blocks must be vertically separated'
  ).toBeGreaterThanOrEqual(gap - 1);
}

export async function contained(child: Locator, parent: Locator) {
  const c = await box(child);
  const p = await box(parent);
  expect(c.x).toBeGreaterThanOrEqual(p.x - 1);
  expect(c.x + c.width).toBeLessThanOrEqual(p.x + p.width + 1);
  expect(c.y).toBeGreaterThanOrEqual(p.y - 1);
  expect(c.y + c.height).toBeLessThanOrEqual(p.y + p.height + 1);
}

export async function noHorizontalOverflow(locator: Locator) {
  const sizes = await locator.evaluate(el => ({ content: el.scrollWidth, box: el.clientWidth }));
  expect(
    sizes.content,
    'The panel must contain its content without horizontal overflow'
  ).toBeLessThanOrEqual(sizes.box);
}
