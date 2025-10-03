/**
 * Financial Data Hook
 *
 * Provides GraphQL integration for financial data and company information.
 * Includes company details, financial statements, and financial metrics.
 */

import { useState, useCallback } from 'react';
import { useQuery, useLazyQuery } from '@apollo/client/react';
import { gql } from '@apollo/client';

// GraphQL query for getting a company
const GET_COMPANY = gql`
  query GetCompany($id: ID!) {
    company(id: $id) {
      id
      cik
      ticker
      name
      legal_name
      sic_code
      sic_description
      industry
      sector
      business_address
      mailing_address
      phone
      website
      state_of_incorporation
      state_of_incorporation_description
      fiscal_year_end
      entity_type
      entity_size
      is_active
      created_at
      updated_at
    }
  }
`;

// GraphQL query for getting company financial statements
const GET_COMPANY_FINANCIAL_STATEMENTS = gql`
  query GetCompanyFinancialStatements($companyId: ID!, $pagination: PaginationInput) {
    companyFinancialStatements(companyId: $companyId, pagination: $pagination) {
      nodes {
        id
        company_id
        filing_type
        form_type
        accession_number
        filing_date
        period_end_date
        fiscal_year
        fiscal_quarter
        document_type
        document_url
        xbrl_file_oid
        xbrl_file_size_bytes
        xbrl_file_compressed
        xbrl_processing_status
        created_at
        updated_at
      }
      total_count
      page_info {
        has_next_page
        has_previous_page
        start_cursor
        end_cursor
      }
    }
  }
`;

export interface Company {
  id: string;
  cik: string;
  ticker?: string;
  name: string;
  legal_name?: string;
  sic_code?: string;
  sic_description?: string;
  industry?: string;
  sector?: string;
  business_address?: any;
  mailing_address?: any;
  phone?: string;
  website?: string;
  state_of_incorporation?: string;
  state_of_incorporation_description?: string;
  fiscal_year_end?: string;
  entity_type?: string;
  entity_size?: string;
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export interface FinancialStatement {
  id: string;
  company_id: string;
  filing_type: string;
  form_type: string;
  accession_number: string;
  filing_date: string;
  period_end_date: string;
  fiscal_year: number;
  fiscal_quarter?: number;
  document_type: string;
  document_url: string;
  xbrl_file_oid?: number;
  xbrl_file_size_bytes?: number;
  xbrl_file_compressed: boolean;
  xbrl_processing_status: string;
  created_at: string;
  updated_at: string;
}

export interface PaginationInput {
  first?: number;
  after?: string;
  last?: number;
  before?: string;
}

export interface FinancialStatementConnection {
  nodes: FinancialStatement[];
  total_count: number;
  page_info: {
    has_next_page: boolean;
    has_previous_page: boolean;
    start_cursor?: string;
    end_cursor?: string;
  };
}

export interface UseFinancialDataReturn {
  company: Company | null;
  financialStatements: FinancialStatement[];
  loading: boolean;
  error?: Error;
  loadCompany: (id: string) => Promise<void>;
  loadFinancialStatements: (companyId: string, pagination?: PaginationInput) => Promise<void>;
  refetch: () => void;
}

export const useFinancialData = (): UseFinancialDataReturn => {
  const [company, setCompany] = useState<Company | null>(null);
  const [financialStatements, setFinancialStatements] = useState<FinancialStatement[]>([]);

  // Lazy query for getting a company
  const [getCompanyQuery, { loading: companyLoading, error: companyError }] = useLazyQuery<{
    company: Company | null;
  }>(GET_COMPANY);

  // Lazy query for getting financial statements
  const [getFinancialStatementsQuery, { loading: statementsLoading, error: statementsError }] =
    useLazyQuery<{
      companyFinancialStatements: FinancialStatementConnection;
    }>(GET_COMPANY_FINANCIAL_STATEMENTS);

  // Load company
  const loadCompany = useCallback(
    async (id: string): Promise<void> => {
      try {
        const result = await getCompanyQuery({
          variables: { id },
        });
        if (result.data?.company) {
          setCompany(result.data.company);
        }
      } catch (error) {
        console.error('Error loading company:', error);
        throw error;
      }
    },
    [getCompanyQuery]
  );

  // Load financial statements
  const loadFinancialStatements = useCallback(
    async (companyId: string, pagination?: PaginationInput): Promise<void> => {
      try {
        const result = await getFinancialStatementsQuery({
          variables: {
            companyId,
            pagination: pagination || { first: 50 },
          },
        });
        if (result.data?.companyFinancialStatements) {
          setFinancialStatements(result.data.companyFinancialStatements.nodes);
        }
      } catch (error) {
        console.error('Error loading financial statements:', error);
        throw error;
      }
    },
    [getFinancialStatementsQuery]
  );

  // Refetch all data
  const refetch = useCallback(() => {
    if (company) {
      loadCompany(company.id);
    }
    if (financialStatements.length > 0) {
      const companyId = financialStatements[0].company_id;
      loadFinancialStatements(companyId);
    }
  }, [company, financialStatements, loadCompany, loadFinancialStatements]);

  return {
    company,
    financialStatements,
    loading: companyLoading || statementsLoading,
    error: companyError || statementsError,
    loadCompany,
    loadFinancialStatements,
    refetch,
  };
};

export default useFinancialData;
