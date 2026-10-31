import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import { describe, expect, it, vi, beforeEach } from 'vitest';
import CompanyResearch from '../CompanyResearch';
const state = vi.hoisted(() => ({
  cik: '0000320193' as string | undefined,
  mode: 'LATEST',
  statementError: false,
  lines: [] as unknown[],
  unclassified: [] as unknown[],
  unavailable: [] as unknown[],
}));
vi.mock('react-router-dom', () => ({
  useParams: () => ({ cik: state.cik }),
  Link: ({ children, to, ...props }: any) => (
    <a href={to} {...props}>
      {children}
    </a>
  ),
}));
vi.mock('../../components/financial/CompanyNotes', () => ({
  default: ({ anchor, targetLabel }: any) => (
    <div data-testid='company-notes' data-anchor={anchor}>
      {targetLabel} notes
    </div>
  ),
}));
vi.mock('../../utils/seriesCsv', async importOriginal => ({
  ...(await importOriginal<typeof import('../../utils/seriesCsv')>()),
  downloadCsv: vi.fn(),
}));
vi.mock('../../graphql/companyResearch', () => ({
  useResearchCompanies: () => ({
    isPending: false,
    isError: false,
    data: {
      nodes: [{ cik: '0000320193', name: 'Example Company', ticker: 'EX' }],
      hasNextPage: false,
    },
  }),
  useResearchCompany: () => ({
    isPending: false,
    isError: false,
    data: {
      cik: '0000320193',
      name: 'Example Company',
      updatedAt: '2025-03-01',
      sicCode: '1234',
      sicDescription: 'Example industry',
    },
  }),
  useCompanyStatement: (_cik: any, _kind: any, revision: any) => {
    state.mode = revision.mode;
    return {
      isFetching: false,
      isError: state.statementError,
      data: {
        kind: 'INCOME_STATEMENT',
        lines: state.lines,
        unclassifiedFacts: state.unclassified,
        unavailableDerivations: state.unavailable,
      },
    };
  },
  useCompanyComparisons: () => ({
    isPending: false,
    isError: false,
    data: {
      companyRatios: [
        {
          name: 'Net margin',
          value: null,
          unit: 'percent',
          unavailableReason: 'Missing compatible revenue',
          provenance: [],
        },
      ],
      companyPeers: [],
    },
  }),
}));
const fact = {
  factId: 'fact1',
  anchor: 'filed-anchor',
  filingAnchor: 'filing-anchor',
  accession: '0000320193-25-000010',
  filed: '2025-02-01',
  taxonomy: 'us-gaap',
  concept: 'Revenue',
  unit: 'USD',
  periodStart: '2024-01-01',
  periodEnd: '2024-12-31',
  value: '9007199254740993.123456789',
};
const line = {
  key: 'revenue',
  label: 'Revenue',
  taxonomy: 'us-gaap',
  concept: 'Revenue',
  periodStart: '2024-01-01',
  periodEnd: '2024-12-31',
  unit: 'USD',
  value: fact.value,
  provenance: [fact],
  restated: true,
  derivation: null,
  anchor: 'filed-anchor',
  subjectAnchor: 'economic-anchor',
};
beforeEach(() => {
  state.cik = '0000320193';
  state.mode = 'LATEST';
  state.statementError = false;
  state.lines = [line];
  state.unclassified = [];
  state.unavailable = [];
});
describe('company research page', () => {
  it('shows exact decimals, source provenance, restatements and unavailable ratio inputs', () => {
    render(<CompanyResearch />);
    expect(screen.getByRole('heading', { name: 'Example Company' })).toBeInTheDocument();
    expect(screen.getByText(fact.value)).toBeInTheDocument();
    expect(screen.getByText('Restated')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: fact.accession })).toHaveAttribute(
      'href',
      expect.stringContaining('www.sec.gov/Archives')
    );
    expect(screen.getByText('Missing compatible revenue')).toBeInTheDocument();
  });
  it('charts matching periods and pins notes to the selected fact', () => {
    state.lines = [
      line,
      {
        ...line,
        periodStart: '2023-01-01',
        periodEnd: '2023-12-31',
        value: '1.25',
        anchor: 'older',
      },
      {
        ...line,
        periodStart: '2024-10-01',
        periodEnd: '2024-12-31',
        value: '2.5',
        anchor: 'quarter',
      },
    ];
    render(<CompanyResearch />);
    fireEvent.click(screen.getByRole('button', { name: 'Chart Revenue' }));
    expect(screen.getByRole('img', { name: 'Revenue line chart' })).toBeInTheDocument();
    expect(screen.getByTestId('company-notes')).toHaveAttribute('data-anchor', 'economic-anchor');
    expect(
      screen.getByRole('img', { name: 'Revenue line chart' }).querySelectorAll('circle')
    ).toHaveLength(2);
  });
  it('keeps unclassified facts chartable and explains missing derivations', () => {
    state.unclassified = [
      {
        ...line,
        key: 'us-gaap:OtherFact',
        concept: 'OtherFact',
        label: 'Other fact',
        value: '123',
      },
    ];
    state.unavailable = [
      {
        taxonomy: 'us-gaap',
        concept: 'EPS',
        unit: 'USD/shares',
        periodStart: '2024-10-01',
        periodEnd: '2024-12-31',
        derivation: 'fourth_quarter',
        reason: 'Per-share data is not additive',
      },
    ];
    render(<CompanyResearch />);
    expect(screen.getByRole('heading', { name: 'Other reported facts' })).toBeInTheDocument();
    expect(screen.getByText(/Per-share data is not additive/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Chart Other fact' }));
    expect(screen.getByRole('img', { name: 'Other fact line chart' })).toBeInTheDocument();
  });
  it('lets the researcher choose filed fact and filing targets explicitly', () => {
    render(<CompanyResearch />);
    fireEvent.click(screen.getByRole('button', { name: 'Chart Revenue' }));
    fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Note target' }));
    fireEvent.click(screen.getByRole('option', { name: 'Filing: 0000320193-25-000010' }));
    expect(screen.getByTestId('company-notes')).toHaveAttribute('data-anchor', 'filing-anchor');
    fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Note target' }));
    fireEvent.click(
      screen.getByRole('option', { name: 'Reported fact: Revenue, 0000320193-25-000010' })
    );
    expect(screen.getByTestId('company-notes')).toHaveAttribute('data-anchor', 'filed-anchor');
  });
  it('never silently changes an unsupported derived target to company notes', () => {
    state.lines = [{ ...line, subjectAnchor: null, anchor: null, derivation: 'fourth_quarter' }];
    render(<CompanyResearch />);
    fireEvent.click(screen.getByRole('button', { name: 'Chart Revenue' }));
    expect(screen.getByText(/This note target is unavailable/)).toBeInTheDocument();
    expect(screen.queryByTestId('company-notes')).not.toBeInTheDocument();
  });
  it('offers all three statements and explicit revision selection', () => {
    render(<CompanyResearch />);
    fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Statement' }));
    expect(screen.getByRole('option', { name: 'Balance sheet' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('option', { name: 'Cash flow' }));
    fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Revision' }));
    fireEvent.click(screen.getByRole('option', { name: 'Originally reported' }));
    expect(state.mode).toBe('ORIGINAL');
  });
  it('keeps statement errors visible', () => {
    state.statementError = true;
    state.lines = [];
    render(<CompanyResearch />);
    expect(screen.getByText('The statement could not be loaded.')).toBeInTheDocument();
  });
  it('searches and links actual company CIKs', () => {
    state.cik = undefined;
    render(<CompanyResearch />);
    expect(screen.getByRole('link', { name: 'Example Company' })).toHaveAttribute(
      'href',
      '/companies/0000320193'
    );
    expect(screen.getByRole('textbox', { name: 'Find company' })).toBeInTheDocument();
  });
});
