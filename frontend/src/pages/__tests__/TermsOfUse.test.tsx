/**
 * ECO-881: the Terms of Use pass each public source's conditions on to the people who reuse its
 * data. The list of public sources must be the realm's `catalog-public` role, and every notice and
 * terms link must be one the backend serves (source_notices.rs).
 */

import React from 'react';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { screen, within } from '@testing-library/react';
import { renderWithProviders } from '../../test-utils/test-providers';
import TermsOfUse, { FRED_NOTICE, FRED_TERMS_URL, PUBLIC_SOURCES } from '../TermsOfUse';

const REPO_ROOT = resolve(import.meta.dirname, '../../../..');

interface RealmRole {
  name: string;
  composites?: { client?: Record<string, string[]> };
}

/**
 * The catalog codes the realm's `catalog-public` role grants, from `catalog.<code>:read`.
 * @returns The codes, sorted.
 */
function catalogPublicCodes(): string[] {
  const realm = JSON.parse(
    readFileSync(resolve(REPO_ROOT, 'config/keycloak/econ-graph-realm.json'), 'utf8')
  ) as { roles: { realm: RealmRole[] } };
  const role = realm.roles.realm.find(r => r.name === 'catalog-public');
  expect(role, 'catalog-public realm role').toBeDefined();
  const grants = Object.values(role?.composites?.client ?? {}).flat();
  return grants
    .map(grant => /^catalog\.(\w+):read$/.exec(grant)?.[1])
    .filter((code): code is string => code !== undefined)
    .sort();
}

/**
 * Every string literal in source_notices.rs, with Rust's escapes and `\` line continuations
 * resolved, so a notice split over several source lines compares as one string.
 * @returns The literals' values.
 */
