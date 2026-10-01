/**
 * UseCountryData Hook Tests.
 *
 * Ported from PR #154 to the real-data model: values keyed by ISO numeric code, value and date
 * ranges, and color schemes.
 */

import { renderHook } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { useCountryData } from '../useCountryData';
import type { MapCountryValue } from '../useWorldMapData';

const country = (
  key: string,
  isoNumeric: number | null,
  value: string,
  date = '2023-01-01'
): MapCountryValue => ({
  key,
  name: key,
  isoNumeric,
  seriesId: `s-${key}`,
  date,
  value,
  numericValue: Number(value),
});

const SAMPLE = [
  country('USA', 840, '82769.4'),
  country('CHN', 156, '12614.1', '2022-01-01'),
  country('DEU', 276, '54343.2', '2021-01-01'),
];

describe('useCountryData', () => {
  describe('data processing', () => {
    it('indexes countries by ISO numeric code', () => {
      const { result } = renderHook(() => useCountryData(SAMPLE));

      expect(result.current.valuesByIsoNumeric.size).toBe(3);
      expect(result.current.valuesByIsoNumeric.get(840)?.key).toBe('USA');
      expect(result.current.countriesWithData).toHaveLength(3);
      expect(result.current.countriesWithoutData).toEqual([]);
    });

    it('handles an empty array', () => {
      const { result } = renderHook(() => useCountryData([]));

      expect(result.current.valuesByIsoNumeric.size).toBe(0);
      expect(result.current.countriesWithData).toEqual([]);
      expect(result.current.countriesWithoutData).toEqual([]);
      expect(result.current.dateRange).toBeNull();
    });

    it('cannot place a country without an ISO numeric code, such as Kosovo', () => {
      const kosovo = country('XKX', null, '5943.1');
      const { result } = renderHook(() => useCountryData([...SAMPLE, kosovo]));

      expect(result.current.countriesWithData).toHaveLength(3);
      expect(result.current.countriesWithoutData).toEqual([kosovo]);
      expect([...result.current.valuesByIsoNumeric.values()]).not.toContain(kosovo);
    });

    it('treats a value that is not a number as no data', () => {
      const broken = { ...country('FRA', 250, '1'), numericValue: NaN };
      const { result } = renderHook(() => useCountryData([broken]));

      expect(result.current.countriesWithData).toEqual([]);
      expect(result.current.countriesWithoutData).toEqual([broken]);
    });
  });

  describe('color scaling', () => {
    it('spans the values', () => {
      const { result } = renderHook(() => useCountryData(SAMPLE));

      expect(result.current.dataRange).toEqual({ min: 12614.1, max: 82769.4 });
      expect(result.current.colorScale.domain()).toEqual([12614.1, 82769.4]);
      expect(result.current.colorScale(12614.1)).not.toBe(result.current.colorScale(82769.4));
    });

    it('falls back to 0 to 1 without values', () => {
      const { result } = renderHook(() => useCountryData([]));

      expect(result.current.dataRange).toEqual({ min: 0, max: 1 });
    });

    it('uses each color scheme, and viridis for an unknown one', () => {
      const colorOf = (scheme: string) =>
        renderHook(() => useCountryData(SAMPLE, scheme)).result.current.colorScale(82769.4);

      const colors = ['viridis', 'blues', 'reds', 'greens'].map(colorOf);
      expect(new Set(colors).size).toBe(4);
      expect(colorOf('invalid')).toBe(colorOf('viridis'));
    });
  });

  describe('dates', () => {
    it('gives the earliest and latest value dates', () => {
      const { result } = renderHook(() => useCountryData(SAMPLE));

      expect(result.current.dateRange).toEqual({ earliest: '2021-01-01', latest: '2023-01-01' });
    });
  });

  describe('performance', () => {
    it('handles a thousand countries', () => {
      const many = Array.from({ length: 1000 }, (_, i) => country(`C${i}`, i, String(i)));
      const { result } = renderHook(() => useCountryData(many));

      expect(result.current.valuesByIsoNumeric.size).toBe(1000);
      expect(result.current.dataRange).toEqual({ min: 0, max: 999 });
    });
  });
});
