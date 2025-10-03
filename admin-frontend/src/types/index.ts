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

export interface SecCrawlResult {
  success: boolean;
  message: string;
  documentsProcessed: number;
  documentsSkipped: number;
  documentsFailed: number;
  totalSizeBytes: number;
  processingTimeMs: number;
  errors: string[];
  warnings: string[];
}

export interface SecCrawlInput {
  companyId: string;
  startDate?: string;
  endDate?: string;
  formTypes?: string[];
  maxDocuments?: number;
  includeAmendments?: boolean;
  includeExhibits?: boolean;
  rateLimit?: number;
  retryAttempts?: number;
  timeout?: number;
}

export interface SecRssImportInput {
  feedUrl: string;
  maxItems?: number;
  includeAmendments?: boolean;
  includeExhibits?: boolean;
  rateLimit?: number;
  retryAttempts?: number;
  timeout?: number;
}

export interface SecRssImportResult {
  success: boolean;
  message: string;
  itemsProcessed: number;
  itemsSkipped: number;
  itemsFailed: number;
  newCompanies: number;
  updatedCompanies: number;
  processingTimeMs: number;
  errors: string[];
  warnings: string[];
}
