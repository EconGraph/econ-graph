/**
 * ECO-881: the Terms of Use pass each public source's conditions on to the people who reuse its
 * data. The notices must match the ones the backend serves (source_notices.rs), word for word.
 */

import React from 'react';
import { screen, within } from '@testing-library/react';
import { renderWithProviders } from '../../test-utils/test-providers';
import TermsOfUse from '../TermsOfUse';

/** The public sources' notices, as backend/crates/econ-graph-graphql/src/source_notices.rs has them. */
const PUBLIC_NOTICES: Record<string, { notice: string; termsUrl: string }> = {
  'U.S. Bureau of Labor Statistics (BLS)': {
    notice:
      'BLS.gov cannot vouch for the data or analyses derived from these data after the data have been retrieved from BLS.gov.',
    termsUrl: 'https://www.bls.gov/developers/termsOfService.htm',
  },
  'U.S. Bureau of Economic Analysis (BEA)': {
    notice:
      'This product uses the Bureau of Economic Analysis (BEA) Data API but is not endorsed or certified by BEA.',
    termsUrl: 'https://apps.bea.gov/API/_pdf/bea_api_tos.pdf',
  },
  'U.S. Census Bureau': {
    notice:
      'This product uses the Census Bureau Data API but is not endorsed or certified by the Census Bureau.',
    termsUrl: 'https://www.census.gov/data/developers/about/terms-of-service.html',
  },
  'Federal Housing Finance Agency (FHFA)': {
    notice: 'This product uses FHFA Data but is neither endorsed nor certified by FHFA.',
    termsUrl: 'https://www.fhfa.gov/about/fhfa-policies/api-terms-of-service',
  },
  'The World Bank (World Development Indicators)': {
    notice:
      'World Bank data are licensed under CC BY 4.0. Anyone who reuses them must attribute The World Bank and its data providers in the format "The World Bank: Dataset name: Data source (if known)", as the World Bank\'s dataset terms of use require.',
    termsUrl: 'https://www.worldbank.org/ext/en/legal/terms-conditions/datasets',
  },
};

/**
 * The block for one public source: its heading and everything up to the next source's.
 * @param name - The source's heading.
 * @returns The element holding that source's notice, duties and terms link.
 */
function sourceBlock(name: string): HTMLElement {
  const block = screen.getByRole('heading', { level: 3, name }).parentElement;
  expect(block).not.toBeNull();
  return block as HTMLElement;
}

describe('TermsOfUse', () => {
  it('has the page heading', () => {
    renderWithProviders(<TermsOfUse />);
    expect(screen.getByRole('heading', { level: 1, name: 'Terms of Use' })).toBeInTheDocument();
  });

  it('lists exactly the public sources, each with its notice and a link to its terms', () => {
    renderWithProviders(<TermsOfUse />);
    const section = screen.getByRole('region', { name: 'Reusing data from the public sources' });
    const names = within(section)
      .getAllByRole('heading', { level: 3 })
      .map(h => h.textContent);
    expect(names).toEqual(Object.keys(PUBLIC_NOTICES));

    for (const [name, { notice, termsUrl }] of Object.entries(PUBLIC_NOTICES)) {
      const block = sourceBlock(name);
      expect(block).toHaveTextContent(`Notice: “${notice}”`);
      expect(within(block).getByRole('link', { name: `${name} terms of use` })).toHaveAttribute(
        'href',
        termsUrl
      );
    }
  });

  it("states each public source's pass-on conditions", () => {
    renderWithProviders(<TermsOfUse />);
    // ECO-769
    const bls = sourceBlock('U.S. Bureau of Labor Statistics (BLS)');
    expect(bls).toHaveTextContent('Cite the U.S. Bureau of Labor Statistics as the source.');
    expect(bls).toHaveTextContent('Give the date the data were retrieved from BLS.');
    // ECO-770
    expect(sourceBlock('U.S. Bureau of Economic Analysis (BEA)')).toHaveTextContent(
      'Cite "Source: U.S. Bureau of Economic Analysis".'
    );
    // ECO-771
    expect(sourceBlock('U.S. Census Bureau')).toHaveTextContent(
      'Cite the Census Bureau as the source of the original data only'
    );
    // ECO-772
    const fhfa = sourceBlock('Federal Housing Finance Agency (FHFA)');
    expect(fhfa).toHaveTextContent('Cite "Source: FHFA HPI®"');
    expect(fhfa).toHaveTextContent('Keep the ®');
    // ECO-768: the attribution format, the change indication, and the pass-through by URL
    const worldBank = sourceBlock('The World Bank (World Development Indicators)');
    expect(worldBank).toHaveTextContent(
      'Attribute the data as "The World Bank: Dataset name: Data source (if known)"'
    );
    expect(worldBank).toHaveTextContent('Say when you have changed the values');
    expect(worldBank).toHaveTextContent('pass the same attribution requirement on');
    expect(
      within(worldBank).getByRole('link', { name: 'World Bank dataset terms of use' })
    ).toHaveAttribute('href', 'https://www.worldbank.org/ext/en/legal/terms-conditions/datasets');
  });

  it('binds users of FRED data to the FRED API terms, whatever FRED access is', () => {
    renderWithProviders(<TermsOfUse />);
    const fred = screen.getByRole('region', { name: 'FRED data' });
    expect(fred).toHaveTextContent(
      'This product uses the FRED® API but is not endorsed or certified by the Federal Reserve Bank of St. Louis.'
    );
    expect(fred).toHaveTextContent('If you use FRED data, you are bound by the FRED® API Terms of Use');
    expect(within(fred).getByRole('link', { name: 'FRED® API Terms of Use' })).toHaveAttribute(
      'href',
      'https://fred.stlouisfed.org/docs/api/terms_of_use.html'
    );
  });

  it('covers restricted sources, acceptable use and the absence of a warranty', () => {
    renderWithProviders(<TermsOfUse />);
    expect(screen.getByRole('region', { name: 'Other sources' })).toHaveTextContent(
      'EconGraph does not license it for reuse'
    );
    const use = screen.getByRole('region', { name: 'Using the API and downloads' });
    expect(use).toHaveTextContent('The GraphQL API and the CSV and JSON downloads');
    expect(screen.getByRole('region', { name: 'No warranty' })).toHaveTextContent(
      'provided as they are'
    );
  });
});
