import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import { describe, expect, it, vi, beforeEach } from 'vitest';
import CompanyResearch from '../CompanyResearch';
import { downloadCsv } from '../../utils/seriesCsv';
const state = vi.hoisted(() => ({
  cik: '0000320193' as string | undefined,
  mode: 'LATEST',
  statementError: false,
  lines: [] as unknown[],
  unclassified: [] as unknown[],
  unavailable: [] as unknown[],
  nextUnclassified: [] as any[],
  otherHasNext: false,
  otherAfter: null as string | null,
  historyAfter: null as string | null,
  historyHasNext: false,
  historyNodes: [] as any[],
  nextHistory: [] as any[],
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
        periods: [...state.lines, ...state.unclassified, ...state.nextUnclassified].map(
          (item: any) => ({
            start: item.periodStart,
            end: item.periodEnd,
            kind: item.periodStart ? 'duration' : 'instant',
          })
        ),
        unavailableDerivations: state.unavailable,
      },
    };
  },
  useCompanyOtherFacts: (
    _cik: any,
    _revision: any,
    _start: any,
    _end: any,
    after: string | null
  ) => {
    state.otherAfter = after;
    return {
      isPending: false,
      isError: false,
      data: {
        unclassifiedFacts: after ? state.nextUnclassified : state.unclassified,
        unclassifiedHasNextPage: !after && state.otherHasNext,
        unclassifiedEndCursor: !after && state.otherHasNext ? '100' : null,
      },
    };
  },
  useCompanyFactHistory: (_cik: any, line: any, _revision: any, after: string | null) => {
    state.historyAfter = after;
    return {
      isPending: false,
      isError: false,
      data: {
        nodes: after
          ? state.nextHistory
          : state.historyNodes.length
            ? state.historyNodes
            : state.unclassified.map((item: any) => ({
                ...item,
                accession: item.provenance[0].accession,
                filed: item.provenance[0].filed,
              })),
        hasNextPage: !after && state.historyHasNext,
        endCursor: !after && state.historyHasNext ? '100' : null,
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
  state.nextUnclassified = [];
  state.otherHasNext = false;
  state.otherAfter = null;
  state.historyAfter = null;
  state.historyHasNext = false;
  state.historyNodes = [];
  state.nextHistory = [];
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
  it('pages other facts without hiding later values or changing the economic period', () => {
    const first = {
      ...line,
      key: 'other-a',
      concept: 'OtherA',
      label: 'Other A',
      value: '123.456789',
    };
    const next = {
      ...line,
      key: 'other-b',
      concept: 'OtherB',
      label: 'Other B',
      value: '987.654321',
    };
    state.unclassified = [first];
    state.nextUnclassified = [next];
    state.otherHasNext = true;
    render(<CompanyResearch />);
    expect(screen.getByRole('button', { name: 'Chart Other A' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Next other facts' }));
    expect(state.otherAfter).toBe('100');
    expect(screen.queryByRole('button', { name: 'Chart Other A' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Chart Other B' })).toBeInTheDocument();
    expect(screen.getByRole('combobox', { name: 'Period' })).toHaveTextContent(
      '2024-01-01 – 2024-12-31'
    );
    fireEvent.click(screen.getByRole('button', { name: 'Previous other facts' }));
    expect(state.otherAfter).toBeNull();
    expect(screen.getByRole('button', { name: 'Chart Other A' })).toBeInTheDocument();
  });
  it('pages unclassified source history and preserves page-specific exact CSV evidence', () => {
    state.unclassified = [{ ...line, key: 'other', concept: 'OtherFact', label: 'Other fact' }];
    state.historyNodes = [{ ...fact, concept: 'OtherFact', value: '123456789.123456789' }];
    state.nextHistory = [
      {
        ...fact,
        concept: 'OtherFact',
        value: '987654321.987654321',
        accession: '0000320193-24-000001',
        periodStart: '2023-01-01',
        periodEnd: '2023-12-31',
      },
    ];
    state.historyHasNext = true;
    render(<CompanyResearch />);
    fireEvent.click(screen.getByRole('button', { name: 'Chart Other fact' }));
    fireEvent.click(screen.getByRole('button', { name: 'Next history page' }));
    expect(state.historyAfter).toBe('100');
    expect(screen.getByRole('img', { name: 'Other fact line chart' })).toHaveTextContent(
      '987654321.987654321'
    );
    fireEvent.click(screen.getByRole('button', { name: 'Download line CSV' }));
    const calls = vi.mocked(downloadCsv).mock.calls;
    const csv = calls[calls.length - 1][1];
    expect(csv).toContain('987654321.987654321');
    expect(csv).toContain('0000320193-24-000001');
    expect(csv).not.toContain('123456789.123456789');
    fireEvent.click(screen.getByRole('button', { name: 'Previous history page' }));
    expect(state.historyAfter).toBeNull();
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
