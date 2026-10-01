/**
 * The map's dataset and cross-section parsing.
 */

import { describe, expect, it } from 'vitest';
import { pickMapDataset, toCountryValues } from '../useWorldMapData';

const indicator = {
  name: 'indicator',
  label: 'Indicator',
  codelist: null,
  codes: [{ code: 'NY.GDP.PCAP.CD', label: 'GDP per capita (current US$)', unit: null }],
};
const area = { name: 'area', label: 'Area', codelist: 'countries', codes: [] };

describe('pickMapDataset', () => {
  it('finds the WDI dataset and its area and indicator dimensions', () => {
    const picked = pickMapDataset([
      { id: '1', code: 'bds', name: 'BDS', dimensions: [] },
      { id: '2', code: 'wdi', name: 'WDI', dimensions: [indicator, area] },
    ]);

    expect(picked).toEqual({
      id: '2',
      name: 'WDI',
      areaDimension: 'area',
      indicatorDimension: 'indicator',
      indicators: indicator.codes,
    });
  });

  it('is null when the dataset is missing or not shaped as expected', () => {
    expect(pickMapDataset([])).toBeNull();
    expect(pickMapDataset([{ id: '2', code: 'wdi', name: 'WDI', dimensions: [indicator] }])).toBeNull();
    expect(
      pickMapDataset([
        { id: '2', code: 'wdi', name: 'WDI', dimensions: [indicator, indicator, area] },
      ])
    ).toBeNull();
  });
});

describe('toCountryValues', () => {
  const entry = (overrides: object) => ({
    key: 'USA',
    seriesId: 's',
    date: '2023-01-01',
    value: '82769.4',
    area: { name: 'United States', iso3: 'USA', isoNumeric: 840, kind: 'COUNTRY' as const },
    ...overrides,
  });

  it('keeps countries with a value', () => {
    expect(toCountryValues([entry({})])).toEqual([
      {
        key: 'USA',
        name: 'United States',
        isoNumeric: 840,
        seriesId: 's',
        date: '2023-01-01',
        value: '82769.4',
        numericValue: 82769.4,
      },
    ]);
  });

  it('leaves out aggregates, keys without an area and missing values', () => {
    expect(
      toCountryValues([
        entry({ key: 'WLD', area: { name: 'World', iso3: null, isoNumeric: null, kind: 'AGGREGATE' } }),
        entry({ area: null }),
        entry({ value: null, date: null }),
        entry({ value: 'not a number' }),
      ])
    ).toEqual([]);
  });

  it('keeps Kosovo, which the map then cannot place', () => {
    const kosovo = entry({
      key: 'XKX',
      area: { name: 'Kosovo', iso3: null, isoNumeric: null, kind: 'COUNTRY' },
    });

    expect(toCountryValues([kosovo])).toHaveLength(1);
  });
});
