import { useQuery } from '@tanstack/react-query';
import { executeGraphQL } from '../utils/graphql';

export type CompanyStatementKind = 'INCOME_STATEMENT' | 'BALANCE_SHEET' | 'CASH_FLOW';
export interface CompanyRevisionInput {
  mode: 'LATEST' | 'ORIGINAL' | 'AS_OF' | 'ACCESSION';
  asOf?: string;
  accession?: string;
}
export interface CompanyResearchCompany {
  id: string;
  cik: string;
  name: string;
  ticker: string | null;
  sicCode: string | null;
  sicDescription: string | null;
  fiscalYearEnd: string | null;
  updatedAt: string;
}
export interface CompanyFactEvidence {
  factId: string;
  anchor: string | null;
  filingAnchor: string | null;
  accession: string;
  filed: string;
  taxonomy: string;
  concept: string;
  unit: string;
  periodStart: string | null;
  periodEnd: string;
  value: string;
}
export interface CompanyStatementLine {
  key: string;
  label: string;
  taxonomy: string;
  concept: string;
  periodStart: string | null;
  periodEnd: string;
  unit: string;
  value: string;
  provenance: CompanyFactEvidence[];
  derivation: string | null;
  restated: boolean;
  anchor: string | null;
  subjectAnchor: string | null;
}
export interface CompanyResearchStatement {
  kind: CompanyStatementKind;
  lines: CompanyStatementLine[];
  periods: { start: string | null; end: string; kind: string }[];
  notice: string;
  unavailableDerivations: {
    taxonomy: string;
    concept: string;
    unit: string;
    periodStart: string | null;
    periodEnd: string;
    derivation: string;
    reason: string;
  }[];
}

export const FIND_COMPANIES = `query FindResearchCompanies($search: String, $first: Int, $after: String) {
  companies(search: $search, first: $first, after: $after) {
    nodes { id cik name ticker sicCode sicDescription fiscalYearEnd updatedAt }
    hasNextPage endCursor
  }
}`;
export const GET_RESEARCH_COMPANY = `query GetResearchCompany($cik: String!) {
  company(cik: $cik) { id cik name ticker sicCode sicDescription fiscalYearEnd updatedAt }
}`;
export const GET_COMPANY_STATEMENT = `query GetCompanyResearchStatement($cik: String!, $kind: CompanyStatementKind!, $revision: CompanyRevisionInput) {
  financialStatement(cik: $cik, kind: $kind, revision: $revision, unclassifiedFirst: 0) {
    kind notice periods {start end kind} unavailableDerivations { taxonomy concept unit periodStart periodEnd derivation reason } lines { key label taxonomy concept periodStart periodEnd unit value derivation restated anchor subjectAnchor provenance { factId anchor filingAnchor accession filed taxonomy concept unit periodStart periodEnd value } }
  }
}`;

export function useResearchCompanies(search: string, after: string | null) {
  return useQuery({
    queryKey: ['researchCompanies', search, after],
    queryFn: async () => {
      const result = await executeGraphQL<{
        companies: {
          nodes: CompanyResearchCompany[];
          hasNextPage: boolean;
          endCursor: string | null;
        };
      }>({
        query: FIND_COMPANIES,
        variables: { search, first: 25, after },
        operationName: 'FindResearchCompanies',
      });
      if (!result.data?.companies) throw new Error('Company search returned no response.');
      return result.data.companies;
    },
  });
}
export function useResearchCompany(cik: string) {
  return useQuery({
    queryKey: ['researchCompany', cik],
    queryFn: async () => {
      const result = await executeGraphQL<{ company: CompanyResearchCompany | null }>({
        query: GET_RESEARCH_COMPANY,
        variables: { cik },
        operationName: 'GetResearchCompany',
      });
      if (!result.data) throw new Error('Company details returned no response.');
      return result.data.company;
    },
    enabled: /^\d{1,10}$/.test(cik),
  });
}
export function useCompanyStatement(
  cik: string,
  kind: CompanyStatementKind,
  revision: CompanyRevisionInput,
  enabled: boolean
) {
  return useQuery({
    queryKey: ['companyResearchStatement', cik, kind, revision],
    queryFn: async () => {
      const result = await executeGraphQL<{ financialStatement: CompanyResearchStatement }>({
        query: GET_COMPANY_STATEMENT,
        variables: { cik, kind, revision },
        operationName: 'GetCompanyResearchStatement',
      });
      if (!result.data?.financialStatement)
        throw new Error('Financial statement returned no response.');
      return result.data.financialStatement;
    },
    enabled,
  });
}

