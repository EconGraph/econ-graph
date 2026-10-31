// LatestValueCard: how an entry's displayScale divides the stored value.

import React from 'react';
import { render, screen } from '@testing-library/react';
import { vi } from 'vitest';
import { TestProviders } from '../../../test-utils/test-providers';
import { LatestValueCard } from '../LatestValueCard';
import { DISPLAY_SCALES, type DashboardSeriesEntry } from '../../../config/dashboardSeries';
import { executeGraphQL } from '../../../utils/graphql';

vi.mock('../../../utils/graphql', async () => {
  const actual =
    await vi.importActual<typeof import('../../../utils/graphql')>('../../../utils/graphql');
  return { ...actual, executeGraphQL: vi.fn() };
});

const mockedExecuteGraphQL = vi.mocked(executeGraphQL);

const entry: DashboardSeriesEntry = {
  id: 'scaled',
  label: 'Scaled Series',
  sourceName: 'Bureau of Economic Analysis (BEA)',
  sourceLabel: 'BEA',
  externalId: 'bea_nipa/T10105.1.Q',
  fractionDigits: 3,
  units: 'Scaled units',
};

/**
 * Render the card with a stored latest value.
 * @param value - The stored value.
 * @param overrides - Entry fields to change.
 */
function renderCard(value: string, overrides: Partial<DashboardSeriesEntry>) {
  mockedExecuteGraphQL.mockResolvedValue({
    data: {
      seriesByExternalId: {
        id: '0192f0a1-0000-7000-8000-0000000000aa',
        units: 'Current Dollars',
        latestObservation: { date: '2024-10-01', value, revisionDate: '2024-10-01' },
      },
    },
  } as Awaited<ReturnType<typeof executeGraphQL>>);
  render(
    <TestProviders>
      <LatestValueCard entry={{ ...entry, ...overrides }} />
    </TestProviders>
  );
}

describe('LatestValueCard displayScale', () => {
  beforeEach(() => {
    mockedExecuteGraphQL.mockReset();
  });

  test('names exactly the powers of ten it divides by', () => {
    expect(DISPLAY_SCALES).toEqual({ thousands: 3, millions: 6, billions: 9, trillions: 12 });
  });

  // 28,624,702,123,456 dollars in each scale, to three decimals.
  test.each([
    [undefined, '28,624,702,123,456.000'],
    ['thousands', '28,624,702,123.456'],
    ['millions', '28,624,702.123'],
    ['billions', '28,624.702'],
    ['trillions', '28.625'],
  ] as const)('shows the value in %s as %s', async (displayScale, shown) => {
    renderCard('28624702123456', displayScale ? { displayScale } : {});
    expect(await screen.findByText(shown)).toBeInTheDocument();
    expect(screen.getByText('Scaled units')).toBeInTheDocument();
  });
});
