/**
 * REQUIREMENT: The DataSource type names exactly the fields GET_DATA_SOURCES selects.
 * PURPOSE: Static check for the ECO-403 bug class: the type (and the page) used snake_case
 * names the GraphQL API never returns, so every value silently fell back to a default.
 */

import { QUERIES } from '../../utils/graphql';
import type { DataSource } from '../useSeriesData';

// `satisfies` makes this list fail to compile when DataSource gains or loses a field.
const DATA_SOURCE_FIELDS = {
  id: true,
  name: true,
  description: true,
  baseUrl: true,
  apiKeyRequired: true,
  rateLimitPerMinute: true,
  seriesCount: true,
  createdAt: true,
  updatedAt: true,
  notice: true,
} satisfies Record<keyof DataSource, true>;

describe('DataSource', () => {
  it('has exactly the fields GET_DATA_SOURCES selects', () => {
    // Collapse nested selections such as `notice { ... }` to their field name first.
    const query = QUERIES.GET_DATA_SOURCES.replace(/(\w+)\s*{[^{}]*}(?=[^{}]*})/g, '$1');
    const body = query.match(/dataSources\s*{([^}]*)}/)?.[1] ?? '';
    const selected = body.split(/\s+/).filter(Boolean).sort();

    expect(selected).toEqual(Object.keys(DATA_SOURCE_FIELDS).sort());
  });
});
