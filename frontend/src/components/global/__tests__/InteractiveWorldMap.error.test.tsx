/**
 * A failed atlas load, such as a stale chunk after a deploy, shows an error in
 * place of the map rather than crashing the page.
 */

import React from 'react';
import { render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { describe, expect, it, vi } from 'vitest';
import InteractiveWorldMap from '../InteractiveWorldMap';

// The shared setup file stubs d3-geo and d3-zoom; the map hook needs the real ones.
vi.unmock('d3-geo');
vi.unmock('d3-zoom');

vi.mock('../worldAtlas', () => ({
  loadWorldAtlas: vi.fn(() => Promise.reject(new Error('chunk failed to load'))),
}));

describe('InteractiveWorldMap atlas load failure', () => {
  it('shows an error instead of the map', async () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false, retryDelay: 0 } },
      logger: { log: () => {}, warn: () => {}, error: () => {} },
    });
    render(
      <QueryClientProvider client={queryClient}>
        <InteractiveWorldMap
          data={[]}
          selectedIndicator='gdp'
          timeRange={{ start: new Date('2020-01-01'), end: new Date('2024-01-01') }}
          onCountryClick={vi.fn()}
          onCountryHover={vi.fn()}
          mapView={{ scale: 1, translation: [0, 0], rotation: [0, 0, 0] }}
          onMapViewChange={vi.fn()}
          width={800}
          height={400}
        />
      </QueryClientProvider>
    );

    // The query retries once (with no delay here) before reporting the error.
    expect(
      await screen.findByText('Failed to load world map data')
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Reload' })).toBeInTheDocument();
  });
});
