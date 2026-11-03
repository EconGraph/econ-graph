import { csvField, csvTextField } from './seriesCsv';

/** A reported or derived value; numbers remain decimal strings until plotting. */
export interface CompanyValue {
  concept: string;
  label: string;
  taxonomy: string;
  unit: string;
  value: string;
  periodStart: string | null;
  periodEnd: string;
  accession: string | null;
  filed: string | null;
  derivation: string | null;
  sourceAccessions: string[];
}

/** Exact source evidence exported alongside every value. */
export function companyValuesCsv(
  company: { cik: string; name: string },
  values: readonly CompanyValue[],
  chartUrl: string
): string {
  const metadata = [
    ['Company', company.name],
    ['CIK', company.cik],
    ['Source', 'U.S. Securities and Exchange Commission (SEC), companyfacts'],
    [
      'Source URL',
      `https://data.sec.gov/api/xbrl/companyfacts/CIK${company.cik.padStart(10, '0')}.json`,
    ],
    ['Chart URL', chartUrl],
  ].map(row => row.map(csvTextField).join(','));
  const header =
    'taxonomy,concept,label,unit,period_start,period_end,value,accession,filed,derivation,source_accessions';
  const rows = values.map(value => {
    // Values are untrusted API input too. Keep decimals exact, but neutralize formula text.
    const decimal = /^-?\d+(\.\d+)?([eE][+-]?\d+)?$/.test(value.value)
      ? csvField(value.value)
      : csvTextField(value.value);
    return [
      ...[
        value.taxonomy,
        value.concept,
        value.label,
        value.unit,
        value.periodStart ?? '',
        value.periodEnd,
      ].map(csvTextField),
      decimal,
      ...[
        value.accession ?? '',
        value.filed ?? '',
        value.derivation ?? '',
        value.sourceAccessions.join(';'),
      ].map(csvTextField),
    ].join(',');
  });
  return [...metadata, header, ...rows, ''].join('\r\n');
}

/** SEC source filing link; accepts only canonical accession/CIK input. */
export function companyFilingUrl(cik: string, accession: string): string | null {
  if (!/^\d{1,10}$/.test(cik) || !/^\d{10}-\d{2}-\d{6}$/.test(accession)) return null;
  return `https://www.sec.gov/Archives/edgar/data/${Number(cik)}/${accession.replace(/-/g, '')}/${accession}-index.html`;
}

/** Distinguish instant and duration periods even when their end dates match. */
export function companyPeriodLabel(value: Pick<CompanyValue, 'periodStart' | 'periodEnd'>): string {
  return value.periodStart
    ? `${value.periodStart} – ${value.periodEnd}`
    : `As of ${value.periodEnd}`;
}
