/**
 * Company Search Hook
 * 
 * Provides GraphQL integration for company search functionality.
 * Includes fulltext search with PostgreSQL indices and fuzzy matching.
 */

import { useState, useCallback } from 'react';
import { useQuery, useLazyQuery } from '@apollo/client';
import { gql } from '@apollo/client';

// GraphQL query for searching companies
const SEARCH_COMPANIES = gql`
  query SearchCompanies($input: CompanySearchInput!) {
    searchCompanies(input: $input) {
      nodes {
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

// GraphQL query for getting a specific company
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

export interface CompanySearchInput {
  query: string;
  limit?: number;
  include_inactive?: boolean;
}

export interface CompanySearchResult {
  nodes: Company[];
  total_count: number;
  page_info: {
    has_next_page: boolean;
    has_previous_page: boolean;
    start_cursor?: string;
    end_cursor?: string;
  };
}

export interface UseCompanySearchReturn {
  companies: Company[];
  loading: boolean;
  error?: Error;
  totalCount: number;
  searchCompanies: (input: CompanySearchInput) => Promise<void>;
  getCompany: (id: string) => Promise<Company | null>;
}

export const useCompanySearch = (): UseCompanySearchReturn => {
  const [companies, setCompanies] = useState<Company[]>([]);
  const [totalCount, setTotalCount] = useState(0);

  // Lazy query for searching companies
  const [searchCompaniesQuery, { loading: searchLoading, error: searchError }] = useLazyQuery<{
    searchCompanies: CompanySearchResult;
  }>(SEARCH_COMPANIES, {
    onCompleted: (data) => {
      if (data.searchCompanies) {
        setCompanies(data.searchCompanies.nodes);
        setTotalCount(data.searchCompanies.total_count);
      }
    },
    onError: (error) => {
      console.error('Company search error:', error);
      setCompanies([]);
      setTotalCount(0);
    },
  });

  // Lazy query for getting a specific company
  const [getCompanyQuery, { loading: getCompanyLoading }] = useLazyQuery<{
    company: Company | null;
  }>(GET_COMPANY);

  // Search companies function
  const searchCompanies = useCallback(
    async (input: CompanySearchInput): Promise<void> => {
      try {
        await searchCompaniesQuery({
          variables: { input },
          fetchPolicy: 'cache-first',
        });
      } catch (error) {
        console.error('Error searching companies:', error);
        throw error;
      }
    },
    [searchCompaniesQuery]
  );

  // Get specific company function
  const getCompany = useCallback(
    async (id: string): Promise<Company | null> => {
      try {
        const result = await getCompanyQuery({
          variables: { id },
          fetchPolicy: 'cache-first',
        });
        return result.data?.company || null;
      } catch (error) {
        console.error('Error getting company:', error);
        return null;
      }
    },
    [getCompanyQuery]
  );

  return {
    companies,
    loading: searchLoading || getCompanyLoading,
    error: searchError,
    totalCount,
    searchCompanies,
    getCompany,
  };
};

export default useCompanySearch;
