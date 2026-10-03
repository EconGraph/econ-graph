/**
 * The world outline is bundled from the pinned `world-atlas` package, so the
 * map must render its countries without any network request.
 */

import React from 'react';
import { render, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import InteractiveWorldMap from '../InteractiveWorldMap';
import { loadWorldAtlas } from '../worldAtlas';

// The shared setup file stubs d3-geo and d3-zoom; this test needs real paths.
// These unmocks stay local only while test files run isolated. vitest.config.ts
// asks for isolate: false under test.poolOptions, which Vitest 5 ignores.
vi.unmock('d3-geo');
vi.unmock('d3-zoom');

// The 1:50m Natural Earth file has 241 country geometries.
const COUNTRY_COUNT = 241;
// The first dynamic import of the atlas chunk can be slow on a cold CI runner.
const WAIT = { timeout: 5000 };

describe('bundled world atlas', () => {
  let fetchSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    fetchSpy = vi
      .spyOn(globalThis, 'fetch')
      .mockRejectedValue(new Error('the world map must not use the network'));
  });

  it('loads the countries topology from the package', async () => {
    const atlas = await loadWorldAtlas();

    expect(atlas.type).toBe('Topology');
    expect(atlas.objects.countries.geometries).toHaveLength(COUNTRY_COUNT);
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it('renders InteractiveWorldMap countries without fetching', async () => {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={queryClient}>
        <InteractiveWorldMap
          valuesByIsoNumeric={new Map()}
          colorScale={() => '#000'}
          width={800}
          height={400}
        />
      </QueryClientProvider>
    );

    await waitFor(() => {
      expect(document.querySelectorAll('path.country')).toHaveLength(COUNTRY_COUNT);
    }, WAIT);
    expect(document.querySelector('path.border')).not.toBeNull();
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it('labels countries by name (world-atlas uses lowercase properties.name)', async () => {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={queryClient}>
        <InteractiveWorldMap
          valuesByIsoNumeric={new Map()}
          colorScale={() => '#000'}
          width={800}
          height={400}
          showLabels
        />
      </QueryClientProvider>
    );

    await waitFor(() => {
      const labels = document.querySelectorAll('text.country-label');
      expect(labels).toHaveLength(COUNTRY_COUNT);
      expect(Array.from(labels).some(label => label.textContent?.trim())).toBe(true);
    }, WAIT);
  });
});
