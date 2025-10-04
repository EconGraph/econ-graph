/**
 * UseCountryData Hook Tests.
 *
 * Test suite for the useCountryData hook covering data processing,
 * color scaling, and statistical calculations.
 */

import { renderHook } from '@testing-library/react';
import { useCountryData } from '../useCountryData';
import { sampleCountryData } from '../../../../data/sampleCountryData';
import { CountryData } from '../../../../types/globalAnalysis';

// Mock D3 modules
vi.mock('d3-scale', () => ({
  scaleSequential: vi.fn(() => ({
    domain: vi.fn().mockReturnThis(),
  })),
}));

vi.mock('d3-scale-chromatic', () => ({
  interpolateViridis: vi.fn(),
  interpolateBlues: vi.fn(),
  interpolateReds: vi.fn(),
  interpolateGreens: vi.fn(),
}));

vi.mock('d3', () => ({
  mean: vi.fn((values: any[]) => values.reduce((a: any, b: any) => a + b, 0) / values.length),
  median: vi.fn((values: any[]) => {
    const sorted = [...values].sort((a: any, b: any) => a - b);
    const mid = Math.floor(sorted.length / 2);
    return sorted.length % 2 === 0 ? (sorted[mid - 1] + sorted[mid]) / 2 : sorted[mid];
  }),
  variance: vi.fn((values: any[]) => {
    const mean = values.reduce((a: any, b: any) => a + b, 0) / values.length;
    return values.reduce((sum: any, value: any) => sum + Math.pow(value - mean, 2), 0) / values.length;
  }),
}));

