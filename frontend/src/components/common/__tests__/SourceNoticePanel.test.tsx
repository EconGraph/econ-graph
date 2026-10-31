/**
 * The series page shows the notice its source's terms require, the retrieval date where the
 * terms ask for it, and says when shown values are EconGraph's transformation (ECO-767).
 */

import React from 'react';
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { SourceNoticePanel } from '../SourceNoticePanel';

const fhfa = {
  citation: 'Source: FHFA HPI®',
  notice: 'This product uses FHFA Data but is neither endorsed nor certified by FHFA.',
  termsUrl: 'https://www.fhfa.gov/about/fhfa-policies/api-terms-of-service',
  retrievalDateRequired: false,
};

describe('SourceNoticePanel', () => {
  it('shows the citation with its trademark sign, the notice and a terms link', () => {
    render(
      <SourceNoticePanel
        sourceName='Federal Housing Finance Agency (FHFA)'
        notice={fhfa}
        lastUpdated='2026-10-29T08:15:00Z'
        transformationLabel=''
      />
    );
    expect(screen.getByText('Source: FHFA HPI®')).toBeInTheDocument();
    const notice = screen.getByTestId('source-notice');
    expect(notice).toHaveTextContent(fhfa.notice);
    // FHFA's terms don't ask for the retrieval date, and levels aren't derived.
    expect(notice).not.toHaveTextContent('retrieved');
    expect(notice).not.toHaveTextContent('EconGraph');
    expect(screen.getByRole('link', { name: 'Terms of use' })).toHaveAttribute(
      'href',
      fhfa.termsUrl
    );
  });

  it('adds the retrieval date when the terms require it, and omits it when unknown', () => {
    const bls = { ...fhfa, citation: null, notice: 'BLS.gov cannot vouch.', retrievalDateRequired: true };
    const { rerender } = render(
      <SourceNoticePanel
        sourceName='Bureau of Labor Statistics (BLS)'
        notice={bls}
        lastUpdated='2026-10-29T08:15:00Z'
        transformationLabel=''
      />
    );
    expect(screen.getByTestId('source-notice')).toHaveTextContent(
      'BLS.gov cannot vouch. Data retrieved from Bureau of Labor Statistics (BLS) on 2026-10-29.'
    );
    rerender(
      <SourceNoticePanel
        sourceName='Bureau of Labor Statistics (BLS)'
        notice={bls}
        lastUpdated={null}
        transformationLabel=''
      />
    );
    expect(screen.getByTestId('source-notice')).toHaveTextContent(
      'BLS.gov cannot vouch. Retrieval date not recorded.'
    );
  });

  it('marks transformed values as derived, even for a source without a notice', () => {
    render(
      <SourceNoticePanel
        sourceName='International Monetary Fund (IMF)'
        notice={null}
        lastUpdated={null}
        transformationLabel='Year-over-Year % Change'
      />
    );
    expect(screen.getByTestId('source-notice')).toHaveTextContent(
      'Values show the Year-over-Year % Change, computed by EconGraph from data published by International Monetary Fund (IMF); they are not figures published by International Monetary Fund (IMF).'
    );
    expect(screen.queryByRole('link')).not.toBeInTheDocument();
  });

  it('renders nothing for a source without a notice showing levels', () => {
    const { container } = render(
      <SourceNoticePanel sourceName='' notice={undefined} lastUpdated={null} transformationLabel='' />
    );
    expect(container).toBeEmptyDOMElement();
  });
});
