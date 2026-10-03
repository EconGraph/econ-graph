/**
 * Type definitions for the admin frontend
 */

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

export type {
  SecCrawlInput,
  SecCrawlResult,
  SecRssImportInput,
  SecRssImportResult,
} from "../hooks/useSecCrawler";
