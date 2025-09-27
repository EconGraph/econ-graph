/**
 * UseCountryData Hook.
 *
 * Custom hook for managing country data processing, color scaling,
 * and economic indicator calculations for the world map.
 */

import { useMemo, useCallback } from 'react';
import * as d3 from 'd3';
import { scaleSequential } from 'd3-scale';
import {
  interpolateViridis,
  interpolateBlues,
  interpolateReds,
  interpolateGreens,
} from 'd3-scale-chromatic';
import { CountryData } from '../../../types/globalAnalysis';

export interface ProcessedCountryData extends CountryData {
  /** Color value for this country based on selected indicator */
  colorValue?: number;
  /** Normalized value (0-1) for this country */
  normalizedValue?: number;
  /** Whether this country has data for the selected indicator */
  hasData: boolean;
}

const COLOR_SCHEMES = {
  viridis: interpolateViridis,
  blues: interpolateBlues,
  reds: interpolateReds,
  greens: interpolateGreens,
};

export const useCountryData = (
  countries: CountryData[],
  selectedIndicator: string,
  colorScheme = 'viridis'
) => {
  // Process country data
  const processedData = useMemo(() => {
    return countries.map(country => {
      const indicator = country.economicIndicators?.find(ind => ind.name === selectedIndicator);

      const hasData = !!indicator;
      const colorValue = indicator?.value;

      // Normalize value to 0-1 range for color scaling
      const normalizedValue = colorValue !== undefined ? colorValue : 0;

      return {
        ...country,
        colorValue,
        normalizedValue,
        hasData,
      };
    });
  }, [countries, selectedIndicator]);

  // Calculate data range for color scaling
  const dataRange = useMemo(() => {
    const values = processedData
      .filter(country => country.hasData)
      .map(country => country.colorValue!)
      .filter(value => value !== undefined && !isNaN(value));

    if (values.length === 0) {
      return { min: 0, max: 1 };
    }

    return {
      min: Math.min(...values),
      max: Math.max(...values),
    };
  }, [processedData]);

  // Create color scale
  const colorScale = useMemo(() => {
    const interpolator =
      COLOR_SCHEMES[colorScheme as keyof typeof COLOR_SCHEMES] || interpolateViridis;

    return scaleSequential(interpolator).domain([dataRange.min, dataRange.max]);
  }, [dataRange, colorScheme]);

  // Get available indicators
  const availableIndicators = useMemo(() => {
    const indicators = new Set<string>();

    countries.forEach(country => {
      country.economicIndicators?.forEach(indicator => {
        indicators.add(indicator.name);
      });
    });

    return Array.from(indicators).sort();
  }, [countries]);

  // Get countries with data for selected indicator
  const countriesWithData = useMemo(() => {
    return processedData.filter(country => country.hasData);
  }, [processedData]);

  // Get countries without data for selected indicator
  const countriesWithoutData = useMemo(() => {
    return processedData.filter(country => !country.hasData);
  }, [processedData]);

  // Calculate statistics for selected indicator
  const statistics = useMemo(() => {
    const values = countriesWithData.map(country => country.colorValue!);

    if (values.length === 0) {
      return {
        count: 0,
        mean: 0,
        median: 0,
        min: 0,
        max: 0,
        stdDev: 0,
      };
    }

    // const sortedValues = [...values].sort((a, b) => a - b);
    const mean = d3.mean(values) || 0;
    const median = d3.median(values) || 0;
    const min = Math.min(...values);
    const max = Math.max(...values);
    const variance = d3.variance(values) || 0;
    const stdDev = Math.sqrt(variance);

    return {
      count: values.length,
      mean,
      median,
      min,
      max,
      stdDev,
    };
  }, [countriesWithData]);

  // Get top countries by indicator value
  const getTopCountries = useCallback(
    (limit = 10) => {
      return countriesWithData
        .sort((a, b) => (b.colorValue || 0) - (a.colorValue || 0))
        .slice(0, limit);
    },
    [countriesWithData]
  );

  // Get bottom countries by indicator value
  const getBottomCountries = useCallback(
    (limit = 10) => {
      return countriesWithData
        .sort((a, b) => (a.colorValue || 0) - (b.colorValue || 0))
        .slice(0, limit);
    },
    [countriesWithData]
  );

  // Filter countries by value range
  const filterCountriesByRange = useCallback(
    (minValue: number, maxValue: number) => {
      return countriesWithData.filter(country => {
        const value = country.colorValue || 0;
        return value >= minValue && value <= maxValue;
      });
    },
    [countriesWithData]
  );

  // Get countries by region
  const getCountriesByRegion = useCallback(
    (region: string) => {
      return processedData.filter(country => country.region === region);
    },
    [processedData]
  );

  // Get countries by subregion
  const getCountriesBySubregion = useCallback(
    (subregion: string) => {
      return processedData.filter(country => country.subregion === subregion);
    },
    [processedData]
  );

  // Get unique regions
  const regions = useMemo(() => {
    const regionSet = new Set<string>();
    processedData.forEach(country => {
      if (country.region) {
        regionSet.add(country.region);
      }
    });
    return Array.from(regionSet).sort();
  }, [processedData]);

  // Get unique subregions
  const subregions = useMemo(() => {
    const subregionSet = new Set<string>();
    processedData.forEach(country => {
      if (country.subregion) {
        subregionSet.add(country.subregion);
      }
    });
    return Array.from(subregionSet).sort();
  }, [processedData]);

  return {
    processedData,
    colorScale,
    dataRange,
    availableIndicators,
    countriesWithData,
    countriesWithoutData,
    statistics,
    getTopCountries,
    getBottomCountries,
    filterCountriesByRange,
    getCountriesByRegion,
    getCountriesBySubregion,
    regions,
    subregions,
  };
};
