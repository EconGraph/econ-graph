/**
 * REQUIREMENT: Show source identity and series availability without crawler administration
 * details or invented operational statistics.
 * PURPOSE: Regression tests for ECO-403: the page read snake_case fields the GraphQL API never
 * returns (so every count was 0 and every rate limit 60/min), and showed hard-coded uptime,
 * health, last-crawl and crawl-schedule figures.
 */

import React from 'react';
import { screen, within } from '@testing-library/react';
import { vi } from 'vitest';
import { renderWithProviders } from '../../test-utils/test-providers';
import DataSources from '../DataSources';
import { useDataSources, type DataSource } from '../../hooks/useSeriesData';

vi.mock('../../hooks/useSeriesData', () => ({ useDataSources: vi.fn() }));

// The shape GET_DATA_SOURCES returns: GraphQL's camelCase field names.
const sources: DataSource[] = [
  {
    id: '0199a0e0-0000-7000-8000-000000000001',
    name: 'Federal Reserve Economic Data (FRED)',
    description: 'Economic data from the Federal Reserve Bank of St. Louis',
    baseUrl: 'https://api.stlouisfed.org/fred',
    apiKeyRequired: true,
    rateLimitPerMinute: 120,
    seriesCount: 1234,
    createdAt: '2026-10-01T00:00:00Z',
    updatedAt: '2026-10-01T00:00:00Z',
  },
  {
    id: '0199a0e0-0000-7000-8000-000000000002',
    name: 'World Bank Open Data',
    description: '',
    baseUrl: 'https://api.worldbank.org/v2',
    apiKeyRequired: false,
    rateLimitPerMinute: 1000,
    seriesCount: 18,
    createdAt: '2026-10-01T00:00:00Z',
    updatedAt: '2026-10-01T00:00:00Z',
  },
];

const mockSources = (data: DataSource[] | undefined, extra: object = {}) =>
  vi.mocked(useDataSources).mockReturnValue({
    data,
    isPending: false,
    error: null,
    ...extra,
  } as unknown as ReturnType<typeof useDataSources>);

describe('DataSources', () => {
  it("shows each source's series count and their total", () => {
    mockSources(sources);
    renderWithProviders(<DataSources />);

    const fred = within(screen.getByRole('region', { name: sources[0].name }));
    expect(fred.getByText('1,234')).toBeInTheDocument();

    const worldBank = within(screen.getByRole('region', { name: sources[1].name }));
    expect(worldBank.getByText('18')).toBeInTheDocument();
    expect(worldBank.getByText('Economic data source')).toBeInTheDocument();

    expect(screen.getByText('1,252')).toBeInTheDocument();
    expect(screen.getByText('Total Series')).toBeInTheDocument();
  });

  it('links each source to the explorer filtered to it', () => {
    mockSources(sources);
    renderWithProviders(<DataSources />);

    const fred = within(screen.getByRole('region', { name: sources[0].name }));
    expect(fred.getByRole('link', { name: /browse series/i })).toHaveAttribute(
      'href',
      `/explore?source=${encodeURIComponent(sources[0].id)}`
    );
  });

  it('hides crawler administration details even when the API supplies them', () => {
    mockSources(sources);
    renderWithProviders(<DataSources />);

    for (const administrative of [/rate limit/i, /\/min/, /api key/i, /api\.stlouisfed\.org/i]) {
      expect(screen.queryByText(administrative)).not.toBeInTheDocument();
    }
  });

  it('shows no uptime, health, last-crawl or schedule figures the API does not report', () => {
    mockSources(sources);
    renderWithProviders(<DataSources />);

    for (const invented of [
      /uptime/i,
      /healthy/i,
      /health:/i,
      /last crawl/i,
      /crawl schedule/i,
      /next scheduled/i,
      /98\.5%/,
    ]) {
      expect(screen.queryByText(invented)).not.toBeInTheDocument();
    }
  });

  it('shows an error when the sources cannot be loaded', () => {
    mockSources(undefined, { error: new Error('boom') });
    renderWithProviders(<DataSources />);

    expect(screen.getByRole('alert')).toHaveTextContent('Could not load data sources: boom');
  });

  it('shows a spinner while loading', () => {
    mockSources(undefined, { isPending: true });
    renderWithProviders(<DataSources />);

    expect(screen.getByLabelText('Loading data sources')).toBeInTheDocument();
  });
});
