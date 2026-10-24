// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import type { Page, Request } from '@playwright/test';

/**
 * Deterministic fault injection for the failure specs (ECO-264): the browser's requests to the
 * real backend fail on cue, so a spec can check what the page shows while they fail and that it
 * recovers once they stop failing. Nothing is mocked once a fault is healed; every request that
 * doesn't match goes to the seeded backend as usual.
 */

/** How a matched request fails. */
export type FaultKind =
  /** HTTP 503 with a plain-text body, like a proxy in front of a restarting backend. */
  | 'http-503'
  /** HTTP 200 with a GraphQL `errors` array and no data, like a resolver error. */
  | 'graphql-error'
  /** The connection is dropped with no response, like a network failure. */
  | 'network';

/** A fault on the page's requests. */
export interface Fault {
  /** Requests failed so far. */
  readonly failed: () => number;
  /** Stops failing requests; later ones reach the backend. */
  readonly heal: () => void;
}

/**
 * The operation name in a GraphQL request, read from its query text: the frontend sends
 * `query Name(` / `mutation Name(` without a separate `operationName`.
 * @param request - A request to /graphql.
 * @returns The operation name, or null for anything else.
 */
export function graphqlOperation(request: Request): string | null {
  if (request.method() !== 'POST' || !new URL(request.url()).pathname.endsWith('/graphql')) {
    return null;
  }
  let body: { query?: unknown; operationName?: unknown } | null;
  try {
    body = request.postDataJSON();
  } catch {
    return null;
  }
  if (typeof body?.operationName === 'string') return body.operationName;
  const match =
    typeof body?.query === 'string' ? /\b(?:query|mutation)\s+(\w+)/.exec(body.query) : null;
  return match ? match[1] : null;
}

/**
 * Fails the page's GraphQL requests for `operation` until the fault is healed.
 * @param page - The test's page.
 * @param operation - The operation name, e.g. `SearchSeries`.
 * @param kind - How each matched request fails.
 * @param when - Optional extra condition on the request's variables.
 * @returns The fault.
 */
export async function failGraphQL(
  page: Page,
  operation: string,
  kind: FaultKind = 'http-503',
  when: (variables: Record<string, unknown>) => boolean = () => true
): Promise<Fault> {
  let failed = 0;
  let healed = false;
  await page.route('**/graphql', async route => {
    const request = route.request();
    const variables = (request.postDataJSON()?.variables ?? {}) as Record<string, unknown>;
    if (healed || graphqlOperation(request) !== operation || !when(variables)) {
      await route.fallback();
      return;
    }
    failed += 1;
    if (kind === 'network') {
      await route.abort('connectionreset');
    } else if (kind === 'graphql-error') {
      await route.fulfill({
        json: { data: null, errors: [{ message: 'Injected failure (ECO-264 failure spec)' }] },
      });
    } else {
      await route.fulfill({ status: 503, contentType: 'text/plain', body: 'Service Unavailable' });
    }
  });
  return { failed: () => failed, heal: () => (healed = true) };
}

/**
 * Counts the page's GraphQL requests for `operation` that reached the backend and succeeded.
 * @param page - The test's page.
 * @param operation - The operation name.
 * @returns A function reading the count.
 */
export function countGraphQL(page: Page, operation: string): () => number {
  let count = 0;
  page.on('requestfinished', async request => {
    if (graphqlOperation(request) !== operation) return;
    const response = await request.response();
    if (response?.ok()) count += 1;
  });
  return () => count;
}

/**
 * The time a query's error takes to show: the app retries a failed query three times, waiting
 * 1, 2 and 4 seconds (src/index.tsx), so allow for that and the requests themselves.
 */
export const QUERY_ERROR_TIMEOUT = 20_000;
