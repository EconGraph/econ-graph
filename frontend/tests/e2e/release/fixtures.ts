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
     * The fixture has four quarters, 2025-04-01 to 2026-04-01, with 2025-10-01 missing. Its first
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
        label: 'Change since first observation',
        description: '% Change since First Observation',
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
    title: 'All items in U.S. city average, all urban consumers, not seasonally adjusted',
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
    title: 'bds/national..ESTAB',
  },
  fhfaHpi: {
    source: 'FHFA',
    sourceName: 'Federal Housing Finance Agency (FHFA)',
    externalId: 'USHPI',
    title: 'U.S. House Price Index',
  },
} as const;
