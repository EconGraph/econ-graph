/**
 * The series page's Download CSV button writes the points the chart shows: the shown
 * transformation, only dates inside the chart's range, and the metadata header rows.
 */

import React from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { TestProviders } from '../../test-utils/test-providers';
import type { SeriesChartProps } from '../../components/charts/SeriesChart';
import SeriesDetail from '../SeriesDetail';

const SERIES_ID = '11111111-2222-3333-4444-555555555555';
const RETRIEVED_AT = Date.UTC(2026, 8, 26, 12, 0, 0);

vi.mock('react-router-dom', () => ({
  useParams: () => ({ id: SERIES_ID }),
  useNavigate: () => vi.fn(),
}));

const hooks = vi.hoisted(() => ({ isPreviousData: false }));

vi.mock('../../hooks/useSeriesData', () => ({
  useSeriesDetail: () => ({
    isLoading: false,
    isError: false,
    data: {
      id: SERIES_ID,
      externalId: 'CPIAUCSL',
      title: 'Consumer Price Index, "All Items"',
      description: null,
      source: { name: 'FRED' },
      frequency: 'Monthly',
      units: 'Index 1982-1984=100',
      seasonalAdjustment: null,
      startDate: '2024-01-01',
      endDate: '2024-03-01',
      lastUpdated: null,
      isActive: true,
    },
  }),
  useSeriesData: () => ({
    isLoading: false,
    isError: false,
    isPreviousData: hooks.isPreviousData,
    dataUpdatedAt: RETRIEVED_AT,
    data: {
      transformation: 'YEAR_OVER_YEAR',
      points: [
        { date: '2024-03-01', value: 3.5, revisionDate: '2024-04-10', isOriginalRelease: true },
        { date: '2024-01-01', value: 3.1, revisionDate: '2024-02-13', isOriginalRelease: true },
        { date: '2024-02-01', value: null, revisionDate: '2024-03-12', isOriginalRelease: true },
      ],
    },
  }),
}));

// The real chart needs a canvas; this stand-in lets the test narrow the range like a user.
vi.mock('../../components/charts/SeriesChart', () => ({
  default: ({ onDateRangeChange }: SeriesChartProps) => (
    <button onClick={() => onDateRangeChange?.({ start: new Date(2024, 1, 1), end: null })}>
      Narrow range
    </button>
  ),
}));

const readBlob = (blob: Blob): Promise<string> =>
  new Promise((resolve, reject) => {
    const reader = new window.FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = () => reject(reader.error);
    reader.readAsText(blob);
  });

describe('SeriesDetail CSV download', () => {
  const originalCreate = URL.createObjectURL;
  const originalRevoke = URL.revokeObjectURL;
  let blobs: Blob[];
  let clicked: InstanceType<typeof window.HTMLAnchorElement>[];

  beforeEach(() => {
    hooks.isPreviousData = false;
    blobs = [];
    clicked = [];
    URL.createObjectURL = vi.fn((blob: Blob) => {
      blobs.push(blob);
      return 'blob:series-csv';
    });
    URL.revokeObjectURL = vi.fn();
    vi.spyOn(window.HTMLAnchorElement.prototype, 'click').mockImplementation(function (
      this: InstanceType<typeof window.HTMLAnchorElement>
    ) {
      clicked.push(this);
    });
  });

  afterEach(() => {
    URL.createObjectURL = originalCreate;
    URL.revokeObjectURL = originalRevoke;
    vi.restoreAllMocks();
  });

  const renderPage = () =>
    render(
      <TestProviders>
        <SeriesDetail />
      </TestProviders>
    );

  it('downloads every shown point with the header rows', async () => {
    renderPage();
    fireEvent.click(screen.getByRole('button', { name: /download csv/i }));

    expect(clicked).toHaveLength(1);
    expect(clicked[0].download).toBe('CPIAUCSL-year_over_year.csv');
    expect(blobs[0].type).toBe('text/csv;charset=utf-8');
    // readAsText drops the byte-order mark; seriesCsv.test.ts checks it is written.
    const text = await readBlob(blobs[0]);
    expect(text.split('\r\n')).toEqual([
      'Title,"Consumer Price Index, ""All Items"""',
      'Source,FRED',
      'Units,%',
      'Transformation,Year-over-Year % Change',
      'Retrieved at,2026-09-26T12:00:00.000Z',
      'date,value',
      '2024-01-01,3.1',
      '2024-02-01,',
      '2024-03-01,3.5',
      '',
    ]);
  });

  it('writes only the dates inside the chart range', async () => {
    renderPage();
    fireEvent.click(screen.getByRole('button', { name: 'Narrow range' }));
    fireEvent.click(screen.getByRole('button', { name: /download csv/i }));

    const rows = (await readBlob(blobs[0])).split('\r\n').slice(6);
    expect(rows).toEqual(['2024-02-01,', '2024-03-01,3.5', '']);
  });
});

describe('SeriesDetail CSV download while a transformation loads', () => {
  afterEach(() => {
    hooks.isPreviousData = false;
  });

  it('is disabled, so old values never get the new label', () => {
    hooks.isPreviousData = true;
    render(
      <TestProviders>
        <SeriesDetail />
      </TestProviders>
    );
    expect(screen.getByRole('button', { name: /download csv/i })).toBeDisabled();
  });
});
