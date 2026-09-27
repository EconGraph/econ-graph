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
} as const;