describe('useCountryData', () => {
  describe('Data Processing', () => {
    it('should process country data correctly', () => {
      const { result } = renderHook(() =>
        useCountryData(sampleCountryData, 'GDP', 'viridis')
      );

      expect(result.current.processedData).toBeDefined();
      expect(result.current.processedData).toHaveLength(sampleCountryData.length);
    });

    it('should handle empty data array', () => {
      const { result } = renderHook(() =>
        useCountryData([], 'GDP', 'viridis')
      );

      expect(result.current.processedData).toEqual([]);
      expect(result.current.availableIndicators).toEqual([]);
      expect(result.current.countriesWithData).toEqual([]);
      expect(result.current.countriesWithoutData).toEqual([]);
    });

    it('should identify countries with and without data', () => {
      const { result } = renderHook(() =>
        useCountryData(sampleCountryData, 'GDP', 'viridis')
      );

      expect(result.current.countriesWithData.length).toBeGreaterThan(0);
      expect(result.current.countriesWithoutData.length).toBe(0); // All sample countries have GDP data
    });

    it('should handle countries without selected indicator', () => {
      const dataWithoutIndicator: CountryData[] = [
        {
          id: 'test',
          name: 'Test Country',
          isoAlpha2: 'TC',
          isoAlpha3: 'TST',
          latitude: 0,
          longitude: 0,
          economicIndicators: [
            {
              name: 'Inflation',
              value: 2.5,
              unit: '%',
              year: 2023,
              source: 'Test',
            },
          ],
        },
      ];

      const { result } = renderHook(() =>
        useCountryData(dataWithoutIndicator, 'GDP', 'viridis')
      );

      expect(result.current.countriesWithData).toEqual([]);
      expect(result.current.countriesWithoutData).toHaveLength(1);
    });
  });

  describe('Color Scaling', () => {
    it('should create color scale for valid data', () => {
      const { result } = renderHook(() =>
        useCountryData(sampleCountryData, 'GDP', 'viridis')
      );

      expect(result.current.colorScale).toBeDefined();
      expect(result.current.dataRange).toBeDefined();
      expect(result.current.dataRange.min).toBeGreaterThan(0);
      expect(result.current.dataRange.max).toBeGreaterThan(result.current.dataRange.min);
    });

    it('should handle different color schemes', () => {
      const schemes = ['viridis', 'blues', 'reds', 'greens'] as const;

      schemes.forEach(scheme => {
        const { result } = renderHook(() =>
          useCountryData(sampleCountryData, 'GDP', scheme)
        );

        expect(result.current.colorScale).toBeDefined();
      });
    });

    it('should handle invalid color scheme', () => {
      const { result } = renderHook(() =>
        useCountryData(sampleCountryData, 'GDP', 'invalid' as any)
      );

      expect(result.current.colorScale).toBeDefined();
    });

    it('should handle data with no valid values', () => {
      const dataWithInvalidValues: CountryData[] = [
        {
          id: 'test',
          name: 'Test Country',
          isoAlpha2: 'TC',
          isoAlpha3: 'TST',
          latitude: 0,
          longitude: 0,
          economicIndicators: [
            {
              name: 'GDP',
              value: NaN,
              unit: 'USD',
              year: 2023,
              source: 'Test',
            },
          ],
        },
      ];

      const { result } = renderHook(() =>
        useCountryData(dataWithInvalidValues, 'GDP', 'viridis')
      );

      expect(result.current.dataRange).toEqual({ min: 0, max: 1 });
    });
  });

  describe('Available Indicators', () => {
    it('should extract available indicators from data', () => {
      const { result } = renderHook(() =>
        useCountryData(sampleCountryData, 'GDP', 'viridis')
      );

      expect(result.current.availableIndicators).toContain('GDP');
      expect(result.current.availableIndicators).toContain('Inflation');
      expect(result.current.availableIndicators).toContain('Unemployment');
    });

    it('should handle data with no indicators', () => {
      const dataWithoutIndicators: CountryData[] = [
        {
          id: 'test',
          name: 'Test Country',
          isoAlpha2: 'TC',
          isoAlpha3: 'TST',
          latitude: 0,
          longitude: 0,
        },
      ];

      const { result } = renderHook(() =>
        useCountryData(dataWithoutIndicators, 'GDP', 'viridis')
      );

      expect(result.current.availableIndicators).toEqual([]);
    });
  });

  describe('Statistical Calculations', () => {
    it('should calculate statistics for valid data', () => {
      const { result } = renderHook(() =>
        useCountryData(sampleCountryData, 'GDP', 'viridis')
      );

      expect(result.current.statistics.count).toBeGreaterThan(0);
      expect(result.current.statistics.mean).toBeGreaterThan(0);
      expect(result.current.statistics.median).toBeGreaterThan(0);
      expect(result.current.statistics.min).toBeGreaterThan(0);
      expect(result.current.statistics.max).toBeGreaterThan(result.current.statistics.min);
      expect(result.current.statistics.stdDev).toBeGreaterThanOrEqual(0);
    });

    it('should handle empty statistics', () => {
      const { result } = renderHook(() =>
        useCountryData([], 'GDP', 'viridis')
      );

      expect(result.current.statistics.count).toBe(0);
      expect(result.current.statistics.mean).toBe(0);
      expect(result.current.statistics.median).toBe(0);
      expect(result.current.statistics.min).toBe(0);
      expect(result.current.statistics.max).toBe(0);
      expect(result.current.statistics.stdDev).toBe(0);
    });
  });

  describe('Country Filtering Functions', () => {
    it('should get top countries by value', () => {
      const { result } = renderHook(() =>
        useCountryData(sampleCountryData, 'GDP', 'viridis')
      );

      const topCountries = result.current.getTopCountries(5);
      expect(topCountries).toHaveLength(5);
      expect(topCountries[0].colorValue).toBeGreaterThanOrEqual(topCountries[1].colorValue || 0);
    });

    it('should get bottom countries by value', () => {
      const { result } = renderHook(() =>
        useCountryData(sampleCountryData, 'GDP', 'viridis')
      );

      const bottomCountries = result.current.getBottomCountries(5);
      expect(bottomCountries).toHaveLength(5);
      expect(bottomCountries[0].colorValue).toBeLessThanOrEqual(bottomCountries[1].colorValue || 0);
    });

    it('should filter countries by value range', () => {
      const { result } = renderHook(() =>
        useCountryData(sampleCountryData, 'GDP', 'viridis')
      );

      const filteredCountries = result.current.filterCountriesByRange(1000000, 10000000);
      expect(filteredCountries).toBeDefined();
      filteredCountries.forEach(country => {
        expect(country.colorValue).toBeGreaterThanOrEqual(1000000);
        expect(country.colorValue).toBeLessThanOrEqual(10000000);
      });
    });

    it('should get countries by region', () => {
      const { result } = renderHook(() =>
        useCountryData(sampleCountryData, 'GDP', 'viridis')
      );

      const asiaCountries = result.current.getCountriesByRegion('Asia');
      expect(asiaCountries).toBeDefined();
      asiaCountries.forEach(country => {
        expect(country.region).toBe('Asia');
      });
    });

    it('should get countries by subregion', () => {
      const { result } = renderHook(() =>
        useCountryData(sampleCountryData, 'GDP', 'viridis')
      );

      const easternAsiaCountries = result.current.getCountriesBySubregion('Eastern Asia');
      expect(easternAsiaCountries).toBeDefined();
      easternAsiaCountries.forEach(country => {
        expect(country.subregion).toBe('Eastern Asia');
      });
    });
  });

  describe('Region and Subregion Lists', () => {
    it('should extract unique regions', () => {
      const { result } = renderHook(() =>
        useCountryData(sampleCountryData, 'GDP', 'viridis')
      );

      expect(result.current.regions).toContain('North America');
      expect(result.current.regions).toContain('Asia');
      expect(result.current.regions).toContain('Europe');
      expect(result.current.regions).toContain('South America');
    });

    it('should extract unique subregions', () => {
      const { result } = renderHook(() =>
        useCountryData(sampleCountryData, 'GDP', 'viridis')
      );

      expect(result.current.subregions).toContain('Northern America');
      expect(result.current.subregions).toContain('Eastern Asia');
      expect(result.current.subregions).toContain('Western Europe');
      expect(result.current.subregions).toContain('South America');
    });

    it('should handle data with missing region information', () => {
      const dataWithoutRegions: CountryData[] = [
        {
          id: 'test',
          name: 'Test Country',
          isoAlpha2: 'TC',
          isoAlpha3: 'TST',
          latitude: 0,
          longitude: 0,
        },
      ];

      const { result } = renderHook(() =>
        useCountryData(dataWithoutRegions, 'GDP', 'viridis')
      );

      expect(result.current.regions).toEqual([]);
      expect(result.current.subregions).toEqual([]);
    });
  });

  describe('Edge Cases', () => {
    it('should handle null/undefined values in indicators', () => {
      const dataWithNullValues: CountryData[] = [
        {
          id: 'test',
          name: 'Test Country',
          isoAlpha2: 'TC',
          isoAlpha3: 'TST',
          latitude: 0,
          longitude: 0,
          economicIndicators: [
            {
              name: 'GDP',
              value: null as any,
              unit: 'USD',
              year: 2023,
              source: 'Test',
            },
          ],
        },
      ];

      const { result } = renderHook(() =>
        useCountryData(dataWithNullValues, 'GDP', 'viridis')
      );

      expect(result.current.countriesWithData).toHaveLength(1);
      expect(result.current.countriesWithoutData).toEqual([]);
    });

    it('should handle missing economic indicators', () => {
      const dataWithoutIndicators: CountryData[] = [
        {
          id: 'test',
          name: 'Test Country',
          isoAlpha2: 'TC',
          isoAlpha3: 'TST',
          latitude: 0,
          longitude: 0,
          economicIndicators: undefined,
        },
      ];

      const { result } = renderHook(() =>
        useCountryData(dataWithoutIndicators, 'GDP', 'viridis')
      );

      expect(result.current.countriesWithData).toEqual([]);
      expect(result.current.countriesWithoutData).toHaveLength(1);
    });

    it('should handle indicator name case sensitivity', () => {
      const dataWithCaseVariations: CountryData[] = [
        {
          id: 'test',
          name: 'Test Country',
          isoAlpha2: 'TC',
          isoAlpha3: 'TST',
          latitude: 0,
          longitude: 0,
          economicIndicators: [
            {
              name: 'gdp', // lowercase
              value: 1000000,
              unit: 'USD',
              year: 2023,
              source: 'Test',
            },
          ],
        },
      ];

      const { result } = renderHook(() =>
        useCountryData(dataWithCaseVariations, 'GDP', 'viridis')
      );

      // Should not find the indicator due to case mismatch
      expect(result.current.countriesWithData).toEqual([]);
      expect(result.current.countriesWithoutData).toHaveLength(1);
    });
  });

  describe('Performance', () => {
    it('should handle large datasets efficiently', () => {
      const largeDataset: CountryData[] = Array.from({ length: 1000 }, (_, i) => ({
        id: `country-${i}`,
        name: `Country ${i}`,
        isoAlpha2: `C${i.toString().padStart(2, '0')}`,
        isoAlpha3: `CT${i.toString().padStart(2, '0')}`,
        latitude: Math.random() * 180 - 90,
        longitude: Math.random() * 360 - 180,
        region: `Region ${i % 10}`,
        subregion: `Subregion ${i % 20}`,
        economicIndicators: [
          {
            name: 'GDP',
            value: Math.random() * 1000000,
            unit: 'USD',
            year: 2023,
            source: 'Test',
          },
        ],
      }));

      const { result } = renderHook(() =>
        useCountryData(largeDataset, 'GDP', 'viridis')
      );

      expect(result.current.processedData).toHaveLength(1000);
      expect(result.current.availableIndicators).toContain('GDP');
    });
  });
});
