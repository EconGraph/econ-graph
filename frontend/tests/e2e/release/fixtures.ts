// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

/**
 * Series the release stack is seeded with, from
 * backend/crates/econ-graph-crawler/tests/fixtures/e2e-seed.json. Specs assert on these rather
 * than on literals of their own, so a fixture change shows up in one place. `sourceName` is the
 * source's `dataSources` name, which the pages show.
 */
export const SEEDED = {
  fredGdp: {
    source: 'FRED',
    sourceName: 'Federal Reserve Economic Data (FRED)',
    externalId: 'GDP',
    title: 'Gross Domestic Product',
    units: 'Billions of Dollars',
    /** Its newest observation as the series page's Recent Data table shows it. */
    latestShown: { date: 'Apr 1, 2026', value: '32,101.60' },
    /**
     * Deployed mode: the most days its newest observation may lag today. GDP is quarterly and
     * dated at the quarter's start; BEA's advance estimate comes about a month after the quarter
     * ends, so the newest point is up to seven months old just before a release. The 2025
     * shutdown delayed a release until the point was about 265 days old, hence the slack.
     */
    liveMaxAgeDays: 300,
    /**
     * Every transformation the series page offers, in the order the journey applies them, with the newest value it shows for it. Null:
     * the page says there aren't enough observations (a quarterly series has no monthly change).
     * The fixture has five quarters, 2025-04-01 to 2026-04-01, with 2025-10-01 missing. Its first
     * point is exactly a year before the last, so year-over-year and change since the first
     * observation show the same value; the column header tells them apart.
     */
    transformations: [
      {
        label: 'Year-over-Year',
        description: 'Year-over-Year % Change',
        latestShown: { date: 'Apr 1, 2026', value: '5.30' },
      },
      {
        label: 'Quarter-over-Quarter',
        description: 'Quarter-over-Quarter % Change',
        latestShown: { date: 'Apr 1, 2026', value: '1.20' },
      },
      { label: 'Month-over-Month', description: 'Month-over-Month % Change', latestShown: null },
      {
        label: 'Change since first available value',
        description: '% Change since First Available Value',
        latestShown: { date: 'Apr 1, 2026', value: '5.30' },
      },
      // Last, back to levels.
      { label: 'None', description: '', latestShown: { date: 'Apr 1, 2026', value: '32,101.60' } },
    ],
  },
  blsCpi: {
    source: 'BLS',
    sourceName: 'Bureau of Labor Statistics (BLS)',
    externalId: 'CUUR0000SA0',
    title:
      'Consumer Price Index for All Urban Consumers: All Items in U.S. City Average (Not Seasonally Adjusted)',
    units: 'Index 1982-1984=100',
    // cpi_monthly.json's newest two points: March 2024 (312.332) over February 2024 (310.326).
    transformations: [
      {
        label: 'Month-over-Month',
        description: 'Month-over-Month % Change',
        latestShown: { date: 'Mar 1, 2024', value: '0.65' },
      },
    ],
  },
  censusEstablishments: {
    source: 'CENSUS',
    sourceName: 'U.S. Census Bureau',
    externalId: 'bds/national..ESTAB',
    title: 'Number of establishments - United States',
    units: 'Count',
  },
  beaGdp: {
    source: 'BEA',
    sourceName: 'Bureau of Economic Analysis (BEA)',
    // BEA's own ids: NIPA table T10105, line 1 (GDP), quarterly.
    externalId: 'bea_nipa/T10105.1.Q',
    title: 'Gross domestic product (Gross Domestic Product, quarterly)',
    // UNIT_MULT 6 is applied, so values are in dollars, not millions.
    units: 'Current Dollars',
  },
  fhfaHpi: {
    source: 'FHFA',
    sourceName: 'Federal Housing Finance Agency (FHFA)',
    externalId: 'fhfa_hpi/traditional.purchase-only.monthly.usa-or-census-division.USA.sa',
    title: 'United States House Price Index: Purchase-Only, Monthly, Seasonally Adjusted',
    units: 'Index (January 1991 = 100)',
  },
  wdiGdpPerCapitaUsa: {
    source: 'WORLD_BANK',
    sourceName: 'World Bank Open Data',
    externalId: 'wdi/NY.GDP.PCAP.CD.USA',
    title: 'GDP per capita (current US$): United States',
    units: 'current US$',
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

/** One train 1 source's series as the six-source sweep (sources/six-source-sweep.spec.ts) checks it. */
export interface SweptSeries {
  source: string;
  sourceName: string;
  externalId: string;
  title: string;
  units: string;
  /** Its newest observation as the series page's Recent observations table shows it. */
  latestShown: { date: string; value: string };
  /** Data rows in its CSV at the chart's default range: every point, missing ones included. */
  csvRows: number;
  /**
   * Every transformation the page offers but None, by its label, with the newest value it shows
   * in fixture mode; null where the page says there aren't enough observations to compute it.
   */
  transformations: Record<string, { date: string; value: string } | null>;
  /**
   * Deployed mode: transformations the series' frequency can never have (month-over-month on a
   * quarterly series). Every other one must compute on a live series' full history.
   */
  liveNotEnough: readonly string[];
  /**
   * Deployed mode: the most days its newest observation may lag today, from the source's
   * publication cadence (REL-QA check 3). Observations are dated at the period's start.
   */
  liveMaxAgeDays: number;
}

/**
 * A point as the series page's Recent observations table shows it.
 * @param date - The date cell.
 * @param value - The value cell.
 * @returns The cells.
 */
const shown = (date: string, value: string) => ({ date, value });

/** Neither quarter-over-quarter nor month-over-month exists for an annual series. */
const ANNUAL_NOT_ENOUGH = ['Quarter-over-Quarter', 'Month-over-Month'] as const;

/**
 * One series per train 1 source, for the six-source sweep. Values are what the series page shows
 * for the seeded fixtures, rounded to 2 decimal places; the comments say where each comes from.
 */
export const SWEPT_SOURCES: readonly SweptSeries[] = [
  {
    ...SEEDED.fredGdp,
    // fred/observations_gdp_e2e.json: 2025-04-01 to 2026-04-01 quarterly, 2025-10-01 missing.
    csvRows: 5,
    transformations: {
      'Year-over-Year': shown('Apr 1, 2026', '5.30'),
      'Quarter-over-Quarter': shown('Apr 1, 2026', '1.20'),
      'Month-over-Month': null,
      'Change since first available value': shown('Apr 1, 2026', '5.30'),
      // ln(32101.6 / 31722.514)
      'Log difference': shown('Apr 1, 2026', '0.01'),
    },
    liveNotEnough: ['Month-over-Month'],
  },
  {
    ...SEEDED.blsCpi,
    // bls/cpi_monthly.json: 2023-11 (missing) to 2024-03, monthly.
    latestShown: { date: 'Mar 1, 2024', value: '312.33' },
    csvRows: 5,
    transformations: {
      // Five months: no year-ago point.
      'Year-over-Year': null,
      // 312.332 / 306.746 (December)
      'Quarter-over-Quarter': shown('Mar 1, 2024', '1.82'),
      'Month-over-Month': shown('Mar 1, 2024', '0.65'),
      // The first observation (2023-11) is missing, so the base is the next usable value,
      // December's 306.746: 312.332 / 306.746, the same comparison as QoQ here.
      'Change since first available value': shown('Mar 1, 2024', '1.82'),
      'Log difference': shown('Mar 1, 2024', '0.01'),
    },
    liveNotEnough: [],
    // Monthly, released about two weeks after the month ends; slack for a shutdown delay.
    liveMaxAgeDays: 120,
  },
  {
    ...SEEDED.censusEstablishments,
    // census/bds_estab_us.json: 2019 to 2022, annual, 2021 missing.
    latestShown: { date: 'Jan 1, 2022', value: '7,324,017.00' },
    csvRows: 4,
    transformations: {
      // 2022's year-ago point (2021) is missing, so the newest is 2020 over 2019.
      'Year-over-Year': shown('Jan 1, 2020', '1.03'),
      'Quarter-over-Quarter': null,
      'Month-over-Month': null,
      // 7324017 / 7106316 (2019)
      'Change since first available value': shown('Jan 1, 2022', '3.06'),
      'Log difference': shown('Jan 1, 2020', '0.01'),
    },
    liveNotEnough: ANNUAL_NOT_ENOUGH,
    // Annual, released about 21 months after the year ends (BDS 2022 came out in September
    // 2024), so just before a release the newest point is about three years and eight months old.
    liveMaxAgeDays: 1460,
  },
  {
    ...SEEDED.beaGdp,
    // bea/nipa_t10105_q.json: 2024Q1 to 2024Q4, in millions (UNIT_MULT 6). The recorded values
    // barely move, so every change rounds to 0.00.
    latestShown: { date: 'Oct 1, 2024', value: '28,624,702,000,000.00' },
    csvRows: 4,
    transformations: {
      // Four quarters: no year-ago point.
      'Year-over-Year': null,
      'Quarter-over-Quarter': shown('Oct 1, 2024', '0.00'),
      'Month-over-Month': null,
      'Change since first available value': shown('Oct 1, 2024', '0.00'),
      'Log difference': shown('Oct 1, 2024', '0.00'),
    },
    liveNotEnough: ['Month-over-Month'],
    // Quarterly GDP, the same cadence as FRED's copy (SEEDED.fredGdp.liveMaxAgeDays).
    liveMaxAgeDays: SEEDED.fredGdp.liveMaxAgeDays,
  },
  {
    ...SEEDED.fhfaHpi,
    // fhfa/hpi_master.csv's seasonally adjusted column: 2024-10 to 2025-03, monthly.
    latestShown: { date: 'Mar 1, 2025', value: '209.93' },
    csvRows: 6,
    transformations: {
      // Six months: no year-ago point.
      'Year-over-Year': null,
      // 209.93 / 206.24 (December)
      'Quarter-over-Quarter': shown('Mar 1, 2025', '1.79'),
      // 209.93 / 208.70
      'Month-over-Month': shown('Mar 1, 2025', '0.59'),
      // 209.93 / 203.78 (October)
      'Change since first available value': shown('Mar 1, 2025', '3.02'),
      'Log difference': shown('Mar 1, 2025', '0.01'),
    },
    liveNotEnough: [],
    // Monthly, released about two months after the month ends.
    liveMaxAgeDays: 150,
  },
  {
    ...SEEDED.wdiGdpPerCapitaUsa,
    // world_bank/gdp_per_capita.json's United States rows: 2021 to 2023, annual.
    latestShown: { date: 'Jan 1, 2023', value: '82,769.40' },
    csvRows: 3,
    transformations: {
      // 82769.4 / 77860.9
      'Year-over-Year': shown('Jan 1, 2023', '6.30'),
      'Quarter-over-Quarter': null,
      'Month-over-Month': null,
      // 82769.4 / 71055.9 (2021)
      'Change since first available value': shown('Jan 1, 2023', '16.48'),
      // ln(82769.4 / 77860.9)
      'Log difference': shown('Jan 1, 2023', '0.06'),
    },
    liveNotEnough: ANNUAL_NOT_ENOUGH,
    // Annual, published around July of the next year, so just before that the newest point is
    // two and a half years old.
    liveMaxAgeDays: 1000,
  },
];
