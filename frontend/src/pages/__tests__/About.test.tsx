/**
 * REQUIREMENT: The About page shows the version that was built, not a hard-coded one.
 * PURPOSE: Regression test for ECO-404 (the 4.0.0 build said "Version 3.7.2").
 */

import React from 'react';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { screen } from '@testing-library/react';
import { vi } from 'vitest';
import { renderWithProviders } from '../../test-utils/test-providers';
import About from '../About';

const sources = vi.hoisted(() => ({
  data: [
    {
      id: '1',
      name: 'Bureau of Economic Analysis (BEA)',
      notice: {
        citation: 'Source: U.S. Bureau of Economic Analysis',
        notice:
          'This product uses the Bureau of Economic Analysis (BEA) Data API but is not endorsed or certified by BEA.',
        termsUrl: 'https://apps.bea.gov/API/_pdf/bea_api_tos.pdf',
        retrievalDateRequired: false,
      },
    },
    { id: '2', name: 'International Monetary Fund (IMF)', notice: null },
  ] as unknown[] | undefined,
}));

vi.mock('../../hooks/useSeriesData', () => ({
  useDataSources: () => ({ data: sources.data }),
}));

const packageVersion: string = JSON.parse(
  readFileSync(resolve(import.meta.dirname, '../../../package.json'), 'utf8')
).version;

describe('About', () => {
  it("shows package.json's version", () => {
    renderWithProviders(<About />);

    expect(screen.getByText(`Version ${packageVersion}`)).toBeInTheDocument();
  });

  it("collects every source's notice, skipping sources without one", () => {
    renderWithProviders(<About />);

    const section = screen.getByRole('region', { name: 'Source notices' });
    expect(section).toHaveTextContent('Bureau of Economic Analysis (BEA)');
    expect(section).toHaveTextContent('Source: U.S. Bureau of Economic Analysis');
    expect(section).toHaveTextContent(
      'This product uses the Bureau of Economic Analysis (BEA) Data API but is not endorsed or certified by BEA.'
    );
    expect(section).not.toHaveTextContent('International Monetary Fund (IMF)');
  });

  it('shows no notices section while the source list is unavailable', () => {
    sources.data = undefined;
    renderWithProviders(<About />);

    expect(screen.queryByRole('region', { name: 'Source notices' })).not.toBeInTheDocument();
  });

  it('has no version number written into the page source', () => {
    const source = readFileSync(resolve(import.meta.dirname, '../About.tsx'), 'utf8');

    expect(source).not.toMatch(/Version\s+\d+\.\d+/);
  });
});
