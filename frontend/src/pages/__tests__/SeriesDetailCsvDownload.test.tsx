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

const hooks = vi.hoisted(() => ({ isPlaceholderData: false, transformation: 'YEAR_OVER_YEAR' }));

vi.mock('../../hooks/useSeriesData', () => ({
  useSeriesDetail: () => ({
    isPending: false,
    isError: false,
    data: {
      id: SERIES_ID,
      externalId: 'CPIAUCSL',
      title: 'Consumer Price Index, "All Items"',
      description: null,
      source: {
        name: 'Bureau of Labor Statistics (BLS)',
        notice: {
          citation: 'Source: U.S. Bureau of Labor Statistics',
          notice: 'BLS.gov cannot vouch for the data.',
          termsUrl: 'https://www.bls.gov/developers/termsOfService.htm',
          retrievalDateRequired: true,
        },
      },
      frequency: 'Monthly',
      units: 'Index 1982-1984=100',
      seasonalAdjustment: null,
      startDate: '2024-01-01',
      endDate: '2024-03-01',
      lastUpdated: '2026-10-29T08:15:00Z',
      isActive: true,
    },
  }),
  useSeriesData: () => ({
    isPending: false,
    isError: false,
    isPlaceholderData: hooks.isPlaceholderData,
    dataUpdatedAt: RETRIEVED_AT,
    data: {
      transformation: hooks.transformation,
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
    <>
      <button onClick={() => onDateRangeChange?.({ start: new Date(2024, 1, 1), end: null })}>
        Narrow range
      </button>
      <button
        onClick={() =>
          onDateRangeChange?.({ start: new Date(2024, 1, 1), end: new Date(2024, 1, 1) })
        }
      >
        Range without values
      </button>
    </>
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
    hooks.isPlaceholderData = false;
    hooks.transformation = 'YEAR_OVER_YEAR';
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
      'Source,Bureau of Labor Statistics (BLS)',
      'Units,%',
      'Transformation,Year-over-Year % Change',
      'Retrieved at,2026-09-26T12:00:00.000Z',
      'Derived values,"Values show the Year-over-Year % Change, computed by EconGraph from data published by Bureau of Labor Statistics (BLS); they are not figures published by Bureau of Labor Statistics (BLS)."',
      'Citation,Source: U.S. Bureau of Labor Statistics',
      'Notice,BLS.gov cannot vouch for the data.',
      'Retrieved from source,2026-10-29',
      'Terms of use,https://www.bls.gov/developers/termsOfService.htm',
      'date,value',
      '2024-01-01,3.1',
      '2024-02-01,',
      '2024-03-01,3.5',
      '',
    ]);
  });

  it("shows the source's notice, retrieval date and the derived-values statement", () => {
    renderPage();
    const notice = screen.getByTestId('source-notice');
    expect(notice).toHaveTextContent('Source: U.S. Bureau of Labor Statistics');
    expect(notice).toHaveTextContent(
      'BLS.gov cannot vouch for the data. Data retrieved from Bureau of Labor Statistics (BLS) on 2026-10-29.'
    );
    expect(notice).toHaveTextContent('computed by EconGraph');
    expect(screen.getByRole('link', { name: 'Terms of use' })).toHaveAttribute(
      'href',
      'https://www.bls.gov/developers/termsOfService.htm'
    );
  });

  it('writes only the dates inside the chart range', async () => {
    renderPage();
    fireEvent.click(screen.getByRole('button', { name: 'Narrow range' }));
    fireEvent.click(screen.getByRole('button', { name: /download csv/i }));

    const rows = (await readBlob(blobs[0])).split('\r\n');
    rows.splice(0, rows.indexOf('date,value') + 1);
    expect(rows).toEqual(['2024-02-01,', '2024-03-01,3.5', '']);
  });

  it('is disabled when the chart range holds no values, since the file would be empty', () => {
    renderPage();
    expect(screen.getByRole('button', { name: /download csv/i })).toBeEnabled();
    fireEvent.click(screen.getByRole('button', { name: 'Range without values' }));
    expect(screen.getByRole('button', { name: /download csv/i })).toBeDisabled();
  });

  it('exports log difference with dimensionless units, not the series percent unit', async () => {
    hooks.transformation = 'LOG_DIFFERENCE';
    renderPage();
    fireEvent.click(screen.getByRole('button', { name: /download csv/i }));

    const text = await readBlob(blobs[0]);
    expect(text.split('\r\n')).toContain('Units,');
    expect(text.split('\r\n')).not.toContain('Units,%');
  });
});

describe('SeriesDetail CSV download while a transformation loads', () => {
  afterEach(() => {
    hooks.isPlaceholderData = false;
  });

  it('is disabled, so old values never get the new label', () => {
    hooks.isPlaceholderData = true;
    render(
      <TestProviders>
        <SeriesDetail />
      </TestProviders>
    );
    expect(screen.getByRole('button', { name: /download csv/i })).toBeDisabled();
  });
});