export interface CompanyFinancialRatio {
  name: string;
  value: string | null;
  unit: string;
  unavailableReason: string | null;
  provenance: CompanyFactEvidence[];
}
export interface CompanyPeerComparison {
  company: CompanyResearchCompany;
  ratios: CompanyFinancialRatio[];
}
export const GET_COMPANY_COMPARISONS = `query GetCompanyResearchComparisons($cik: String!, $periodStart: NaiveDate, $periodEnd: NaiveDate!, $revision: CompanyRevisionInput) {
  companyRatios(cik: $cik, periodStart: $periodStart, periodEnd: $periodEnd, revision: $revision) {
    name value unit unavailableReason provenance { factId anchor filingAnchor accession filed taxonomy concept unit periodStart periodEnd value }
  }
  companyPeers(cik: $cik, periodStart: $periodStart, periodEnd: $periodEnd, revision: $revision, first: 10) {
    company { id cik name ticker sicCode sicDescription fiscalYearEnd updatedAt }
    ratios { name value unit unavailableReason provenance { factId anchor filingAnchor accession filed taxonomy concept unit periodStart periodEnd value } }
  }
}`;
export function useCompanyComparisons(
  cik: string,
  periodStart: string | null,
  periodEnd: string,
  revision: CompanyRevisionInput,
  enabled: boolean
) {
  return useQuery({
    queryKey: ['companyResearchComparisons', cik, periodStart, periodEnd, revision],
    queryFn: async () => {
      const result = await executeGraphQL<{
        companyRatios: CompanyFinancialRatio[];
        companyPeers: CompanyPeerComparison[];
      }>({
        query: GET_COMPANY_COMPARISONS,
        variables: { cik, periodStart, periodEnd, revision },
        operationName: 'GetCompanyResearchComparisons',
      });
      if (!result.data) throw new Error('Company ratios returned no response.');
      return result.data;
    },
    enabled,
  });
}

export interface CompanyOtherFactsPage {
  unclassifiedFacts: CompanyStatementLine[];
  unclassifiedHasNextPage: boolean;
  unclassifiedEndCursor: string | null;
}
export const GET_COMPANY_OTHER_FACTS = `query GetCompanyOtherFacts($cik: String!, $revision: CompanyRevisionInput, $periodStart: NaiveDate, $periodEnd: NaiveDate!, $instantOnly: Boolean!, $after: String) {
  financialStatement(cik: $cik, kind: INCOME_STATEMENT, revision: $revision, periodStart: $periodStart, periodEnd: $periodEnd, instantOnly: $instantOnly, unclassifiedFirst: 100, unclassifiedAfter: $after) {
    unclassifiedHasNextPage unclassifiedEndCursor
    unclassifiedFacts { key label taxonomy concept periodStart periodEnd unit value derivation restated anchor subjectAnchor provenance { factId anchor filingAnchor accession filed taxonomy concept unit periodStart periodEnd value } }
  }
}`;
/** Other facts are requested once for the selected economic period, independent of statement kind. */
export function useCompanyOtherFacts(
  cik: string,
  revision: CompanyRevisionInput,
  periodStart: string | null,
  periodEnd: string,
  after: string | null,
  enabled: boolean
) {
  return useQuery({
    queryKey: ['companyOtherFacts', cik, revision, periodStart, periodEnd, after],
    staleTime: 60_000,
    queryFn: async () => {
      const result = await executeGraphQL<{ financialStatement: CompanyOtherFactsPage }>({
        query: GET_COMPANY_OTHER_FACTS,
        variables: {
          cik,
          revision,
          periodStart,
          periodEnd,
          instantOnly: periodStart === null,
          after,
        },
        operationName: 'GetCompanyOtherFacts',
      });
      if (!result.data?.financialStatement)
        throw new Error('Other reported facts returned no response.');
      return result.data.financialStatement;
    },
    enabled,
  });
}
export interface CompanyHistoryFact {
  taxonomy: string;
  concept: string;
  label: string | null;
  unit: string;
  value: string;
  periodStart: string | null;
  periodEnd: string;
  accession: string;
  filed: string;
}
export interface CompanyFactHistoryPage {
  nodes: CompanyHistoryFact[];
  hasNextPage: boolean;
  endCursor: string | null;
}
export const GET_COMPANY_FACT_HISTORY = `query GetCompanyFactHistory($cik: String!, $filter: CompanyFactFilterInput, $after: String) {
  companyFacts(cik: $cik, filter: $filter, first: 100, after: $after) {
    hasNextPage endCursor nodes { taxonomy concept label unit value periodStart periodEnd accession filed }
  }
}`;
/** Chart source history remains bounded and explicitly paginated, with exact decimal values. */
export function useCompanyFactHistory(
  cik: string,
  line: CompanyStatementLine | undefined,
  revision: CompanyRevisionInput,
  after: string | null,
  enabled: boolean
) {
  return useQuery({
    staleTime: 60_000,
    queryKey: [
      'companyFactHistory',
      cik,
      line?.taxonomy,
      line?.concept,
      line?.unit,
      revision,
      after,
    ],
    queryFn: async () => {
      const result = await executeGraphQL<{ companyFacts: CompanyFactHistoryPage }>({
        query: GET_COMPANY_FACT_HISTORY,
        variables: {
          cik,
          filter: { taxonomy: line?.taxonomy, concept: line?.concept, unit: line?.unit, revision },
          after,
        },
        operationName: 'GetCompanyFactHistory',
      });
      if (!result.data?.companyFacts) throw new Error('Fact history returned no response.');
      return result.data.companyFacts;
    },
    enabled,
  });
}
