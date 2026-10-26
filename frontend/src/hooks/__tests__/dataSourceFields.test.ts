/**
 * REQUIREMENT: The DataSource type names exactly the fields GET_DATA_SOURCES selects.
 * PURPOSE: Static check for the ECO-403 bug class: the type (and the page) used snake_case
 * names the GraphQL API never returns, so every value silently fell back to a default.
 */

import { Kind, parse } from 'graphql';
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
  lifecycleCounts: true,
  createdAt: true,
  updatedAt: true,
} satisfies Record<keyof DataSource, true>;

describe('DataSource', () => {
  it('has exactly the fields GET_DATA_SOURCES selects', () => {
    const document = parse(QUERIES.GET_DATA_SOURCES);
    const operation = document.definitions.find(
      definition => definition.kind === Kind.OPERATION_DEFINITION
    );
    if (operation?.kind !== Kind.OPERATION_DEFINITION) throw new Error('missing operation');
    const source = operation.selectionSet.selections.find(
      selection => selection.kind === Kind.FIELD && selection.name.value === 'dataSources'
    );
    if (source?.kind !== Kind.FIELD) throw new Error('missing dataSources');
    const selected = source.selectionSet?.selections
      .filter(selection => selection.kind === Kind.FIELD)
      .map(selection => selection.name.value)
      .sort();

    expect(selected).toEqual(Object.keys(DATA_SOURCE_FIELDS).sort());
  });
});
