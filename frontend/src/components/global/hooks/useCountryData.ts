/**
 * UseCountryData Hook.
 *
 * Indexes one indicator's country values by ISO numeric code, which is how world-atlas
 * identifies its features, and derives the color scale, value range and date range the map
 * and its legend show.
 */

import { useCallback, useMemo } from 'react';
import { scaleSequential } from 'd3-scale';
import {
  interpolateViridis,
  interpolateBlues,
  interpolateReds,
  interpolateGreens,
} from 'd3-scale-chromatic';
import type { MapCountryValue } from './useWorldMapData';

export const COLOR_SCHEMES = {
  viridis: interpolateViridis,
  blues: interpolateBlues,
  reds: interpolateReds,
  greens: interpolateGreens,
};

export type ColorScheme = keyof typeof COLOR_SCHEMES;

/**
 * Index and scale one indicator's country values for the map.
 * @param countries - The countries with a value.
 * @param colorScheme - A key of `COLOR_SCHEMES`; unknown keys use viridis.
 * @param drawableIds - ISO numeric codes the outline has a shape for; when given, values without
 * a shape are left out of the scale, ranges and count. Undefined counts every code as drawable.
 * @returns The values by code, the color scale, value and date ranges, and which countries can
 * and can't be drawn.
 */
export const useCountryData = (
  countries: MapCountryValue[],
  colorScheme: string = 'viridis',
  drawableIds?: ReadonlySet<number>
) => {
  // Values that can be drawn: a finite number and an ISO numeric code with a shape to join on.
  // Kosovo has no ISO numeric code, so it shows as no data; there is deliberately no join by
  // name. A few microstates (Tuvalu, Gibraltar) have a code but no shape at 1:50m.
  const isDrawable = useCallback(
    (country: MapCountryValue) =>
      country.isoNumeric !== null &&
      Number.isFinite(country.numericValue) &&
      (!drawableIds || drawableIds.has(country.isoNumeric)),
    [drawableIds]
  );

  const countriesWithData = useMemo(() => countries.filter(isDrawable), [countries, isDrawable]);

  const countriesWithoutData = useMemo(
    () => countries.filter(country => !isDrawable(country)),
    [countries, isDrawable]
  );

  const valuesByIsoNumeric = useMemo(() => {
    const byId = new Map<number, MapCountryValue>();
    countriesWithData.forEach(country => byId.set(country.isoNumeric as number, country));
    return byId;
  }, [countriesWithData]);

  const dataRange = useMemo(() => {
    if (countriesWithData.length === 0) {
      return { min: 0, max: 1 };
    }
    const values = countriesWithData.map(country => country.numericValue);
    return { min: Math.min(...values), max: Math.max(...values) };
  }, [countriesWithData]);

  // Earliest and latest value dates: `latest` gives each country its own date.
  const dateRange = useMemo(() => {
    if (countriesWithData.length === 0) return null;
    const dates = countriesWithData.map(country => country.date).sort();
    return { earliest: dates[0], latest: dates[dates.length - 1] };
  }, [countriesWithData]);

  const colorScale = useMemo(() => {
    const interpolator = COLOR_SCHEMES[colorScheme as ColorScheme] || interpolateViridis;
    return scaleSequential(interpolator).domain([dataRange.min, dataRange.max]);
  }, [dataRange, colorScheme]);

  return {
    valuesByIsoNumeric,
    colorScale,
    dataRange,
    dateRange,
    countriesWithData,
    countriesWithoutData,
  };
};