function backendNoticeStrings(): Set<string> {
  const source = readFileSync(
    resolve(REPO_ROOT, 'backend/crates/econ-graph-graphql/src/source_notices.rs'),
    'utf8'
  );
  return new Set(
    Array.from(source.matchAll(/"((?:[^"\\]|\\.)*)"/gs), m =>
      m[1]
        .replace(/\\\n\s*/g, '')
        .replace(/\\"/g, '"')
        .replace(/\\\\/g, '\\')
    )
  );
}

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

  it("lists exactly the sources the realm's catalog-public role makes public", () => {
    expect(PUBLIC_SOURCES.map(source => source.catalog).sort()).toEqual(catalogPublicCodes());
  });

  it('uses only notices and terms links the backend serves, word for word', () => {
    const backend = backendNoticeStrings();
    for (const source of PUBLIC_SOURCES) {
      expect(backend, `${source.name} notice`).toContain(source.notice);
      expect(backend, `${source.name} terms URL`).toContain(source.termsUrl);
    }
    expect(backend, 'FRED notice').toContain(FRED_NOTICE);
    expect(backend, 'FRED terms URL').toContain(FRED_TERMS_URL);
  });

  it('shows each public source with its notice and a link to its terms', () => {
    renderWithProviders(<TermsOfUse />);
    const section = screen.getByRole('region', { name: 'Reusing data from the public sources' });
    const names = within(section)
      .getAllByRole('heading', { level: 3 })
      .map(h => h.textContent);
    expect(names).toEqual(PUBLIC_SOURCES.map(source => source.name));

    for (const { name, notice, noticeIsQuote, termsUrl } of PUBLIC_SOURCES) {
      const block = sourceBlock(name);
      // Quotation marks only around notices quoted from the source's terms.
      const shown = noticeIsQuote ? `“${notice}”` : notice;
      expect(block).toHaveTextContent(`EconGraph shows this notice with the data: ${shown}`);
      expect(within(block).getByRole('link', { name: `${name} terms of use` })).toHaveAttribute(
        'href',
        termsUrl
      );
    }
    // The World Bank notice is EconGraph's wording, not a quotation.
    expect(sourceBlock('The World Bank (World Development Indicators)')).not.toHaveTextContent(
      '“World Bank data are licensed'
    );
  });

  it("states each public source's pass-on conditions", () => {
    renderWithProviders(<TermsOfUse />);
    // ECO-769
    const bls = sourceBlock('U.S. Bureau of Labor Statistics (BLS)');
    expect(bls).toHaveTextContent('Cite the U.S. Bureau of Labor Statistics as the source.');
    expect(bls).toHaveTextContent('Give the date the data were retrieved from BLS, where known.');
    // ECO-770: a suggested credit, not a required one
    expect(sourceBlock('U.S. Bureau of Economic Analysis (BEA)')).toHaveTextContent(
      'BEA suggests "Source: U.S. Bureau of Economic Analysis".'
    );
    // ECO-771
    expect(sourceBlock('U.S. Census Bureau')).toHaveTextContent(
      'Cite the Census Bureau as the source of the original data only'
    );
    // ECO-772: a suggested credit, and the trademarks
    const fhfa = sourceBlock('Federal Housing Finance Agency (FHFA)');
    expect(fhfa).toHaveTextContent('FHFA suggests at least "Source: FHFA HPI®"');
    expect(fhfa).toHaveTextContent('Keep the ®');
    // ECO-768: the attribution format, other licences, changes, the pass-through by URL, the name
    const worldBank = sourceBlock('The World Bank (World Development Indicators)');
    expect(worldBank).toHaveTextContent(
      'in the format "The World Bank: Dataset name: Data source (if known)"'
    );
    expect(worldBank).toHaveTextContent(
      'The World Bank gives the data source in its metadata for each indicator.'
    );
    expect(worldBank).toHaveTextContent(
      "If an indicator's World Bank metadata names a different licence, that licence applies, not CC BY 4.0."
    );
    expect(worldBank).toHaveTextContent('Say when you have changed the values');
    expect(worldBank).toHaveTextContent('pass the same attribution requirement on');
    expect(worldBank).toHaveTextContent('Using the name in the attribution above is required.');
    expect(
      within(worldBank).getByRole('link', { name: 'World Bank dataset terms of use' })
    ).toHaveAttribute('href', 'https://www.worldbank.org/ext/en/legal/terms-conditions/datasets');
  });

  it('binds users of FRED data to the FRED API terms, whatever FRED access is', () => {
    renderWithProviders(<TermsOfUse />);
    const fred = screen.getByRole('region', { name: 'FRED data' });
    expect(fred).toHaveTextContent(`Where EconGraph shows FRED® data, it shows this notice: “${FRED_NOTICE}”`);
    expect(fred).toHaveTextContent('If you use FRED data, you are bound by the FRED® API Terms of Use');
    expect(fred).toHaveTextContent('prior written permission of the Federal Reserve Bank of St. Louis');
    expect(within(fred).getByRole('link', { name: 'FRED® API Terms of Use' })).toHaveAttribute(
      'href',
      FRED_TERMS_URL
    );
  });

  it('says FRED is not public while it is not in the public list', () => {
    renderWithProviders(<TermsOfUse />);
    const other = screen.getByRole('region', { name: 'Other sources' });
    expect(other).toHaveTextContent('EconGraph does not license it for reuse');
    const fredIsPublic = PUBLIC_SOURCES.some(source => source.catalog === 'fred');
    const sentence = 'FRED is not one of the public sources';
    if (fredIsPublic) expect(other).not.toHaveTextContent(sentence);
    else expect(other).toHaveTextContent(sentence);
  });

  it('describes the downloads and GraphQL accurately, and disclaims any warranty', () => {
    renderWithProviders(<TermsOfUse />);
    const use = screen.getByRole('region', { name: 'Using the API and downloads' });
    expect(use).toHaveTextContent(
      "The CSV download on a series page includes the source's citation, notice and terms link"
    );
    expect(use).toHaveTextContent(
      'The CSV and JSON downloads of search results hold series details only, not values or notices.'
    );
    expect(use).toHaveTextContent(
      'notice field is an object with its citation, notice, termsUrl and retrievalDateRequired'
    );
    expect(screen.getByRole('region', { name: 'No warranty' })).toHaveTextContent(
      'provided as they are'
    );
  });

  it('opens every external link in a new tab', () => {
    renderWithProviders(<TermsOfUse />);
    for (const link of screen.getAllByRole('link')) {
      expect(link, link.getAttribute('href') ?? '').toHaveAttribute('target', '_blank');
      expect(link).toHaveAttribute('rel', 'noopener noreferrer');
    }
  });
});
