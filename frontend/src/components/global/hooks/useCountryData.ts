/**
 * UseCountryData Hook.
 *
 * Indexes one indicator's country values by ISO numeric code, which is how world-atlas
 * identifies its features, and derives the color scale, value range and date range the map
 * and its legend show.
 */

import { useCallback, useMemo } from 'react';
import { scaleSequential, scaleSequentialLog } from 'd3-scale';
import {
  interpolateViridis,
  interpolateBlues,
  interpolateReds,
  interpolateGreens,
} from 'd3-scale-chromatic';
import type { MapCountryValue } from './useWorldMapData';

export const COLOR_SCHEMES = {
  viridis: interpolateViridis,
  // Keep valid low values visibly separate from the pale ocean and No data fill.
  blues: (t: number) => interpolateBlues(0.25 + 0.75 * t),
  reds: (t: number) => interpolateReds(0.25 + 0.75 * t),
  greens: (t: number) => interpolateGreens(0.25 + 0.75 * t),
};

export type ColorScheme = keyof typeof COLOR_SCHEMES;
export type ColorScaleMode = 'linear' | 'log';

/**
 * Index and scale one indicator's country values for the map.
 * @param countries - The countries with a value.
 * @param colorScheme - A key of `COLOR_SCHEMES`; unknown keys use viridis.
 * @param drawableIds - ISO numeric codes the outline has a shape for; when given, values without
 * a shape are left out of the scale, ranges and count. Undefined counts every code as drawable.
 * @param requestedScale - Requested transform; log falls back to linear without positive data.
 * @returns The values by code, the color scale, value and date ranges, and which countries can
 * and can't be drawn.
 */
export const useCountryData = (
  countries: MapCountryValue[],
  colorScheme: string = 'viridis',
  drawableIds?: ReadonlySet<number>,
  requestedScale: ColorScaleMode = 'linear'
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

  const logUnavailableReason =
    countriesWithData.length === 0
      ? 'Log scaling needs at least one positive country value.'
      : dataRange.min <= 0
        ? 'Log scaling is unavailable because this indicator has zero or negative country values.'
        : null;
  const scaleMode: ColorScaleMode =
    requestedScale === 'log' && !logUnavailableReason ? 'log' : 'linear';

  // Shared by the legend and its ticks: equal positions represent equal ratios in log mode.
  const valueAtPosition = useCallback(
    (position: number) => {
      if (position === 0) return dataRange.min;
      if (position === 1) return dataRange.max;
      return scaleMode === 'log'
        ? Math.exp(
            Math.log(dataRange.min) + position * (Math.log(dataRange.max) - Math.log(dataRange.min))
          )
        : dataRange.min + position * (dataRange.max - dataRange.min);
    },
    [dataRange, scaleMode]
  );

  const colorScale = useMemo(() => {
    const interpolator = COLOR_SCHEMES[colorScheme as ColorScheme] || interpolateViridis;
    const scale =
      scaleMode === 'log' ? scaleSequentialLog(interpolator) : scaleSequential(interpolator);
    return scale.domain([dataRange.min, dataRange.max]).clamp(true);
  }, [dataRange, colorScheme, scaleMode]);

  return {
    valuesByIsoNumeric,
    colorScale,
    scaleMode,
    logUnavailableReason,
    valueAtPosition,
    dataRange,
    dateRange,
    countriesWithData,
    countriesWithoutData,
  };
};
