// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

/**
 * Series the release stack is seeded with, from
 * backend/crates/econ-graph-crawler/tests/fixtures/e2e-seed.json. Specs assert on these rather
 * than on literals of their own, so a fixture change shows up in one place.
 */
export const SEEDED = {
  fredGdp: { source: 'FRED', externalId: 'GDP', title: 'Gross Domestic Product' },
  blsCpi: {
    source: 'BLS',
    externalId: 'CUUR0000SA0',
    title: 'All items in U.S. city average, all urban consumers, not seasonally adjusted',
  },
  censusEstablishments: {
    source: 'CENSUS',
    externalId: 'CENSUS_BDS_ESTAB_us',
    title: 'CENSUS_BDS_ESTAB_us',
  },
  fhfaHpi: { source: 'FHFA', externalId: 'USHPI', title: 'U.S. House Price Index' },
  wdiGdpPerCapitaUsa: {
    source: 'WORLD_BANK',
    externalId: 'wdi/NY.GDP.PCAP.CD.USA',
    title: 'GDP per capita (current US$): United States',
  },
} as const;

/**
 * World Development Indicators seeded for the world map (MAP-7) and REL-4: three indicators for
 * the countries below, from backend/crates/econ-graph-crawler/tests/fixtures/world_bank/. Series
 * ids are `wdi/{indicator}.{area}` and titles `{indicator name}: {country name}`. `latest`
 * is each country's latest non-null value (India has no 2023 inflation value in the fixture).
 */
export const SEEDED_WDI = {
  source: 'WORLD_BANK',
  dataset: 'wdi',
  countries: {
    USA: 'United States',
    CHN: 'China',
    DEU: 'Germany',
    JPN: 'Japan',
    IND: 'India',
    BRA: 'Brazil',
  },
  indicators: [
    {
      code: 'NY.GDP.PCAP.CD',
      name: 'GDP per capita (current US$)',
      unit: 'current US$',
      latest: {
        USA: { date: '2023-01-01', value: 82769.4 },
        CHN: { date: '2023-01-01', value: 12614.1 },
        DEU: { date: '2023-01-01', value: 54343.2 },
        JPN: { date: '2023-01-01', value: 33834.4 },
        IND: { date: '2023-01-01', value: 2480.8 },
        BRA: { date: '2023-01-01', value: 10043.6 },
      },
    },
    {
      code: 'SP.POP.TOTL',
      name: 'Population, total',
      unit: 'persons',
      latest: {
        USA: { date: '2023-01-01', value: 334914895 },
        CHN: { date: '2023-01-01', value: 1410710000 },
        DEU: { date: '2023-01-01', value: 84482267 },
        JPN: { date: '2023-01-01', value: 124516650 },
        IND: { date: '2023-01-01', value: 1438069596 },
        BRA: { date: '2023-01-01', value: 216422446 },
      },
    },
    {
      code: 'FP.CPI.TOTL.ZG',
      name: 'Inflation, consumer prices (annual %)',
      unit: '% change',
      latest: {
        USA: { date: '2023-01-01', value: 4.1 },
        CHN: { date: '2023-01-01', value: 0.2 },
        DEU: { date: '2023-01-01', value: 5.9 },
        JPN: { date: '2023-01-01', value: 3.3 },
        IND: { date: '2022-01-01', value: 6.7 },
        BRA: { date: '2023-01-01', value: 4.6 },
      },
    },
  ],
} as const;

