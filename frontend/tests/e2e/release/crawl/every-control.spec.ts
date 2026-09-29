// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { expect, test, type APIRequestContext } from '@playwright/test';

import { seededSeriesId } from '../auth/helpers';
import { SEEDED } from '../fixtures';

import { ALLOWLIST, type AllowlistEntry } from './allowlist';
import { clickAndObserve, gotoSettled, listControls, type Control } from './controls';

// Live QA: the same crawl against the deployed build (REL-QA check 3).
//
// Exit criterion 5: every visible control on every route in App.tsx does something, whether or
// not any page links to the route. In scope: elements with a control's ARIA role (CONTROL_ROLES in
// controls.ts) that a route shows when it loads, signed out. Out of scope: controls inside menus
// and dialogs, the signed-in ones (annotation editing, the user menu), which the area specs
// exercise, and clickable elements with no role at all, which should get one.

const APP_TSX = readFileSync(
  fileURLToPath(new URL('../../../../src/App.tsx', import.meta.url)),
  'utf8'
);

/**
 * Every route App.tsx declares, as its `path` prop is written there: a string literal, or the
 * name of the constant it uses (`CALLBACK_PATH`). Read from the source so a new route can't be
 * missed.
 * @returns The paths, in declaration order.
 */
function appRoutes(): string[] {
  return Array.from(
    APP_TSX.matchAll(/<Route\b[^>]*?\bpath=(?:'([^']+)'|"([^"]+)"|\{(\w+)\})/g),
    m => (m[1] ?? m[2] ?? m[3]) as string
  );
}

/** The values of the constants App.tsx uses as paths (src/auth/oidcConfig.ts). */
const PATH_CONSTANTS: Record<string, string> = { CALLBACK_PATH: '/auth/callback' };

/**
 * The URLs to crawl for each route, keyed by its `path` as App.tsx writes it. A route with a
 * parameter gets the seeded FHFA series, which no spec annotates: an annotation another spec adds
 * mid-crawl would shift the controls the crawl finds by position. A key App.tsx no longer has is
 * ignored, so a PR that removes a route doesn't have to touch this file.
 */
const ROUTE_URLS: Record<string, (request: APIRequestContext) => Promise<string[]>> = {
  '/': async () => ['/'],
  '/explore': async () => ['/explore'],
  '/series/:id': async request => [`/series/${await seededSeriesId(request, SEEDED.fhfaHpi)}`],
  '/sources': async () => ['/sources'],
  '/about': async () => ['/about'],
  '/global': async () => ['/global'],
  '/privacy': async () => ['/privacy'],
  // Behind the build_canary flag (docs/build-flags.md), off in release: the route isn't
  // rendered, so this crawls whatever the app shows instead (the catch-all), which is the
  // point — a flagged-off route should 404 cleanly, not break.
  '/flags/canary': async () => ['/flags/canary'],
  // Keycloak's redirect target. Opened directly, with no authorization response to finish, it
  // shows its error and a way home.
  CALLBACK_PATH: async () => [PATH_CONSTANTS.CALLBACK_PATH],
  // The catch-all: any URL none of the routes above serves.
  '*': async () => ['/this-page-does-not-exist'],
};

/**
 * The header and sidebar, which every route shares; crawled once, from the dashboard. Distinct
 * from the `'*'` route path (App.tsx's catch-all, rendering NotFound) so an allowlist entry for
 * one is never mistaken for the other.
 */
const LAYOUT = 'layout';

const routes = appRoutes();

/**
 * A route's path as a regular expression source: `:id` is one segment, `:id?` an optional one.
 * @param path - The path as React Router reads it.
 * @returns The pattern, without anchors.
 */
const pathPattern = (path: string) =>
  path
    .split('/')
    .filter(Boolean)
    .map(seg =>
      !seg.startsWith(':')
        ? `/${seg.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}`
        : seg.endsWith('?')
          ? '(?:/[^/]+)?'
          : '/[^/]+'
    )
    .join('');

/**
 * Matches the pathnames a specific route in App.tsx serves, with or without a trailing slash. The
 * catch-all ('*') is excluded: it isn't a page of its own, so a navigation only it would serve
 * still counts as broken.
 */
