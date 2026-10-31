import { describe, expect, it } from 'vitest';
import {
  companyFilingUrl,
  companyPeriodLabel,
  companyValuesCsv,
  type CompanyValue,
} from '../companyResearch';
const fact: CompanyValue = {
  taxonomy: 'us-gaap',
  concept: 'Revenue',
  label: 'Revenue',
  unit: 'USD',
  value: '9007199254740993.123456789',
  periodStart: '2024-01-01',
  periodEnd: '2024-12-31',
  accession: '0000320193-25-000010',
  filed: '2025-02-01',
  derivation: null,
  sourceAccessions: ['0000320193-25-000010'],
};
describe('company research evidence', () => {
  it('exports exact decimal strings with source, filing and chart attribution', () => {
    const csv = companyValuesCsv(
      { cik: '0000320193', name: 'Example, Inc.' },
      [fact],
      'https://example.test/companies/0000320193'
    );
    expect(csv).toContain('"Example, Inc."');
    expect(csv).toContain('U.S. Securities and Exchange Commission (SEC)');
    expect(csv).toContain('CIK0000320193.json');
    expect(csv).toContain('Chart URL,https://example.test/companies/0000320193');
    expect(csv).toContain(',9007199254740993.123456789,0000320193-25-000010,2025-02-01,');
  });
  it('neutralizes spreadsheet formulas without changing negative decimals', () => {
    const csv = companyValuesCsv(
      { cik: '1', name: '=IMPORTXML("evil")' },
      [
        { ...fact, label: '@command', value: '=SUM(1,2)' },
        { ...fact, value: '-123.456' },
      ],
      'https://example.test'
    );
    expect(csv).toContain("'@command");
    expect(csv).toContain('"\'=SUM(1,2)"');
    expect(csv).toContain(',-123.456,');
  });
  it('retains derived inputs without inventing a filed accession', () => {
    expect(
      companyValuesCsv(
        { cik: '1', name: 'Example' },
        [
          {
            ...fact,
            accession: null,
            filed: null,
            derivation: 'Q4 = FY - 9M',
            sourceAccessions: ['a', 'b'],
          },
        ],
        'https://example.test'
      )
    ).toContain(',,,Q4 = FY - 9M,a;b');
  });
  it('distinguishes instant and duration periods', () => {
    expect(companyPeriodLabel(fact)).toBe('2024-01-01 – 2024-12-31');
    expect(companyPeriodLabel({ ...fact, periodStart: null })).toBe('As of 2024-12-31');
  });
  it('links only canonical SEC filings', () => {
    expect(companyFilingUrl('0000320193', '0000320193-25-000010')).toBe(
      'https://www.sec.gov/Archives/edgar/data/320193/000032019325000010/0000320193-25-000010-index.html'
    );
    expect(companyFilingUrl('javascript:evil', 'anything')).toBeNull();
  });
});