const APP_PATH = new RegExp(
  `^(?:${routes
    .filter(r => r !== '*')
    .map(r => pathPattern(PATH_CONSTANTS[r] ?? r))
    .join('|')})/?$`
);
const isAppPath = (pathname: string) => APP_PATH.test(pathname);

const describeControl = (c: Control) => `${c.role} "${c.name}"${c.href ? ` (${c.href})` : ''}`;

const allowlisted = (scope: string, control: Control): AllowlistEntry | undefined =>
  ALLOWLIST.find(e => e.route === scope && e.role === control.role && e.name === control.name);

test.describe('every control on every route does something', () => {
  test('App.tsx declares routes, and the crawl knows how to visit each', () => {
    expect(routes.length, 'routes found in src/App.tsx').toBeGreaterThan(0);
    // Every <Route> has a path the pattern above could read (no index or pathless routes).
    expect(APP_TSX.match(/<Route\b/g)?.length, '<Route> elements in src/App.tsx').toBe(routes.length);
    const unknown = routes.filter(r => !(r in ROUTE_URLS));
    expect(unknown, 'routes to add to ROUTE_URLS in crawl/every-control.spec.ts').toEqual([]);
  });

  test('every allowlist entry names a crawled route and gives a reason', () => {
    for (const entry of ALLOWLIST) {
      expect([...routes, LAYOUT], JSON.stringify(entry)).toContain(entry.route);
      expect(entry.reason.trim().length, JSON.stringify(entry)).toBeGreaterThan(10);
    }
  });

  for (const route of routes.filter(r => r in ROUTE_URLS)) {
    test(`controls on ${route}`, async ({ page, request }) => {
      test.slow();
      const crawlsLayout = route === '/';
      const failures: string[] = [];
      // Entries that excused a control this run; the rest are stale.
      const used = new Set<AllowlistEntry>();
      let clicked = 0;

      // A page that throws has broken something, whether or not a control still reacts.
      page.on('pageerror', err => failures.push(`${page.url()}: uncaught error: ${err.message}`));

      for (const url of await ROUTE_URLS[route](request)) {
        await gotoSettled(page, url);
        const controls = (await listControls(page)).filter(c => crawlsLayout || !c.inLayout);
        if (controls.length === 0) failures.push(`${url}: no visible controls (did it render?)`);

        let pageIsFresh = true;
        for (const control of controls) {
          if (control.isCurrent) continue;
          const entry = allowlisted(control.inLayout ? LAYOUT : route, control);
          const fail = (detail: string) => {
            if (entry) used.add(entry);
            else failures.push(`${url}: ${describeControl(control)}: ${detail}`);
          };

          // Links off the site (another origin, mailto:) are not followed, so the crawl stays off
          // the network: they only need an http(s) or mailto address, not a working one.
          if (control.href && new URL(control.href).origin !== new URL(url, page.url()).origin) {
            if (!/^(https?|mailto):/.test(control.href)) fail('goes nowhere');
            continue;
          }

          // Each click starts from the page as it loads, so one control's effect (an open menu,
          // a changed filter) can't hide or fake another's.
          if (!pageIsFresh) await gotoSettled(page, url);
          const outcome = await clickAndObserve(page, control, isAppPath);
          clicked++;
          pageIsFresh = !outcome.disturbed;
          if (!outcome.worked) fail(outcome.detail);
        }
      }

      test.info().annotations.push({ type: 'controls clicked', description: String(clicked) });

      // An entry that excused nothing (its control works now, or is gone) is reported, not
      // failed: the PR that fixes the control shouldn't have to know about this list. Delete the
      // entry when you see this.
      const scopes = crawlsLayout ? [route, LAYOUT] : [route];
      for (const stale of ALLOWLIST.filter(e => scopes.includes(e.route) && !used.has(e))) {
        test.info().annotations.push({
          type: 'stale allowlist entry',
          description: `${stale.route} ${stale.role} "${stale.name}": delete it from crawl/allowlist.ts`,
        });
      }

      expect(
        failures,
        'controls that did nothing or went nowhere: make them work, remove them, or allowlist them with a reason in crawl/allowlist.ts'
      ).toEqual([]);
    });
  }
});
